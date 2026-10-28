//! The flight snapshot every menu reads ([`FlightData`]): plain numbers and
//! names, gathered by one system from `SimState`, the navball state,
//! `Apsides`, `LandingPrediction`, `Rendezvous` and `Comms`, ten times a
//! second (so values do not jiggle). The menus' content functions are pure
//! functions of it.
//!
//! Every number keeps its owner: Ap and Pe come from `trajectory::apsides`
//! (D076), never osculating; the elements from `relations` (about the
//! body's equator); impact and braking from `sim::landing` through
//! `landing::LandingPrediction`; closest approaches from `rendezvous`;
//! stages from `staging`.

use crate::comms::{Comms, Signal};
use crate::landing::Landing;
use crate::navball::{ApsisReadout, NavState, NavTarget};
use crate::rendezvous::Rendezvous;
use crate::state::SimState;
use crate::tracking::Tracked;
use crate::trajectory::apsides::Apsides;
use bevy::prelude::*;
use sim::vessel::{BurnEnd, Phase};

/// Seconds between refreshes.
const REFRESH: f64 = 0.1;
/// How many parts the Thermal section lists.
const HOTTEST: usize = 4;

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct FlightData {
    /// The dominant body (D056): what the readouts are about.
    pub body: String,
    pub landed: bool,
    /// The next apsides (D076).
    pub ap: Option<ApsisReadout>,
    pub pe: Option<ApsisReadout>,
    pub elements: Option<Elements>,
    pub target: TargetData,
    /// Planned burns not yet ignited, in order.
    pub burns: Vec<Burn>,
    pub surface: Option<Surface>,
    pub impact: Option<ImpactData>,
    pub braking: Option<Brake>,
    /// In an atmosphere now.
    pub air: Option<AirData>,
    /// Outside one, the predicted path's way into an atmosphere.
    pub entry: Option<Entry>,
    pub g_load: f64,
    /// Heading, pitch, roll (deg).
    pub attitude: Option<[f64; 3]>,
    pub chute: ChuteData,
    pub craft: CraftData,
    pub comms: CommsData,
    pub stages: Vec<crate::staging::Stage>,
    pub diagnostics: Diagnostics,
    /// What the craft knows (D106): every readout through it.
    pub known: crate::format::Known,
}

/// Osculating elements about the dominant body's equator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Elements {
    pub a: f64,
    pub e: f64,
    /// Degrees.
    pub inclination: f64,
    pub lan: f64,
    pub argp: f64,
    pub true_anomaly: f64,
    pub period: Option<f64>,
    /// Speed relative to the body's centre (m/s).
    pub speed: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetData {
    pub current: Option<NavTarget>,
    /// Every pick: none, the bodies, the other vessels.
    pub choices: Vec<(String, Option<NavTarget>)>,
    pub info: Option<TargetInfo>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetInfo {
    pub name: String,
    pub distance: f64,
    pub rel_speed: f64,
    pub approach: Option<Approach>,
    /// Degrees, when the target orbits the same body.
    pub rel_inclination: Option<f64>,
}

/// The nearest approach to the target (`rendezvous`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Approach {
    /// Centre to centre for a vessel, above the surface for a body (m).
    pub distance: f64,
    pub in_s: f64,
    pub speed: f64,
    /// A pass over a body (not a vessel's closest approach).
    pub over_body: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Burn {
    /// m/s (`None`: a burn of fixed duration).
    pub dv: Option<f64>,
    pub in_s: f64,
    pub duration: f64,
}

/// Motion relative to the nearest surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    /// Lowest contact point above the ground (m).
    pub radar: f64,
    pub v_vertical: f64,
    pub v_horizontal: f64,
    /// Full thrust at the ambient pressure against local gravity.
    pub twr: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImpactData {
    pub body: String,
    pub in_s: f64,
    pub v_vertical: f64,
    pub v_horizontal: f64,
    /// Degrees.
    pub lat: f64,
    pub lon: f64,
}

/// The suicide-burn solution for a near impact (`sim::landing`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Brake {
    Burn {
        in_s: f64,
        ignite_height: f64,
    },
    /// No full-thrust burn stops the craft before the ground.
    CannotStop,
}

/// Where the predicted path enters an atmosphere, seen from outside it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    /// The atmosphere's top above the reference ellipsoid: the entry
    /// interface (m).
    pub top: f64,
    /// Seconds to it, where the stored trajectory marks the entry.
    pub in_s: Option<f64>,
    /// The next periapsis, when it lies inside the atmosphere (m).
    pub periapsis: Option<f64>,
}

