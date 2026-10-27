//! Crew and life support (D104; ship-model.md §5.8; falcon9.md §5.5, §6.6).
//!
//! * **The cabin** is one lumped volume of air (its life support's cabin
//!   volume, Crew Dragon's 9.3 m³): the partial pressures of O₂, N₂, CO₂ and
//!   water vapour (`CabinState`) from the gases' moles by the ideal-gas law
//!   at its temperature. Each crew member breathes per NASA's BVAD
//!   (nominal: 0.82 kg O₂ in, 1.04 kg CO₂ and 1.9 kg of water vapour out per
//!   day) and gives ~100 W of heat. The life support, while it has power and
//!   its function, removes CO₂ and water up to its rates, feeds O₂ up to its
//!   rate to hold 21.3 kPa and N₂ to hold the total at 101.3 kPa, and holds
//!   the temperature at 295 K (18–27 °C, crew-selectable); without it the
//!   air goes stale and the temperature drifts to the capsule's.
//! * **Acceleration limits** (NASA-STD-3001 Vol. 2, falcon9.md §6.6):
//!   sustained (over 0.5 s) acceleration of the cabin by axis against the
//!   limit for how long it has been held (log-linear between the table's
//!   points). The crew sit facing up the stack: body +Z is their +Ax ("eyeballs
//!   in"); the seats' sideways axes are not data, so a sideways acceleration
//!   is judged against the more tolerant of ±Ay and ±Az (not sure, don't
//!   fail). Over a limit the crew are hurt: an alarm, and the injury
//!   recorded with its axis, level and time.
//!
//! On the lattice too (coasting: the cabin's air; no acceleration then).

use crate::craft::ship::ShipDef;
use crate::craft::state::{CabinState, ShipState};
use crate::fluids::R_GAS;
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// Per crew member per second (kg): BVAD nominal values (NASA/TP-2015-218570,
/// falcon9.md §5.5).
pub const O2_USE: f64 = 0.82 / 86_400.0;
pub const CO2_MADE: f64 = 1.04 / 86_400.0;
pub const H2O_MADE: f64 = 1.9 / 86_400.0;
/// Metabolic heat per crew member into the cabin (W; E, BVAD's nominal
/// ~137 W total, the rest leaves as vapour's latent heat).
pub const CREW_HEAT: f64 = 100.0;
/// Molar masses (kg/mol).
const M_O2: f64 = 0.031_998;
const M_CO2: f64 = 0.044_01;
const M_H2O: f64 = 0.018_015;
/// What the life support holds (Pa, K): sea-level air, 21 % O₂, ~1 kPa of
/// water vapour (~40 % humidity at 295 K).
pub const O2_SET: f64 = 21_300.0;
pub const TOTAL_SET: f64 = 101_325.0;
pub const H2O_SET: f64 = 1_000.0;
pub const CABIN_SET_K: f64 = 295.0;
/// How fast an unconditioned cabin's air follows the capsule (s; E).
const CABIN_TAU: f64 = 7_200.0;

/// The crew's state beyond the cabin's air: per axis the levels held and
/// for how long, and the injuries recorded.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CrewRun {
    /// Per axis ([`AXES`]): the windows ending now, each (its length s,
    /// the least acceleration over it m/s²), longest first with rising
    /// levels: every level over the axis's lowest limit held for its time,
    /// to within [`MERGE`] ([`hold`]).
    pub held: Vec<Vec<(f64, f64)>>,
    pub injuries: Vec<Injury>,
}

/// A new window whose level is within this factor of the last one's merges
/// into it: held over the longer time at the lower level, under-counted by
/// at most 1 % (D105: not sure, don't fail). A slowly rising acceleration
/// then adds a window per percent, not one per tick.
pub const MERGE: f64 = 1.01;

/// The windows an acceleration `a` held over `dt` adds to `w` (see
/// [`CrewRun::held`]): each window grows by `dt` and its least level is at
/// most `a`; a new window of `dt` at `a` starts unless within [`MERGE`] of
/// the last; windows of the same level merge into the longest, and levels
/// at or under `floor` go (an axis's lowest limit: a window's level only
/// falls, so it can never be over).
pub fn hold(w: &mut Vec<(f64, f64)>, a: f64, dt: f64, floor: f64) {
    for x in w.iter_mut() {
        x.0 += dt;
        x.1 = x.1.min(a);
    }
    if !w.last().is_some_and(|x| a <= x.1 * MERGE) {
        w.push((dt, a));
    }
    w.dedup_by(|later, earlier| later.1 <= earlier.1);
    w.retain(|x| x.1 > floor);
}

