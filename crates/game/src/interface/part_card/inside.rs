//! What a part carries inside it (its components: Dragon's tanks,
//! SuperDracos, Dracos, batteries…), summed by kind on its card: the part
//! and its totals, never each component (owner, 2026-10-29: "those
//! components should not be listed, just the part and aggregate stats").
//! The components themselves are on the systems screen (the card's
//! "more"). A total is measured only when every item in it is.

use super::content::{against, line, shown, Live, Section, Value};
use crate::format;
use sim::systems::{Quantity, Reading};

/// The indices (in `ShipDef`'s lists) of what a part's components carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inside {
    pub engines: Vec<usize>,
    pub tanks: Vec<usize>,
    pub bottles: Vec<usize>,
    pub batteries: Vec<usize>,
    pub thrusters: Vec<usize>,
    pub valves: Vec<usize>,
    pub mechanisms: Vec<usize>,
}

impl Inside {
    /// What the components of part `part` (`Part::id`, its own id `own`)
    /// carry: every item on the part not named after it (the part's own
    /// are named after it; a tank `owner:compartment`).
    pub fn of(def: &sim::craft::ship::ShipDef, part: u32, own: &str) -> Inside {
        let inside = |p: u32, owner: &str| p == part && owner != own;
        let pick = |n: usize, f: &dyn Fn(usize) -> bool| (0..n).filter(|&i| f(i)).collect::<Vec<usize>>();
        Inside {
            engines: pick(def.engines.len(), &|i| inside(def.engines[i].part, &def.engines[i].owner)),
            tanks: pick(def.tanks.len(), &|i| {
                let t = &def.tanks[i];
                inside(t.part, t.id.split(':').next().unwrap_or(&t.id))
            }),
            bottles: pick(def.bottles.len(), &|i| inside(def.bottles[i].part, &def.bottles[i].id)),
            batteries: pick(def.batteries.len(), &|i| inside(def.batteries[i].part, &def.batteries[i].id)),
            thrusters: pick(def.thrusters.len(), &|i| inside(def.thrusters[i].part, &def.thrusters[i].id)),
            valves: pick(def.valves.len(), &|i| inside(def.valves[i].part, &def.valves[i].id)),
            mechanisms: pick(def.mechanisms.len(), &|i| inside(def.mechanisms[i].part, &def.mechanisms[i].id)),
        }
    }
}

/// How many of each, in the order first met: "6 running · 2 off".
pub fn counts<'a>(words: impl IntoIterator<Item = &'a str>) -> String {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for w in words {
        match seen.iter_mut().find(|(s, _)| *s == w) {
            Some((_, n)) => *n += 1,
            None => seen.push((w, 1)),
        }
    }
    seen.iter().map(|(w, n)| format!("{n} {w}")).collect::<Vec<_>>().join(" · ")
}

/// The sum of the readings of `items` (quantity, true value), if every one
/// is measured; else the first unmeasured quantity.
fn total(live: &Live, items: impl IntoIterator<Item = (Quantity, f64)>) -> Result<Reading, Quantity> {
    let mut sum = Reading { value: 0.0, precision: 0.0 };
    for (q, truth) in items {
        let r = live.knowledge.read(q, truth).ok_or(q)?;
        sum.value += r.value;
        sum.precision = sum.precision.max(r.precision);
    }
    Ok(sum)
}

/// A total as shown, or unknown with the sensor that would measure it.
fn value(live: &Live, total: Result<Reading, Quantity>, show: impl Fn(Reading) -> String) -> Value {
    match total {
        Ok(r) => Value::Shown(show(r)),
        Err(q) => Value::Unknown(super::content::sensor_for(live.def, q)),
    }
}

/// The sections of what the part carries inside it, by kind.
pub fn sections(live: &Live, inside: &Inside) -> Vec<Section> {
    let (def, state) = (live.def, live.state);
    let mut out = Vec::new();
    let n = |v: &[usize]| v.len();
    if !inside.engines.is_empty() {
        out.push(engines(live, &inside.engines));
    }
    if !inside.tanks.is_empty() {
        out.push(tanks(live, &inside.tanks));
    }
    if !inside.bottles.is_empty() {
        out.push(bottles(live, &inside.bottles));
    }
    if !inside.batteries.is_empty() {
        let b = &inside.batteries;
        let cap: f64 = b.iter().map(|&j| def.batteries[j].item.capacity).sum::<f64>().max(1.0);
        let charge = total(live, b.iter().map(|&j| (Quantity::BatteryCharge(j as u32), state.batteries[j].charge)));
        let lines = vec![
            line("charge", value(live, charge, |r| format!("{:.0} %", r.shown() / cap * 100.0))),
            shown("capacity", format!("{:.0} Wh", cap / 3600.0)),
        ];
        out.push(Section { heading: format!("BATTERIES · {}", n(b)), lines });
    }
    if !inside.thrusters.is_empty() {
        let t = &inside.thrusters;
        let words = t.iter().map(|&j| {
            if state.runtime.out(&def.thrusters[j].id) {
                "LOST"
            } else if state.thrusters.get(j).is_some_and(|s| s.enabled) {
                "enabled"
            } else {
                "disabled"
            }
        });
        out.push(Section { heading: format!("RCS · {} blocks", n(t)), lines: vec![shown("state", counts(words))] });
    }
    if !inside.valves.is_empty() {
        let v = &inside.valves;
        let words = v.iter().map(|&j| match (state.valves[j].open, state.valves[j].stuck) {
            (_, true) => "STUCK",
            (true, false) => "open",
            (false, false) => "closed",
        });
        out.push(Section { heading: format!("VALVES · {}", n(v)), lines: vec![shown("state", counts(words))] });
    }
    if !inside.mechanisms.is_empty() {
        let m = &inside.mechanisms;
        let words = m.iter().map(|&j| {
            let (d, s) = (&def.mechanisms[j], &state.mechanisms[j]);
            if (s.angle - s.target).abs() > 0.5 {
                "moving"
            } else if (s.angle - d.item.deployed_deg).abs() < 0.5 {
                "deployed"
            } else if (s.angle - d.item.stowed_deg).abs() < 0.5 {
                "stowed"
            } else {
                "part way"
            }
        });
        out.push(Section { heading: format!("MECHANISMS · {}", n(m)), lines: vec![shown("state", counts(words))] });
    }
    out
}

