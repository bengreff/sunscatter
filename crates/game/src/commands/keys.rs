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

/// The help's groups of keys (`interface::help`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Flight,
    Time,
    Camera,
    Screens,
}

impl Action {
    /// Every action, in the help's order within its group.
    pub const ALL: [Action; 18] = [
        Action::ThrottleFull,
        Action::ThrottleCut,
        Action::Sas,
        Action::Rcs,
        Action::Chutes,
        Action::NextStage,
        Action::PreviousVessel,
        Action::NextVessel,
        Action::WarpUp,
        Action::WarpDown,
        Action::Warp1x,
        Action::FocusBody,
        Action::FocusShip,
        Action::SystemsScreen,
        Action::TrackingStation,
        Action::QuickSave,
        Action::QuickLoad,
        Action::Help,
    ];

    /// The help's group it is listed in.
    pub fn group(self) -> Group {
        use Action::*;
        match self {
            ThrottleFull | ThrottleCut | Sas | Rcs | Chutes | NextStage | PreviousVessel | NextVessel => Group::Flight,
            WarpUp | WarpDown | Warp1x => Group::Time,
            FocusBody | FocusShip => Group::Camera,
            SystemsScreen | TrackingStation | QuickSave | QuickLoad | Help => Group::Screens,
        }
    }

    /// What it does, as the help says it.
    pub fn about(self) -> &'static str {
        use Action::*;
        match self {
            ThrottleFull => "full throttle",
            ThrottleCut => "cut the throttle",
            Sas => "SAS on / off (the hold modes: click them on the navball)",
            Rcs => "RCS on / off, and its menu with it",
            Chutes => "parachutes: drogues now, mains low; again: the mains now",
            NextStage => "the next stage of the craft's sequence",
            PreviousVessel => "the previous vessel, nearest first",
            NextVessel => "the next vessel, nearest first",
            WarpUp => "warp up (from stopped time: 1x)",
            WarpDown => "warp down (at 1x: time stops)",
            Warp1x => "warp 1x",
            FocusBody => "the camera on the nearest body",
            FocusShip => "the camera on the ship",
            SystemsScreen => "the systems screen: the craft by layer, its commands, stages and functions",
            TrackingStation => "the tracking station",
            QuickSave => "quicksave",
            QuickLoad => "quickload",
            Help => "this help: the keys and every way in",
        }
    }
}

/// A key's name as the player reads it.
pub fn key_name(k: KeyCode) -> &'static str {
    use KeyCode::*;
    match k {
        KeyA => "A",
        KeyB => "B",
        KeyC => "C",
        KeyD => "D",
        KeyE => "E",
        KeyF => "F",
        KeyG => "G",
        KeyH => "H",
        KeyI => "I",
        KeyJ => "J",
        KeyK => "K",
        KeyL => "L",
        KeyM => "M",
        KeyN => "N",
        KeyO => "O",
        KeyP => "P",
        KeyQ => "Q",
        KeyR => "R",
        KeyS => "S",
        KeyT => "T",
        KeyU => "U",
        KeyV => "V",
        KeyW => "W",
        KeyX => "X",
        KeyY => "Y",
        KeyZ => "Z",
        Digit0 => "0",
        Digit1 => "1",
        Digit2 => "2",
        Digit3 => "3",
        Digit4 => "4",
        Digit5 => "5",
        Digit6 => "6",
        Digit7 => "7",
        Digit8 => "8",
        Digit9 => "9",
        Space => "Space",
        Tab => "Tab",
        Escape => "Esc",
        Period => ".",
        Comma => ",",
        Slash => "/",
        BracketLeft => "[",
        BracketRight => "]",
        Backquote => "`",
        _ => "?",
    }
}

/// A chord as the player reads it: `mac` names Cmd, elsewhere Ctrl+Alt;
/// Shift+/ is `?`.
pub fn chord_label(c: Chord, mac: bool) -> String {
    match c.with {
        Modifier::None => key_name(c.key).to_string(),
        Modifier::Shift if c.key == KeyCode::Slash => "?".to_string(),
        Modifier::Shift => format!("Shift+{}", key_name(c.key)),
        Modifier::Command if mac => format!("Cmd+{}", key_name(c.key)),
        Modifier::Command => format!("Ctrl+Alt+{}", key_name(c.key)),
    }
}

/// The keys bound to `what` (`mac`: as on a Mac), joined; empty when
/// none is.
pub fn labels(what: Action, mac: bool) -> String {
    let all: Vec<String> = BINDINGS.iter().filter(|(_, a)| *a == what).map(|(c, _)| chord_label(*c, mac)).collect();
    all.join(" / ")
}

/// The keys bound to `what` on this machine: the one source of every key
/// the interface names (the help, the buttons' labels).
pub fn label(what: Action) -> String {
    labels(what, cfg!(target_os = "macos"))
}

/// The held flight keys as the help lists them: (keys, what), from
/// [`ROTATE`], [`TRANSLATE`] and the throttle's modifiers (`throttle_after`).
pub fn held_rows() -> Vec<(String, &'static str)> {
    let pair = |(p, n): (KeyCode, KeyCode)| format!("{} {}", key_name(n), key_name(p));
    let set = |s: [(KeyCode, KeyCode); 3]| s.map(pair).join(" / ");
    vec![
        ("Shift / Ctrl".to_string(), "throttle up / down (hold Alt: fine)"),
        (set(ROTATE), "pitch / yaw / roll"),
        (set(TRANSLATE), "translate left right / up down / back forward (RCS on)"),
    ]
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
    agent: Res<crate::agent::HeldKeys>,
) {
    let held = Held::of(&keys);
    let just: Vec<Action> = keys.get_just_pressed().filter_map(|k| action(*k, held)).collect();
    if ctx.allows(Keys::Warp) {
        let (level, stopped) = (sim.effective_warp(), sim.paused);
        for c in just.iter().filter_map(|a| warp_command(*a, level, stopped)) {
            commands.write(c);
        }
    }
    let active = sim.ship().id();
    // P is the parachutes' press (D126: the manual deploy runs the same
    // function): it arms them, and a second calls the mains, by the
    // programs' one rule (nothing where nothing is left to open).
    let chutes = just.contains(&Action::Chutes).then(|| {
        let x = crate::systems_screen::programs::Inputs::of(sim.ship(), sim.clock)?;
        crate::systems_screen::programs::press(&crate::systems_screen::programs::rows(&x), active, "chutes").ok()
    });
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
            Action::Chutes => {
                commands.write_batch(chutes.clone().flatten());
            }
            Action::NextStage => {
                commands.write(GameCommand::Stage(active));
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
    // An agent aboard holds the rotation and translation keys like the
    // pilot (D044).
    let held = agent.of(active);
    c.rotate = (on(ROTATE) + held.rotate).clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
    c.translate = (on(TRANSLATE) + held.translate).clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
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
}
