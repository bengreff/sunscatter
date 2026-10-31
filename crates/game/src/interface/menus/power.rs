//! The Power menu (5, D111): the electrical system as it stands. Each bus
//! with its voltage, load, solar input and whether the pad feeds it; each
//! battery's charge as measured (D106); everything without power and why
//! (`systems::power`'s shedding). The systems screen's Electrical layer
//! has the rest. Its snapshot ([`PowerData`]) is gathered here too.

use super::{Section, Tone};
use crate::format::DASH;
use sim::systems::Quantity;
use sim::vessel::Vessel;

/// A battery under this share of its charge is amber, empty red.
pub const BATTERY_LOW: f64 = 0.2;

/// The electrical system (a craft of parts with buses).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerData {
    pub buses: Vec<sim::systems::power::BusRun>,
    pub batteries: Vec<Battery>,
    /// What has no power: (owner, why).
    pub unpowered: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    pub id: String,
    /// J now and full.
    pub charge: f64,
    pub capacity: f64,
    /// Measured (D106).
    pub known: bool,
}

/// The electrical system of `v` (`None`: the test craft, or a craft of
/// parts with neither buses nor batteries).
pub fn gather(v: &Vessel) -> Option<PowerData> {
    let (def, state) = (v.ship_def()?, v.ship()?);
    if def.buses.is_empty() && def.batteries.is_empty() {
        return None;
    }
    let knowledge = v.knowledge();
    let batteries = (def.batteries.iter().zip(&state.batteries).enumerate())
        .map(|(j, (d, s))| Battery {
            id: d.id.clone(),
            charge: s.charge,
            capacity: d.item.capacity,
            known: knowledge.knows(Quantity::BatteryCharge(j as u32)),
        })
        .collect();
    let run = &state.runtime;
    Some(PowerData {
        buses: run.buses.clone(),
        batteries,
        unpowered: run.unpowered.iter().map(|l| (l.owner.clone(), l.why.clone())).collect(),
    })
}

pub fn sections(p: Option<&PowerData>) -> Vec<Section> {
    let mut s = Section::summary();
    let Some(p) = p else {
        s.note("no electrical system", Tone::Dim);
        return vec![s];
    };
    for b in &p.buses {
        let mut text = format!("{:.1} V · {:.0} W", b.volts, b.load);
        if b.solar > 0.0 {
            text += &format!(" · +{:.0} W solar", b.solar);
        }
        if b.ground {
            text += " · ground";
        }
        let tone = if b.volts <= 0.0 { Tone::Alert } else { Tone::Normal };
        s.toned(b.name.to_uppercase(), text, tone);
    }
    if p.buses.is_empty() {
        s.note("no buses: nothing draws power", Tone::Dim);
    }
    let mut cells = Section::new("batteries", format!("BATTERIES · {}", p.batteries.len()));
    for b in &p.batteries {
        let share = if b.capacity > 0.0 { b.charge / b.capacity } else { 0.0 };
        let (text, tone) = if !b.known {
            (format!("{DASH} / {:.0} Wh", b.capacity / 3600.0), Tone::Normal)
        } else {
            let text = format!("{:.0} / {:.0} Wh ({:.0} %)", b.charge / 3600.0, b.capacity / 3600.0, share * 100.0);
            let tone = match share {
                x if x <= 0.0 => Tone::Alert,
                x if x < BATTERY_LOW => Tone::Warn,
                _ => Tone::Normal,
            };
            (text, tone)
        };
        cells.toned(b.id.to_uppercase(), text, tone);
    }
    let mut off = Section::new("unpowered", format!("WITHOUT POWER · {}", p.unpowered.len()));
    for (owner, why) in &p.unpowered {
        off.toned(owner.to_uppercase(), why.clone(), Tone::Alert);
    }
    vec![s, cells, off]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::menus::find;
    use sim::systems::power::BusRun;

    fn bus(volts: f64, solar: f64, ground: bool) -> BusRun {
        BusRun { name: "dragon".into(), volts, load: 410.0, solar, ground }
    }

    /// A bus: volts and load, solar and the pad's feed when there; red at
    /// no voltage.
    #[test]
    fn buses_read_volts_load_and_supply() {
        // (bus, value, tone)
        let table = [
            (bus(28.0, 0.0, true), "28.0 V · 410 W · ground", Tone::Normal),
            (bus(27.4, 120.0, false), "27.4 V · 410 W · +120 W solar", Tone::Normal),
            (bus(0.0, 0.0, false), "0.0 V · 410 W", Tone::Alert),
        ];
        for (b, value, tone) in table {
            let p = PowerData { buses: vec![b], ..Default::default() };
            let s = sections(Some(&p));
            assert_eq!((s[0].value("DRAGON"), s[0].tone("DRAGON")), (Some(value), Some(tone)));
        }
    }

    /// Batteries by share of charge, a dash unmeasured; the unpowered in red.
    #[test]
    fn batteries_and_the_unpowered() {
        let cell = |charge: f64, known| Battery { id: "b".into(), charge, capacity: 3600.0 * 100.0, known };
        let table = [
            (cell(3600.0 * 50.0, true), "50 / 100 Wh (50 %)", Tone::Normal),
            (cell(3600.0 * 10.0, true), "10 / 100 Wh (10 %)", Tone::Warn),
            (cell(0.0, true), "0 / 100 Wh (0 %)", Tone::Alert),
            (cell(0.0, false), "— / 100 Wh", Tone::Normal),
        ];
        for (b, value, tone) in table {
            let p = PowerData {
                batteries: vec![b],
                unpowered: vec![("s2.mvac".into(), "no power".into())],
                ..Default::default()
            };
            let s = sections(Some(&p));
            let cells = find(&s, "batteries").unwrap();
            assert_eq!((cells.value("B"), cells.tone("B")), (Some(value), Some(tone)));
            assert_eq!(find(&s, "unpowered").unwrap().tone("S2.MVAC"), Some(Tone::Alert));
        }
    }

    /// The Falcon 9 on the pad: its buses on ground power, nothing out.
    #[test]
    fn the_falcon_9_stands_on_ground_power() {
        assert!(gather(crate::state::SimState::new().ship()).is_some(), "the small rocket's bus");
        if !crate::state::craft_exists("falcon9-crew") {
            return;
        }
        let sim = crate::state::SimState::with_craft(&crate::state::craft_or_test("falcon9-crew"));
        let p = gather(sim.ship()).expect("electrical");
        assert!(!p.batteries.is_empty(), "{p:?}");
        assert!(p.batteries.iter().all(|b| b.capacity > 0.0 && b.charge > 0.0), "{p:?}");
        assert!(p.unpowered.is_empty(), "{:?}", p.unpowered);
    }
}