/// The entry ahead, from what is known (a pure rule): the predicted path's
/// marked entry (seconds to it, the atmosphere's top), a periapsis inside
/// an atmosphere (its altitude, the top), or an impact on a body with one
/// (its top). The marked entry's atmosphere wins; `None` if none applies.
pub fn entry_ahead(
    marked: Option<(f64, f64)>,
    periapsis: Option<(f64, f64)>,
    impact_top: Option<f64>,
) -> Option<Entry> {
    let top = marked.map(|m| m.1).or(periapsis.map(|p| p.1)).or(impact_top)?;
    Some(Entry { top, in_s: marked.map(|m| m.0), periapsis: periapsis.map(|p| p.0) })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirData {
    /// Dynamic pressure (Pa).
    pub q: f64,
    pub mach: f64,
    /// Angle of attack and sideslip (deg).
    pub aoa_slip: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChuteData {
    /// The parachute command was given (P or Space).
    pub commanded: bool,
    pub state: sim::chute::ChuteState,
    /// Dynamic pressure now (Pa; `None` outside an atmosphere).
    pub q: Option<f64>,
    /// The first canopy's design opening q (Pa, D079).
    pub design_q: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CraftData {
    pub debug: bool,
    pub propellant: f64,
    /// The full load (kg).
    pub full: f64,
    pub dv_vac: f64,
    /// In the dominant body's gravity here; `None` in free space.
    pub twr_vac: Option<f64>,
    pub mass: f64,
    pub thrust_vac: f64,
    pub isp_vac: f64,
    pub isp_sl: f64,
    /// RCS couples (pitch, yaw, roll; N·m) and their propellant use at
    /// full command (kg/s).
    pub rcs_torque: [f64; 3],
    pub rcs_flow: [f64; 3],
    /// The skin cell nearest its own limit, and the hottest interior node,
    /// with their limits (K).
    pub skin: (f64, f64),
    pub interior: (f64, f64),
    /// The parts nearest their limits: name, temperature, limit (K).
    pub hottest: Vec<(String, f64, f64)>,
    /// Ship clock minus the origin's clock, the time shown (s, D012, D088).
    pub clock_offset: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommsData {
    /// Mission control's name.
    pub home: String,
    /// The active vessel's link to it.
    pub link: Option<Signal>,
    /// An uncrewed probe: the delay its controls arrive after (`None`: no
    /// signal, they do not arrive).
    pub probe: Option<Option<f64>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnostics {
    pub anchor: String,
    pub fps: String,
    pub vessels: usize,
    /// The profiler's latest summary (empty when it is off).
    pub profile: Vec<(String, String)>,
}

/// What the snapshot is gathered from.
pub struct Sources<'a> {
    pub sim: &'a SimState,
    /// The active vessel's predicted segment while it flies live.
    pub pred: &'a crate::state::Prediction,
    pub nav: Option<&'a NavState>,
    pub target: Option<NavTarget>,
    pub aps: &'a Apsides,
    pub landing: Option<&'a Landing>,
    pub rv: &'a Rendezvous,
    pub comms: &'a Comms,
    pub tracked: &'a Tracked,
    pub fps: String,
}

/// Refreshes [`FlightData`] every [`REFRESH`] seconds (not in the
/// tracking station, where the flight view is not drawn).
#[allow(clippy::too_many_arguments)]
pub fn update(
    time: Res<Time>,
    sim: Res<SimState>,
    pred: Res<crate::state::Prediction>,
    nav: Res<crate::navball::Navball>,
    aps: Res<Apsides>,
    landing: Res<crate::landing::LandingPrediction>,
    rv: Res<Rendezvous>,
    comms: Res<Comms>,
    tracked: Res<Tracked>,
    fps: Res<crate::hud::FpsMeter>,
    station: Res<crate::tracking::TrackingStation>,
    profile: Res<crate::profile::Profile>,
    mut data: ResMut<FlightData>,
    mut last: Local<f64>,
) {
    let now = time.elapsed_secs_f64();
    if station.open || now - *last < REFRESH {
        return;
    }
    *last = now;
    let sources = Sources {
        sim: &sim,
        pred: &pred,
        nav: nav.now.as_ref(),
        target: nav.target,
        aps: &aps,
        landing: landing.latest.as_ref(),
        rv: &rv,
        comms: &comms,
        tracked: &tracked,
        fps: fps.text(),
    };
    let mut d = gather(&sources);
    d.diagnostics.profile.clone_from(&profile.summary);
    data.set_if_neq(d);
}

/// The snapshot for the active vessel now.
pub fn gather(s: &Sources) -> FlightData {
    let sim = s.sim;
    let ship = sim.ship();
    let body = sim.dominant_of(sim.active);
    let (anchor, _, _) = ship.state_at(&sim.world, sim.clock);
    let nav = s.nav;
    FlightData {
        body: sim.world.eph.node(body).name.clone(),
        landed: matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }),
        ap: nav.and_then(|n| n.ap.clone()),
        pe: nav.and_then(|n| n.pe.clone()),
        // A craft on the ground has no orbit worth showing.
        elements: elements(sim, body).filter(|_| !matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. })),
        target: target(s, body),
        burns: burns(sim),
        g_load: nav.map_or(0.0, |n| n.g.load),
        attitude: nav.map(|n| [n.angles.heading, n.angles.pitch, n.angles.roll]),
        craft: craft(sim, body),
        comms: comms(s),
        stages: crate::staging::stack_of(ship, &sim.controls),
        known: crate::format::Known::of(ship),
        diagnostics: Diagnostics {
            anchor: sim.world.eph.node(anchor).name.clone(),
            fps: s.fps.clone(),
            vessels: sim.fleet.len(),
            profile: Vec::new(),
        },
        ..landing(s)
    }
}

