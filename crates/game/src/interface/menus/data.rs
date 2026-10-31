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
//! the canopies from `staging`; the RCS from `rcs_data`, the crew and power from
//! their menus' modules.

use crate::comms::{Comms, Signal};
use crate::landing::Landing;
use crate::navball::{ApsisReadout, NavState, NavTarget};
use crate::rendezvous::Rendezvous;
use crate::state::SimState;
use crate::tracking::Tracked;
use crate::trajectory::apsides::Apsides;
use bevy::prelude::*;
use sim::vessel::{Phase, VesselId};

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
    /// The ship computer's programs (`systems_screen::programs`: each
    /// one's switch, what it does now, its press).
    pub programs: Vec<crate::systems_screen::programs::Row>,
    pub surface: Option<Surface>,
    pub impact: Option<ImpactData>,
    /// The impact is still being predicted: no prediction of this flight
    /// yet (just after a load or a switch of vessel).
    pub predicting: bool,
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
    /// Its parachutes' canopies in their sequence (`staging::canopies`;
    /// none for a craft without parachutes).
    pub canopies: Vec<crate::staging::Part>,
    /// The RCS menu's (`rcs_data`), the Crew and Power menus' (a craft of
    /// parts with a cabin, with buses).
    pub rcs: super::rcs::RcsData,
    pub crew: Option<super::crew::CrewData>,
    pub power: Option<super::power::PowerData>,
    pub diagnostics: Diagnostics,
    /// What the craft knows (D106): every readout through it.
    pub known: crate::format::Known,
    /// The clock it was gathered at (the agent's `read {menu}` waits for
    /// the stopped clock's, `agent::read::fresh`).
    pub clock: Option<sim::time::Epoch>,
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
    /// Its number, as the ship's log counts burns (from 1).
    pub number: usize,
    /// A node's maneuver program while it flies the craft (turning,
    /// settling, burning).
    pub step: Option<sim::systems::maneuver::Step>,
}

/// Motion relative to the nearest surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    /// Lowest contact point above the ground (m).
    pub radar: f64,
    pub v_vertical: f64,
    pub v_horizontal: f64,
    /// Full thrust at the ambient pressure against local gravity, to the
    /// precision the craft knows its mass ([`local_twr`]).
    pub twr: sim::systems::Reading,
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
    /// The skin cell nearest its own limit, with its limit (K).
    pub skin: (f64, f64),
    /// The shock layer at the nose this tick (D109), and the highest
    /// stagnation flux so far (W/m²).
    pub shock: Option<sim::vessel::ShockLayer>,
    pub peak_heating: f64,
    /// The measured parts nearest their limits, and how many no sensor
    /// measures (`hottest`, the systems screen's rule).
    pub hottest: super::hottest::Hottest,
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
    /// The plasma sheath cuts the link (D109): its plasma frequency and the
    /// link's (Hz).
    pub blackout: Option<(f64, f64)>,
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

