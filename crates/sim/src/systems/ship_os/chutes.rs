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
//! - **The heights** ([`arm_height`], [`main_height`]) are the craft's
//!   (`ChuteSpec`) until the pilot sets them (D126's config menu, [`set`]):
//!   then they are the deploy's own, saved with its switch.
//! - **The main** opens below [`main_height`] by the altitude reading, once
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

use super::program::Setting;
use super::Switch;
use crate::chute::{CanopyState, ChuteSpec, ChuteState};
use serde::{Deserialize, Serialize};

/// The deploy's switch, settings and state (saved with the vessel).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Chutes {
    #[serde(default)]
    pub switch: Switch,
    /// The pilot pressed deploy with the chutes armed: the main opens as
    /// soon as the drogue lets it.
    #[serde(default)]
    pub main_called: bool,
    /// The heights it works to as the pilot set them (m above the ground;
    /// [`set`]): the drogues' arming and the mains' opening. `None`: the
    /// craft's ([`arm_height`], [`main_height`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arm_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_height: Option<f64>,
}

/// What a landing prediction reads of the deploy: the switch, the press
/// and the heights, and whether the computer lacks an altitude reading.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

/// The height (m) below which the mains open: the pilot's, or the craft's
/// (`ChuteSpec::main_height`).
pub fn main_height(spec: &ChuteSpec, c: &Chutes) -> f64 {
    c.main_height.unwrap_or(spec.main_height)
}

/// The height (m) below which `Auto` arms: the drogues' (the pilot's, or
/// the craft's `drogue_height`), or the mains' for a chute without a
/// drogue.
pub fn arm_height(spec: &ChuteSpec, c: &Chutes) -> Option<f64> {
    if spec.drogue.is_some() {
        c.arm_height.or(spec.drogue_height)
    } else {
        Some(main_height(spec, c))
    }
}

/// Sets one of the deploy's settings (D126's config menu): the drogues'
/// arming height ([`Setting::ArmHeight`], on a chute with drogues) or the
/// mains' ([`Setting::MainHeight`]), each a height above the ground (finite,
/// not negative). The order of the two is the pilot's: mains set above the
/// arming height open as soon as the drogues let them.
pub fn set(c: &mut Chutes, spec: &ChuteSpec, s: &Setting) -> Result<(), String> {
    let height = |h: f64| {
        if h.is_finite() && h >= 0.0 {
            Ok(Some(h))
        } else {
            Err(format!("a height above the ground is 0 m or more, not {h}"))
        }
    };
    match s {
        Setting::ArmHeight(_) if spec.drogue.is_none() => {
            Err("no drogues: the mains open at their own height (its main height)".into())
        }
        Setting::ArmHeight(h) => height(*h).map(|h| c.arm_height = h),
        Setting::MainHeight(h) => height(*h).map(|h| c.main_height = h),
        other => Err(format!("the parachutes' deploy has no {}", other.name())),
    }
}

/// Sets `settings` in order ([`set`]), all or none.
pub fn configure(c: &mut Chutes, spec: &ChuteSpec, settings: &[Setting]) -> Result<(), String> {
    let mut next = *c;
    for s in settings {
        set(&mut next, spec, s)?;
    }
    *c = next;
    Ok(())
}

/// Whether the computer arms the chutes by itself now: switched `Auto`,
/// not armed, descending, at or below [`arm_height`] by the `altitude`
/// reading (`None`: unknown).
pub fn arm_now(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>, descending: bool) -> bool {
    c.switch == Switch::Auto
        && !chute.armed
        && descending
        && arm_height(spec, c).zip(altitude).is_some_and(|(arm, h)| h <= arm)
}

/// Whether the altitude reading puts the vessel at or below the main's
/// height (`None`: unknown, never).
pub fn below_main(spec: &ChuteSpec, c: &Chutes, altitude: Option<f64>) -> bool {
    altitude.is_some_and(|h| h <= main_height(spec, c))
}