fn elements(sim: &SimState, body: sim::frame::NodeId) -> Option<Elements> {
    let (anchor, r, v) = sim.ship().state_at(&sim.world, sim.clock);
    let o = crate::relations::equatorial_orbit(&sim.world, sim.clock, anchor, r, v, body)?;
    let el = o.elements;
    let k = sim.world.snapshot(sim.clock).relative(body, anchor);
    let deg = |x: f64| x.to_degrees().rem_euclid(360.0);
    Some(Elements {
        a: el.a,
        e: el.e,
        inclination: el.i.to_degrees(),
        lan: deg(el.raan),
        argp: deg(el.argp),
        true_anomaly: deg(sim::kepler::true_from_mean(el.mean_anomaly, el.e)),
        period: o.period(),
        speed: (v - k.v).length(),
    })
}

fn target(s: &Sources, body: sim::frame::NodeId) -> TargetData {
    let sim = s.sim;
    let mut choices = vec![("none".to_string(), None)];
    choices.extend(sim.world.surfaces().map(|src| (src.name.clone(), Some(NavTarget::Body(src.node)))));
    let others = sim.fleet.iter().map(|v| v.id()).filter(|&id| id != sim.ship().id());
    choices.extend(others.map(|id| (s.tracked.name(id), Some(NavTarget::Vessel(id)))));
    let info = s.target.and_then(|tg| {
        let (anchor, r, v) = sim.ship().state_at(&sim.world, sim.clock);
        let (name, tr, tv) = crate::navball::target_state(sim, s.tracked, tg, anchor, sim.clock)?;
        let approach = s.rv.approaches.first().filter(|_| s.rv.target == Some(tg)).map(|ca| Approach {
            distance: crate::rendezvous::pass_distance(sim, Some(tg), ca.distance),
            in_s: ca.t.seconds_since(sim.clock),
            speed: ca.speed,
            over_body: matches!(tg, NavTarget::Body(_)),
        });
        // Both about the same body: the ship's dominant one, and the
        // target's (a vessel's dominant body, a body's display primary).
        let target_body = match tg {
            NavTarget::Body(node) => crate::relations::primary(&sim.world.eph, node),
            NavTarget::Vessel(id) => sim.index_of(id).map(|i| sim.dominant_of(i)),
        };
        let k = sim.world.snapshot(sim.clock).relative(body, anchor);
        let rel_inclination = (target_body == Some(body))
            .then(|| crate::relations::relative_inclination(r - k.r, v - k.v, tr - k.r, tv - k.v))
            .flatten()
            .map(f64::to_degrees);
        Some(TargetInfo { name, distance: (tr - r).length(), rel_speed: (tv - v).length(), approach, rel_inclination })
    });
    TargetData { current: s.target, choices, info }
}

