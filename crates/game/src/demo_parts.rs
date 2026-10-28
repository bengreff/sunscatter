//! The demo of a craft of catalog parts (`SUNSCATTER_CRAFT=falcon9-crew
//! SUNSCATTER_DEMO=<dir>`): the craft on the pad from several angles and in
//! the systems screen by layer (physical; plumbing with the first stage's
//! tank selected; structural with the interstage; electrical with the
//! capsule), then
//! a scripted ascent through its sequence: ignition and liftoff (Space),
//! full throttle, a pitch kick and then SAS holding surface prograde (a
//! gravity turn as a pilot flies it), shots at max-Q, at MECO, at stage
//! separation, of the upper stage after its engine lights and of the
//! booster after a switch to it; on the pad, at max-Q and at separation the
//! flight interface too (`parts_ui_*`: menus open, RCS on).
//! `SUNSCATTER_DEMO_STOP_AFTER=Pad` ends after the pad's shots. Every stage goes through `GameCommand::NextStage`,
//! the player's Space.

use crate::camera::{CameraRig, Focus};
use crate::commands::GameCommand;
use crate::demo::{Demo, Step};
use crate::interface::layout::InterfaceSettings;
use crate::interface::menus::MenuId;
use crate::state::SimState;
use crate::systems_screen::{inspect::Selection, rules::Layer, SystemsScreen};
use bevy::prelude::*;
use glam::DVec3;
use sim::vessel::{HoldMode, SpeedReference, VesselId};

/// The pad's views: (shot, camera distance from the centre of mass (m),
/// pitch (rad), yaw turned from the start (rad)).
const PAD_VIEWS: [(&str, f64, f64, f64); 4] = [
    // Far enough that the base stands clear of the navball cluster.
    ("parts_pad_side", 170.0, 0.05, 0.0),
    ("parts_pad_three_quarter", 150.0, 0.45, 0.8),
    ("parts_pad_low", 45.0, -0.05, 2.2),
    ("parts_pad_top", 110.0, 1.45, 0.0),
];
/// The systems screen's shots on the pad (D107): (shot, layer, the part
/// selected by its craft-file id).
const SYSTEMS_VIEWS: [(&str, Layer, Option<&str>); 4] = [
    ("parts_systems_physical", Layer::Physical, None),
    ("parts_systems_plumbing", Layer::Plumbing, Some("s1.tank")),
    ("parts_systems_structural", Layer::Structural, Some("s1.interstage")),
    ("parts_systems_electrical", Layer::Electrical, Some("dragon.capsule")),
];
/// The flight interface's shots (D111): after the craft's own shot `.0`,
/// the same view `.1` with RCS on (its menu heads the dock) and the menus
/// `.2` open; then closed again and RCS off, the flight as before.
const INTERFACE: [(&str, &str, &[MenuId]); 3] = [
    ("parts_pad_side", "parts_ui_pad", &[MenuId::Orbital, MenuId::Crew, MenuId::Power]),
    ("parts_max_q", "parts_ui_ascent", &[MenuId::Orbital, MenuId::Landing]),
    ("parts_separation", "parts_ui_separation", &[MenuId::Orbital, MenuId::Maneuver, MenuId::Comms]),
];
/// The gravity turn as a pilot flies it (RUNTIME's `falcon9_flight` test):
/// SAS holds the attitude until KICK_AT seconds after liftoff, then a pitch
/// kick of KICK (a share of the control authority) towards the east for
/// KICK_FOR seconds, then SAS holds surface prograde.
const KICK_AT: f64 = 10.0;
const KICK_FOR: f64 = 2.5;
const KICK: f64 = 0.04;
/// MECO: the first stage's propellant down to this share (the booster's
/// reserve), or this long after liftoff (Crew-2: T+2:36).
const MECO_RESERVE: f64 = 0.08;
const MECO_T: f64 = 156.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Phase {
    #[default]
    Pad,
    /// Ignition sent; liftoff next.
    Ignition,
    Ascent,
    /// MECO sent at this demo time; separation next.
    Meco(f64),
    /// Separation sent at this demo time with this many vessels in the fleet.
    Separating(f64, usize),
    /// Separated at this demo time; the upper stage's engine lights 3 s later.
    Upper(f64, bool),
    /// After the upper stage's shot (at this demo time): switched to the booster.
    Booster(f64, bool),
    Done,
}

