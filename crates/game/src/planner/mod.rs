//! The burn planner (realism-1 §6a): a draft of burns for a vessel, edited
//! here and sent to it as one `GameCommand::SetPlan`.
//!
//! A burn is its ignition time and a Δv in prograde, normal and radial-out
//! components relative to a named body (a display choice of axes: the law is
//! a direction function of the state, rule 1). The sim integrates the plan
//! with the same laws it flies, so the drawn prediction is what happens
//! (burns under any warp, D024, D029).
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
use sim::time::Epoch;
use sim::vessel::{BurnEnd, DirectionLaw, FlightPlan, PlannedBurn, Vessel, VesselId, G0};

/// One burn being edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftBurn {
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

/// The draft form of a vessel's plan: the burns not yet ignited.
pub fn draft_from(plan: &FlightPlan, now: Epoch) -> Vec<DraftBurn> {
    plan.burns
        .iter()
        .filter(|b| b.t_start.seconds_since(now) > 0.0)
        .filter_map(|b| match (b.law.direction, b.end) {
            (DirectionLaw::Tracking { reference, axes }, BurnEnd::DeltaV(dv)) => {
                Some(DraftBurn { t_start: b.t_start, dv: axes.normalize_or_zero() * dv, reference })
            }
            _ => None,
        })
        .collect()
}

/// The plan to send: the vessel's burns already ignited (kept as they
/// are: they cannot change) followed by the draft, sorted by ignition.
/// Burns with no Δv are dropped.
pub fn plan_from(vessel: &Vessel, draft: &[DraftBurn], now: Epoch) -> FlightPlan {
    let mut burns: Vec<PlannedBurn> =
        vessel.plan().burns.iter().filter(|b| b.t_start.seconds_since(now) <= 0.0).copied().collect();
    let mut new: Vec<PlannedBurn> = draft
        .iter()
        .filter(|d| d.dv.length() > 1e-6)
        .map(|d| PlannedBurn::delta_v_with(d.t_start, d.dv, &vessel.craft.engine, Some(d.reference)))
        .collect();
    new.sort_by(|a, b| a.t_start.seconds_since(b.t_start).total_cmp(&0.0));
    burns.extend(new);
    FlightPlan { burns }
}
/// A warning when a burn `until` seconds away is sooner than the turn the
/// maneuver hold needs before it (`lead`, s): the burn then starts before
/// the craft is aligned.
pub fn turn_warning(until: f64, lead: f64) -> Option<String> {
    (until < lead).then(|| format!("turning takes ~{lead:.0} s: ignition before the craft is aligned"))
}

