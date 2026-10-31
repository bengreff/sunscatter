//! The burn planner (realism-1 §6a): a draft of burns for a vessel, edited
//! here and sent to it as one command ([`command_for`]).
//!
//! A burn is a time and a Δv in prograde, normal and radial-out components
//! relative to a named body (a display choice of axes, rule 1). For a craft
//! of parts it is a node (D126: the time is the burn's middle), flown
//! closed-loop by the maneuver program (`sim::systems::maneuver`); the
//! window shows the sim's prediction of it (`Vessel::predict_node`) and the
//! line the coast after it (`trajectory::predict`), labelled as a
//! prediction: the stored trajectory is what was flown.
//!
//! From anywhere but aboard the vessel, the plan travels at light speed
//! (D063, D067); without a signal it cannot be sent.

mod view;

use crate::commands::{GameCommand, InFlight};
use crate::comms::{Comms, Location};
use crate::format::{delay as fmt_delay, distance as fmt_distance, duration as fmt_duration};
use crate::interface::toasts::Toasts;
use crate::state::SimState;
use crate::trajectory::apsides::{Apsides, VesselApsides};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::frame::NodeId;
use sim::systems::maneuver::Node;
use sim::time::Epoch;
use sim::vessel::{Vessel, VesselId};

/// One burn being edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftBurn {
    /// A node's time (the burn's middle).
    pub t_start: Epoch,
    /// Δv (m/s): prograde, normal, radial-out.
    pub dv: DVec3,
    /// The body the axes are relative to.
    pub reference: NodeId,
}

#[derive(Resource, Default)]
pub struct Planner {
    pub open: bool,
    /// The vessel the draft is for (the active one when opened).
    pub vessel: Option<VesselId>,
    pub draft: Vec<DraftBurn>,
    /// The handle being dragged.
    drag: Option<Drag>,
    /// Where the left button went down outside the windows (a click on the
    /// line is a press and release in place).
    press: Option<Vec2>,
}

/// A handle drag in progress: which burn and Δv component, the axis's
/// positive screen direction, where the drag started and the component's
/// value then.
#[derive(Clone, Copy, Debug)]
struct Drag {
    burn: usize,
    axis: usize,
    dir: Vec2,
    from: Vec2,
    dv0: f64,
}

/// Δv change (m/s) for a handle dragged `px` pixels outward (negative:
/// inward): cubic, so small drags trim by centimetres per second and long
/// ones reach kilometres per second (10 px → 1 cm/s, 100 px → 10 m/s,
/// 500 px → 1.25 km/s).
pub fn drag_dv(px: f64) -> f64 {
    10.0 * (px / 100.0).powi(3)
}

/// The draft form of a vessel's plan: its nodes that may still change.
pub fn draft_from(vessel: &Vessel, now: Epoch) -> Vec<DraftBurn> {
    let nodes = vessel.nodes().get(vessel.first_editable_node()..).unwrap_or_default();
    nodes
        .iter()
        .filter(|n| n.t.seconds_since(now) > 0.0)
        .map(|n| DraftBurn { t_start: n.t, dv: n.dv, reference: n.reference })
        .collect()
}

/// The nodes to send to a craft of parts: those that can no longer change
/// (flown, or being flown) followed by the draft's, in time order. Nodes
/// with no Δv are dropped.
pub fn nodes_from(vessel: &Vessel, draft: &[DraftBurn]) -> Vec<Node> {
    let kept = vessel.nodes().get(..vessel.first_editable_node()).unwrap_or(vessel.nodes());
    let mut new: Vec<Node> = draft
        .iter()
        .filter(|d| d.dv.length() > 1e-6)
        .map(|d| Node { t: d.t_start, dv: d.dv, reference: d.reference })
        .collect();
    new.sort_by(|a, b| a.t.seconds_since(b.t).total_cmp(&0.0));
    kept.iter().copied().chain(new).collect()
}

/// The command that sets `draft` on `vessel`: its nodes (a craft of parts
/// flies them; the sim refuses them for any other).
pub fn command_for(vessel: &Vessel, draft: &[DraftBurn]) -> GameCommand {
    GameCommand::SetNodes { vessel: vessel.id(), nodes: nodes_from(vessel, draft) }
}

