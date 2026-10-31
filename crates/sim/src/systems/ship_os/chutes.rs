//! The parachutes' deploy (D126; D128 layer 2; entry-ablation.md §3.2):
//! when the drogue's mortar fires (arming) and when the main opens, from
//! what the computer knows (the altitude in `Knowledge`). The canopies'
//! reefing timers, filling and strength are the chute's own (layer 1,
//! `sim::chute`), which opens the main when this says so.
//!
//! - **The switch** (D126, [`Switch`]): `Auto` arms by itself below its
//!   arming height while the navigation solution says it descends (Crew
//!   Dragon's drogues at ~5.5 km, falcon9.md §4.5; the real trigger logic
//!   is not public: E), and the deploy command arms it too; `Press` (the
//!   default) arms on the deploy command; `Off` opens nothing by itself:
//!   each press opens the next canopy.
//! - **The main** opens below `main_height` by the altitude reading, once
//!   the drogue is out of its reefed stages or gone ([`main_now`]). Without
//!   an altitude reading, or switched off, it opens when the pilot presses
//!   deploy again ([`press`]: the manual deploy runs the same function,
//!   D126). A chute without a drogue opens its main on arming.
//! - **The landing prediction** reads the same rule ([`main_expected`],
//!   [`armed_at`]), with each canopy fully open from the moment it would
//!   open.
//!
//! Not built (entry-ablation.md §3.2): the drogue's wait above its
//! deploy_max_q, the mains' cut on water contact, the chute part's own
//! baroswitch (layer 1's backup, open for the owner there).

use super::Switch;
use crate::chute::{CanopyState, ChuteSpec, ChuteState};
use serde::{Deserialize, Serialize};

/// The deploy's switch and state (saved with the vessel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chutes {
    #[serde(default)]
    pub switch: Switch,
    /// The pilot pressed deploy with the chutes armed: the main opens as
    /// soon as the drogue lets it.
    #[serde(default)]
    pub main_called: bool,
}

/// What a landing prediction reads of the deploy: the switch and the
/// press, and whether the computer lacks an altitude reading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Deploy {
    pub chutes: Chutes,
    pub blind: bool,
}

/// The deploy command (a press: `ShipCommand::ArmChutes`): arms the chutes
/// (the drogue's mortar), or, armed already, calls the main.
pub fn press(c: &mut Chutes, chute: &mut ChuteState) {
    if chute.armed {
        c.main_called = true;
    } else {
        chute.armed = true;
    }
}

/// The height (m) below which `Auto` arms: the drogue's (`drogue_height`),
/// or the main's for a chute without a drogue.
pub fn arm_height(spec: &ChuteSpec) -> Option<f64> {
    if spec.drogue.is_some() {
        spec.drogue_height
    } else {
        Some(spec.main_height)
    }
}

/// Whether the computer arms the chutes by itself now: switched `Auto`,
/// not armed, descending, at or below [`arm_height`] by the `altitude`
/// reading (`None`: unknown).
pub fn arm_now(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>, descending: bool) -> bool {
    c.switch == Switch::Auto
        && !chute.armed
        && descending
        && arm_height(spec).zip(altitude).is_some_and(|(arm, h)| h <= arm)
}

/// Whether the altitude reading puts the vessel at or below the main's
/// height (`None`: unknown, never).
pub fn below_main(spec: &ChuteSpec, altitude: Option<f64>) -> bool {
    altitude.is_some_and(|h| h <= spec.main_height)
}

/// Whether the main is the next canopy to open, by the switch, the press
/// and the altitude reading (not yet whether the drogue lets it).
pub fn main_expected(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>) -> bool {
    chute.armed
        && chute.main == CanopyState::Stowed
        && (c.main_called || spec.drogue.is_none() || (c.switch != Switch::Off && below_main(spec, altitude)))
}

/// Whether the main opens now: [`main_expected`], and the drogue out of its
/// reefed stages, failed or released (the chute's timers, which the
/// computer started).
pub fn main_now(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>) -> bool {
    main_expected(spec, c, chute, altitude) && chute.drogue_done(spec)
}

/// Whether a prediction falling through `altitude` has the chutes armed:
/// armed already, or `Auto` arming there (a fall descends).
pub fn armed_at(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>) -> bool {
    chute.armed || arm_now(spec, c, chute, altitude, true)
}

