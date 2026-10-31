//! The RCS menu (D111, D112): shown while RCS is on (the navball's RCS
//! button, R), at the top of the left dock. One page: the mode and its
//! overrides; what the RCS can do along and about each axis, with a
//! warning where a translation also turns the craft; the propellant of
//! every feed (kg/kg); every thruster block of the vessel, each switched
//! on or off here (`ShipCommand::Rcs`), with its state and temperature. The
//! craft file's thruster sequence stands (the owner, 2026-10-27): a block
//! it starts disabled reads "disabled" until its stage enables it or the
//! pilot does. The systems screen's plumbing layer has the rest.

use super::{landing::heat_tone, Item, MenuAction, Section, Tone};
use crate::hud::temperature;
use sim::craft::command::ShipCommand;
use sim::systems::rcs::{Authority, RcsMode, RcsOnly};

/// A line of thrust that misses the centre of mass by more than this (m)
/// turns the craft as it translates: warned.
pub const IMBALANCE: f64 = 0.1;

/// The RCS as the menu shows it (`rcs_data::gather`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RcsData {
    pub mode: RcsMode,
    /// What it can do now.
    pub authority: Authority,
    pub feeds: Vec<Feed>,
    pub blocks: Vec<Block>,
}

/// A tank or bottle feeding thrusters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feed {
    pub name: String,
    /// What it holds as measured (D106: a tank's level, a bottle's
    /// pressure as mass, `inspect::bottle_contents`); `None` unmeasured.
    pub kg: Option<sim::systems::Reading>,
    /// What it holds full (kg).
    pub full: f64,
}

/// A thruster block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub id: String,
    pub nozzles: usize,
    /// Thrust per nozzle (N).
    pub thrust: f64,
    /// Enabled (the craft file's sequence, or the pilot).
    pub on: bool,
    pub state: BlockState,
    /// Its part's skin nearest its limit and that limit (K), if measured.
    pub kelvin: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockState {
    #[default]
    Ready,
    /// Its feed is empty.
    Dry,
    /// Failed, or without power.
    Out,
}

/// The six directions of `Authority::translate` (body axes: +X right, +Y
/// the bottom, +Z the nose) and their keys.
const DIRECTIONS: [&str; 6] = ["RIGHT (L)", "LEFT (J)", "DOWN (K)", "UP (I)", "FORWARD (H)", "BACK (N)"];

pub fn sections(r: &RcsData) -> Vec<Section> {
    vec![mode(r), authority(r), feeds(r), blocks(r)]
}

/// The mode and its overrides, and how many blocks are on.
fn mode(r: &RcsData) -> Section {
    let mut s = Section::summary();
    let m = r.mode;
    let set = |only: RcsOnly, fine: bool| MenuAction::SetRcs(RcsMode { only, fine, ..m });
    let options = [
        (RcsOnly::Both, "translate and rotate"),
        (RcsOnly::Translate, "translate only"),
        (RcsOnly::Rotate, "rotate only"),
    ];
    let current = options.iter().find(|o| o.0 == m.only).map_or("", |o| o.1).to_string();
    let options = options.iter().map(|(o, name)| (name.to_string(), *o == m.only, set(*o, m.fine))).collect();
    s.items.push(Item::Choice { label: "USE".into(), current, options });
    let fine = if m.fine { "inputs at a tenth" } else { "off" };
    s.toggle("FINE CONTROL", m.fine, fine, Tone::Normal, set(m.only, !m.fine));
    if !r.blocks.is_empty() {
        let on = r.blocks.iter().filter(|b| b.on).count();
        let tone = if on == 0 { Tone::Warn } else { Tone::Normal };
        s.toned("BLOCKS ON", format!("{on} of {}", r.blocks.len()), tone);
    }
    s.note("I K · J L · H N translate; W S · A D · Q E turn with the thrusters and gimbals", Tone::Dim);
    s
}