/// What the window shows of one draft burn.
#[derive(Clone, Debug, PartialEq)]
pub struct BurnInfo {
    /// Its ignition (a node's from the program's schedule).
    pub ignite: Epoch,
    /// Its burn time (s), if the engines' thrust is known.
    pub duration: Option<f64>,
    /// When it ends (the first apsides after it are shown from there).
    pub end: Epoch,
    pub warnings: Vec<String>,
}

/// What the draft's burns predict: each node's prediction
/// (`Vessel::predict_node`, one owner: the stage estimate's duration and
/// warnings), and the Δv the craft has (m/s), if known.
pub fn draft_info(world: &sim::world::World, vessel: &Vessel, draft: &[DraftBurn]) -> (Vec<BurnInfo>, Option<f64>) {
    // The draft's first node is the next to fly unless one is being flown.
    let flying = vessel.program().is_some_and(|p| p.step.flying());
    let mut available = None;
    let info = draft
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let node = Node { t: b.t_start, dv: b.dv, reference: b.reference };
            let p = vessel.predict_node(world, &node, k == 0 && !flying, None);
            if k == 0 {
                available = p.as_ref().and_then(|p| p.available);
            }
            let duration = p.as_ref().and_then(|p| p.duration);
            let ignite = p.as_ref().map_or(b.t_start, |p| p.ignite);
            let end = ignite.add_seconds(duration.unwrap_or(0.0));
            BurnInfo { ignite, duration, end, warnings: p.map(|p| p.warnings).unwrap_or_default() }
        })
        .collect();
    (info, available)
}

/// The first apsides after a burn ends, from the predicted trajectory
/// (D076); only once the draft is the plan the trajectory was computed with.
fn after_burn(sim: &SimState, list: &VesselApsides, end: Epoch, predicted: bool) -> String {
    if !predicted {
        return "set the plan to see Ap/Pe after it".into();
    }
    let (ap, pe) = list.next_after(end);
    let show = |a: Option<&crate::trajectory::apsides::Apsis>| {
        a.map_or_else(
            || "-".to_string(),
            |a| format!("{} ({})", fmt_distance(a.altitude), sim.world.eph.node(a.body).name),
        )
    };
    match list.impact.filter(|m| m.t.seconds_since(end) > 0.0 && pe.is_none_or(|p| p.t.seconds_since(m.t) > 0.0)) {
        Some(m) => format!("then Ap {}, impact on {}", show(ap), sim.world.eph.node(m.body).name),
        None => format!("then Ap {}, Pe {}", show(ap), show(pe)),
    }
}

/// A first-guess intercept of vessel `j` by vessel `i` (D008: two-body
/// Lambert arcs about `i`'s dominant body, departures over the next orbit
/// and transfer times of 0.1–1.5 orbits, the cheapest kept). Positions come
/// from both stored trajectories; the burn is then flown and drawn by the
/// N-body sim, so the planner shows how close it really gets.
pub fn intercept(sim: &SimState, i: usize, j: usize) -> Option<DraftBurn> {
    let (a, b) = (sim.fleet[i].trajectory()?, sim.fleet[j].trajectory()?);
    let body = sim.dominant_of(i);
    let mu = sim.world.source(body)?.gm;
    let (anchor, r, v) = sim.fleet[i].state_at(&sim.world, sim.clock);
    let k = sim.world.snapshot(sim.clock).relative(body, anchor);
    let el = sim::kepler::Elements::from_state(r - k.r, v - k.v, mu);
    let period = if el.e < 1.0 { std::f64::consts::TAU / el.mean_motion(mu) } else { 3600.0 };
    let rel = |tr: &sim::vessel::Trajectory, t: Epoch| {
        let (n, r, v) = tr.eval(t)?;
        let k = sim.world.snapshot(t).relative(body, n);
        Some((r - k.r, v - k.v))
    };
    let mut best: Option<(f64, Epoch, DVec3, DVec3, DVec3)> = None;
    for kd in 1..=24 {
        let t_dep = sim.clock.add_seconds((period * f64::from(kd) / 24.0).max(60.0));
        let Some((r1, v1)) = rel(a, t_dep) else { continue };
        for kt in 0..24 {
            let tof = period * (0.1 + 1.4 * f64::from(kt) / 23.0);
            let Some((r2, _)) = rel(b, t_dep.add_seconds(tof)) else { continue };
            let Some((w1, _)) = sim::kepler::lambert::lambert(r1, r2, tof, mu, r1.cross(v1)) else { continue };
            let dv = (w1 - v1).length();
            if best.is_none_or(|b| dv < b.0) {
                best = Some((dv, t_dep, w1 - v1, r1, v1));
            }
        }
    }
    let (_, t_start, dv, r1, v1) = best?;
    let (p, n, rad) = sim::vessel::prograde_normal_radial(r1, v1);
    Some(DraftBurn { t_start, dv: DVec3::new(dv.dot(p), dv.dot(n), dv.dot(rad)), reference: body })
}

