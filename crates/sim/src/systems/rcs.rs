//! Reaction control by translation (D104; ship-model.md §5.3): the force
//! and torque the thruster nozzles give from their geometry when they push
//! the craft along a direction — the pilot's translation, or an engine's
//! start sequence settling its propellant (falcon9.md §3.5: the second
//! stage's nitrogen pods fire aft before a restart). A nozzle fires when it
//! pushes within 25° of the wanted direction, at the wanted duty; its block
//! must be enabled and fed. Rotation keeps the attitude authority of
//! `ShipDef::rcs_authority`.
//!
//! The pilot's RCS mode (D111, [`RcsMode`]): off, the thrusters answer only
//! the flight computer (SAS's holds, an engine's settling); on, the
//! translation keys fire them and the rotation keys turn the craft with
//! them as well as the gimbals. Overrides: translate only (the thrusters
//! never turn the craft, SAS's holds included), rotate only (translation is
//! ignored), and fine control (the pilot's inputs at a tenth). One rule,
//! [`pilot`] and [`thrusters_turn`]; [`authority`] is what the RCS menu
//! shows.

use super::ship::block_ready;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::craft::MassProps;
use crate::vessel::{Controls, G0};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The pilot's RCS (see the module).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RcsMode {
    pub on: bool,
    #[serde(default)]
    pub only: RcsOnly,
    /// The pilot's translation and rotation at [`FINE`].
    #[serde(default)]
    pub fine: bool,
}

/// What the thrusters do for the pilot with RCS on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RcsOnly {
    #[default]
    Both,
    /// They push but never turn the craft (SAS's holds included).
    Translate,
    /// They turn the craft; translation is ignored.
    Rotate,
}

/// The pilot's inputs under fine control.
pub const FINE: f64 = 0.1;

/// The controls as the craft acts on them: the pilot's translation only
/// with RCS on and not rotate-only; with fine control both inputs at
/// [`FINE`].
pub fn pilot(c: &Controls) -> Controls {
    let mode = c.rcs;
    let scale = if mode.on && mode.fine { FINE } else { 1.0 };
    let translates = mode.on && mode.only != RcsOnly::Rotate;
    Controls { translate: if translates { c.translate * scale } else { DVec3::ZERO }, rotate: c.rotate * scale, ..*c }
}

/// Whether the thrusters may turn the craft under `c`: with RCS off only
/// for the flight computer (no rotation input: SAS's holds), with RCS on
/// unless translate-only.
pub fn thrusters_turn(c: &Controls) -> bool {
    match (c.rcs.on, c.rcs.only) {
        (false, _) => c.rotate == DVec3::ZERO,
        (true, RcsOnly::Translate) => false,
        (true, _) => true,
    }
}

/// What the RCS can do now (the RCS menu): per direction its translation
/// and the torque it gives, and the angular acceleration about each axis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Authority {
    /// Along +X, −X, +Y, −Y, +Z, −Z (body axes): the acceleration (m/s²)
    /// and how far its line of thrust misses the centre of mass (m:
    /// torque over force; an imbalance turns the craft as it translates).
    pub translate: [(f64, f64); 6],
    /// Angular acceleration about X, Y, Z (rad/s², the weaker sense).
    pub rotate: DVec3,
}

/// The six translation directions of [`Authority::translate`].
pub const DIRECTIONS: [DVec3; 6] = [DVec3::X, DVec3::NEG_X, DVec3::Y, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z];

/// The RCS's authority with the craft's mass properties `props` (body
/// axes): its enabled, fed thrusters (`infinite`: debug mode, all fed).
pub fn authority(def: &ShipDef, state: &ShipState, props: &MassProps, infinite: bool) -> Authority {
    let mut a = Authority::default();
    if props.mass <= 0.0 {
        return a;
    }
    for (k, dir) in DIRECTIONS.iter().enumerate() {
        let p = translate(def, state, props.com, *dir, infinite);
        let f = p.force.dot(*dir).max(0.0);
        let miss = if f > 0.0 { p.torque.length() / f } else { 0.0 };
        a.translate[k] = (f / props.mass, miss);
    }
    let fed: Vec<bool> = (0..def.thrusters.len())
        .map(|i| state.thrusters[i].enabled && (infinite || block_ready(def, state, i)))
        .collect();
    let (torque, _) = def.rcs_authority(props.com, &fed);
    let i = props.inertia;
    let diag = DVec3::new(i.x_axis.x, i.y_axis.y, i.z_axis.z);
    a.rotate = DVec3::select(diag.cmpgt(DVec3::ZERO), torque / diag, DVec3::ZERO);
    a
}

/// A nozzle fires for directions within this cosine (25°).
const ALIGNED: f64 = 0.906;

