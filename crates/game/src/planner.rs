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

use crate::commands::{GameCommand, InFlight};
use crate::comms::{Comms, Location};
use crate::format::{delay as fmt_delay, duration as fmt_duration};
use crate::interface::toasts::Toasts;
use crate::state::SimState;
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

/// Burn time (s) for `dv` from mass `m0` at vacuum thrust and Isp.
pub fn burn_time(dv: f64, m0: f64, thrust: f64, isp: f64) -> f64 {
    let mdot = thrust / (isp * G0);
    m0 * (1.0 - (-dv / (isp * G0)).exp()) / mdot
}

/// Times (s from now) of the next apoapsis and periapsis of vessel `i` about
/// its dominant body, from its osculating orbit.
fn next_apsides(sim: &SimState, i: usize) -> (Option<f64>, Option<f64>) {
    let v = &sim.fleet[i];
    let (anchor, r, vel) = v.state_at(&sim.world, sim.clock);
    let body = sim.dominant_of(i);
    crate::relations::orbit_about(&sim.world, sim.clock, anchor, r, vel, body)
        .map_or((None, None), |o| crate::navball::rules::time_to_apsides(&o.elements, o.mu))
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

/// The time of the line sample nearest to `cursor` on screen, if within
/// `max_px`: (time, screen point) samples in order along the line.
pub fn nearest_on_screen(samples: &[(Epoch, Vec2)], cursor: Vec2, max_px: f32) -> Option<Epoch> {
    samples
        .iter()
        .map(|(t, p)| (*t, p.distance(cursor)))
        .filter(|(_, d)| *d <= max_px)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// With the planner open, a click on the active vessel's line adds a burn
/// there (edit its Δv in the window).
#[allow(clippy::too_many_arguments)]
pub fn pick_on_line(
    mut contexts: EguiContexts,
    mouse: Res<ButtonInput<MouseButton>>,
    sim: Res<SimState>,
    rig: Res<crate::camera::CameraRig>,
    ui: Res<crate::hud::UiState>,
    cam: Query<(&Camera, &Transform, &Projection), With<crate::camera::MainCamera>>,
    window: Query<&Window>,
    mut planner: ResMut<Planner>,
) -> Result {
    if !planner.open || !mouse.just_pressed(MouseButton::Left) || planner.vessel != Some(sim.ship().id()) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    if ctx.is_pointer_over_egui() {
        return Ok(());
    }
    let Some(cursor) = window.single().ok().and_then(Window::cursor_position) else { return Ok(()) };
    let (Some(view), Some(tr)) = (crate::map::view(&cam), sim.ship().trajectory()) else { return Ok(()) };
    let Some(plotter) = crate::trajectory::Plotter::new(&sim, &rig, ui.plot_frame) else { return Ok(()) };
    let end = tr.computed_until();
    let span = end.seconds_since(sim.clock).min(86_400.0);
    let samples: Vec<(Epoch, Vec2)> = (1..=600)
        .filter_map(|k| {
            let t = sim.clock.add_seconds(span * f64::from(k) / 600.0);
            let (anchor, r, _) = tr.eval(t)?;
            Some((t, view.project(plotter.plot(anchor, r, t))?))
        })
        .collect();
    if let Some(t) = nearest_on_screen(&samples, cursor, 8.0) {
        let reference = sim.dominant_of(sim.active);
        planner.draft.push(DraftBurn { t_start: t, dv: DVec3::ZERO, reference });
        planner.draft.sort_by(|a, b| a.t_start.seconds_since(b.t_start).total_cmp(&0.0));
    }
    Ok(())
}

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
    let (next_ap, next_pe) = next_apsides(&sim, i);
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
                mass *= (-dv / (engine.isp_vac * G0)).exp();
                total += dv;
            }
            if let Some(k) = remove {
                planner.draft.remove(k);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let mut add = |t: Option<f64>| {
                    if let Some(t) = t {
                        planner.draft.push(DraftBurn { t_start: now.add_seconds(t), dv: DVec3::ZERO, reference: body });
                    }
                };
                if ui.add_enabled(next_ap.is_some(), egui::Button::new("+ at Ap")).clicked() {
                    add(next_ap);
                }
                if ui.add_enabled(next_pe.is_some(), egui::Button::new("+ at Pe")).clicked() {
                    add(next_pe);
                }
                if ui.button("+ in 10 min").clicked() {
                    add(Some(600.0));
                }
            });
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
    fn a_click_picks_the_nearest_sample_within_reach() {
        let t0 = sim::sol::sol_epoch();
        let samples: Vec<(Epoch, Vec2)> =
            (0..10).map(|k| (t0.add_seconds(f64::from(k)), Vec2::new(10.0 * k as f32, 0.0))).collect();
        assert_eq!(nearest_on_screen(&samples, Vec2::new(41.0, 3.0), 8.0), Some(t0.add_seconds(4.0)));
        assert_eq!(nearest_on_screen(&samples, Vec2::new(41.0, 30.0), 8.0), None);
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
