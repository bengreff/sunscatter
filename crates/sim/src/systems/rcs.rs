//! Reaction control by translation (D104; ship-model.md §5.3): the force
//! and torque the thruster nozzles give from their geometry when they push
//! the craft along a direction — the pilot's translation, or an engine's
//! start sequence settling its propellant (falcon9.md §3.5: the second
//! stage's nitrogen pods fire aft before a restart). A nozzle fires when it
//! pushes within 25° of the wanted direction, at the wanted duty; its block
//! must be enabled and fed. Rotation keeps the attitude authority of
//! `ShipDef::rcs_authority`.

use super::ship::block_fed;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::vessel::G0;
use glam::DVec3;

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
        if !s.enabled || !(infinite || block_fed(&t.feed, state)) {
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
    (def.thrusters.iter().zip(&state.thrusters)).any(|(t, s)| {
        s.enabled && block_fed(&t.feed, state) && t.nozzles.iter().any(|n| n.dir.normalize().dot(d) >= ALIGNED)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
