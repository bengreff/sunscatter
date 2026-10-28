//! Which keys the game takes (D111): one table, [`BINDINGS`], from a key
//! (with its modifiers) to what it does, read by every system that acts on
//! a key ([`pressed`]); the flight keys ([`read_controls`]); the task
//! menus' number keys (`MenuId::key`). No function keys (they need fn on a
//! Mac) and no craft-specific keys: a craft's pinned functions are clicked
//! on the action-group bar, its hold modes on the navball.
//!
//! Quicksave and quickload are Cmd+S / Cmd+L on a Mac and Ctrl+Alt+S /
//! Ctrl+Alt+L elsewhere (Ctrl alone is throttle down); while such a chord
//! is held the flight keys under it do not act (S would pitch). Help is
//! `?` (Shift+/): the throttle Shift ramped on the way is put back.

use crate::commands::{GameCommand, InputContext, Keys};
use crate::state::{throttle_after, SimState};
use bevy::prelude::*;
use glam::DVec3;

/// What a key does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Flight.
    ThrottleFull,
    ThrottleCut,
    Sas,
    /// RCS on or off (and its menu with it).
    Rcs,
    Chutes,
    NextStage,
    PreviousVessel,
    NextVessel,
    /// Time: one level up (from stopped time, 1×), one down (at 1×, time
    /// stops), 1×.
    WarpUp,
    WarpDown,
    Warp1x,
    /// Screens.
    SystemsScreen,
    TrackingStation,
    QuickSave,
    QuickLoad,
    Help,
    /// Camera.
    FocusBody,
    FocusShip,
}

/// A key and the modifiers it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: KeyCode,
    pub with: Modifier,
}

/// The modifier a chord needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    None,
    /// Shift (`?` is Shift+/).
    Shift,
    /// Cmd on a Mac, Ctrl+Alt elsewhere: the saves' chords.
    Command,
}

/// The modifiers held now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Cmd (the Super key).
    pub command: bool,
}

impl Held {
    pub fn of(keys: &ButtonInput<KeyCode>) -> Held {
        let any = |a, b| keys.pressed(a) || keys.pressed(b);
        Held {
            shift: any(KeyCode::ShiftLeft, KeyCode::ShiftRight),
            ctrl: any(KeyCode::ControlLeft, KeyCode::ControlRight),
            alt: any(KeyCode::AltLeft, KeyCode::AltRight),
            command: any(KeyCode::SuperLeft, KeyCode::SuperRight),
        }
    }

    /// A saves' chord is held (Cmd, or Ctrl+Alt): the flight keys under
    /// it do not act.
    pub fn chord(&self) -> bool {
        self.command || (self.ctrl && self.alt)
    }
}

impl Modifier {
    /// Whether `held` satisfies it. A plain key does not act under a
    /// saves' chord; Shift does not stop it (Shift is the throttle, held
    /// while flying).
    pub fn matches(self, held: Held) -> bool {
        match self {
            Modifier::None => !held.chord(),
            Modifier::Shift => held.shift && !held.chord(),
            Modifier::Command => held.chord(),
        }
    }
}

const fn plain(key: KeyCode) -> Chord {
    Chord { key, with: Modifier::None }
}

/// Every key binding but the held flight keys (throttle, rotation,
/// translation: [`read_controls`]) and the menus' (`MenuId::key`).
pub const BINDINGS: &[(Chord, Action)] = &[
    (plain(KeyCode::KeyZ), Action::ThrottleFull),
    (plain(KeyCode::KeyX), Action::ThrottleCut),
    (plain(KeyCode::KeyT), Action::Sas),
    (plain(KeyCode::KeyR), Action::Rcs),
    (plain(KeyCode::KeyP), Action::Chutes),
    (plain(KeyCode::Space), Action::NextStage),
    (plain(KeyCode::BracketLeft), Action::PreviousVessel),
    (plain(KeyCode::BracketRight), Action::NextVessel),
    (plain(KeyCode::Period), Action::WarpUp),
    (plain(KeyCode::Comma), Action::WarpDown),
    (plain(KeyCode::Slash), Action::Warp1x),
    (plain(KeyCode::Tab), Action::SystemsScreen),
    (plain(KeyCode::KeyM), Action::TrackingStation),
    (Chord { key: KeyCode::KeyS, with: Modifier::Command }, Action::QuickSave),
    (Chord { key: KeyCode::KeyL, with: Modifier::Command }, Action::QuickLoad),
    (Chord { key: KeyCode::Slash, with: Modifier::Shift }, Action::Help),
    (plain(KeyCode::KeyF), Action::FocusBody),
    (plain(KeyCode::Backquote), Action::FocusShip),
];