/// Whether the snapshot is refreshed at `now`, last taken at `last.0` of
/// flight `last.1`: every [`REFRESH`] seconds, and at once for another
/// flight (`SimState::flight`: a load, a switch of vessel), so no menu
/// shows the last one's numbers.
fn refresh_due<F: PartialEq>(now: f64, last: (f64, Option<F>), flight: F) -> bool {
    now - last.0 >= REFRESH || last.1 != Some(flight)
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
    mut last: Local<(f64, Option<(VesselId, u64)>)>,
) {
    let now = time.elapsed_secs_f64();
    if station.open || !refresh_due(now, *last, sim.flight()) {
        return;
    }
    *last = (now, Some(sim.flight()));
    let sources = Sources {
        sim: &sim,
        pred: &pred,
        nav: nav.now.as_ref(),
        target: nav.target,
        aps: &aps,
        landing: landing.of(sim.flight()),
        rv: &rv,
        comms: &comms,
        tracked: &tracked,
        fps: fps.text(),
    };
    let mut d = gather(&sources);
    d.diagnostics.profile.clone_from(&profile.summary);
    steady(&mut d, &data, &sim);
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
        programs: crate::systems_screen::programs::Inputs::of(ship, sim.clock)
            .map_or_else(Vec::new, |x| crate::systems_screen::programs::rows(&x)),
        g_load: nav.map_or(0.0, |n| n.g.load),
        attitude: nav.map(|n| [n.angles.heading, n.angles.pitch, n.angles.roll]),
        craft: craft(sim),
        comms: comms(s),
        canopies: if ship.craft.design().parts.iter().any(|p| p.chute) {
            crate::staging::canopies(&ship.craft.chute, &ship.chute)
        } else {
            Vec::new()
        },
        rcs: super::rcs_data::gather(ship, sim.controls().rcs),
        crew: super::crew::gather(ship),
        power: super::power::gather(ship),
        known: crate::format::Known::of(ship),
        clock: Some(sim.clock),
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
    choices.extend(crate::navball::target_choices(sim).into_iter().map(|t| {
        let name = match t {
            NavTarget::Body(node) => sim.world.source(node).map_or_else(String::new, |src| src.name.clone()),
            NavTarget::Vessel(id) => s.tracked.name(id),
        };
        (name, Some(t))
    }));
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

/// The planned burns ahead: the node being flown, then the nodes not yet
/// burnt, with the sim's prediction of each (`Vessel::predict_node`, as
/// the planner shows them).
fn burns(sim: &SimState) -> Vec<Burn> {
    let ship = sim.ship();
    let program = ship.program().filter(|p| p.step.flying());
    let unburnt = ship.nodes_unburnt();
    let first = ship.nodes().len() - unburnt.len();
    let burning = program.filter(|p| p.node < first).map(|p| p.node);
    (burning.into_iter().chain(first..ship.nodes().len()))
        .map(|i| {
            let n = &ship.nodes()[i];
            let next = program.is_none() && i == first;
            let p = ship.predict_node(&sim.world, n, next, None);
            // The program's own schedule for the node it flies.
            let flying = program.filter(|p| p.node == i);
            let ignite = flying.map_or_else(|| p.as_ref().map_or(n.t, |p| p.ignite), |p| p.ignite);
            let step = flying.map(|p| p.step);
            let duration = p.and_then(|p| p.duration).unwrap_or(0.0);
            let in_s = ignite.seconds_since(sim.clock);
            Burn { dv: Some(n.dv.length()), in_s, duration, number: i + 1, step }
        })
        .collect()
}

/// Vessel `v`'s thrust-to-weight ratio at full throttle of the engines its
/// throttle runs, at ambient pressure `pressure` against local gravity
/// `gravity` (`sim::landing::twr`, the braking burn's), to the precision it
/// knows its mass: the staging bar's (`stage_bar::aboard_of`), not dashed
/// whole when a tank is unmeasured (finding 41's rule). The Landing menu's
/// and MCP's `get_landing_prediction`.
pub fn local_twr(v: &sim::vessel::Vessel, pressure: f64, gravity: f64) -> sim::systems::Reading {
    let value = sim::landing::twr(v, pressure, gravity);
    let share = crate::interface::stage_bar::aboard_of(v)
        .filter(|a| a.mass.value > 0.0)
        .map_or(0.0, |a| a.mass.precision / a.mass.value);
    sim::systems::Reading { value, precision: value.abs() * share }
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
            twr: local_twr(ship, m.pressure, m.gravity),
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
        commanded: sim.controls().chute,
        state: ship.chute,
        q: air.as_ref().map(sim::vessel::Air::q),
        design_q: spec.drogue.as_ref().unwrap_or(&spec.main).deploy_max_q,
    };
    FlightData {
        surface,
        impact,
        predicting: s.landing.is_none() && !grounded,
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

/// Nothing flickers (`interface::steady`): the hottest parts change only
/// past a margin over those listed before.
pub fn steady(d: &mut FlightData, before: &FlightData, sim: &SimState) {
    let listed: Vec<String> = before.craft.hottest.parts.iter().map(|h| h.0.clone()).collect();
    d.craft.hottest = super::hottest::listed(sim.ship(), &listed, HOTTEST);
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

fn craft(sim: &SimState) -> CraftData {
    let ship = sim.ship();
    CraftData {
        shock: ship.thermal().shock,
        peak_heating: ship.thermal().peak_heating,
        debug: ship.debug(),
        skin: ship.skin_nearest_limit(),
        hottest: super::hottest::listed(ship, &[], HOTTEST),
        clock_offset: sim.ship_clock_offset(sim.active),
    }
}

fn comms(s: &Sources) -> CommsData {
    let ship = s.sim.ship();
    CommsData {
        home: s.comms.data.sites[s.comms.mission_control].name.clone(),
        link: s.comms.home.clone(),
        probe: (ship.crew() == 0).then(|| s.comms.signal(ship.id()).map(|sig| sig.delay)),
        blackout: crate::comms::sheath_cut(ship, s.comms.data.link.frequency_hz),
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

    /// The snapshot refreshes every REFRESH seconds, and at once for
    /// another flight (a load gives the vessel's id another timeline).
    #[test]
    fn the_snapshot_refreshes_on_time_and_for_another_flight() {
        let a = (VesselId(1), 0_u64);
        // (now, last taken, of flight, flight now, due)
        let cases = [
            (0.0, 0.0, None, a, true),
            (REFRESH / 2.0, 0.0, Some(a), a, false),
            (REFRESH, 0.0, Some(a), a, true),
            (REFRESH / 2.0, 0.0, Some(a), (VesselId(1), 1), true),
            (REFRESH / 2.0, 0.0, Some(a), (VesselId(2), 0), true),
        ];
        for (now, at, of, flight, due) in cases {
            assert_eq!(refresh_due(now, (at, of), flight), due, "{now} {at} {of:?} {flight:?}");
        }
    }

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

    /// The local TWR is known to the precision of the mass (Dragon's
    /// unmeasured tanks widen it, as on the staging bar), exactly in debug.
    #[test]
    fn the_local_twr_reads_to_the_precision_of_the_mass() {
        if !crate::state::craft_exists("falcon9-crew") {
            return;
        }
        let mut sim = SimState::with_craft(&crate::state::craft_or_fallback("falcon9-crew"));
        let twr = local_twr(sim.ship(), 101_325.0, 9.81);
        assert!(twr.value > 0.0 && twr.precision > 0.0 && twr.precision < 0.05 * twr.value, "{twr:?}");
        assert_eq!(twr.value, sim::landing::twr(sim.ship(), 101_325.0, 9.81));
        let (world, active) = (sim.world.clone(), sim.active);
        sim.fleet[active].set_debug(&world, true);
        assert_eq!(local_twr(sim.ship(), 101_325.0, 9.81).precision, 0.0);
    }

    #[test]
    fn on_the_pad_the_snapshot_is_landed_with_its_tanks_loaded_and_stowed_chutes() {
        let sim = SimState::new();
        let f = fixture();
        let d = gather(&sources(&sim, &f));
        assert_eq!(d.body, "Earth");
        assert!(d.landed && d.impact.is_none() && d.braking.is_none() && d.air.is_none());
        let s = d.surface.expect("over Earth");
        assert!(s.radar.abs() < 1.0, "on its feet: {}", s.radar);
        assert!(s.twr.value > 1.0, "the small rocket lifts off: {:?}", s.twr);
        assert!(!d.chute.commanded && d.chute.design_q > 1000.0);
        let chutes = sim.ship().craft.design().parts.iter().any(|p| p.chute);
        assert_eq!(d.canopies.is_empty(), !chutes);
        // No temperature sensor on the small rocket: every part counted,
        // none ranked by the truth (D106).
        assert!(d.craft.hottest.parts.is_empty() && d.craft.hottest.unmeasured > 0, "{:?}", d.craft.hottest);
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
            sim::vessel::Vessel::coasting(&sim.world, id, sim.clock, earth.node, r, v, sim::craft::small::rocket_ref());
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
