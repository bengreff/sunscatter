//! Control locations and what reaches them (D067, D068).
//!
//! The player (and their agent) is at a control location: flying a vessel
//! means being its crew; the tracking station means being at mission
//! control. Every frame the relay network (`sim::comms`) is rebuilt and each
//! vessel's path to the location found: its delay and rate, or no signal.
//! The station shows vessels as their light arrives (the retarded state).
//!
//! Vessels are not yet crewed or uncrewed in data, and all carry the test
//! craft's antenna; commands to other vessels arrive with the path's delay
//! once vessel-directed commands exist (the burn planner).

use crate::state::SimState;
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use glam::DVec3;
use sim::comms::{self, Antenna, CommsData, Graph, Node};
use sim::time::Epoch;
use sim::vessel::{Phase, VesselId};
use std::collections::HashMap;

/// Where the player is (D067).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    /// Aboard a crewed vessel.
    Vessel(VesselId),
    /// At a mission control site (index into the comms data's sites).
    Site(usize),
}

/// The location from the view: the tracking station is mission control;
/// flying a crewed vessel is being aboard it; flying an uncrewed probe is
/// doing it from mission control (D034).
pub fn location_rule(station_open: bool, active: VesselId, crewed: bool, mission_control: usize) -> Location {
    if station_open || !crewed {
        Location::Site(mission_control)
    } else {
        Location::Vessel(active)
    }
}

/// A vessel's signal at the location this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    /// One-way light delay (s); 0 aboard.
    pub delay: f64,
    /// Data rate (bit/s) of the slowest hop; infinite aboard.
    pub rate: f64,
    /// Names of the relays in between (sites and vessels).
    pub via: Vec<String>,
}

/// The antenna every vessel carries for now (the test craft's, D064).
pub const VESSEL_ANTENNA: Antenna = Antenna { gain_dbi: 20.0, power_w: 20.0 };

#[derive(Resource)]
pub struct Comms {
    pub data: CommsData,
    pub mission_control: usize,
    pub location: Location,
    /// Each vessel's signal at the location (`None`: no path).
    pub signals: HashMap<VesselId, Option<Signal>>,
    /// When each vessel was last heard at the location (game clock).
    pub last_contact: HashMap<VesselId, Epoch>,
    /// The active vessel's link to mission control, wherever the player is.
    pub home: Option<Signal>,
}

impl Comms {
    pub fn load() -> Self {
        let data = CommsData::load(&CommsData::default_path()).unwrap_or_else(|e| panic!("{e}"));
        let mission_control = data
            .sites
            .iter()
            .position(|s| s.kind == comms::SiteKind::MissionControl)
            .expect("a mission control site in comms.ron");
        Comms {
            data,
            mission_control,
            location: Location::Site(mission_control),
            signals: HashMap::new(),
            last_contact: HashMap::new(),
            home: None,
        }
    }

    pub fn signal(&self, id: VesselId) -> Option<&Signal> {
        self.signals.get(&id).and_then(Option::as_ref)
    }

    /// The name of the location.
    pub fn location_name(&self) -> String {
        match self.location {
            Location::Site(i) => self.data.sites[i].name.clone(),
            Location::Vessel(id) => format!("aboard Vessel {}", id.0),
        }
    }
}

/// One line about a signal: "21 ms via Canberra · 4.3 Mbit/s", "aboard", or
/// "no signal".
pub fn describe(signal: Option<&Signal>) -> String {
    match signal {
        None => "no signal".into(),
        Some(s) if s.delay == 0.0 => "aboard".into(),
        Some(s) => {
            let via = if s.via.is_empty() { String::new() } else { format!(" via {}", s.via.join(", ")) };
            format!("{}{via} · {}", crate::format::delay(s.delay), crate::format::rate(s.rate))
        }
    }
}

/// Where the light seen now left a vessel (`r`, `v` now; `delay` s ago),
/// to first order: r − v·delay. The error is a·delay²/2, a few metres at
/// the Moon's 1.3 s.
pub fn retarded(r: DVec3, v: DVec3, delay: f64) -> DVec3 {
    r - v * delay
}