/// The held flight keys: pitch, yaw, roll (W S, A D, Q E) and translation
/// along X, Y, Z (L J, K I, H N), each as (positive, negative).
pub const ROTATE: [(KeyCode, KeyCode); 3] =
    [(KeyCode::KeyS, KeyCode::KeyW), (KeyCode::KeyD, KeyCode::KeyA), (KeyCode::KeyE, KeyCode::KeyQ)];
/// Body axes: +X right (L), +Y the bottom (K: down; −Y is the top), +Z the
/// nose (H: forward).
pub const TRANSLATE: [(KeyCode, KeyCode); 3] =
    [(KeyCode::KeyL, KeyCode::KeyJ), (KeyCode::KeyK, KeyCode::KeyI), (KeyCode::KeyH, KeyCode::KeyN)];

/// The action of `key` pressed with `held`, if any.
pub fn action(key: KeyCode, held: Held) -> Option<Action> {
    // A chord with its modifier wins over the plain key (Shift+/ is help,
    // not 1×).
    let matching = || BINDINGS.iter().filter(|(c, _)| c.key == key && c.with.matches(held));
    matching().find(|(c, _)| c.with != Modifier::None).or_else(|| matching().next()).map(|(_, a)| *a)
}

/// Whether `what` was pressed this frame.
pub fn pressed(keys: &ButtonInput<KeyCode>, what: Action) -> bool {
    let held = Held::of(keys);
    keys.get_just_pressed().any(|k| action(*k, held) == Some(what))
}

/// The axis of two held keys: +1, −1 or 0.
fn axis(keys: &ButtonInput<KeyCode>, (pos, neg): (KeyCode, KeyCode)) -> f64 {
    f64::from(u8::from(keys.pressed(pos))) - f64::from(u8::from(keys.pressed(neg)))
}

/// The time keys' command at warp `level` (`stopped`: time stopped): `,`
/// at 1× stops time, `.` from stopped time runs it at 1×.
pub fn warp_command(what: Action, level: usize, stopped: bool) -> Option<GameCommand> {
    match what {
        Action::WarpUp if stopped => Some(GameCommand::SetWarp(0)),
        Action::WarpUp => Some(GameCommand::SetWarp(level + 1)),
        Action::WarpDown if stopped => None,
        Action::WarpDown if level == 0 => Some(GameCommand::PauseTime),
        Action::WarpDown => Some(GameCommand::SetWarp(level - 1)),
        Action::Warp1x => Some(GameCommand::SetWarp(0)),
        _ => None,
    }
}