/// Translation per direction and rotation per axis.
fn authority(r: &RcsData) -> Section {
    let mut s = Section::new("authority", "AUTHORITY");
    if r.blocks.is_empty() {
        s.note("no thruster blocks", Tone::Dim);
        return s;
    }
    let a = &r.authority;
    let mut turns = Vec::new();
    for (name, (accel, miss)) in DIRECTIONS.iter().zip(a.translate) {
        if accel <= 0.0 {
            s.toned(*name, "none", Tone::Dim);
            continue;
        }
        let tone = if miss > IMBALANCE { Tone::Warn } else { Tone::Normal };
        s.toned(*name, format!("{accel:.3} m/s²"), tone);
        if miss > IMBALANCE {
            turns.push(format!("{} ({miss:.2} m)", name.split(' ').next().unwrap_or(name).to_lowercase()));
        }
    }
    if !turns.is_empty() {
        let text = format!("These also turn the craft, their thrust missing the centre of mass: {}", turns.join(", "));
        s.note(text, Tone::Warn);
    }
    let deg = |x: f64| if x > 0.0 { format!("{:.2} °/s²", x.to_degrees()) } else { "none".to_string() };
    s.row("PITCH", deg(a.rotate.x));
    s.row("YAW", deg(a.rotate.y));
    s.row("ROLL", deg(a.rotate.z));
    s
}

/// Each feed's propellant of its full load (`format::amount`, the tanks'
/// and bottles' one rule), measured or a dash.
fn feeds(r: &RcsData) -> Section {
    let mut s = Section::new("feeds", "PROPELLANT");
    for f in &r.feeds {
        let tone = match f.kg.map(|r| r.shown()) {
            Some(kg) if kg <= 0.0 => Tone::Alert,
            Some(kg) if kg < 0.1 * f.full => Tone::Warn,
            _ => Tone::Normal,
        };
        s.toned(f.name.to_uppercase(), crate::format::amount(f.kg, f.full), tone);
    }
    s
}