/// The planned burns ahead, with durations from the rocket equation at the
/// mass each starts with (as the planner shows them).
fn burns(sim: &SimState) -> Vec<Burn> {
    let ship = sim.ship();
    let mut mass = ship.mass();
    let ahead = ship.plan().burns.iter().filter(|b| b.t_start.seconds_since(sim.clock) > 0.0);
    ahead
        .map(|b| {
            let ve = b.law.thrust / b.law.mass_flow.max(1e-12);
            let (dv, duration) = match b.end {
                BurnEnd::DeltaV(dv) => {
                    (Some(dv), crate::planner::burn_time(dv, mass, b.law.thrust, ve / sim::vessel::G0))
                }
                BurnEnd::Duration(d) => (None, d),
            };
            if !ship.debug() {
                mass = (mass - b.law.mass_flow * duration).max(ship.craft.mass.dry_mass);
            }
            Burn { dv, in_s: b.t_start.seconds_since(sim.clock), duration }
        })
        .collect()
}

/// Surface motion, impact, braking, air, entry and the chute.
fn landing(s: &Sources) -> FlightData {
    let sim = s.sim;
    let ship = sim.ship();
    let (anchor, r, v) = ship.state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let grounded = matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. });
    let surface = sim::landing::nearest_surface(&sim.world, &snap, anchor, r).map(|b| {
        let m = sim::landing::surface_motion(&sim.world, &snap, anchor, b, r, v);
        Surface {
            radar: m.height - ship.contact_height(),
            v_vertical: m.v_vertical,
            v_horizontal: m.v_horizontal,
            twr: sim::landing::twr(&ship.craft, ship.propellant(), m.pressure, m.gravity),
        }
    });
    let latest = s.landing.filter(|l| l.vessel == ship.id() && !grounded);
    let hit = latest.and_then(|l| l.impact).filter(|h| h.t.seconds_since(sim.clock) > 0.0);
    let impact = hit.map(|h| ImpactData {
        body: sim.world.eph.node(h.body).name.clone(),
        in_s: h.t.seconds_since(sim.clock),
        v_vertical: h.v_vertical,
        v_horizontal: h.v_horizontal,
        lat: h.lat.to_degrees(),
        lon: h.lon.to_degrees(),
    });
    let braking = match (hit, latest.and_then(|l| l.braking)) {
        (Some(_), Some(b)) => {
            Some(Brake::Burn { in_s: b.t_ignite.seconds_since(sim.clock), ignite_height: b.ignite_height })
        }
        (Some(h), None) if h.t.seconds_since(sim.clock) < crate::landing::BRAKING_WITHIN => Some(Brake::CannotStop),
        _ => None,
    };
    let air = sim::vessel::air_at(&sim.world, &snap, anchor, r, v).filter(|_| !grounded);
    let atmosphere_top = |body| sim.world.source(body)?.physical.as_ref()?.atmosphere.as_ref().map(|a| a.top);
    let entry = air.is_none().then(|| {
        let impact_top = hit.and_then(|h| atmosphere_top(h.body));
        entry_ahead(marked_entry(s), periapsis_inside(sim, s.aps), impact_top)
    });
    let entry = entry.flatten().filter(|_| !grounded);
    let spec = &ship.craft.chute;
    let chute = ChuteData {
        commanded: sim.controls.chute,
        state: ship.chute,
        q: air.as_ref().map(sim::vessel::Air::q),
        design_q: spec.drogue.as_ref().unwrap_or(&spec.main).deploy_max_q,
    };
    FlightData {
        surface,
        impact,
        braking,
        air: air.map(|a| AirData {
            q: a.q(),
            mach: a.wind.length() / a.speed_of_sound.max(1e-9),
            aoa_slip: s.nav.and_then(|n| n.aoa_sideslip),
        }),
        entry,
        chute,
        ..FlightData::default()
    }
}

/// The next periapsis, if it is inside an atmosphere: its altitude and the
/// atmosphere's top.
fn periapsis_inside(sim: &SimState, aps: &Apsides) -> Option<(f64, f64)> {
    let (_, pe) = aps.get(sim.ship().id())?.next_after(sim.clock);
    let pe = pe?;
    let top = sim.world.source(pe.body)?.physical.as_ref()?.atmosphere.as_ref()?.top;
    (pe.altitude < top).then_some((pe.altitude, top))
}