/// The time of the point on a drawn run nearest to `cursor` on screen, if
/// within `max_px`, and its distance: `run` is (time, screen point) in order
/// along the line; the time is interpolated along the nearest piece.
pub fn nearest_on_screen(run: &[(Epoch, Vec2)], cursor: Vec2, max_px: f32) -> Option<(Epoch, f32)> {
    let mut best: Option<(Epoch, f32)> = None;
    for w in run.windows(2) {
        let ((ta, a), (tb, b)) = (w[0], w[1]);
        let ab = b - a;
        let f =
            if ab.length_squared() > 0.0 { ((cursor - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let d = cursor.distance(a + ab * f);
        if d <= max_px && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((ta.add_seconds(f64::from(f) * tb.seconds_since(ta)), d));
        }
    }
    best
}

pub use view::pick_on_line;

impl Planner {
    /// Opens the planner on `vessel`'s plan (the Orbit menu's Maneuvers).
    /// Already open on it, the draft is kept: unsent edits are not lost.
    pub fn open_for(&mut self, vessel: &Vessel, now: Epoch) {
        if self.open && self.vessel == Some(vessel.id()) {
            return;
        }
        self.open = true;
        self.vessel = Some(vessel.id());
        self.draft = draft_from(vessel, now);
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    sim: Res<SimState>,
    comms: Res<Comms>,
    nav: Res<crate::navball::Navball>,
    mut planner: ResMut<Planner>,
    mut in_flight: ResMut<InFlight>,
    mut toasts: ResMut<Toasts>,
    mut commands: MessageWriter<GameCommand>,
    aps: Res<Apsides>,
) -> Result {
    if !planner.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let Some(id) = planner.vessel else { return Ok(()) };
    let Some(i) = sim.index_of(id) else {
        planner.open = false;
        return Ok(());
    };
    let vessel = &sim.fleet[i];
    let now = sim.clock;
    let body = sim.dominant_of(i);
    let body_name = sim.world.eph.node(body).name.clone();
    // The next apsides on the predicted trajectory (D076), and whether it
    // shows this draft (the plan set on the vessel).
    let list = aps.get(id).cloned().unwrap_or_default();
    let (next_ap, next_pe) = list.next_after(now);
    let (next_ap, next_pe) = (next_ap.copied(), next_pe.copied());
    let predicted = planner.draft == draft_from(vessel, now);
    let (info, available) = draft_info(&sim.world, vessel, &planner.draft);
    let mut open = planner.open;
    let mut send = false;
    egui::Window::new(format!("Burn planner: {} (N)", crate::tracking::vessel_name(id)))
        .open(&mut open)
        .default_width(380.0)
        .show(ctx, |ui| {
            let mut remove = None;
            let mut total = 0.0;
            for ((k, b), info) in planner.draft.iter_mut().enumerate().zip(&info) {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(format!("Burn {}", k + 1));
                    let mut dt = b.t_start.seconds_since(now);
                    ui.label("node in");
                    if ui.add(egui::DragValue::new(&mut dt).range(1.0..=1e9).speed(1.0).suffix(" s")).changed() {
                        b.t_start = now.add_seconds(dt);
                    }
                    ui.label(fmt_duration(dt));
                    if ui.small_button("✖").clicked() {
                        remove = Some(k);
                    }
                });
                let name = sim.world.eph.node(b.reference).name.clone();
                ui.horizontal(|ui| {
                    for (label, v) in [("pro", &mut b.dv.x), ("nrm", &mut b.dv.y), ("rad", &mut b.dv.z)] {
                        ui.label(label);
                        ui.add(egui::DragValue::new(v).speed(0.5).suffix(" m/s"));
                    }
                });
                let dv = b.dv.length();
                let time =
                    info.duration.map_or_else(|| "burn time unknown".into(), |t| format!("burn {}", fmt_duration(t)));
                let ignite = fmt_duration(info.ignite.seconds_since(now));
                let text = format!("predicted: {dv:.1} m/s about {name}, ignition in {ignite}, {time}");
                ui.label(egui::RichText::new(text).small());
                ui.label(egui::RichText::new(after_burn(&sim, &list, info.end, predicted)).small());
                for text in &info.warnings {
                    ui.label(egui::RichText::new(text).small().color(crate::interface::theme::WARN));
                }
                total += dv;
            }
            if let Some(k) = remove {
                planner.draft.remove(k);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let mut add = |t: Epoch, reference: NodeId| {
                    planner.draft.push(DraftBurn { t_start: t, dv: DVec3::ZERO, reference });
                    planner.draft.sort_by(|a, b| a.t_start.seconds_since(b.t_start).total_cmp(&0.0));
                };
                if ui.add_enabled(next_ap.is_some(), egui::Button::new("+ at next Ap")).clicked() {
                    next_ap.inspect(|a| add(a.t, a.body));
                }
                if ui.add_enabled(next_pe.is_some(), egui::Button::new("+ at next Pe")).clicked() {
                    next_pe.inspect(|a| add(a.t, a.body));
                }
                if ui.button("+ in 10 min").clicked() {
                    add(now.add_seconds(600.0), body);
                }
            });
            ui.label(egui::RichText::new("or click the line; drag a burn's handles to shape it").small());
            let target = match nav.target {
                Some(crate::navball::NavTarget::Vessel(t)) => sim.index_of(t),
                _ => None,
            };
            let button = egui::Button::new("+ intercept the target (first guess)");
            if ui.add_enabled(target.is_some(), button).clicked() {
                match target.and_then(|j| intercept(&sim, i, j)) {
                    Some(b) => planner.draft.push(b),
                    None => toasts.push(time.elapsed_secs_f64(), "No intercept found in the computed lines", true),
                }
            }
            ui.label(format!("axes about {body_name} (prograde, normal, radial-out)"));
            let short = available.is_some_and(|a| total > a);
            let colour =
                if short && !vessel.debug() { crate::interface::theme::WARN } else { crate::interface::theme::TEXT };
            let of = available
                .map_or_else(|| "(the craft's Δv is not measured)".into(), |a| format!("of {a:.0} m/s available"));
            ui.label(egui::RichText::new(format!("total {total:.0} m/s {of}")).color(colour));
            let delay = plan_delay(&comms, id);
            ui.horizontal(|ui| {
                let label = match delay {
                    Some(0.0) => "Set plan".to_string(),
                    Some(d) => format!("Send plan ({} away)", fmt_delay(d)),
                    None => "No signal".into(),
                };
                send = ui.add_enabled(delay.is_some(), egui::Button::new(label)).clicked();
                if ui.button("Revert").clicked() {
                    planner.draft = draft_from(vessel, now);
                }
            });
        });
    planner.open = open;
    if send {
        let delay = plan_delay(&comms, id).unwrap_or(0.0);
        let cmd = command_for(vessel, &planner.draft);
        if delay > 0.0 {
            in_flight.send(now, delay, cmd);
            toasts.push(
                time.elapsed_secs_f64(),
                format!("Plan sent to {} ({})", crate::tracking::vessel_name(id), fmt_delay(delay)),
                false,
            );
        } else {
            commands.write(cmd);
        }
    }
    Ok(())
}

