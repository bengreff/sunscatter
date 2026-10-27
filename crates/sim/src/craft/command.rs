//! Ship commands (D107, D108; `docs/design/ship-model.md` §8): one data
//! type for everything that commands a craft of parts — the stage sequence
//! and saved functions of its craft file, the systems screen, the MCP
//! server and later scripts. A command names what it acts on by the craft
//! file's ids: a part or component id, a pattern's copies with `#*`
//! (`s1.merlin#*`), or an engine group's name; joints by their name.
//!
//! What a command does is the systems' (`sim::systems`, RUNTIME): this
//! module only defines the data and how a selector matches ids.

use serde::{Deserialize, Serialize};

/// One command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShipCommand {
    /// Start the selected engines (each needs an ignition left).
    Ignite(String),
    /// Shut them down.
    Shutdown(String),
    /// Set their throttle (a fraction of full thrust, held within each
    /// engine's range); the pilot's throttle scales it.
    Throttle(String, f64),
    /// Make these the engines the pilot's throttle and readouts refer to.
    Select(String),
    /// Valves.
    Open(String),
    Close(String),
    /// Mechanisms: to their deployed or stowed angle, or to an angle (deg).
    Deploy(String),
    Retract(String),
    Angle(String, f64),
    /// Fire the separation system of the named joint.
    Separate(String),
    /// The pad's hold-down clamps let go.
    ReleaseClamps,
    /// Arm the parachutes (`sim::chute`'s sequence).
    ArmChutes,
    /// Enable or disable thruster blocks.
    Rcs(String, bool),
    /// Switch a component's power.
    Power(String, bool),
}

/// A named list of commands: a stage of the sequence or a saved function
/// (with the key that runs it from the mini systems panel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    #[serde(default)]
    pub key: Option<String>,
    pub actions: Vec<ShipCommand>,
}

impl ShipCommand {
    /// The selector it acts on, if any.
    pub fn target(&self) -> Option<&str> {
        match self {
            ShipCommand::Ignite(s)
            | ShipCommand::Shutdown(s)
            | ShipCommand::Throttle(s, _)
            | ShipCommand::Select(s)
            | ShipCommand::Open(s)
            | ShipCommand::Close(s)
            | ShipCommand::Deploy(s)
            | ShipCommand::Retract(s)
            | ShipCommand::Angle(s, _)
            | ShipCommand::Separate(s)
            | ShipCommand::Rcs(s, _)
            | ShipCommand::Power(s, _) => Some(s),
            ShipCommand::ReleaseClamps | ShipCommand::ArmChutes => None,
        }
    }
}

/// Whether `selector` names `id`: equal, or `base#*` for any copy `base#k`
/// of a pattern, or a prefix ending in `.` for everything under it
/// (`s1.` is every part and component whose id starts with `s1.`).
pub fn matches(selector: &str, id: &str) -> bool {
    if let Some(base) = selector.strip_suffix("#*") {
        return id.strip_prefix(base).is_some_and(|rest| rest.starts_with('#'));
    }
    if selector.ends_with('.') {
        return id.starts_with(selector);
    }
    selector == id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_match_ids() {
        // (selector, id, matches)
        let cases = [
            ("s1.merlin#*", "s1.merlin#3", true),
            ("s1.merlin#*", "s1.merlin", false),
            ("s1.merlin#*", "s1.merlinx#3", false),
            ("s1.merlin#3", "s1.merlin#3", true),
            ("s1.merlin#3", "s1.merlin#30", false),
            ("s1.", "s1.leg#2", true),
            ("s1.", "s2.tank", false),
            ("s2.tank", "s2.tank", true),
        ];
        for (selector, id, want) in cases {
            assert_eq!(matches(selector, id), want, "{selector} vs {id}");
        }
    }

    #[test]
    fn commands_round_trip_through_ron() {
        let stage = Stage {
            name: "Ignition".into(),
            key: Some("1".into()),
            actions: vec![
                ShipCommand::Ignite("s1.merlin#*".into()),
                ShipCommand::Throttle("s1".into(), 0.6),
                ShipCommand::ReleaseClamps,
                ShipCommand::Rcs("dragon.draco#*".into(), true),
            ],
        };
        let text = ron::to_string(&stage).unwrap();
        assert_eq!(ron::from_str::<Stage>(&text).unwrap(), stage);
        assert_eq!(stage.actions[1].target(), Some("s1"));
        assert_eq!(stage.actions[2].target(), None);
    }
}
