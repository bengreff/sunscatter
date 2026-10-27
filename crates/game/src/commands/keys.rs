//! Which keys the game takes, and the keys a craft's pinned functions get
//! (D107: the mini systems panel's functions run by a click or a key). A
//! function keeps the key its craft file or the systems screen gave it when
//! no game key uses it and no earlier pinned function took it; otherwise
//! it gets the first free key of [`FREE_KEYS`] (shown on the panel), so a
//! function key never shadows a flight key (`3` is a hold mode, `G` the
//! Staging menu, `A` yaw).

use bevy::prelude::KeyCode;

/// Every key the game acts on (the key help's, `interface::help::KEYS`,
/// plus F8, the systems screen): what a function key may not take.
pub const RESERVED: &[KeyCode] = &[
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::KeyZ,
    KeyCode::KeyX,
    KeyCode::KeyW,
    KeyCode::KeyS,
    KeyCode::KeyA,
    KeyCode::KeyD,
    KeyCode::KeyQ,
    KeyCode::KeyE,
    KeyCode::KeyT,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::KeyP,
    KeyCode::Space,
    KeyCode::KeyR,
    KeyCode::KeyO,
    KeyCode::KeyL,
    KeyCode::KeyU,
    KeyCode::KeyG,
    KeyCode::KeyN,
    KeyCode::Period,
    KeyCode::Comma,
    KeyCode::Slash,
    KeyCode::Escape,
    KeyCode::KeyF,
    KeyCode::Backquote,
    KeyCode::BracketLeft,
    KeyCode::BracketRight,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::KeyH,
];

/// Keys given, in order, to pinned functions whose own key is taken or
/// missing.
pub const FREE_KEYS: &[KeyCode] = &[
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyV,
    KeyCode::KeyY,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyM,
];

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];
const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// A function key as a craft file writes it: one letter or digit, either
/// case (`None` for anything else).
pub fn key_code(name: &str) -> Option<KeyCode> {
    let mut chars = name.trim().chars();
    let (c, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return None;
    }
    let c = c.to_ascii_uppercase();
    match c {
        'A'..='Z' => Some(LETTERS[(c as u8 - b'A') as usize]),
        '0'..='9' => Some(DIGITS[(c as u8 - b'0') as usize]),
        _ => None,
    }
}

/// A key as the panel shows it (`KeyB` → `B`, `Digit7` → `7`).
pub fn key_label(k: KeyCode) -> String {
    let text = format!("{k:?}");
    text.strip_prefix("Key").or_else(|| text.strip_prefix("Digit")).unwrap_or(&text).to_string()
}

/// The key each of a craft's pinned functions runs from, in order, given
/// the keys they ask for (see the module): `None` only when every free key
/// is taken.
pub fn bind_keys(requested: &[Option<&str>]) -> Vec<Option<KeyCode>> {
    let mut taken: Vec<KeyCode> = Vec::new();
    let mut out: Vec<Option<KeyCode>> = (requested.iter())
        .map(|r| {
            let k = r.and_then(key_code).filter(|k| !RESERVED.contains(k) && !taken.contains(k))?;
            taken.push(k);
            Some(k)
        })
        .collect();
    for slot in out.iter_mut().filter(|s| s.is_none()) {
        // Keys asked for by later functions stay theirs.
        let asked: Vec<KeyCode> = requested.iter().filter_map(|r| r.and_then(key_code)).collect();
        let free = FREE_KEYS.iter().find(|k| !taken.contains(k) && !asked.contains(k)).copied();
        let free = free.or_else(|| FREE_KEYS.iter().find(|k| !taken.contains(k)).copied());
        if let Some(k) = free {
            taken.push(k);
            *slot = Some(k);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_keys_never_shadow_a_game_key() {
        // The Falcon 9's functions ask for 3, 1, 0, G, L, C, N and A.
        let asked = [Some("3"), Some("1"), Some("0"), Some("G"), Some("L"), Some("C"), Some("N"), Some("a")];
        let got: Vec<String> = bind_keys(&asked).into_iter().map(|k| key_label(k.unwrap())).collect();
        // 0 and C are free and kept; the rest take free keys in order,
        // skipping those asked for.
        assert_eq!(got, ["7", "8", "0", "9", "B", "C", "V", "Y"]);
        // (asked, bound)
        type Case<'a> = (&'a [Option<&'a str>], &'a [Option<KeyCode>]);
        let table: [Case; 4] = [
            (&[Some("k"), Some("K")], &[Some(KeyCode::KeyK), Some(KeyCode::Digit7)]),
            (&[None, Some("7")], &[Some(KeyCode::Digit8), Some(KeyCode::Digit7)]),
            (&[Some("F10"), Some("")], &[Some(KeyCode::Digit7), Some(KeyCode::Digit8)]),
            (&[], &[]),
        ];
        for (asked, want) in table {
            assert_eq!(bind_keys(asked), want, "{asked:?}");
        }
        for k in bind_keys(&[Some("Z"); 20]).into_iter().flatten() {
            assert!(!RESERVED.contains(&k));
        }
        assert_eq!(bind_keys(&[None; 13]).iter().filter(|k| k.is_none()).count(), 1, "12 free keys");
    }

    #[test]
    fn keys_parse_and_print() {
        // (text, key)
        let table = [("a", Some(KeyCode::KeyA)), ("Z", Some(KeyCode::KeyZ)), ("7", Some(KeyCode::Digit7))];
        for (text, key) in table {
            assert_eq!(key_code(text), key, "{text}");
        }
        for bad in ["", "ab", "F1", "-", "é"] {
            assert_eq!(key_code(bad), None, "{bad}");
        }
        assert_eq!((key_label(KeyCode::KeyB), key_label(KeyCode::Digit0)), ("B".into(), "0".into()));
    }

    /// Every key the help lists is reserved, and no free key is.
    #[test]
    fn the_reserved_keys_cover_the_help() {
        let named = |t: &str| -> Option<KeyCode> {
            Some(match t {
                "Space" => KeyCode::Space,
                "Esc" => KeyCode::Escape,
                "." => KeyCode::Period,
                "," => KeyCode::Comma,
                "/" => KeyCode::Slash,
                "[" => KeyCode::BracketLeft,
                "]" => KeyCode::BracketRight,
                "`" => KeyCode::Backquote,
                "Shift" => KeyCode::ShiftLeft,
                "Ctrl" => KeyCode::ControlLeft,
                "Alt" => KeyCode::AltLeft,
                t if t.starts_with('F') && t.len() > 1 => [
                    KeyCode::F2,
                    KeyCode::F3,
                    KeyCode::F4,
                    KeyCode::F5,
                    KeyCode::F6,
                    KeyCode::F7,
                    KeyCode::F8,
                    KeyCode::F9,
                ][t[1..].parse::<usize>().ok()?.checked_sub(2)?.min(7)],
                t => key_code(t)?,
            })
        };
        let mut seen = 0;
        for (_, keys) in crate::interface::help::KEYS {
            for (k, _) in *keys {
                let tokens = k.split(|c: char| c.is_whitespace() || c == '/').filter(|t| !t.is_empty());
                for t in tokens.flat_map(|t| if t == "1–6" { vec!["1", "6"] } else { vec![t] }) {
                    if let Some(code) = named(t) {
                        assert!(RESERVED.contains(&code), "{t} ({k}) is not reserved");
                        seen += 1;
                    }
                }
            }
        }
        assert!(seen > 30, "{seen}");
        assert!(FREE_KEYS.iter().all(|k| !RESERVED.contains(k)));
    }
}
