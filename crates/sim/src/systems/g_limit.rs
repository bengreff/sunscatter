//! The g-load limit (D111): the proper acceleration a craft of parts may
//! reach in the direction it accelerates now, the lowest of its structure's
//! and its crew's, and what sets it.
//!
//! * **Structure:** the acceleration at which its most loaded intact joint
//!   reaches its ultimate load, with every force on the craft scaled
//!   together from the last live tick's (the joints' own ratios at that
//!   scale, pressures and liquid heads included: `structure::ultimate_scale`,
//!   kept per joint as `JointState::to_ultimate`). A joint without capacity
//!   data never limits (D105: not sure, don't fail).
//! * **Crew** (a crewed cabin): NASA-STD-3001's limit along that direction
//!   for the time the level now has been held (`crew::limit_along`).
//!
//! Only while the craft accelerates ([`ACCELERATING`]): coasting, what
//! loads its joints is mostly couples (RCS and SAS turning it) and the
//! thin air, which scaling the acceleration does not scale; the limit read
//! from them meant nothing (the Falcon 9's upper stage after separation,
//! turning on its thrusters at 0.00 g, showed a limit of 1.3 g).
//!
//! Pure: reads the ship state the live ticks left (`Runtime::accel`).

use super::crew;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;

/// What sets the limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limiter {
    /// A joint (its index in `ShipDef::joints`).
    Joint(usize),
    /// The crew along an axis (`crew::AXES`).
    Crew(usize),
}

/// The least proper acceleration (m/s²) a limit is given at: 0.01 g, the
/// G row's last digit.
pub const ACCELERATING: f64 = 0.01 * crate::vessel::G0;

/// The proper acceleration (m/s², a magnitude along the craft's
/// acceleration now) the craft may reach, and what sets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GLimit {
    pub accel: f64,
    pub by: Limiter,
}

/// The limit of the craft of `def` in `state` (see the module): `None`
/// when it accelerates less than [`ACCELERATING`], or nothing it has
/// limits it within 100 g.
pub fn g_limit(def: &ShipDef, state: &ShipState) -> Option<GLimit> {
    let a = state.runtime.accel;
    let mag = a.length();
    if mag < ACCELERATING {
        return None;
    }
    let intact = |j: usize| state.joints.get(j).is_some_and(|s| s.intact);
    let joints = (state.joints.iter().enumerate())
        .filter(|&(j, _)| j < def.joints.len() && intact(j))
        .filter_map(|(j, run)| run.to_ultimate.map(|k| (k * mag, Limiter::Joint(j))));
    let crewed = crew::crew_of(def) > 0 && state.cabin.is_some();
    let crew = crewed.then(|| crew::limit_along(a, &state.runtime.crew.held)).flatten();
    joints
        .chain(crew.map(|(l, k)| (l, Limiter::Crew(k))))
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(accel, by)| GLimit { accel, by })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn falcon9() -> std::sync::Arc<ShipDef> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        crate::craft::load_definition(&path).unwrap().ship.unwrap()
    }

    /// Table: no acceleration (or under 0.01 g, coasting), no limit; the
    /// weakest joint's scale times the acceleration; the crew's when lower
    /// (eyeballs in, sustained 73.5 m/s²); a broken joint or one without
    /// data never limits.
    #[test]
    fn the_lowest_limit_and_what_sets_it() {
        let def = falcon9();
        let mut s = ShipState::initial(&def);
        assert!(def.joints.len() >= 3 && crew::crew_of(&def) > 0 && s.cabin.is_some());
        assert_eq!(g_limit(&def, &s), None, "at rest: no acceleration");
        // Coasting, a joint loaded by a couple: no limit under 0.01 g.
        s.joints[1].to_ultimate = Some(3000.0);
        s.runtime.accel = DVec3::Z * 0.004;
        assert_eq!(g_limit(&def, &s), None, "coasting");
        s.runtime.accel = DVec3::Z * ACCELERATING;
        s.joints[1].to_ultimate = Some(300.0);
        assert_eq!(g_limit(&def, &s).map(|l| l.by), Some(Limiter::Joint(1)), "at 0.01 g");
        s.joints[1].to_ultimate = None;
        s.runtime.accel = DVec3::Z * 9.80665;
        // Only the crew: eyeballs in, sustained.
        let l = g_limit(&def, &s).expect("crewed");
        assert_eq!(l.by, Limiter::Crew(0));
        assert!((l.accel - 73.5).abs() < 1e-9, "{l:?}");
        // Joint 1 at 5× the load now (5 g), joint 2 at 3×.
        s.joints[1].to_ultimate = Some(5.0);
        s.joints[2].to_ultimate = Some(3.0);
        let l = g_limit(&def, &s).expect("joints");
        assert_eq!(l.by, Limiter::Joint(2));
        assert!((l.accel - 3.0 * 9.80665).abs() < 1e-9);
        // Joint 2 broken: joint 1 sets it; at 10× it is the crew again.
        s.joints[2].intact = false;
        assert_eq!(g_limit(&def, &s).map(|l| l.by), Some(Limiter::Joint(1)));
        s.joints[1].to_ultimate = Some(10.0);
        assert_eq!(g_limit(&def, &s).map(|l| l.by), Some(Limiter::Crew(0)));
        // Sideways: the crew's sideways limit.
        s.runtime.accel = DVec3::X * 9.80665;
        assert_eq!(g_limit(&def, &s).map(|l| l.by), Some(Limiter::Crew(2)));
        // Uncrewed (no cabin): only the joints.
        s.cabin = None;
        assert_eq!(g_limit(&def, &s).map(|l| l.by), Some(Limiter::Joint(1)));
        s.joints[1].to_ultimate = None;
        assert_eq!(g_limit(&def, &s), None);
    }
}