/// Burn time (s) for `dv` from mass `m0` at vacuum thrust and Isp.
pub fn burn_time(dv: f64, m0: f64, thrust: f64, isp: f64) -> f64 {
    let mdot = thrust / (isp * G0);
    m0 * (1.0 - (-dv / (isp * G0)).exp()) / mdot
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

/// Toggles the planner (N) for the active vessel.
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    ctx: Res<crate::commands::InputContext>,
    sim: Res<SimState>,
    mut planner: ResMut<Planner>,
) {
    if ctx.allows(crate::commands::Keys::Menus) && keys.just_pressed(KeyCode::KeyN) {
        planner.open = !planner.open;
        if planner.open {
            let v = sim.ship();
            planner.vessel = Some(v.id());
            planner.draft = draft_from(v.plan(), sim.clock);
        }
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
    let engine = &vessel.craft.engine;
    let now = sim.clock;
    let body = sim.dominant_of(i);
    let body_name = sim.world.eph.node(body).name.clone();
    // The next apsides on the predicted trajectory (D076), and whether it
    // shows this draft (the plan set on the vessel).
    let list = aps.get(id).cloned().unwrap_or_default();
    let (next_ap, next_pe) = list.next_after(now);
    let (next_ap, next_pe) = (next_ap.copied(), next_pe.copied());
    let predicted = planner.draft == draft_from(vessel.plan(), now);
    let mut open = planner.open;
    let mut send = false;
    egui::Window::new(format!("Burn planner: Vessel {} (N)", id.0)).open(&mut open).default_width(380.0).show(
        ctx,
        |ui| {
            let mut remove = None;
            let mut mass = vessel.mass_props().mass;
            let available = crate::hud::delta_v(engine.isp_vac, mass, vessel.propellant());
            let mut total = 0.0;
            for (k, b) in planner.draft.iter_mut().enumerate() {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(format!("Burn {}", k + 1));
                    let mut dt = b.t_start.seconds_since(now);
                    ui.label("in");
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
                let t = burn_time(dv, mass, engine.thrust_vac, engine.isp_vac);
                ui.label(egui::RichText::new(format!("{dv:.1} m/s about {name}, burn {}", fmt_duration(t))).small());
                ui.label(egui::RichText::new(after_burn(&sim, &list, b.t_start.add_seconds(t), predicted)).small());
                // The maneuver hold turns the craft this long before
                // ignition (D075); a burn sooner than that starts unaligned.
                let lead = sim::vessel::BurnCraft::of(&vessel.craft).turn_lead(mass);
                if let Some(text) = turn_warning(b.t_start.seconds_since(now), lead) {
                    ui.label(egui::RichText::new(text).small().color(crate::interface::theme::WARN));
                }
                mass *= (-dv / (engine.isp_vac * G0)).exp();
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
            let short = total > available;
            let colour =
                if short && !vessel.debug() { crate::interface::theme::WARN } else { crate::interface::theme::TEXT };
            ui.label(
                egui::RichText::new(format!("total {total:.0} m/s of {available:.0} m/s available")).color(colour),
            );
            let m0 = vessel.mass_props().mass;
            ui.label(
                egui::RichText::new(format!(
                    "propellant {:.0} kg of {:.0} kg on board",
                    m0 - mass,
                    vessel.propellant()
                ))
                .color(colour),
            );
            let signal = comms.signal(id);
            let delay = if comms.location == Location::Vessel(id) { Some(0.0) } else { signal.map(|s| s.delay) };
            ui.horizontal(|ui| {
                let label = match delay {
                    Some(0.0) => "Set plan".to_string(),
                    Some(d) => format!("Send plan ({} away)", fmt_delay(d)),
                    None => "No signal".into(),
                };
                send = ui.add_enabled(delay.is_some(), egui::Button::new(label)).clicked();
                if ui.button("Revert").clicked() {
                    planner.draft = draft_from(vessel.plan(), now);
                }
            });
        },
    );
    planner.open = open;
    if send {
        let delay =
            if comms.location == Location::Vessel(id) { 0.0 } else { comms.signal(id).map_or(0.0, |s| s.delay) };
        let plan = plan_from(vessel, &planner.draft, now.add_seconds(delay));
        let cmd = GameCommand::SetPlan { vessel: id, plan };
        if delay > 0.0 {
            in_flight.send(now, delay, cmd);
            toasts.push(time.elapsed_secs_f64(), format!("Plan sent to Vessel {} ({})", id.0, fmt_delay(delay)), false);
        } else {
            commands.write(cmd);
        }
    }
    Ok(())
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
    fn a_burn_sooner_than_the_turn_warns() {
        assert!(turn_warning(10.0, 38.0).is_some());
        assert!(turn_warning(38.0, 38.0).is_none());
        assert!(turn_warning(600.0, 38.0).is_none());
    }

    #[test]
    fn burn_time_follows_the_rocket_equation() {
        // 1 km/s from 20 t at 300 kN, Isp 320 s: m0(1 − e^(−Δv/ve))/ṁ.
        let t = burn_time(1000.0, 20_000.0, 300e3, 320.0);
        let ve = 320.0 * G0;
        assert!((t - 20_000.0 * (1.0 - (-1000.0 / ve).exp()) / (300e3 / ve)).abs() < 1e-9);
        assert!(t > 55.0 && t < 60.0, "{t}");
    }

    #[test]
    fn a_plan_round_trips_through_the_draft_and_keeps_ignited_burns() {
        let sim = SimState::new();
        let v = sim.ship();
        let now = sim.clock;
        let body = sim.world.find("Earth").unwrap().node;
        let past = PlannedBurn::delta_v_with(now.add_seconds(-5.0), DVec3::X * 10.0, &v.craft.engine, Some(body));
        let draft = vec![
            DraftBurn { t_start: now.add_seconds(900.0), dv: DVec3::new(50.0, 0.0, 5.0), reference: body },
            DraftBurn { t_start: now.add_seconds(300.0), dv: DVec3::new(100.0, 0.0, 0.0), reference: body },
            DraftBurn { t_start: now.add_seconds(600.0), dv: DVec3::ZERO, reference: body },
        ];
        let plan = plan_from(v, &draft, now);
        assert!(plan.is_sorted());
        assert_eq!(plan.burns.len(), 2, "zero burns are dropped");
        let back = draft_from(&plan, now);
        assert_eq!(back[0].t_start, now.add_seconds(300.0));
        assert!((back[1].dv - DVec3::new(50.0, 0.0, 5.0)).length() < 1e-9);
        // An ignited burn stays at the front of a new plan.
        let mut plan = FlightPlan { burns: vec![past] };
        plan.burns.extend(plan_from(v, &draft, now).burns);
        assert!(draft_from(&plan, now).iter().all(|d| d.t_start.seconds_since(now) > 0.0));
    }
}