/// Keyboard → controls and warp (in the input stage; the discrete
/// changes as commands).
pub fn read_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ctx: Res<InputContext>,
    mut sim: ResMut<SimState>,
    mut commands: MessageWriter<GameCommand>,
    mut shift_from: Local<Option<f64>>,
) {
    let held = Held::of(&keys);
    let just: Vec<Action> = keys.get_just_pressed().filter_map(|k| action(*k, held)).collect();
    if ctx.allows(Keys::Warp) {
        let (level, stopped) = (sim.effective_warp(), sim.paused);
        for c in just.iter().filter_map(|a| warp_command(*a, level, stopped)) {
            commands.write(c);
        }
    }
    let c = sim.controls_mut();
    // Typing in a text field, the pause menu and the tracking station do
    // not fly the ship.
    if !ctx.allows(Keys::Flight) {
        c.rotate = DVec3::ZERO;
        c.translate = DVec3::ZERO;
        return;
    }
    // `?` is Shift+/: the throttle Shift ramped on the way is put back.
    if keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight) {
        *shift_from = Some(c.throttle);
    }
    if just.contains(&Action::Help) {
        c.throttle = shift_from.unwrap_or(c.throttle);
    }
    let free = !held.chord() && !keys.pressed(KeyCode::Slash);
    if free {
        c.throttle = throttle_after(c.throttle, held.shift, held.ctrl, held.alt, time.delta_secs_f64());
    }
    for a in &just {
        match a {
            Action::ThrottleFull => c.throttle = 1.0,
            Action::ThrottleCut => c.throttle = 0.0,
            Action::Sas => c.sas = !c.sas,
            Action::Rcs => c.rcs.on = !c.rcs.on,
            Action::Chutes => c.chute = true,
            Action::NextStage => {
                commands.write(GameCommand::NextStage);
            }
            _ => {}
        }
    }
    let on = |set: [(KeyCode, KeyCode); 3]| {
        if free {
            DVec3::new(axis(&keys, set[0]), axis(&keys, set[1]), axis(&keys, set[2]))
        } else {
            DVec3::ZERO
        }
    };
    c.rotate = on(ROTATE);
    c.translate = on(TRANSLATE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::menus::MenuId;

    const NONE: Held = Held { shift: false, ctrl: false, alt: false, command: false };

    /// No two bindings share a chord, no function key is used, and no
    /// binding uses a held flight key or a menu's number key.
    #[test]
    fn every_key_does_one_thing() {
        for (i, (a, _)) in BINDINGS.iter().enumerate() {
            for (b, _) in &BINDINGS[i + 1..] {
                assert!(a != b, "{a:?} twice");
            }
            assert!(!format!("{:?}", a.key).starts_with('F') || a.key == KeyCode::KeyF, "{a:?}");
            if a.with == Modifier::None {
                let held = ROTATE.iter().chain(&TRANSLATE).any(|(p, n)| *p == a.key || *n == a.key);
                assert!(!held, "{a:?} is a held flight key");
                assert!(MenuId::ALL.iter().all(|m| m.key() != Some(a.key)), "{a:?} opens a menu");
            }
        }
    }

    /// The chords (table): a plain key under no modifier or Shift, not
    /// under Cmd or Ctrl+Alt; the saves under Cmd or Ctrl+Alt; `?`.
    #[test]
    fn chords_and_modifiers() {
        let shift = Held { shift: true, ..NONE };
        let cmd = Held { command: true, ..NONE };
        let ctrl_alt = Held { ctrl: true, alt: true, ..NONE };
        let ctrl = Held { ctrl: true, ..NONE };
        // (key, held, action)
        let table = [
            (KeyCode::KeyZ, NONE, Some(Action::ThrottleFull)),
            (KeyCode::KeyZ, shift, Some(Action::ThrottleFull)),
            (KeyCode::KeyZ, cmd, None),
            (KeyCode::KeyS, NONE, None),
            (KeyCode::KeyS, cmd, Some(Action::QuickSave)),
            (KeyCode::KeyS, ctrl_alt, Some(Action::QuickSave)),
            (KeyCode::KeyS, ctrl, None),
            (KeyCode::KeyL, cmd, Some(Action::QuickLoad)),
            (KeyCode::KeyL, NONE, None),
            (KeyCode::Slash, NONE, Some(Action::Warp1x)),
            (KeyCode::Slash, shift, Some(Action::Help)),
            (KeyCode::Tab, NONE, Some(Action::SystemsScreen)),
            (KeyCode::KeyM, NONE, Some(Action::TrackingStation)),
            (KeyCode::KeyR, NONE, Some(Action::Rcs)),
            (KeyCode::Digit1, NONE, None),
            (KeyCode::F8, NONE, None),
        ];
        for (key, held, want) in table {
            assert_eq!(action(key, held), want, "{key:?} {held:?}");
        }
    }

    /// The time keys (table): `,` at 1× stops time, `.` from there runs it
    /// at 1×, `/` at 1× from anywhere.
    #[test]
    fn time_keys_stop_and_resume() {
        use Action::*;
        // (key, level, stopped, command)
        let table = [
            (WarpUp, 0, false, Some(GameCommand::SetWarp(1))),
            (WarpUp, 3, true, Some(GameCommand::SetWarp(0))),
            (WarpDown, 3, false, Some(GameCommand::SetWarp(2))),
            (WarpDown, 0, false, Some(GameCommand::PauseTime)),
            (WarpDown, 0, true, None),
            (Warp1x, 7, false, Some(GameCommand::SetWarp(0))),
            (Warp1x, 0, true, Some(GameCommand::SetWarp(0))),
            (Sas, 0, false, None),
        ];
        for (what, level, stopped, want) in table {
            assert_eq!(warp_command(what, level, stopped), want, "{what:?} {level} {stopped}");
        }
    }

    /// The help lists every binding's key, and every menu's.
    #[test]
    fn the_help_lists_every_key() {
        let text: Vec<&str> =
            crate::interface::help::KEYS.iter().flat_map(|(_, keys)| keys.iter().map(|(k, _)| *k)).collect();
        let listed = |label: &str| text.iter().any(|k| *k == label || k.split([' ', '/']).any(|t| t == label));
        let label = |c: &Chord| match c.key {
            KeyCode::Space => "Space".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Period => ".".to_string(),
            KeyCode::Comma => ",".to_string(),
            KeyCode::Slash if c.with == Modifier::Shift => "?".to_string(),
            KeyCode::Slash => "/".to_string(),
            KeyCode::BracketLeft => "[".to_string(),
            KeyCode::BracketRight => "]".to_string(),
            KeyCode::Backquote => "`".to_string(),
            k => format!("{k:?}").trim_start_matches("Key").to_string(),
        };
        for (c, a) in BINDINGS {
            let l = label(c);
            let l = if c.with == Modifier::Command { format!("Cmd+{l}") } else { l };
            assert!(listed(&l), "{a:?} ({l}) is not in the help");
        }
        for m in MenuId::ALL.iter().filter(|m| m.key().is_some()) {
            assert!(text.iter().any(|k| k.contains(m.key_label())), "{m:?}");
        }
    }
}