/// The first atmosphere entry ahead on the predicted path, as the stored
/// segments mark it (`Segment::entry`): seconds to it and the atmosphere's
/// top.
fn marked_entry(s: &Sources) -> Option<(f64, f64)> {
    let sim = s.sim;
    let segments = crate::trajectory::apsides::vessel_segments(sim, s.pred, sim.active);
    segments.iter().find_map(|seg| {
        let (t, body) = seg.entry?;
        let in_s = seg.t0.add_seconds(t).seconds_since(sim.clock);
        let top = sim.world.source(body)?.physical.as_ref()?.atmosphere.as_ref()?.top;
        (in_s > 0.0).then_some((in_s, top))
    })
}

fn craft(sim: &SimState, body: sim::frame::NodeId) -> CraftData {
    let ship = sim.ship();
    let engine = &ship.craft.engine;
    // The vehicle's: not the pad's launch mount it stands on.
    let mass = crate::hud::vehicle_mass(ship);
    let (anchor, r, _) = ship.state_at(&sim.world, sim.clock);
    let g = sim.world.source(body).map_or(0.0, |b| {
        let d = (r - sim.world.snapshot(sim.clock).relative_r(b.node, anchor)).length();
        b.gm / (d * d)
    });
    let (node, _) = ship.max_node_temperature();
    CraftData {
        debug: ship.debug(),
        propellant: ship.propellant(),
        full: ship.craft.initial_propellant.max(1.0),
        dv_vac: crate::hud::delta_v(engine.isp_vac, mass, ship.propellant()),
        twr_vac: crate::hud::twr(engine.thrust_vac, mass, g),
        mass,
        thrust_vac: engine.thrust_vac,
        isp_vac: engine.isp_vac,
        isp_sl: engine.isp_sl,
        rcs_torque: ship.craft.torque.to_array(),
        rcs_flow: ship.craft.rcs_flow.to_array(),
        skin: ship.skin_nearest_limit(),
        interior: (node, ship.craft.thermal.internal_max_k),
        hottest: super::hottest::parts(ship, HOTTEST),
        clock_offset: sim.ship_clock_offset(sim.active),
    }
}

fn comms(s: &Sources) -> CommsData {
    let ship = s.sim.ship();
    CommsData {
        home: s.comms.data.sites[s.comms.mission_control].name.clone(),
        link: s.comms.home.clone(),
        probe: (ship.crew() == 0).then(|| s.comms.signal(ship.id()).map(|sig| sig.delay)),
    }
}

