//! A load manager (D128 layer 2: "simple hard coded logic" making the
//! inputs a player makes in the systems screen): at each of its epochs it
//! reads its batteries' charge and switches its loads off below `low` (a
//! share of the capacity it reads) and back on at or above `high`. With
//! layer 1 alone a short bus sheds by its own protection (`systems::power`)
//! once it runs dry; this acts earlier, on the loads it names.

use super::program::{Commands, Context, Setting};
use crate::craft::command::ShipCommand;
use crate::craft::ship::ShipDef;
use crate::systems::data::Quantity;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerGuard {
    /// The components it switches (their ids).
    pub loads: Vec<String>,
    /// Off below this share of the capacity, on again at or above `high`.
    pub low: f64,
    pub high: f64,
    /// It has switched them off.
    #[serde(default)]
    pub shed: bool,
}

/// The share of their capacity the batteries the computer reads hold
/// (`None`: it reads none).
pub fn charge_share(cx: &Context) -> Option<f64> {
    let (mut charge, mut capacity) = (0.0, 0.0);
    for (i, b) in cx.def.batteries.iter().enumerate() {
        if let Some(r) = cx.read(Quantity::BatteryCharge(i as u32)) {
            charge += r.value;
            capacity += b.item.capacity;
        }
    }
    (capacity > 0.0).then(|| charge / capacity)
}

/// The components a load manager may switch: those that draw power when
/// on (`ComponentDef::power`), by id, in the craft's order.
pub fn switchable(def: &ShipDef) -> Vec<String> {
    def.components.iter().filter(|c| c.power > 0.0).map(|c| c.id.clone()).collect()
}

impl PowerGuard {
    /// Sets one of its settings (D126's config menu): its loads (each one
    /// of `offered`, [`switchable`]'s, once), or the line it switches them
    /// off below or on again at (a share of the charge, 0 to 1). The two
    /// lines together are [`Self::check`]'s.
    pub fn set(&mut self, offered: &[String], s: &Setting) -> Result<(), String> {
        let share = |x: f64| {
            if (0.0..=1.0).contains(&x) {
                Ok(x)
            } else {
                Err(format!("a line is a share of the charge, 0 to 1, not {x}"))
            }
        };
        match s {
            Setting::Loads(loads) => {
                if let Some(bad) = loads.iter().find(|l| !offered.contains(l)) {
                    return Err(format!(
                        "{bad} is not a component that draws power (it can switch {})",
                        offered.join(", ")
                    ));
                }
                if let Some((i, l)) = loads.iter().enumerate().find(|(i, l)| loads[..*i].contains(l)) {
                    return Err(format!("{l} is listed twice (at {i})"));
                }
                self.loads = loads.clone();
            }
            Setting::Low(x) => self.low = share(*x)?,
            Setting::High(x) => self.high = share(*x)?,
            other => return Err(format!("a load manager has no {}", other.name())),
        }
        Ok(())
    }

    /// Whether its lines make a guard: on again at or above the line it
    /// switches off below (else it would switch at every run).
    pub fn check(&self) -> Result<(), String> {
        if self.high >= self.low {
            Ok(())
        } else {
            Err(format!(
                "the on line ({:.0} %) is below the off line ({:.0} %): it would switch at every run",
                self.high * 100.0,
                self.low * 100.0
            ))
        }
    }

    /// One run: the switch commands, if the charge crossed its line.
    pub fn step(&mut self, cx: &Context) -> Commands {
        let Some(share) = charge_share(cx) else { return Commands::default() };
        let on = match self.shed {
            false if share < self.low => false,
            true if share >= self.high => true,
            _ => return Commands::default(),
        };
        self.shed = !on;
        Commands {
            ship: self.loads.iter().map(|id| ShipCommand::Power(id.clone(), on)).collect(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::state::ShipState;
    use crate::systems::ship_os::{health, View};
    use crate::time::Epoch;

    /// Off below its low line, on at its high line, nothing between; no
    /// battery read, nothing.
    #[test]
    fn the_guard_switches_its_loads_at_its_lines() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut state = ShipState::initial(&def);
        let h = health(&def, &state);
        let off = vec![ShipCommand::Power("dragon.docking".into(), false)];
        let on = vec![ShipCommand::Power("dragon.docking".into(), true)];
        // (share of the charge, shed before, commands, shed after)
        let cases = [
            (0.9, false, vec![], false),
            (0.29, false, off.clone(), true),
            (0.29, true, vec![], true),
            (0.5, true, vec![], true),
            (0.8, true, on, false),
        ];
        for (share, shed, cmds, after) in cases {
            for (b, d) in state.batteries.iter_mut().zip(&def.batteries) {
                b.charge = share * d.item.capacity;
            }
            let view = View {
                knowledge: crate::systems::data::knowledge(&def, &state, false),
                attitude: None,
                nav: None,
                engines: Vec::new(),
            };
            let cx = Context::new(Epoch::new(0, 0.0), &view, &h, &def, &state);
            let mut g = PowerGuard { loads: vec!["dragon.docking".into()], low: 0.3, high: 0.8, shed };
            assert_eq!(g.step(&cx).ship, cmds, "{share} shed {shed}");
            assert_eq!(g.shed, after);
        }
        let blind = View { knowledge: Default::default(), attitude: None, nav: None, engines: Vec::new() };
        let cx = Context::new(Epoch::new(0, 0.0), &blind, &h, &def, &state);
        assert_eq!(charge_share(&cx), None);
        let loads = switchable(&def);
        assert!(loads.iter().any(|l| l == "dragon.docking"), "{loads:?}");
        assert!(def.components.iter().filter(|c| c.power <= 0.0).all(|c| !loads.contains(&c.id)));
    }

    /// Its settings by its rule: loads only of those offered, once each;
    /// lines are shares; the on line at or above the off line (else it
    /// would switch at every run).
    #[test]
    fn the_guards_settings_by_its_rule() {
        let offered = vec!["a".to_string(), "b".into()];
        let g0 = PowerGuard { loads: vec!["a".into()], low: 0.3, high: 0.8, shed: false };
        let list = |l: &[&str]| Setting::Loads(l.iter().map(|s| s.to_string()).collect());
        // (setting, accepted)
        let cases = [
            (list(&["b", "a"]), true),
            (list(&[]), true),
            (list(&["c"]), false),
            (list(&["a", "a"]), false),
            (Setting::Low(0.0), true),
            (Setting::High(1.0), true),
            (Setting::Low(1.2), false),
            (Setting::High(-0.1), false),
            (Setting::Low(f64::NAN), false),
            (Setting::MainHeight(100.0), false),
        ];
        for (k, (setting, ok)) in cases.into_iter().enumerate() {
            let mut g = g0.clone();
            assert_eq!(g.set(&offered, &setting).is_ok(), ok, "case {k}");
            if !ok {
                assert_eq!(g, g0, "case {k}: unchanged");
            }
        }
        // (low, high, a guard)
        for (low, high, ok) in [(0.3, 0.8, true), (0.5, 0.5, true), (0.6, 0.5, false)] {
            assert_eq!(PowerGuard { low, high, ..g0.clone() }.check().is_ok(), ok, "{low} {high}");
        }
    }
}