/// How long a plan takes to reach vessel `id` (s; `None`: no signal), by
/// the one routing rule (`commands::route`, D067).
fn plan_delay(comms: &crate::comms::Comms, id: VesselId) -> Option<f64> {
    let aboard = comms.location == Location::Vessel(id);
    match crate::commands::route(aboard, comms.signal(id).map(|s| s.delay)) {
        crate::commands::Route::Now => Some(0.0),
        crate::commands::Route::After(d) => Some(d),
        crate::commands::Route::NoSignal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_intercept_between_two_test_ships_is_found_and_affordable() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(2);
        let until = sim.clock.add_seconds(40_000.0);
        let world = sim.world.clone();
        for v in &mut sim.fleet {
            v.extend_coast(&world, until, 200_000);
        }
        let b = intercept(&sim, 1, 2).expect("an intercept");
        assert!(b.t_start.seconds_since(sim.clock) >= 60.0);
        let dv = b.dv.length();
        assert!(dv > 1.0 && dv < 3000.0, "{dv}");
    }

    #[test]
    fn a_click_picks_the_nearest_point_on_the_line_within_reach() {
        let t0 = sim::sol::sol_epoch();
        let samples: Vec<(Epoch, Vec2)> =
            (0..10).map(|k| (t0.add_seconds(f64::from(k)), Vec2::new(10.0 * k as f32, 0.0))).collect();
        // Between samples 4 and 5, a tenth of the way: interpolated.
        let (t, d) = nearest_on_screen(&samples, Vec2::new(41.0, 3.0), 8.0).expect("near");
        assert!((t.seconds_since(t0) - 4.1).abs() < 1e-6 && (d - 3.0).abs() < 1e-6);
        assert_eq!(nearest_on_screen(&samples, Vec2::new(41.0, 30.0), 8.0), None);
        // Past the end: the end point, if within reach.
        let (t, _) = nearest_on_screen(&samples, Vec2::new(95.0, 0.0), 8.0).expect("near the end");
        assert_eq!(t, t0.add_seconds(9.0));
    }

    #[test]
    fn handle_drags_map_to_delta_v_cubically() {
        // (pixels, m/s): fine near the node, coarse far out, odd.
        let table =
            [(0.0, 0.0), (10.0, 0.01), (50.0, 1.25), (100.0, 10.0), (200.0, 80.0), (500.0, 1250.0), (-100.0, -10.0)];
        for (px, dv) in table {
            assert!((drag_dv(px) - dv).abs() < 1e-9, "{px} px: {}", drag_dv(px));
        }
        assert!((1..600).all(|k| drag_dv(f64::from(k)) > drag_dv(f64::from(k - 1))), "monotonic");
    }

    #[test]
    fn opening_the_planner_again_keeps_unsent_edits() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(1);
        let (a, b) = (&sim.fleet[0], &sim.fleet[1]);
        let mut p = Planner::default();
        p.open_for(a, sim.clock);
        assert!(p.open && p.vessel == Some(a.id()) && p.draft.is_empty());
        let earth = sim.world.find("Earth").unwrap().node;
        let edit = DraftBurn { t_start: sim.clock.add_seconds(600.0), dv: DVec3::X * 10.0, reference: earth };
        p.draft.push(edit);
        p.open_for(a, sim.clock);
        assert_eq!(p.draft, [edit], "open on the same vessel: kept");
        p.open_for(b, sim.clock);
        assert!(p.vessel == Some(b.id()) && p.draft.is_empty(), "another vessel: its own plan");
        p.open = false;
        p.vessel = Some(a.id());
        p.draft = vec![edit];
        p.open_for(a, sim.clock);
        assert!(p.draft.is_empty(), "closed, then opened: the vessel's plan again");
    }

    /// The small rocket of parts coasting in low orbit, its coast computed
    /// an hour ahead.
    fn parts_in_orbit() -> SimState {
        let craft = sim::craft::small::rocket();
        let mut sim = SimState::with_craft(&craft);
        let earth = sim.world.find("Earth").unwrap();
        let el = sim::kepler::Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(earth.gm);
        let mut ship = Vessel::coasting(&sim.world, sim.fleet[0].id(), sim.clock, earth.node, r, v, &craft);
        ship.extend_coast(&sim.world, sim.clock.add_seconds(3600.0), usize::MAX);
        sim.fleet[0] = ship;
        sim
    }

    #[test]
    fn a_craft_of_parts_gets_nodes_that_round_trip_and_keep_the_one_being_flown() {
        let mut sim = parts_in_orbit();
        let now = sim.clock;
        let body = sim.world.find("Earth").unwrap().node;
        let draft = vec![
            DraftBurn { t_start: now.add_seconds(1500.0), dv: DVec3::new(50.0, 0.0, 5.0), reference: body },
            DraftBurn { t_start: now.add_seconds(900.0), dv: DVec3::new(40.0, 0.0, 0.0), reference: body },
            DraftBurn { t_start: now.add_seconds(1200.0), dv: DVec3::ZERO, reference: body },
        ];
        let GameCommand::SetNodes { vessel, nodes } = command_for(sim.ship(), &draft) else {
            panic!("a craft of parts is sent nodes")
        };
        assert_eq!(vessel, sim.ship().id());
        assert_eq!(nodes.len(), 2, "zero Δv is dropped");
        assert!(nodes[0].t < nodes[1].t, "in time order");
        sim.fleet[0].set_nodes(nodes.clone()).unwrap();
        let back = draft_from(sim.ship(), now);
        assert_eq!(back.len(), 2);
        assert_eq!((back[0].t_start, back[0].dv), (nodes[0].t, nodes[0].dv));
        assert_eq!(nodes_from(sim.ship(), &back), nodes, "the draft of the plan is the plan");
        // Woken for the first node, the program turns to it: it can no
        // longer change, and a new draft keeps it at the front.
        let wake = sim.ship().program().unwrap().wake;
        let world = sim.world.clone();
        sim.fleet[0].advance(&world, wake.add_seconds(1.0), &sim::vessel::Controls::default(), usize::MAX);
        assert!(sim.ship().program().is_some_and(|p| p.step.flying()));
        let later = draft_from(sim.ship(), sim.clock);
        assert_eq!(later.len(), 1, "only the second node is editable");
        let edited = [DraftBurn { dv: DVec3::new(20.0, 0.0, 0.0), ..later[0] }];
        let sent = nodes_from(sim.ship(), &edited);
        assert_eq!(sent[0], nodes[0]);
        assert_eq!(sent[1].dv, DVec3::new(20.0, 0.0, 0.0));
        sim.fleet[0].set_nodes(sent).expect("the edit is accepted");
    }

    #[test]
    fn the_window_shows_the_sims_prediction_of_each_node() {
        let sim = parts_in_orbit();
        let now = sim.clock;
        let body = sim.world.find("Earth").unwrap().node;
        let soon = DraftBurn { t_start: now.add_seconds(10.0), dv: DVec3::new(40.0, 0.0, 0.0), reference: body };
        let later = DraftBurn { t_start: now.add_seconds(900.0), ..soon };
        let (info, available) = draft_info(&sim.world, sim.ship(), &[soon, later]);
        // The prediction (`predict_node`): the stage estimate's burn time
        // about the node, its ignition before it, and the warnings.
        for (b, d) in info.iter().zip([soon, later]) {
            let t = b.duration.expect("the engines' thrust is known");
            assert!(t > 0.1 && t < 10.0, "{t}");
            assert!(b.ignite < d.t_start && d.t_start < b.end, "{b:?}");
            assert!(b.warnings.iter().any(|w| w.contains("cannot be settled")), "{:?}", b.warnings);
        }
        assert!(info[0].warnings.iter().any(|w| w.contains("too soon")), "{:?}", info[0].warnings);
        assert!(!info[1].warnings.iter().any(|w| w.contains("too soon")), "only the first can come too soon");
        // The small rocket reads no tank levels: its Δv is not measured.
        assert_eq!(available, None);
    }
}