/// Every thruster block, a switch each: disabled ones unchecked and said
/// so, failed or dry ones in their colour.
fn blocks(r: &RcsData) -> Section {
    let mut s = Section::new("blocks", format!("THRUSTERS · {}", r.blocks.len()));
    for b in &r.blocks {
        let (word, tone) = match (b.on, b.state) {
            (_, BlockState::Out) => ("failed", Tone::Alert),
            (true, BlockState::Dry) => ("dry", Tone::Warn),
            (true, BlockState::Ready) => ("on", Tone::Good),
            (false, _) => ("disabled", Tone::Dim),
        };
        let size = format!("{}×{:.0} N", b.nozzles, b.thrust);
        let heat = b.kelvin.map_or(String::new(), |(t, limit)| format!(" · {}", temperature(t, limit)));
        let tone = match b.kelvin {
            Some((t, limit)) if b.state == BlockState::Ready && heat_tone(t, limit) != Tone::Normal => {
                heat_tone(t, limit)
            }
            _ => tone,
        };
        let action = MenuAction::Ship(ShipCommand::Rcs(b.id.clone(), !b.on));
        s.toggle(b.id.to_uppercase(), b.on, format!("{word} · {size}{heat}"), tone, action);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::menus::find;
    use glam::DVec3;
    use sim::systems::Reading;

    fn block(id: &str, on: bool, state: BlockState) -> Block {
        Block { id: id.into(), nozzles: 4, thrust: 400.0, on, state, kelvin: None }
    }

    fn data() -> RcsData {
        let mut a = Authority { rotate: DVec3::new(0.02, 0.02, 0.0), ..Default::default() };
        a.translate[0] = (0.05, 0.0);
        a.translate[4] = (0.08, 0.4);
        RcsData {
            mode: RcsMode { on: true, ..Default::default() },
            authority: a,
            feeds: vec![Feed { name: "s2.n2".into(), kg: Some(Reading { value: 3.0, precision: 0.1 }), full: 50.0 }],
            blocks: vec![
                block("s2.rcs#1", true, BlockState::Ready),
                block("s1.coldgas#1", false, BlockState::Ready),
                block("dragon.draco#1", true, BlockState::Dry),
                block("dragon.draco#2", true, BlockState::Out),
            ],
        }
    }

    /// Each block is a switch whose click flips it; its state reads in
    /// its colour, a disabled one dim and said so.
    #[test]
    fn every_block_is_a_switch() {
        let s = sections(&data());
        let b = find(&s, "blocks").expect("drawn");
        assert_eq!(b.title, "THRUSTERS · 4");
        // (label, value starts with, tone, on)
        let table = [
            ("S2.RCS#1", "on · 4×400 N", Tone::Good, true),
            ("S1.COLDGAS#1", "disabled", Tone::Dim, false),
            ("DRAGON.DRACO#1", "dry", Tone::Warn, true),
            ("DRAGON.DRACO#2", "failed", Tone::Alert, true),
        ];
        for (label, value, tone, on) in table {
            assert!(b.value(label).is_some_and(|v| v.starts_with(value)), "{label}: {:?}", b.value(label));
            assert_eq!(b.tone(label), Some(tone), "{label}");
            let flip = b.items.iter().find_map(|i| match i {
                Item::Toggle { label: l, on: o, action, .. } if l == label => Some((*o, action.clone())),
                _ => None,
            });
            let id = label.to_lowercase();
            assert_eq!(flip, Some((on, MenuAction::Ship(ShipCommand::Rcs(id, !on)))), "{label}");
        }
        let summary = find(&s, crate::interface::menus::SUMMARY).unwrap();
        assert_eq!(summary.value("BLOCKS ON"), Some("3 of 4"));
    }

    /// A translation whose thrust misses the centre of mass is amber and
    /// explained; a direction without thrusters reads none.
    #[test]
    fn an_imbalanced_translation_is_warned() {
        let s = sections(&data());
        let a = find(&s, "authority").unwrap();
        assert_eq!(a.value("RIGHT (L)"), Some("0.050 m/s²"));
        assert_eq!(a.tone("RIGHT (L)"), Some(Tone::Normal));
        assert_eq!(a.tone("FORWARD (H)"), Some(Tone::Warn));
        assert_eq!(a.value("LEFT (J)"), Some("none"));
        assert_eq!(a.value("ROLL"), Some("none"));
        let warned = a.items.iter().any(|i| matches!(i, Item::Note(t, Tone::Warn) if t.contains("forward (0.40 m)")));
        assert!(warned, "{:?}", a.items);
    }

    /// The mode's choice and the fine switch keep the rest of the mode.
    #[test]
    fn the_mode_controls_set_the_mode() {
        let mut r = data();
        r.mode.only = RcsOnly::Translate;
        let s = sections(&r);
        let summary = &s[0];
        let Some(Item::Choice { current, options, .. }) = summary.items.first() else { panic!("{summary:?}") };
        assert_eq!(current, "translate only");
        let rotate = RcsMode { on: true, only: RcsOnly::Rotate, fine: false };
        assert!(options.iter().any(|o| o.2 == MenuAction::SetRcs(rotate) && !o.1));
        let fine = RcsMode { on: true, only: RcsOnly::Translate, fine: true };
        let switch = summary
            .items
            .iter()
            .any(|i| matches!(i, Item::Toggle { action, on: false, .. } if *action == MenuAction::SetRcs(fine)));
        assert!(switch, "{:?}", summary.items);
    }

    /// Propellant amber under a tenth, red when empty, a dash unmeasured.
    #[test]
    fn feeds_read_their_level() {
        let feed = |kg: Option<f64>| Feed {
            name: "n2".into(),
            kg: kg.map(|value| Reading { value, precision: 0.12 }),
            full: 50.0,
        };
        // (feed, text, tone): to the transducer's precision, `amount`'s rule.
        let table = [
            (feed(Some(30.4)), "30/50 kg", Tone::Normal),
            (feed(Some(3.0)), "3/50 kg", Tone::Warn),
            (feed(Some(0.0)), "0/50 kg", Tone::Alert),
            (feed(None), "—/50 kg", Tone::Normal),
        ];
        for (f, text, tone) in table {
            let r = RcsData { feeds: vec![f.clone()], ..Default::default() };
            let s = sections(&r);
            let feeds = find(&s, "feeds").unwrap();
            assert_eq!((feeds.value("N2"), feeds.tone("N2")), (Some(text), Some(tone)), "{f:?}");
        }
    }

    /// A craft without thruster blocks: said so, no authority per
    /// direction, no block count.
    #[test]
    fn a_craft_without_thrusters_says_so() {
        let s = sections(&RcsData::default());
        assert!(find(&s, "blocks").unwrap().items.is_empty());
        let authority = find(&s, "authority").unwrap();
        assert!(authority.items.iter().any(|i| matches!(i, Item::Note(t, _) if t == "no thruster blocks")));
        assert!(authority.rows().is_empty(), "{:?}", authority.rows());
        assert!(s[0].value("BLOCKS ON").is_none());
    }
}