/// Rebuilds the network and every vessel's signal at the location.
pub fn update(sim: Res<SimState>, station: Res<TrackingStation>, mut comms: ResMut<Comms>) {
    let active = sim.ship().id();
    comms.location = location_rule(station.open, active, sim.ship().crew() > 0, comms.mission_control);
    let t = sim.clock;
    let (anchor, _, _) = sim.ship().state_at(&sim.world, t);
    let bodies = comms::occluders(&sim.world, t, anchor);
    let sites = comms::site_nodes(&sim.world, &comms.data, t, &bodies);
    // Nodes: the sites that exist in this world, then every vessel.
    let mut nodes = Vec::new();
    let mut names = Vec::new();
    let mut site_node = vec![None; sites.len()];
    for (i, s) in sites.iter().enumerate() {
        if let Some(n) = s {
            site_node[i] = Some(nodes.len());
            nodes.push(*n);
            names.push(comms.data.sites[i].name.clone());
        }
    }
    let snap = sim.world.snapshot(t);
    let first_vessel = nodes.len();
    for v in &sim.fleet {
        let (a, r, _) = v.state_at(&sim.world, t);
        // On the ground a vessel sees what a site there would (the horizon
        // rule); its body's sphere alone would not block a path through it.
        let ground = match v.phase {
            Phase::Landed { body, .. } | Phase::Crashed { body, .. } => {
                bodies.iter().position(|(n, _)| *n == body).map(|k| (k, (r + snap.relative_r(a, body)).normalize()))
            }
            _ => None,
        };
        let pos = snap.relative_r(a, anchor) + r;
        // On a launch pad: wired through its umbilical.
        let wired = ground.is_some()
            && comms.data.sites.iter().zip(&sites).any(|(site, node)| {
                site.kind == comms::SiteKind::LaunchSite
                    && node.is_some_and(|n| (n.pos - pos).length() < comms.data.link.umbilical_range_m)
            });
        nodes.push(Node { pos, antenna: Some(VESSEL_ANTENNA), ground, wired });
        names.push(format!("Vessel {}", v.id().0));
    }
    let occluders: Vec<_> = bodies.iter().map(|b| b.1).collect();
    let graph = Graph::new(&nodes, &occluders, &comms.data.link);
    let from = match comms.location {
        Location::Site(i) => site_node[i],
        Location::Vessel(id) => sim.index_of(id).map(|i| first_vessel + i),
    };
    let mut signals = HashMap::new();
    for (i, v) in sim.fleet.iter().enumerate() {
        let to = first_vessel + i;
        let signal = if from == Some(to) {
            Some(Signal { delay: 0.0, rate: f64::INFINITY, via: Vec::new() })
        } else {
            from.and_then(|f| comms::best_path(&graph, f, to)).map(|p| to_signal(&p, &names))
        };
        if let Some(s) = &signal {
            comms.last_contact.insert(v.id(), t.add_seconds(-s.delay));
        }
        signals.insert(v.id(), signal);
    }
    comms.signals = signals;
    comms.home = site_node[comms.mission_control]
        .and_then(|f| comms::best_path(&graph, f, first_vessel + sim.active))
        .map(|p| to_signal(&p, &names));
}

fn to_signal(p: &comms::Path, names: &[String]) -> Signal {
    Signal {
        delay: p.delay,
        rate: p.rate,
        via: p.nodes[1..p.nodes.len() - 1].iter().map(|&n| names[n].clone()).collect(),
    }
}

/// The delay with which the location sees vessel `id` now (0 aboard or
/// without signal: then it is shown where its predicted trajectory puts it).
pub fn seen_delay(comms: &Comms, id: VesselId) -> f64 {
    comms.signal(id).map_or(0.0, |s| s.delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_station_is_mission_control_and_flying_is_aboard() {
        let id = VesselId(7);
        assert_eq!(location_rule(true, id, true, 0), Location::Site(0));
        assert_eq!(location_rule(false, id, true, 0), Location::Vessel(id));
        assert_eq!(location_rule(false, id, false, 0), Location::Site(0), "a probe is flown from mission control");
    }

    #[test]
    fn signals_read_as_one_line() {
        let s = Signal { delay: 0.0213, rate: 4.26e6, via: vec!["Canberra".into()] };
        assert_eq!(describe(Some(&s)), "21 ms via Canberra · 4.3 Mbit/s");
        assert_eq!(describe(None), "no signal");
        assert_eq!(describe(Some(&Signal { delay: 0.0, rate: f64::INFINITY, via: vec![] })), "aboard");
    }

    #[test]
    fn retarded_position_moves_back_along_the_velocity() {
        let r = retarded(DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.0, 2.0, 0.0), 0.5);
        assert_eq!(r, DVec3::new(1.0, -1.0, 0.0));
    }

    #[test]
    fn the_ship_on_the_pad_is_heard_through_its_umbilical() {
        let mut app = App::new();
        app.insert_resource(SimState::new());
        app.insert_resource(TrackingStation::default());
        app.insert_resource(Comms::load());
        app.add_systems(Update, update);
        app.update();
        let comms = app.world().resource::<Comms>();
        let home = comms.home.as_ref().expect("the pad is wired to Houston");
        assert_eq!(home.rate, f64::INFINITY);
        assert!(home.delay > 0.0 && home.delay < 0.02, "{}", home.delay);
    }

    #[test]
    fn mission_control_hears_ships_in_orbit_through_the_dsn() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(4);
        let mut app = App::new();
        let mut ts = TrackingStation::default();
        ts.open = true;
        app.insert_resource(sim);
        app.insert_resource(ts);
        app.insert_resource(Comms::load());
        app.add_systems(Update, update);
        app.update();
        let comms = app.world().resource::<Comms>();
        let sim = app.world().resource::<SimState>();
        assert_eq!(comms.location, Location::Site(comms.mission_control));
        let heard = sim.fleet.iter().filter(|v| comms.signal(v.id()).is_some()).count();
        assert!(heard >= 1, "some LEO ship is above a DSN horizon");
        for v in sim.fleet.iter().filter(|v| !matches!(v.phase, Phase::Landed { .. })) {
            if let Some(s) = comms.signal(v.id()) {
                assert!(s.delay > 0.0 && s.delay < 0.1, "{}", s.delay);
                assert!(!s.via.is_empty(), "through a ground station");
            }
        }
    }
}