/// The script's progress.
#[derive(Default)]
pub struct Script {
    phase: Phase,
    view: usize,
    /// The game clock at liftoff, the highest q seen and whether max-Q was shot.
    liftoff_at: Option<sim::time::Epoch>,
    q_max: f64,
    max_q_shot: bool,
    upper: Option<VesselId>,
    /// The demo time from which the script goes on after a shot.
    resume_at: f64,
    /// The interface shot under way.
    ui: Ui,
}

/// An interface shot ([`INTERFACE`], by index): due after its craft shot,
/// its menus open (the shot next), shot (undone next).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Ui {
    #[default]
    Idle,
    Due(usize),
    Open(usize),
    Shot,
}

/// How long the script waits after a capture ends before it changes the
/// view (s): the screenshot is written from a frame drawn after it.
const AFTER_SHOT: f64 = 0.5;

/// Captures `name` (the clock frozen meanwhile); the script waits until
/// the shot is written.
fn shoot(demo: &mut Demo, script: &mut Script, name: &'static str) {
    demo.capture(name);
    script.resume_at = demo.timer + AFTER_SHOT;
    if let Some(k) = INTERFACE.iter().position(|v| v.0 == name) {
        script.ui = Ui::Due(k);
    }
}

/// The interface shot after a craft shot: its menus open and RCS on, the
/// shot, then both undone. Whether the frame was spent on it.
fn interface_shot(
    demo: &mut Demo,
    script: &mut Script,
    iface: &mut InterfaceSettings,
    cmds: &mut MessageWriter<GameCommand>,
) -> bool {
    let rcs = |on| GameCommand::SetRcs(sim::systems::rcs::RcsMode { on, ..Default::default() });
    match script.ui {
        Ui::Idle => return false,
        Ui::Due(k) => {
            for m in INTERFACE[k].2 {
                iface.menus.set_open(*m, true);
            }
            cmds.write(rcs(true));
            // A few frames for the menus' snapshot to catch up.
            script.resume_at = demo.timer + 0.3;
            script.ui = Ui::Open(k);
        }
        Ui::Open(k) => {
            demo.capture(INTERFACE[k].1);
            script.resume_at = demo.timer + AFTER_SHOT;
            script.ui = Ui::Shot;
        }
        Ui::Shot => {
            iface.menus.show_only(None);
            cmds.write(rcs(false));
            script.ui = Ui::Idle;
        }
    }
    true
}

/// The first stage's share of propellant left: the tanks feeding the
/// starting or running main engines (the booster's, until MECO).
pub fn feeding_share(v: &sim::vessel::Vessel) -> Option<f64> {
    let (def, state) = (v.ship_def()?, v.ship()?);
    use sim::craft::state::EngineStatus;
    let running = (def.engines.iter().zip(&state.engines))
        .filter(|(e, s)| e.main && matches!(s.status, EngineStatus::Starting | EngineStatus::Running));
    let mut tanks: Vec<usize> = running.flat_map(|(e, _)| e.feeds.iter().flatten().map(|&t| t as usize)).collect();
    tanks.sort_unstable();
    tanks.dedup();
    let capacity: f64 = tanks.iter().map(|&t| def.tanks[t].capacity).sum();
    let liquid: f64 = tanks.iter().map(|&t| state.tanks[t].liquid).sum();
    (capacity > 0.0).then(|| liquid / capacity)
}