/// What the translating nozzles give over a tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Push {
    /// Force (body axes, N) and torque about the centre of mass (N·m).
    pub force: DVec3,
    pub torque: DVec3,
    /// Each thruster block's flow (kg/s, aligned with `ShipDef::thrusters`).
    pub flows: Vec<f64>,
}

/// The push of the nozzles along `want` (body axes; its length, at most 1,
/// is the duty) about the centre of mass `com` (`infinite`: debug mode,
/// fed whatever the bottles hold).
pub fn translate(def: &ShipDef, state: &ShipState, com: DVec3, want: DVec3, infinite: bool) -> Push {
    let mut push = Push { flows: vec![0.0; def.thrusters.len()], ..Push::default() };
    let duty = want.length().min(1.0);
    if duty <= 0.0 {
        return push;
    }
    let dir = want / want.length();
    for (i, (t, s)) in def.thrusters.iter().zip(&state.thrusters).enumerate() {
        if !s.enabled || !(infinite || block_ready(def, state, i)) {
            continue;
        }
        for n in t.nozzles.iter().filter(|n| n.dir.normalize().dot(dir) >= ALIGNED) {
            let f = n.dir.normalize() * (t.thrust * duty);
            push.force += f;
            push.torque += (n.pos - com).cross(f);
            push.flows[i] += t.thrust * duty / (t.isp * G0);
        }
    }
    push
}

/// Whether any enabled, fed nozzle can push along `dir` (body axes).
pub fn can_push(def: &ShipDef, state: &ShipState, dir: DVec3) -> bool {
    let d = dir.normalize();
    (def.thrusters.iter().zip(&state.thrusters).enumerate()).any(|(i, (t, s))| {
        s.enabled && block_ready(def, state, i) && t.nozzles.iter().any(|n| n.dir.normalize().dot(d) >= ALIGNED)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pilot's mode (table): what reaches the craft, and whether the
    /// thrusters may turn it.
    #[test]
    fn the_pilots_rcs_mode() {
        let input = Controls { translate: DVec3::X, rotate: DVec3::Y, ..Controls::default() };
        let sas = Controls { sas: true, ..Controls::default() };
        let mode = |on, only, fine| RcsMode { on, only, fine };
        // (controls, mode, translation reaching the craft, rotation, thrusters turn)
        let table = [
            (input, mode(false, RcsOnly::Both, false), DVec3::ZERO, DVec3::Y, false),
            (sas, mode(false, RcsOnly::Both, false), DVec3::ZERO, DVec3::ZERO, true),
            (input, mode(true, RcsOnly::Both, false), DVec3::X, DVec3::Y, true),
            (input, mode(true, RcsOnly::Translate, false), DVec3::X, DVec3::Y, false),
            (sas, mode(true, RcsOnly::Translate, false), DVec3::ZERO, DVec3::ZERO, false),
            (input, mode(true, RcsOnly::Rotate, false), DVec3::ZERO, DVec3::Y, true),
            (input, mode(true, RcsOnly::Both, true), DVec3::X * FINE, DVec3::Y * FINE, true),
            // Fine control is the RCS's: off, the rotation is whole.
            (input, mode(false, RcsOnly::Both, true), DVec3::ZERO, DVec3::Y, false),
        ];
        for (i, (c, rcs, translate, rotate, turn)) in table.into_iter().enumerate() {
            let c = Controls { rcs, ..c };
            let p = pilot(&c);
            assert_eq!((p.translate, p.rotate), (translate, rotate), "case {i}");
            assert_eq!(thrusters_turn(&p), turn, "case {i}");
            assert_eq!((p.sas, p.rcs, p.throttle), (c.sas, c.rcs, c.throttle), "case {i}");
        }
    }

    /// The Falcon 9's second stage settles with its two pods' aft-firing
    /// nozzles: 2 × 200 N forward, balanced (no torque about the axis).
    #[test]
    fn the_second_stage_pushes_forward_on_its_nitrogen() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let state = ShipState::initial(&def);
        let com = DVec3::new(0.0, 0.0, -20.0);
        let p = translate(&def, &state, com, DVec3::Z, false);
        // Every enabled block with a forward nozzle: the second stage's two
        // pods (the first stage's are disabled until separation, Dragon's
        // Dracos too).
        assert!((p.force - DVec3::Z * 400.0).length() < 1e-9, "{}", p.force);
        assert!(p.torque.length() < 1e-9, "{}", p.torque);
        let flow: f64 = p.flows.iter().sum();
        assert!((flow - 400.0 / (70.0 * G0)).abs() < 1e-12);
        assert!(can_push(&def, &state, DVec3::Z) && !can_push(&def, &state, -DVec3::Z));
        assert_eq!(
            translate(&def, &state, com, DVec3::ZERO, false),
            Push { flows: p.flows.iter().map(|_| 0.0).collect(), ..Push::default() }
        );
    }
}
