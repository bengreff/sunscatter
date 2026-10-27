//! The stage and function editors' rules (D107; rule 8): the commands a
//! craft takes (the pick list), and edits to a list of named command lists
//! (the stage sequence or the saved functions: add, remove, rename, move,
//! add and remove commands, keys and pins). The editors work on a draft
//! and send it whole (`GameCommand::SetStages`, `SetFunctions`); the sim
//! checks every command and refuses the whole list with its reasons.

use sim::craft::assembly::JointKind;
use sim::craft::command::{ShipCommand, Stage};
use sim::craft::ship::ShipDef;

/// A pattern's copies (`base#k`) named together: `base#*`; any other id as is.
fn together(id: &str) -> String {
    match id.split_once('#') {
        Some((base, k)) if k.chars().all(|c| c.is_ascii_digit()) => format!("{base}#*"),
        _ => id.to_string(),
    }
}

/// Every command this craft takes, for the editors' pick list, in order:
/// engine groups (ignite, shut down, select), each engine's owner, the
/// mechanisms (deploy, retract), valves (open, close), separations, the
/// clamps, the parachutes, thruster blocks (on, off). A pattern's copies
/// come once, together (`leg#*`), then each.
pub fn catalogue(def: &ShipDef) -> Vec<ShipCommand> {
    let mut out: Vec<ShipCommand> = Vec::new();
    let mut push = |c: ShipCommand| {
        if !out.contains(&c) {
            out.push(c);
        }
    };
    let engines = def.groups.keys().cloned().chain(def.engines.iter().map(|e| e.owner.clone()));
    for s in engines {
        push(ShipCommand::Ignite(s.clone()));
        push(ShipCommand::Shutdown(s.clone()));
        push(ShipCommand::Select(s));
    }
    let both = |id: &str| [together(id), id.to_string()];
    for m in &def.mechanisms {
        for s in both(&m.id) {
            push(ShipCommand::Deploy(s.clone()));
            if m.item.retractable {
                push(ShipCommand::Retract(s));
            }
        }
    }
    for v in &def.valves {
        for s in both(&v.id) {
            push(ShipCommand::Open(s.clone()));
            push(ShipCommand::Close(s));
        }
    }
    for j in &def.joints {
        match j.kind {
            JointKind::Separation { .. } => push(ShipCommand::Separate(j.name.clone())),
            JointKind::HoldDown => push(ShipCommand::ReleaseClamps),
            _ => {}
        }
    }
    push(ShipCommand::ArmChutes);
    for t in &def.thrusters {
        for s in both(&t.id) {
            push(ShipCommand::Rcs(s.clone(), true));
            push(ShipCommand::Rcs(s, false));
        }
    }
    out
}

/// An edit of a list of stages (the sequence) or functions.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// A new, empty entry at the end.
    Add(String),
    Remove(usize),
    /// Move entry `i` one place earlier or later.
    Up(usize),
    Down(usize),
    Rename(usize, String),
    AddAction(usize, ShipCommand),
    RemoveAction(usize, usize),
    ActionUp(usize, usize),
    /// A function's key (`None`: none) and whether it is pinned.
    Key(usize, Option<String>),
    Pin(usize, bool),
}

/// Applies `edit` to `list`; an edit naming what is not there does nothing.
pub fn apply(list: &mut Vec<Stage>, edit: Edit) {
    let n = list.len();
    match edit {
        Edit::Add(name) => list.push(Stage { name, key: None, actions: Vec::new(), pinned: false }),
        Edit::Remove(i) if i < n => {
            list.remove(i);
        }
        Edit::Up(i) if i > 0 && i < n => list.swap(i - 1, i),
        Edit::Down(i) if i + 1 < n => list.swap(i, i + 1),
        Edit::Rename(i, name) if i < n => list[i].name = name,
        Edit::AddAction(i, c) if i < n => list[i].actions.push(c),
        Edit::RemoveAction(i, j) if i < n && j < list[i].actions.len() => {
            list[i].actions.remove(j);
        }
        Edit::ActionUp(i, j) if i < n && j > 0 && j < list[i].actions.len() => list[i].actions.swap(j - 1, j),
        Edit::Key(i, key) if i < n => list[i].key = key.filter(|k| !k.trim().is_empty()),
        Edit::Pin(i, on) if i < n => list[i].pinned = on,
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_rocket::rocket;

    #[test]
    fn the_catalogue_lists_what_the_craft_takes_and_each_checks() {
        let craft = rocket();
        let def = craft.ship.as_deref().unwrap();
        let all = catalogue(def);
        let labels: Vec<String> = all.iter().map(crate::staging::parts::label).collect();
        for want in [
            "IGNITE main",
            "SELECT pair",
            "IGNITE engine#3",
            "DEPLOY leg#*",
            "DEPLOY leg#2",
            "SEPARATE separation",
            "ARM PARACHUTES",
            "RCS rcs#* off",
        ] {
            assert!(labels.iter().any(|l| l == want), "{want} in {labels:?}");
        }
        assert!(!labels.iter().any(|l| l.starts_with("RETRACT")), "legs that cannot retract");
        assert!(!labels.iter().any(|l| l == "RELEASE CLAMPS"), "no pad mount");
        // Every command in the list is one the craft accepts.
        for c in &all {
            assert!(def.check(c).is_ok(), "{c:?}");
        }
        let unique: std::collections::BTreeSet<String> = labels.iter().cloned().collect();
        assert_eq!(unique.len(), labels.len());
    }

    #[test]
    fn edits_table() {
        let stage = |name: &str, n: usize| Stage {
            name: name.into(),
            key: None,
            actions: vec![ShipCommand::ArmChutes; n],
            pinned: false,
        };
        let names = |l: &[Stage]| l.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        // (edit, names after, actions of the first after)
        let table = [
            (Edit::Add("D".into()), vec!["A", "B", "C", "D"], 1),
            (Edit::Remove(1), vec!["A", "C"], 1),
            (Edit::Remove(9), vec!["A", "B", "C"], 1),
            (Edit::Up(1), vec!["B", "A", "C"], 2),
            (Edit::Up(0), vec!["A", "B", "C"], 1),
            (Edit::Down(2), vec!["A", "B", "C"], 1),
            (Edit::Down(0), vec!["B", "A", "C"], 2),
            (Edit::Rename(2, "Z".into()), vec!["A", "B", "Z"], 1),
            (Edit::AddAction(0, ShipCommand::ReleaseClamps), vec!["A", "B", "C"], 2),
            (Edit::RemoveAction(0, 0), vec!["A", "B", "C"], 0),
            (Edit::RemoveAction(0, 5), vec!["A", "B", "C"], 1),
        ];
        for (edit, want, actions) in table {
            let mut list = vec![stage("A", 1), stage("B", 2), stage("C", 0)];
            apply(&mut list, edit.clone());
            assert_eq!(names(&list), want, "{edit:?}");
            assert_eq!(list[0].actions.len(), actions, "{edit:?}");
        }
        let mut list =
            vec![Stage { actions: vec![ShipCommand::ArmChutes, ShipCommand::ReleaseClamps], ..stage("A", 0) }];
        apply(&mut list, Edit::ActionUp(0, 1));
        assert_eq!(list[0].actions[0], ShipCommand::ReleaseClamps);
        apply(&mut list, Edit::Key(0, Some("K".into())));
        apply(&mut list, Edit::Pin(0, true));
        assert_eq!((list[0].key.as_deref(), list[0].pinned), (Some("K"), true));
        apply(&mut list, Edit::Key(0, Some("  ".into())));
        assert_eq!(list[0].key, None, "a blank key is none");
    }
}