/// Runs `Step::Parts` (after `demo::run`, before the commands apply).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn run(
    demo: Option<ResMut<Demo>>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut iface: ResMut<InterfaceSettings>,
    mut cmds: MessageWriter<GameCommand>,
    mut systems: ResMut<SystemsScreen>,
    mut script: Local<Script>,
) {
    let Some(mut demo) = demo else { return };
    // A shot is written from a frame drawn after its capture ends: nothing
    // changes until then.
    if demo.step != Step::Parts || demo.capturing() || demo.timer < script.resume_at {
        return;
    }
    if interface_shot(&mut demo, &mut script, &mut iface, &mut cmds) {
        return;
    }
    let t = demo.timer;
    let earth = sim.world.find("Earth").expect("Earth").clone();
    let p = earth.physical.as_ref().expect("Earth is physical");
    let (anchor, r, v) = sim.ship().state(&sim.world);
    // Above the reference ellipsoid (the anchor is Earth near Earth).
    let alt = p.altitude(p.rotation.to_fixed(sim::frame::Vec3::from_raw(r), sim.clock));
    // Game time since liftoff (the script's timer runs in wall seconds).
    let clock = sim.clock;
    let since = |s: &Script| s.liftoff_at.map_or(0.0, |l| clock.seconds_since(l));
    match script.phase {
        Phase::Pad => {
            if t < 1.0 {
                iface.menus.show_only(None);
                return;
            }
            if let Some(&(name, distance, pitch, yaw)) = PAD_VIEWS.get(script.view) {
                (rig.distance, rig.pitch) = (distance, pitch);
                rig.yaw += yaw;
                script.view += 1;
                shoot(&mut demo, &mut script, name);
                return;
            }
            if let Some(&(name, layer, part)) = SYSTEMS_VIEWS.get(script.view - PAD_VIEWS.len()) {
                let def = sim.ship().ship_def();
                systems.open = true;
                systems.layer = layer;
                systems.selected = part.and_then(|p| def.as_ref()?.part_named(p)).map(Selection::Part);
                script.view += 1;
                shoot(&mut demo, &mut script, name);
                return;
            }
            systems.open = false;
            if demo.stops_after("Pad") {
                script.phase = Phase::Done;
                demo.next(Step::Done);
                return;
            }
            sim.controls.throttle = 1.0;
            cmds.write(GameCommand::NextStage);
            (rig.distance, rig.pitch) = (180.0, 0.05);
            script.phase = Phase::Ignition;
            (demo.timer, script.resume_at) = (0.0, 0.0);
        }
        Phase::Ignition if t > 0.5 => {
            cmds.write(GameCommand::NextStage);
            sim.warp = 2;
            script.liftoff_at = Some(sim.clock);
            script.phase = Phase::Ascent;
            shoot(&mut demo, &mut script, "parts_ignition");
        }
        Phase::Ignition => {}
        Phase::Ascent => {
            fly_turn(&mut sim, since(&script), r, v);
            let snap = sim.world.snapshot(sim.clock);
            let q = sim::vessel::air_at(&sim.world, &snap, anchor, r, v)
                .map_or(0.0, |a| 0.5 * a.rho * a.wind.length_squared());
            script.q_max = script.q_max.max(q);
            if !script.max_q_shot && script.q_max > 5_000.0 && q < 0.97 * script.q_max {
                script.max_q_shot = true;
                info!(
                    "demo parts: max-Q {:.1} kPa at {:.1} km, T+{:.0} s",
                    script.q_max / 1e3,
                    alt / 1e3,
                    since(&script)
                );
                (rig.distance, rig.pitch) = (150.0, 0.15);
                shoot(&mut demo, &mut script, "parts_max_q");
                return;
            }
            let share = feeding_share(sim.ship()).unwrap_or(0.0);
            if share <= MECO_RESERVE || since(&script) >= MECO_T {
                info!(
                    "demo parts: MECO at {:.1} km, {:.0} m/s, T+{:.0} s, {:.0} % left",
                    alt / 1e3,
                    v.length(),
                    since(&script),
                    share * 100.0
                );
                cmds.write(GameCommand::NextStage);
                sim.warp = 0;
                script.phase = Phase::Meco(t);
                shoot(&mut demo, &mut script, "parts_meco");
            }
        }
        Phase::Meco(at) if t - at > 2.0 => {
            cmds.write(GameCommand::NextStage);
            script.phase = Phase::Separating(t, sim.fleet.len());
        }
        Phase::Meco(_) => {}
        Phase::Separating(_, n) if sim.fleet.len() > n => {
            (rig.distance, rig.pitch) = (90.0, 0.3);
            script.upper = Some(sim.ship().id());
            script.phase = Phase::Upper(t, false);
            shoot(&mut demo, &mut script, "parts_separation");
        }
        Phase::Separating(at, _) => {
            if t - at > 60.0 {
                error!("demo parts: no separation a minute after the command");
                script.phase = Phase::Done;
                demo.next(Step::Done);
            }
        }
        Phase::Upper(at, false) if t - at > 3.0 => {
            // The second stage's ignition (its sequence goes on).
            cmds.write(GameCommand::NextStage);
            script.phase = Phase::Upper(at, true);
        }
        Phase::Upper(at, lit) => {
            if lit && t - at > 8.0 {
                (rig.distance, rig.pitch) = (160.0, 0.2);
                shoot(&mut demo, &mut script, "parts_upper_stage");
                script.phase = Phase::Booster(t, false);
            }
        }
        Phase::Booster(_, false) => {
            // The booster is a vessel of its own: switch to it (`[`/`]`).
            let booster = sim.fleet.iter().map(|v| v.id()).find(|&id| Some(id) != script.upper);
            if let Some(id) = booster {
                cmds.write(GameCommand::Switch(id));
            }
            script.phase = Phase::Booster(t, true);
        }
        Phase::Booster(at, true) if t - at > 2.0 => {
            rig.focus = Focus::Ship;
            (rig.distance, rig.pitch) = (120.0, 0.2);
            shoot(&mut demo, &mut script, "parts_booster");
            script.phase = Phase::Done;
        }
        Phase::Booster(..) => {}
        Phase::Done => demo.next(Step::Done),
    }
}

