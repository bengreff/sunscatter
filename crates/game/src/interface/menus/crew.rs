//! The Crew menu (4, D111): who is aboard and the air they breathe. The
//! crew count, the cabin's pressure, O₂, CO₂ and temperature as its
//! sensors read them (D106), whether its life support works (unpowered or
//! failed, and why), and the injuries the crew has taken from sustained
//! acceleration (`systems::crew`). The systems screen's Health layer has
//! the rest. Its snapshot ([`CrewData`]) is gathered here too.

use super::{Section, Tone};
use crate::format::Known;
use sim::systems::Quantity;
use sim::vessel::Vessel;

/// Total cabin pressure under which the menu warns, and alerts (Pa): a
/// leak or a cabin not yet at its setpoint (`crew::TOTAL_SET`).
pub const PRESSURE_WARN: f64 = 90_000.0;
pub const PRESSURE_ALERT: f64 = 70_000.0;
/// O₂ partial pressure under which it warns, and alerts (Pa: mild hypoxia
/// below ~18 kPa, impaired below ~16 kPa).
pub const O2_WARN: f64 = 18_000.0;
pub const O2_ALERT: f64 = 16_000.0;
/// CO₂ partial pressure over which it warns (the sim's alarm, the life
/// support not removing it), and alerts (Pa: headaches, dyspnoea).
pub const CO2_WARN: f64 = 1_000.0;
pub const CO2_ALERT: f64 = 2_000.0;

/// The crew and their cabin (a craft of parts with seats or a cabin).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrewData {
    pub crew: u32,
    pub cabin: Option<Cabin>,
    pub life_support: LifeSupport,
    /// Each injury: the axis, its acceleration (m/s²), held for (s), its
    /// limit (m/s²).
    pub injuries: Vec<(String, f64, f64, f64)>,
}

/// The cabin's gases (Pa) and temperature (K).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cabin {
    pub o2: f64,
    pub n2: f64,
    pub co2: f64,
    pub h2o: f64,
    pub k: f64,
}

impl Cabin {
    pub fn total(&self) -> f64 {
        self.o2 + self.n2 + self.co2 + self.h2o
    }
}

/// The cabin's life support.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LifeSupport {
    /// None aboard.
    #[default]
    None,
    Working,
    /// Unpowered or failed: why.
    Out(String),
}

/// The crew of `v` (`None`: the test craft, or a craft of parts with
/// neither seats nor a cabin).
pub fn gather(v: &Vessel) -> Option<CrewData> {
    let (def, state) = (v.ship_def()?, v.ship()?);
    let crew = sim::systems::crew::crew_of(&def);
    if crew == 0 && state.cabin.is_none() {
        return None;
    }
    let cabin = state.cabin.as_ref();
    let life = cabin.and_then(|c| def.life_support.iter().find(|l| l.part == c.part));
    let run = &state.runtime;
    let out = life.and_then(|l| run.lost.iter().chain(&run.unpowered).find(|x| x.owner == l.id));
    let life_support = match (life, out) {
        (None, _) => LifeSupport::None,
        (Some(_), None) => LifeSupport::Working,
        (Some(_), Some(x)) => LifeSupport::Out(x.why.clone()),
    };
    Some(CrewData {
        crew,
        cabin: cabin.map(|c| Cabin { o2: c.o2, n2: c.n2, co2: c.co2, h2o: c.h2o, k: c.k }),
        life_support,
        injuries: (run.crew.injuries.iter()).map(|i| (i.axis.clone(), i.level, i.seconds, i.limit)).collect(),
    })
}

/// How a value against a floor (low is bad) reads.
fn under(v: f64, warn: f64, alert: f64) -> Tone {
    if v < alert {
        Tone::Alert
    } else if v < warn {
        Tone::Warn
    } else {
        Tone::Normal
    }
}

/// How a value against a ceiling (high is bad) reads.
fn over(v: f64, warn: f64, alert: f64) -> Tone {
    if v >= alert {
        Tone::Alert
    } else if v >= warn {
        Tone::Warn
    } else {
        Tone::Normal
    }
}