/// The drag area a prediction assumes at `height` above the ground with
/// the chutes in `chute` (m²; `ChuteState::predicted_cd_area`), by the
/// deploy's rule.
pub fn predicted_cd_area(spec: &ChuteSpec, d: &Deploy, chute: &ChuteState, height: f64) -> f64 {
    let altitude = (!d.blind).then_some(height);
    let chute = ChuteState { armed: armed_at(spec, &d.chutes, chute, altitude), ..*chute };
    chute.predicted_cd_area(spec, main_expected(spec, &d.chutes, &chute, altitude))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chute::Canopy;

    fn canopy(reefing: Vec<crate::chute::Reef>) -> Canopy {
        Canopy { cd_area: 10.0, diameter: 5.0, fill_constant: 8.0, reefing, deploy_max_q: 1e4, max_load: 1e9 }
    }

    fn spec(drogue: bool) -> ChuteSpec {
        let reef = vec![crate::chute::Reef { cd_fraction: 0.5, until: 4.0 }];
        ChuteSpec {
            mount: glam::DVec3::ZERO,
            drogue: drogue.then(|| canopy(reef)),
            main: canopy(Vec::new()),
            main_height: 1_800.0,
            drogue_height: drogue.then_some(5_500.0),
        }
    }

    const OPEN: CanopyState = CanopyState::Open { age: 10.0, diameter: 5.0 };
    const REEFED: CanopyState = CanopyState::Open { age: 1.0, diameter: 2.0 };

    /// The main's rule: armed, stowed, the drogue done, and by height (a
    /// reading), a press, or no drogue at all; switched off, only a press.
    #[test]
    fn the_main_opens_by_its_rule() {
        use CanopyState::{Failed, Stowed};
        let st = |drogue, main| ChuteState { armed: true, drogue, main };
        let sw = |switch, main_called| Chutes { switch, main_called };
        let (auto, press, off) = (Switch::Auto, Switch::Press, Switch::Off);
        // (drogue, chutes, state, altitude, opens)
        let cases = [
            (true, sw(press, false), st(OPEN, Stowed), Some(1_700.0), true),
            (true, sw(auto, false), st(OPEN, Stowed), Some(1_800.0), true),
            (true, sw(press, false), st(OPEN, Stowed), Some(1_900.0), false), // above its height
            (true, sw(press, false), st(REEFED, Stowed), Some(1_000.0), false), // the drogue still reefed
            (true, sw(press, false), st(Failed, Stowed), Some(1_000.0), true), // a failed drogue lets it
            (true, sw(press, false), st(OPEN, Stowed), None, false),          // no altitude: never by height
            (true, sw(press, true), st(OPEN, Stowed), None, true),            // ... a press opens it
            (true, sw(press, true), st(OPEN, Stowed), Some(9_000.0), true),   // ... at any height
            (true, sw(press, true), st(REEFED, Stowed), None, false),         // once the drogue lets it
            (true, sw(off, false), st(OPEN, Stowed), Some(1_000.0), false),   // off: nothing by itself
            (true, sw(off, true), st(OPEN, Stowed), Some(1_000.0), true),
            (true, sw(press, false), st(OPEN, OPEN), Some(1_000.0), false), // open already
            (false, sw(press, false), st(Stowed, Stowed), None, true),      // no drogue: on arming
            (false, sw(off, false), st(Stowed, Stowed), Some(9_000.0), true),
        ];
        for (k, (drogue, c, state, altitude, opens)) in cases.into_iter().enumerate() {
            assert_eq!(main_now(&spec(drogue), &c, &state, altitude), opens, "case {k}");
        }
        let unarmed = ChuteState::default();
        assert!(!main_now(&spec(false), &Chutes::default(), &unarmed, Some(0.0)), "unarmed: nothing");
    }

    /// Arming by itself: only `Auto`, descending, at or below the arming
    /// height by a reading; the press arms first and calls the main after.
    #[test]
    fn auto_arms_below_its_height_descending_and_a_press_arms_then_calls_the_main() {
        let s = spec(true);
        let auto = Chutes { switch: Switch::Auto, main_called: false };
        let none = ChuteState::default();
        // (chutes, altitude, descending, arms)
        let cases = [
            (auto, Some(5_000.0), true, true),
            (auto, Some(5_500.0), true, true),
            (auto, Some(6_000.0), true, false),
            (auto, Some(5_000.0), false, false), // climbing (an ascent)
            (auto, None, true, false),           // no altitude
            (Chutes::default(), Some(5_000.0), true, false),
            (Chutes { switch: Switch::Off, main_called: false }, Some(5_000.0), true, false),
        ];
        for (k, (c, altitude, descending, arms)) in cases.into_iter().enumerate() {
            assert_eq!(arm_now(&s, &c, &none, altitude, descending), arms, "case {k}");
        }
        let armed = ChuteState { armed: true, ..none };
        assert!(!arm_now(&s, &auto, &armed, Some(1.0), true), "armed already");
        assert_eq!(arm_height(&spec(false)), Some(1_800.0), "no drogue: the main's height");
        assert!(armed_at(&s, &auto, &none, Some(5_000.0)) && !armed_at(&s, &Chutes::default(), &none, Some(5_000.0)));

        let (mut c, mut state) = (Chutes::default(), ChuteState::default());
        press(&mut c, &mut state);
        assert!(state.armed && !c.main_called);
        press(&mut c, &mut state);
        assert!(c.main_called);
    }
}