/// The ascent's controls at `t` seconds after liftoff ([`KICK_AT`]): the
/// stack flies itself into a gravity turn with its angle of attack near
/// zero. (Setting the attitude directly along a fixed turn, the main demo's
/// magic hold, put it at 6–19° of α through max-Q and broke its Dragon
/// ring: the loads were real for that flight.)
fn fly_turn(sim: &mut SimState, t: f64, r: DVec3, v: DVec3) {
    let q = sim.fleet[sim.active].attitude.q;
    let c = &mut sim.controls;
    c.rotate = DVec3::ZERO;
    if t < KICK_AT {
        (c.sas, c.hold) = (true, HoldMode::Stability);
    } else if t < KICK_AT + KICK_FOR {
        // Turning about up × east tips the nose east.
        let axis = r.normalize().cross(crate::demo_checks::horizontal(r, v));
        c.sas = false;
        c.rotate = q.inverse() * axis.normalize_or_zero() * KICK;
    } else {
        (c.sas, c.hold, c.speed) = (true, HoldMode::Prograde, SpeedReference::Surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_feeding_share_follows_the_running_engines_tanks() {
        let craft = crate::test_rocket::rocket();
        let mut sim = SimState::with_craft(&craft);
        let a = sim.active;
        assert_eq!(feeding_share(&sim.fleet[a]), None, "nothing runs: no share");
        sim.controls.throttle = 1.0;
        let s = &mut sim;
        s.fleet[a].next_stage(&s.world).expect("the ignition");
        let share = feeding_share(&sim.fleet[a]).unwrap();
        // The oxidizer full, the fuel loaded to 90 %: their liquid over their capacity.
        let def = sim.fleet[a].ship_def().unwrap();
        let full: f64 = def.tanks.iter().map(|t| t.capacity).sum();
        let loaded: f64 = def.tanks.iter().map(|t| t.capacity * t.load.fill).sum();
        assert!((share - loaded / full).abs() < 1e-12, "{share}");
    }
}