/// The unit normal of an orbit's plane (tests).
#[cfg(test)]
fn plane(r: glam::DVec3, v: glam::DVec3) -> glam::DVec3 {
    r.cross(v).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        pred: crate::state::Prediction,
        aps: Apsides,
        rv: Rendezvous,
        comms: Comms,
        tracked: Tracked,
    }

    fn fixture() -> Fixture {
        Fixture {
            pred: crate::state::Prediction::default(),
            aps: Apsides::default(),
            rv: Rendezvous::default(),
            comms: Comms::load(),
            tracked: Tracked::default(),
        }
    }

    fn sources<'a>(sim: &'a SimState, f: &'a Fixture) -> Sources<'a> {
        Sources {
            sim,
            pred: &f.pred,
            nav: None,
            target: None,
            aps: &f.aps,
            landing: None,
            rv: &f.rv,
            comms: &f.comms,
            tracked: &f.tracked,
            fps: "60".into(),
        }
    }

    #[test]
    fn on_the_pad_the_snapshot_is_landed_with_a_full_tank_and_stowed_chutes() {
        let sim = SimState::new();
        let f = fixture();
        let d = gather(&sources(&sim, &f));
        assert_eq!(d.body, "Earth");
        assert!(d.landed && d.impact.is_none() && d.braking.is_none() && d.air.is_none());
        let s = d.surface.expect("over Earth");
        assert!(s.radar.abs() < 1.0, "on its feet: {}", s.radar);
        assert!(s.twr > 1.0, "the test craft lifts off: {}", s.twr);
        assert!((d.craft.propellant - d.craft.full).abs() < 1e-6);
        assert!(d.craft.dv_vac > 1000.0 && d.craft.twr_vac.is_some_and(|t| t > 1.0));
        assert!(!d.chute.commanded && d.chute.design_q > 1000.0);
        assert_eq!(d.stages.len(), 2);
        assert!(!d.craft.hottest.is_empty() && d.craft.hottest.len() <= HOTTEST);
        let names: Vec<_> = d.craft.hottest.iter().map(|h| h.0.as_str()).collect();
        assert!(names.iter().all(|n| !n.starts_with("part ")), "named parts: {names:?}");
        assert_eq!(d.target.choices.first().map(|c| c.0.as_str()), Some("none"));
        assert!(d.target.choices.iter().any(|c| c.0 == "Moon"));
        assert_eq!(d.diagnostics.vessels, 1);
    }

    #[test]
    fn the_entry_comes_from_the_marked_entry_a_periapsis_or_an_impact() {
        // (marked, periapsis inside, impact top, expected)
        let e = |top, in_s, periapsis| Some(Entry { top, in_s, periapsis });
        let table = [
            (None, None, None, None),
            (Some((312.0, 100e3)), None, Some(100e3), e(100e3, Some(312.0), None)),
            (None, Some((45e3, 100e3)), None, e(100e3, None, Some(45e3))),
            (None, None, Some(100e3), e(100e3, None, None)),
            (Some((60.0, 100e3)), Some((45e3, 90e3)), None, e(100e3, Some(60.0), Some(45e3))),
        ];
        for (marked, pe, impact, want) in table {
            assert_eq!(entry_ahead(marked, pe, impact), want, "{marked:?} {pe:?} {impact:?}");
        }
    }

    /// A deorbit: the stored coast marks where it enters the atmosphere, so
    /// Reentry has the entry interface and the time to it before any
    /// periapsis or impact is known.
    #[test]
    fn a_deorbit_coast_shows_the_time_to_entry() {
        let mut sim = SimState::new();
        let earth = sim.world.find("Earth").unwrap().clone();
        let re = earth.physical.as_ref().unwrap().radius_eq;
        let (rp, ra) = (re + 50e3, re + 300e3);
        let el = sim::kepler::Elements {
            a: 0.5 * (rp + ra),
            e: (ra - rp) / (ra + rp),
            i: 0.5,
            raan: 0.0,
            argp: 0.0,
            mean_anomaly: std::f64::consts::PI,
        };
        let (r, v) = el.to_state(earth.gm);
        let id = sim.vessel_ids.allocate();
        let mut ship =
            sim::vessel::Vessel::coasting(&sim.world, id, sim.clock, earth.node, r, v, sim::craft::test_craft());
        ship.extend_coast(&sim.world, sim.clock.add_seconds(3_000.0), 200_000);
        sim.fleet.push(ship);
        sim.active = 1;
        let f = fixture();
        let d = gather(&sources(&sim, &f));
        let entry = d.entry.expect("the coast enters the atmosphere");
        let top = earth.physical.as_ref().unwrap().atmosphere.as_ref().unwrap().top;
        assert_eq!(entry.top, top);
        let t = entry.in_s.expect("marked on the stored coast");
        assert!(t > 600.0 && t < 2_800.0, "half an orbit from apoapsis, before perigee: {t}");
        assert!(d.air.is_none());
    }

    #[test]
    fn in_orbit_the_elements_are_about_the_equator_and_a_vessel_target_has_a_relative_inclination() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(2);
        sim.active = 1;
        let f = fixture();
        let target = Some(NavTarget::Vessel(sim.fleet[2].id()));
        let d = gather(&Sources { target, ..sources(&sim, &f) });
        let el = d.elements.expect("an orbit");
        // spawn_test_ships: a = 6778 km, e = 0.001, i = 0.5 rad about the
        // ICRF equator (Earth's equator within ~0.2° at the test epoch).
        assert!((el.a - 6_778_137.0).abs() < 1.0, "{}", el.a);
        assert!((el.e - 0.001).abs() < 1e-6 && (el.inclination - 0.5f64.to_degrees()).abs() < 0.3, "{el:?}");
        assert!(el.period.is_some_and(|p| (p - 5_553.0).abs() < 10.0), "{:?}", el.period);
        let t = d.target.info.expect("a target");
        assert_eq!(t.name, "Vessel 3");
        assert!(t.distance > 1.0 && t.rel_speed > 1.0);
        // The second ship's orbit: i 0.55 rad, node 0.6 rad apart.
        let (a, b) = (&sim.fleet[1], &sim.fleet[2]);
        let (_, r1, v1) = a.state_at(&sim.world, sim.clock);
        let (_, r2, v2) = b.state_at(&sim.world, sim.clock);
        let want = plane(r1, v1).dot(plane(r2, v2)).acos().to_degrees();
        assert!(t.rel_inclination.is_some_and(|x| (x - want).abs() < 1e-6), "{:?} vs {want}", t.rel_inclination);
        assert!(d.craft.debug == a.debug() && !d.landed);
    }
}