/// An exposure over a limit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Injury {
    pub at: Epoch,
    pub axis: String,
    /// The acceleration (m/s²), held this long (s), against this limit.
    pub level: f64,
    pub seconds: f64,
    pub limit: f64,
}

/// The axes judged: +Ax (eyeballs in), −Ax (eyeballs out), sideways.
pub const AXES: [&str; 3] = ["+Ax", "-Ax", "sideways"];

/// NASA-STD-3001's sustained limits for a non-deconditioned crew (launch)
/// as (duration s, limit m/s²), falcon9.md §6.6: +Ax, −Ax (as magnitudes),
/// and for sideways the more tolerant of ±Ay and +Az at each duration.
const LIMITS: [&[(f64, f64)]; 3] = [
    &[(0.5, 186.0), (5.0, 157.0), (300.0, 73.5)],
    &[(0.5, 216.0), (5.0, 147.0), (120.0, 58.8), (400.0, 39.2)],
    &[(0.5, 81.4), (5.0, 62.8), (1200.0, 39.2)],
];

/// The lowest limit along axis `k` (m/s², the longest duration's).
pub fn floor(k: usize) -> f64 {
    LIMITS[k][LIMITS[k].len() - 1].1
}

/// The limit (m/s²) for an acceleration held `seconds` along axis `k`:
/// log-linear in the duration between the table's points, held at its ends.
pub fn limit(k: usize, seconds: f64) -> f64 {
    let pts = LIMITS[k];
    let t = seconds.max(pts[0].0);
    if t >= pts[pts.len() - 1].0 {
        return pts[pts.len() - 1].1;
    }
    for w in pts.windows(2) {
        let ((t0, a0), (t1, a1)) = (w[0], w[1]);
        if t <= t1 {
            let f = (crate::math::ln(t) - crate::math::ln(t0)) / (crate::math::ln(t1) - crate::math::ln(t0));
            return a0 + (a1 - a0) * f;
        }
    }
    pts[pts.len() - 1].1
}

/// The cabin's proper acceleration in the crew's axes (+Ax, −Ax, sideways
/// magnitudes, m/s²) from the body's (+Z up the stack).
pub fn crew_axes(accel: DVec3) -> [f64; 3] {
    [accel.z.max(0.0), (-accel.z).max(0.0), accel.truncate().length()]
}

