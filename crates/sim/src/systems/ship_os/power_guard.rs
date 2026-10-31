//! A load manager (D128 layer 2: "simple hard coded logic" making the
//! inputs a player makes in the systems screen): at each of its epochs it
//! reads its batteries' charge and switches its loads off below `low` (a
//! share of the capacity it reads) and back on at or above `high`. With
//! layer 1 alone a short bus sheds by its own protection (`systems::power`)
//! once it runs dry; this acts earlier, on the loads it names.

use super::program::{Commands, Context};
use crate::craft::command::ShipCommand;
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

impl PowerGuard {
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
    }
}