/// Engines `e`: their states, total thrust, ignitions left.
fn engines(live: &Live, e: &[usize]) -> Section {
    let (def, state) = (live.def, live.state);
    let thrust = total(live, e.iter().map(|&i| (Quantity::EngineThrust(i as u32), thrust_of(live, i))));
    let rated: f64 = e.iter().map(|&i| def.engines[i].data.thrust_vac).sum();
    let rated = format::force(Some(Reading { value: rated, precision: 0.0 }));
    let left: Vec<u32> = e.iter().map(|&i| state.engines[i].ignitions).collect();
    let (lo, hi) = (left.iter().min().copied().unwrap_or(0), left.iter().max().copied().unwrap_or(0));
    let ignitions = match (state.clamped, lo, hi) {
        (true, ..) => "the pad's supply".to_string(),
        (_, u32::MAX, _) => "unlimited".to_string(),
        (_, lo, hi) if lo == hi => format!("{lo} each"),
        (_, lo, u32::MAX) => format!("{lo} to unlimited"),
        (_, lo, hi) => format!("{lo}–{hi}"),
    };
    let lines = vec![
        shown("state", counts(e.iter().map(|&i| super::content::engine_words(live, i)))),
        line("thrust", value(live, thrust, |r| format!("{} of {rated} vac", format::force(Some(r))))),
        shown("ignitions left", ignitions),
    ];
    Section { heading: format!("ENGINES · {}", e.len()), lines }
}

/// Tanks: each fluid's total, and the pressure of the tank nearest its
/// burst.
fn tanks(live: &Live, tanks: &[usize]) -> Section {
    let (def, state) = (live.def, live.state);
    let mut lines = Vec::new();
    let mut fluids: Vec<&str> = Vec::new();
    for &t in tanks {
        if !fluids.contains(&def.tanks[t].fluid.as_str()) {
            fluids.push(&def.tanks[t].fluid);
        }
    }
    for fluid in fluids {
        let of: Vec<usize> = tanks.iter().copied().filter(|&t| def.tanks[t].fluid == fluid).collect();
        let cap: f64 = of.iter().map(|&t| def.tanks[t].capacity).sum();
        let held = total(live, of.iter().map(|&t| (Quantity::TankLevel(t as u32), state.tanks[t].liquid)));
        let text = |r: Reading| crate::interface::stage_bar::rows::amount(Some(r), cap);
        lines.push(line(fluid, value(live, held, text)));
    }
    // The tank nearest its burst pressure stands for them all.
    let ratio =
        |t: usize| crate::systems_screen::inspect::ullage_pressure(&def.tanks[t], &state.tanks[t]) / def.tanks[t].burst;
    if let Some(&t) = tanks.iter().max_by(|&&a, &&b| ratio(a).total_cmp(&ratio(b))) {
        let (d, s) = (&def.tanks[t], &state.tanks[t]);
        let p = crate::systems_screen::inspect::ullage_pressure(d, s);
        let rated = d.pressure / 1e5;
        let v = live.value(Quantity::TankPressure(t as u32), p, |r| {
            format!("{} of {rated:.1} bar", super::content::scaled(r, 1e5, 2, "bar"))
        });
        lines.push(against("pressure (highest)", v, p, d.burst));
    }
    Section { heading: format!("TANKS · {}", tanks.len()), lines }
}

/// Bottles `b`: the pressure of the one nearest its burst, their gases.
fn bottles(live: &Live, b: &[usize]) -> Section {
    let (def, state) = (live.def, live.state);
    let p = |j: usize| {
        let (d, s) = (&def.bottles[j], &state.bottles[j]);
        sim::fluids::pressure(s.moles, d.volume, s.k)
    };
    let mut lines = Vec::new();
    if let Some(&j) =
        b.iter().max_by(|&&x, &&y| (p(x) / def.bottles[x].burst).total_cmp(&(p(y) / def.bottles[y].burst)))
    {
        let rated = def.bottles[j].pressure / 1e5;
        let v = live.value(Quantity::BottlePressure(j as u32), p(j), |r| {
            format!("{} of {rated:.0} bar", super::content::scaled(r, 1e5, 0, "bar"))
        });
        lines.push(against("pressure (highest)", v, p(j), def.bottles[j].burst));
    }
    let gases = counts(b.iter().map(|&j| def.bottles[j].gas.as_str()));
    lines.push(shown("gas", gases));
    Section { heading: format!("BOTTLES · {}", b.len()), lines }
}

/// Engine `i`'s thrust now (N).
fn thrust_of(live: &Live, i: usize) -> f64 {
    live.thrusts.get(i).copied().unwrap_or(0.0)
}
