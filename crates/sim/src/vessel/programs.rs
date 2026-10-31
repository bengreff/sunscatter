//! The ship computer's scheduled programs on a vessel (D126, D129;
//! `systems::ship_os::program`): each runs at its own due epochs, where the
//! vessel's advance stops as at an order whatever the warp, on the view its
//! computers have, and its commands go through the native boundary as a
//! player's from aboard (no light delay: the computer is aboard).

use super::Vessel;
use crate::craft::command::ShipCommand;
use crate::systems::command;
use crate::systems::ship_os::program::{Context, Installed};
use crate::systems::ship_os::{self, Switch};
use crate::time::Epoch;
use crate::world::World;

impl Vessel {
    /// The scheduled programs on its computer.
    pub fn programs(&self) -> &[Installed] {
        &self.software.programs
    }

    /// Installs a program (it runs after those installed before it at a
    /// shared epoch).
    pub fn install(&mut self, program: Installed) {
        self.software.programs.push(program);
    }

    /// Sets a program's switch (D126): `maneuver` is the planned burns'
    /// program, `chutes` the parachutes' deploy, else an installed one by
    /// name.
    pub fn set_switch(&mut self, name: &str, switch: Switch) -> Result<(), String> {
        match name {
            "maneuver" => self.software.maneuver.switch = switch,
            "chutes" => self.software.chutes.switch = switch,
            _ => {
                let now = self.time;
                let p = self.installed_mut(name)?;
                p.slot.switch = switch;
                p.resume(now);
            }
        }
        Ok(())
    }

    /// The player's press for a program (the names of [`Self::set_switch`]):
    /// a scheduled one runs at its next due epoch, a planned burn at its
    /// wake epoch (or now if that has passed), the parachutes' deploy now
    /// (`ship_os::chutes::press`, as `ShipCommand::ArmChutes`).
    pub fn press(&mut self, name: &str) -> Result<(), String> {
        match name {
            "maneuver" => self.software.maneuver.pressed = true,
            "chutes" => ship_os::chutes::press(&mut self.software.chutes, &mut self.chute),
            _ => {
                let now = self.time;
                let p = self.installed_mut(name)?;
                p.slot.pressed = true;
                p.resume(now);
            }
        }
        Ok(())
    }

    fn installed_mut(&mut self, name: &str) -> Result<&mut Installed, String> {
        self.software.programs.iter_mut().find(|p| p.name == name).ok_or_else(|| format!("no program {name}"))
    }

    /// The epoch the next scheduled program is due, for a craft of parts.
    pub(super) fn programs_stop(&self) -> Option<Epoch> {
        self.ship.as_ref()?;
        self.software.programs.iter().filter_map(Installed::due).reduce(|a, b| if b < a { b } else { a })
    }

    /// Runs every program due by the vessel's time, in order, on one view
    /// of the craft, then applies their commands (`command::apply`: ship
    /// commands and ignitions; a refused command goes to the ship's log).
    pub(super) fn run_programs(&mut self, world: &World) {
        let at = self.time;
        let due = |p: &Installed| p.due().is_some_and(|e| e <= at);
        if !self.software.programs.iter().any(due) {
            return;
        }
        let Some(def) = self.ship_def() else { return };
        self.law_to_now();
        let (health, k, motion) = (self.flight_health(), self.knowledge(), (self.attitude, self.state(world)));
        let Some(state) = self.ship.as_ref() else { return };
        let view = ship_os::sense(k, motion, Some((&def, state)));
        let cx = Context::new(at, &view, &health, &def, state);
        let out: Vec<_> =
            self.software.programs.iter_mut().filter(|p| due(p)).map(|p| (p.name.clone(), p.run(&cx))).collect();
        let state = self.ship.as_mut().expect("a craft of parts");
        let cx = command::Context { at, sequence: false };
        let mut effects = Vec::new();
        for (name, c) in out {
            let ignite =
                c.ignite.iter().map(|&i| ShipCommand::Ignite(def.owner_id(def.engines[i as usize].owner).to_string()));
            for cmd in c.ship.into_iter().chain(ignite) {
                match command::apply(&def, state, &cmd, cx) {
                    Ok(e) => effects.extend(e),
                    Err(why) => state.event(at, format!("{name}: {why}"), false),
                }
            }
        }
        self.after_command(world, &effects);
    }
}