/// The acceleration (m/s²) along `dir` (body axes; only its direction
/// counts) at which the crew reach a limit, and the axis ([`AXES`]): on
/// each axis the direction has a share of, the limit for how long the
/// level now has been held on it (`held`, [`CrewRun::held`]: its last
/// window; nothing over the axis's lowest limit held, the sustained limit)
/// over that share; the lowest. `None` for no direction.
pub fn limit_along(dir: DVec3, held: &[Vec<(f64, f64)>]) -> Option<(f64, usize)> {
    if dir.length() <= 0.0 {
        return None;
    }
    let share = crew_axes(dir / dir.length());
    (0..AXES.len())
        .filter(|&k| share[k] > 1e-9)
        .map(|k| {
            let seconds = held.get(k).and_then(|w| w.last()).map_or(f64::INFINITY, |w| w.0);
            (limit(k, seconds) / share[k], k)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

/// The cabin's crew (seats) and life support (the ship's first).
pub fn crew_of(def: &ShipDef) -> u32 {
    def.seats.iter().map(|s| s.item).sum()
}

/// Moles of each gas in the cabin (O₂, N₂, CO₂, H₂O) at its pressures.
fn moles(c: &CabinState, volume: f64) -> [f64; 4] {
    let n = |p: f64| p * volume / (R_GAS * c.k);
    [n(c.o2), n(c.n2), n(c.co2), n(c.h2o)]
}

/// One step of `dt` seconds of the cabin's air with `crew` aboard, its life
/// support working or not (`working`), the capsule's temperature `outside`
/// (K): see the module.
pub fn air(
    c: &mut CabinState,
    volume: f64,
    crew: u32,
    life: Option<&crate::catalog::LifeSupport>,
    outside: f64,
    dt: f64,
) {
    let people = f64::from(crew);
    let [mut o2, mut n2, mut co2, mut h2o] = moles(c, volume);
    o2 = (o2 - people * O2_USE * dt / M_O2).max(0.0);
    co2 += people * CO2_MADE * dt / M_CO2;
    h2o += people * H2O_MADE * dt / M_H2O;
    let heat_capacity = 1_005.0 * 1.2 * volume + 50_000.0; // air and the cabin's furnishings (E)
    match life {
        Some(l) => {
            let to_moles = |p: f64| p * volume / (R_GAS * c.k);
            co2 = (co2 - l.co2_removal / 86_400.0 * dt / M_CO2).max(0.0);
            h2o = h2o.min(to_moles(H2O_SET)).max(h2o - 2.0 * people * H2O_MADE * dt / M_H2O);
            let o2_add = (to_moles(O2_SET) - o2).clamp(0.0, l.o2_supply / 86_400.0 * dt / M_O2);
            o2 += o2_add;
            let total = o2 + n2 + co2 + h2o;
            n2 += (to_moles(TOTAL_SET) - total).max(0.0);
            c.k += (CABIN_SET_K - c.k) * (dt / 60.0).min(1.0);
        }
        None => {
            c.k += (people * CREW_HEAT * dt) / heat_capacity + (outside - c.k) * (dt / CABIN_TAU).min(1.0);
        }
    }
    let p = |n: f64| n * R_GAS * c.k / volume;
    (c.o2, c.n2, c.co2, c.h2o) = (p(o2), p(n2), p(co2), p(h2o));
}

/// One live tick (or lattice step) of the crew (see the module): the
/// cabin's air, and with `accel` (the cabin's proper acceleration, body
/// axes; `None` on the lattice) the limits. The capsule's temperature is
/// `outside` (K).
pub fn step(def: &ShipDef, state: &mut ShipState, accel: Option<DVec3>, outside: f64, dt: f64, at: Epoch) {
    let crew = crew_of(def);
    let Some(cabin) = state.cabin.as_ref() else { return };
    let part = cabin.part;
    let life = def.life_support.iter().find(|l| l.part == part);
    let volume = life.map_or(9.3, |l| l.item.cabin_volume);
    let working = life.filter(|l| {
        let out = |list: &[crate::systems::runtime::Lost]| list.iter().any(|x| x.owner == l.id);
        !out(&state.runtime.lost) && !out(&state.runtime.unpowered)
    });
    let was_fresh = cabin.co2 < 1_000.0;
    let c = state.cabin.as_mut().expect("checked");
    air(c, volume, crew, working.map(|l| &l.item), outside, dt);
    if was_fresh && c.co2 >= 1_000.0 {
        let text = format!("cabin CO₂ at {:.1} kPa: the life support is not removing it", c.co2 / 1e3);
        state.runtime.event(at, text, true);
    }
    let Some(a) = accel.filter(|_| crew > 0) else { return };
    let run = &mut state.runtime.crew;
    run.held.resize(AXES.len(), Vec::new());
    let now = crew_axes(a);
    let mut hurt = Vec::new();
    for k in 0..AXES.len() {
        hold(&mut run.held[k], now[k], dt, floor(k));
        let already = run.injuries.iter().any(|i| i.axis == AXES[k] && at.seconds_since(i.at) < 10.0);
        let over = run.held[k].iter().find(|&&(secs, level)| secs >= 0.5 && level > limit(k, secs));
        if let Some(&(seconds, level)) = over.filter(|_| !already) {
            hurt.push(Injury { at, axis: AXES[k].into(), level, seconds, limit: limit(k, seconds) });
        }
    }
    for i in hurt {
        let text = format!(
            "crew: {} {:.1} g for {:.0} s, over NASA-STD-3001's {:.1} g",
            i.axis,
            i.level / 9.80665,
            i.seconds,
            i.limit / 9.80665
        );
        state.runtime.event(at, text, true);
        state.runtime.crew.injuries.push(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> CabinState {
        CabinState { part: 0, o2: 21_200.0, n2: 79_000.0, co2: 50.0, h2o: 1_000.0, k: 295.0 }
    }

    /// The limit curves through their points (log-linear between).
    /// The windows: a level held stays timed while it holds, a higher one
    /// starts its own window, a drop ends the higher ones.
    #[test]
    fn windows_of_levels() {
        let mut w = Vec::new();
        for _ in 0..10 {
            hold(&mut w, 15.0, 1.0, 0.1);
        }
        assert_eq!(w, vec![(10.0, 15.0)]);
        for _ in 0..5 {
            hold(&mut w, 40.0, 1.0, 0.1);
        }
        assert_eq!(w, vec![(15.0, 15.0), (5.0, 40.0)]);
        hold(&mut w, 20.0, 1.0, 0.1);
        assert_eq!(w, vec![(16.0, 15.0), (6.0, 20.0)]);
        hold(&mut w, 0.0, 1.0, 0.1);
        assert!(w.is_empty());
        // Within 1 % of the last level: merged (held longer at the lower).
        hold(&mut w, 30.0, 1.0, 0.1);
        hold(&mut w, 30.2, 1.0, 0.1);
        assert_eq!(w, vec![(2.0, 30.0)]);
    }

    /// The limit along a direction (table): eyeballs in and out at their
    /// sustained limits with nothing held, the limit for the time held once
    /// a level is, sideways at its own, and a diagonal at the lower of its
    /// shares' limits over the share.
    #[test]
    fn limit_along_a_direction() {
        let none: Vec<Vec<(f64, f64)>> = Vec::new();
        let held = vec![vec![(5.0, 120.0)], Vec::new(), Vec::new()];
        let d = std::f64::consts::FRAC_1_SQRT_2;
        // (direction, held, limit m/s², axis)
        let table = [
            (DVec3::Z, &none, 73.5, 0),
            (DVec3::Z * 3.0, &none, 73.5, 0),
            (-DVec3::Z, &none, 39.2, 1),
            (DVec3::X, &none, 39.2, 2),
            (DVec3::Z, &held, 157.0, 0),
            // Diagonal: sideways' 39.2 over its share √½ (55.4) is under
            // eyeballs in's 73.5/√½.
            (DVec3::new(d, 0.0, d), &none, 39.2 / d, 2),
        ];
        for (dir, held, want, axis) in table {
            let (a, k) = limit_along(dir, held).expect("a direction");
            assert!((a - want).abs() < 1e-9, "{dir}: {a} vs {want}");
            assert_eq!(k, axis, "{dir}");
        }
        assert_eq!(limit_along(DVec3::ZERO, &none), None);
    }

    /// A full ascent at 50 Hz (the review's case: a window a tick while the
    /// acceleration rises, 14,886 by the MVac's depletion): the booster's
    /// 162 s to ~5.6 g and the second stage's 397 s to ~6.2 g eyeballs in,
    /// with a little sideways. The windows stay few and nobody is hurt; a
    /// rise to 10 g over 80 s, then held, still hurts the crew (over the
    /// limit's 8.8 g for 160 s) with the windows still few.
    #[test]
    fn a_full_ascent_keeps_few_windows() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut s = ShipState::initial(&def);
        let t0 = Epoch::new(845_000_000, 0.0);
        let dt = 0.02;
        let burns = [(7.6e6, 549e3, 2_540.0, 162.0), (981e3, 116e3, 252.0, 397.0)];
        let mut most = 0;
        let mut t = 0.0;
        for (thrust, m0, mdot, seconds) in burns {
            for k in 0..(seconds / dt) as u32 {
                let a = thrust / (m0 - mdot * dt * f64::from(k));
                let side = 0.3 * crate::math::sin(0.01 * f64::from(k));
                step(&def, &mut s, Some(DVec3::new(side, 0.0, a)), 293.0, dt, t0.add_seconds(t));
                most = s.runtime.crew.held.iter().map(Vec::len).fold(most, usize::max);
                t += dt;
            }
        }
        println!("ascent: at most {most} windows on an axis");
        assert!(s.runtime.crew.injuries.is_empty(), "{:?}", s.runtime.crew.injuries);
        assert!(most < 50, "{most} windows");
        for k in 0..12_000 {
            let a = (f64::from(k) * dt + 20.0).min(100.0);
            step(&def, &mut s, Some(DVec3::Z * a), 293.0, dt, t0.add_seconds(t));
            most = s.runtime.crew.held.iter().map(Vec::len).fold(most, usize::max);
            t += dt;
        }
        assert!(!s.runtime.crew.injuries.is_empty());
        assert!(most < 50, "{most} windows");
    }

    #[test]
    fn limits_table() {
        assert_eq!(limit(0, 0.1), 186.0);
        assert_eq!(limit(0, 5.0), 157.0);
        assert!((limit(0, (5.0f64 * 300.0).sqrt()) - 0.5 * (157.0 + 73.5)).abs() < 1e-9);
        assert_eq!(limit(0, 1e6), 73.5);
        assert_eq!(limit(1, 120.0), 58.8);
        assert_eq!(limit(2, 1200.0), 39.2);
        assert_eq!(crew_axes(DVec3::new(3.0, 4.0, -20.0)), [0.0, 20.0, 5.0]);
    }

    /// Four crew in Dragon's 9.3 m³ for six hours without life support:
    /// O₂ down by 0.82 kg, CO₂ up by 1.04 kg (BVAD), to ~7 kPa of CO₂;
    /// with it working, the air stays as set.
    #[test]
    fn four_crew_for_six_hours() {
        let mut stale = fresh();
        let mut kept = fresh();
        let life = crate::catalog::LifeSupport { co2_removal: 4.0, o2_supply: 3.4, cabin_volume: 9.3 };
        for _ in 0..360 {
            air(&mut stale, 9.3, 4, None, 295.0, 60.0);
            air(&mut kept, 9.3, 4, Some(&life), 295.0, 60.0);
        }
        let kg = |p: f64, m: f64, k: f64| p * 9.3 / (R_GAS * k) * m;
        let co2 = kg(stale.co2, M_CO2, stale.k) - kg(50.0, M_CO2, 295.0);
        assert!((co2 - 1.04).abs() < 1e-9, "{co2}");
        let o2 = kg(21_200.0, M_O2, 295.0) - kg(stale.o2, M_O2, stale.k);
        assert!((o2 - 0.82).abs() < 1e-9, "{o2}");
        assert!(stale.co2 > 6_000.0, "{}", stale.co2);
        // (The data's removal, 4.0 kg/day, is a little under four crew's
        // 4.16: CO₂ creeps up, still under NASA's 3 mmHg, 400 Pa.)
        assert!(kept.co2 < 400.0 && (kept.o2 - O2_SET).abs() < 300.0, "{kept:?}");
        let total = kept.o2 + kept.n2 + kept.co2 + kept.h2o;
        assert!((total - TOTAL_SET).abs() < 500.0 && (kept.k - CABIN_SET_K).abs() < 1e-6, "{total}");
    }

    /// Over its limit an exposure hurts the crew once (an alarm and the
    /// injury); at launch's 1.5 g nothing; on the pad nothing.
    #[test]
    fn an_exposure_over_its_limit_hurts_the_crew() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut s = ShipState::initial(&def);
        let t0 = Epoch::new(845_000_000, 0.0);
        for k in 0..500 {
            step(&def, &mut s, Some(DVec3::Z * 14.7), 293.0, 0.02, t0.add_seconds(0.02 * f64::from(k)));
        }
        assert!(s.runtime.crew.injuries.is_empty());
        // 9 g held (88 m/s² "eyeballs in": the limit over 30 s is ~120, over 300 s 73.5).
        for k in 0..20_000 {
            step(&def, &mut s, Some(DVec3::Z * 88.0), 293.0, 0.02, t0.add_seconds(10.0 + 0.02 * f64::from(k)));
            if !s.runtime.crew.injuries.is_empty() {
                break;
            }
        }
        let i = &s.runtime.crew.injuries[0];
        println!("{i:?}");
        assert_eq!(i.axis, "+Ax");
        assert!(i.seconds > 30.0 && i.seconds < 300.0 && (i.limit - 88.0).abs() < 1.0, "{i:?}");
        assert!(s.runtime.events.iter().any(|e| e.alarm && e.text.starts_with("crew: +Ax")));
    }
}