/// Whether the main is the next canopy to open, by the switch, the press
/// and the altitude reading (not yet whether the drogue lets it).
pub fn main_expected(spec: &ChuteSpec, c: &Chutes, chute: &ChuteState, altitude: Option<f64>) -> bool {
    chute.armed
        && chute.main == CanopyState::Stowed
        && (c.main_called || spec.drogue.is_none() || (c.switch != Switch::Off && below_main(spec, c, altitude)))
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
        let sw = |switch, main_called| Chutes { switch, main_called, ..Default::default() };
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
        let auto = Chutes { switch: Switch::Auto, ..Default::default() };
        let none = ChuteState::default();
        // (chutes, altitude, descending, arms)
        let cases = [
            (auto, Some(5_000.0), true, true),
            (auto, Some(5_500.0), true, true),
            (auto, Some(6_000.0), true, false),
            (auto, Some(5_000.0), false, false), // climbing (an ascent)
            (auto, None, true, false),           // no altitude
            (Chutes::default(), Some(5_000.0), true, false),
            (Chutes { switch: Switch::Off, ..Default::default() }, Some(5_000.0), true, false),
        ];
        for (k, (c, altitude, descending, arms)) in cases.into_iter().enumerate() {
            assert_eq!(arm_now(&s, &c, &none, altitude, descending), arms, "case {k}");
        }
        let armed = ChuteState { armed: true, ..none };
        assert!(!arm_now(&s, &auto, &armed, Some(1.0), true), "armed already");
        assert_eq!(arm_height(&spec(false), &auto), Some(1_800.0), "no drogue: the main's height");
        assert!(armed_at(&s, &auto, &none, Some(5_000.0)) && !armed_at(&s, &Chutes::default(), &none, Some(5_000.0)));

        let (mut c, mut state) = (Chutes::default(), ChuteState::default());
        press(&mut c, &mut state);
        assert!(state.armed && !c.main_called);
        press(&mut c, &mut state);
        assert!(c.main_called);
    }

    /// The pilot's heights replace the craft's in every rule; a setting
    /// that is not a height above the ground, or not the deploy's, is
    /// refused, and a call's settings land all or none.
    #[test]
    fn the_pilots_heights_replace_the_crafts() {
        let s = spec(true);
        let mut c = Chutes { switch: Switch::Auto, ..Default::default() };
        assert_eq!((arm_height(&s, &c), main_height(&s, &c)), (Some(5_500.0), 1_800.0), "the craft's");
        configure(&mut c, &s, &[Setting::ArmHeight(7_000.0), Setting::MainHeight(2_500.0)]).unwrap();
        assert_eq!((arm_height(&s, &c), main_height(&s, &c)), (Some(7_000.0), 2_500.0), "the pilot's");
        let none = ChuteState::default();
        assert!(arm_now(&s, &c, &none, Some(6_500.0), true), "arms below the pilot's height");
        let armed = ChuteState { armed: true, drogue: OPEN, ..none };
        assert!(main_now(&s, &c, &armed, Some(2_400.0)) && !main_now(&s, &c, &armed, Some(2_600.0)));
        let d = Deploy { chutes: c, blind: false };
        let big = ChuteSpec { main: Canopy { cd_area: 50.0, ..s.main.clone() }, ..s.clone() };
        let area = |h| predicted_cd_area(&big, &d, &none, h);
        assert!(area(2_400.0) > area(2_600.0), "the prediction's mains from the pilot's height");

        // (setting, accepted)
        let cases = [
            (Setting::MainHeight(0.0), true),
            (Setting::MainHeight(9_000.0), true), // above the arming: once the drogues let them
            (Setting::MainHeight(-1.0), false),
            (Setting::MainHeight(f64::NAN), false),
            (Setting::ArmHeight(f64::INFINITY), false),
            (Setting::ArmHeight(3_000.0), true),
            (Setting::Low(0.3), false),
            (Setting::Period(60.0), false),
        ];
        for (k, (setting, ok)) in cases.into_iter().enumerate() {
            let before = c;
            assert_eq!(set(&mut c, &s, &setting).is_ok(), ok, "case {k}");
            if !ok {
                assert_eq!(c, before, "case {k}: unchanged");
            }
        }
        let before = c;
        assert!(configure(&mut c, &s, &[Setting::MainHeight(1_000.0), Setting::MainHeight(-1.0)]).is_err());
        assert_eq!(c, before, "all or none");

        // Without drogues the one height is the mains': it arms there too.
        let lone = spec(false);
        let mut c = Chutes::default();
        assert!(set(&mut c, &lone, &Setting::ArmHeight(3_000.0)).is_err());
        set(&mut c, &lone, &Setting::MainHeight(3_000.0)).unwrap();
        assert_eq!(arm_height(&lone, &c), Some(3_000.0));
    }
}