pub fn sections(c: Option<&CrewData>, k: &Known) -> Vec<Section> {
    let mut s = Section::summary();
    let Some(c) = c else {
        s.note("no crew and no cabin aboard", Tone::Dim);
        return vec![s];
    };
    s.row("CREW", if c.crew == 0 { "none aboard".to_string() } else { format!("{} aboard", c.crew) });
    if let Some(cabin) = &c.cabin {
        let kpa = |q: Quantity, v: f64| k.value(q, v, 1e3, 2, "kPa");
        let total = cabin.total();
        let tone = |q: Quantity, t: Tone| if k.knows(q) { t } else { Tone::Normal };
        let pressure = under(total, PRESSURE_WARN, PRESSURE_ALERT);
        s.toned("CABIN PRESSURE", kpa(Quantity::CabinPressure, total), tone(Quantity::CabinPressure, pressure));
        let o2 = under(cabin.o2, O2_WARN, O2_ALERT);
        s.toned("O₂", kpa(Quantity::CabinO2, cabin.o2), tone(Quantity::CabinO2, o2));
        let co2 = over(cabin.co2, CO2_WARN, CO2_ALERT);
        s.toned("CO₂", kpa(Quantity::CabinCo2, cabin.co2), tone(Quantity::CabinCo2, co2));
        // Read with the cabin's pressure sensors (no quantity of its own).
        s.row("TEMPERATURE", k.or_dash(k.knows(Quantity::CabinPressure), || format!("{:.0} K", cabin.k)));
    }
    match &c.life_support {
        LifeSupport::None => s.toned("LIFE SUPPORT", "none", Tone::Dim),
        LifeSupport::Working => s.toned("LIFE SUPPORT", "working", Tone::Good),
        LifeSupport::Out(why) => s.toned("LIFE SUPPORT", format!("out: {why}"), Tone::Alert),
    };
    let mut hurt = Section::new("injuries", format!("INJURIES · {}", c.injuries.len()));
    for (axis, level, seconds, limit) in &c.injuries {
        let g = |a: f64| a / sim::vessel::G0;
        let text = format!("{:.1} g for {seconds:.1} s (limit {:.1} g)", g(*level), g(*limit));
        hurt.toned(axis.to_uppercase(), text, Tone::Alert);
    }
    vec![s, hurt]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::menus::find;

    fn crew(cabin: Cabin, life_support: LifeSupport) -> CrewData {
        CrewData { crew: 4, cabin: Some(cabin), life_support, injuries: vec![] }
    }

    const FRESH: Cabin = Cabin { o2: 21_300.0, n2: 79_000.0, co2: 50.0, h2o: 1_000.0, k: 295.0 };

    /// The cabin's gases against their limits: (cabin, row, tone).
    #[test]
    fn the_cabin_reads_against_its_limits() {
        let k = Known::default();
        let table = [
            (FRESH, "CABIN PRESSURE", Tone::Normal),
            (Cabin { n2: 60_000.0, ..FRESH }, "CABIN PRESSURE", Tone::Warn),
            (Cabin { n2: 40_000.0, ..FRESH }, "CABIN PRESSURE", Tone::Alert),
            (FRESH, "O₂", Tone::Normal),
            (Cabin { o2: 17_000.0, ..FRESH }, "O₂", Tone::Warn),
            (Cabin { o2: 15_000.0, ..FRESH }, "O₂", Tone::Alert),
            (FRESH, "CO₂", Tone::Normal),
            (Cabin { co2: 1_200.0, ..FRESH }, "CO₂", Tone::Warn),
            (Cabin { co2: 6_000.0, ..FRESH }, "CO₂", Tone::Alert),
        ];
        for (cabin, row, tone) in table {
            let s = sections(Some(&crew(cabin, LifeSupport::Working)), &k);
            assert_eq!(s[0].tone(row), Some(tone), "{row} {cabin:?}");
        }
        let s = sections(Some(&crew(FRESH, LifeSupport::Working)), &k);
        assert_eq!(s[0].value("CREW"), Some("4 aboard"));
        assert_eq!(s[0].value("CABIN PRESSURE"), Some("101.35 kPa"));
        assert_eq!(s[0].value("TEMPERATURE"), Some("295 K"));
        assert_eq!(s[0].tone("LIFE SUPPORT"), Some(Tone::Good));
    }

    /// Life support out reads why, in red; injuries are listed in g.
    #[test]
    fn life_support_out_and_injuries_are_alerts() {
        let mut c = crew(FRESH, LifeSupport::Out("no power".into()));
        c.injuries.push(("+Ax".into(), 9.0 * sim::vessel::G0, 2.0, 7.5 * sim::vessel::G0));
        let s = sections(Some(&c), &Known::default());
        assert_eq!(s[0].value("LIFE SUPPORT"), Some("out: no power"));
        assert_eq!(s[0].tone("LIFE SUPPORT"), Some(Tone::Alert));
        let hurt = find(&s, "injuries").unwrap();
        assert_eq!(hurt.value("+AX"), Some("9.0 g for 2.0 s (limit 7.5 g)"));
    }

    /// The Falcon 9's Dragon: four seats, a fresh cabin, life support on.
    #[test]
    fn the_falcon_9_carries_four_in_a_fresh_cabin() {
        assert!(gather(crate::state::SimState::new().ship()).is_some(), "the small rocket's seat");
        if !crate::state::craft_exists("falcon9-crew") {
            return;
        }
        let sim = crate::state::SimState::with_craft(&crate::state::craft_or_fallback("falcon9-crew"));
        let c = gather(sim.ship()).expect("crewed");
        assert_eq!(c.crew, 4);
        let cabin = c.cabin.expect("a cabin");
        assert!((cabin.total() - 101_325.0).abs() < 5_000.0, "{cabin:?}");
        assert_eq!(c.life_support, LifeSupport::Working);
        assert!(c.injuries.is_empty());
    }
}
