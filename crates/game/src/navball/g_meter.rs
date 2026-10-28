//! The navball's G row (D111): the proper acceleration in g, the way it
//! pushes the crew (towards the nose, the tail, a side, the top or the
//! bottom), and for a craft of parts accelerating the limit and what sets
//! it (`sim::systems::g_limit`): amber from 80 % of the limit, red at it
//! (`format::limit_level`, the one rule for every value against a limit).
//! Pure (rule 8), table-tested below.

use super::rules::G0;
use glam::DVec3;
use sim::systems::g_limit::{GLimit, Limiter};

/// Where the proper acceleration points, in body axes (+Z the nose, +X
/// the right side, −Y the top).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Nose,
    Tail,
    Right,
    Left,
    Top,
    Bottom,
}

impl Toward {
    /// The body axis `a` (body axes) is mostly along; `None` for none.
    pub fn of(a: DVec3) -> Option<Toward> {
        let m = a.abs();
        if m.max_element() <= 0.0 {
            return None;
        }
        Some(if m.z >= m.x && m.z >= m.y {
            if a.z > 0.0 {
                Toward::Nose
            } else {
                Toward::Tail
            }
        } else if m.x >= m.y {
            if a.x > 0.0 {
                Toward::Right
            } else {
                Toward::Left
            }
        } else if a.y > 0.0 {
            Toward::Bottom
        } else {
            Toward::Top
        })
    }

    /// As shown after the value.
    pub fn symbol(self) -> &'static str {
        match self {
            Toward::Nose => "▲",
            Toward::Tail => "▼",
            Toward::Right => "▶",
            Toward::Left => "◀",
            Toward::Top => "top",
            Toward::Bottom => "bottom",
        }
    }

    /// In words, for the hover.
    pub fn words(self) -> &'static str {
        match self {
            Toward::Nose => "towards the nose",
            Toward::Tail => "towards the tail",
            Toward::Right => "towards the right side",
            Toward::Left => "towards the left side",
            Toward::Top => "towards the top",
            Toward::Bottom => "towards the bottom",
        }
    }
}

/// The G row: the load (g), where it points, and the limit (g) with what
/// sets it.
#[derive(Clone, Debug, PartialEq)]
pub struct GReadout {
    pub load: f64,
    pub toward: Option<Toward>,
    pub limit: Option<(f64, String)>,
}

/// The readout of proper acceleration `a_body` (m/s², body axes) and the
/// craft's limit, if it has one (`joint`: a joint's name by index).
pub fn readout(a_body: DVec3, limit: Option<GLimit>, joint: impl Fn(usize) -> Option<String>) -> GReadout {
    GReadout {
        load: a_body.length() / G0,
        toward: Toward::of(a_body),
        limit: limit.map(|l| (l.accel / G0, limiter(l.by, joint))),
    }
}

/// What sets a limit, in words: the joint's name, or the crew along an
/// axis (NASA-STD-3001's limits, `sim::systems::crew::AXES`).
pub fn limiter(by: Limiter, joint: impl Fn(usize) -> Option<String>) -> String {
    match by {
        Limiter::Joint(j) => joint(j).unwrap_or_else(|| format!("joint {j}")),
        Limiter::Crew(0) => "crew, eyeballs in (+Ax)".into(),
        Limiter::Crew(1) => "crew, eyeballs out (−Ax)".into(),
        Limiter::Crew(_) => "crew, sideways".into(),
    }
}

/// The row's value: `1.02 / 7.5 g ▲`, or `0.00 g` with nothing to show.
pub fn text(r: &GReadout) -> String {
    let load = match &r.limit {
        Some((l, _)) => format!("{:.2} / {} g", r.load, limit_digits(*l)),
        None => format!("{:.2} g", r.load),
    };
    match r.toward {
        Some(t) if r.load >= 0.005 => format!("{load} {}", t.symbol()),
        _ => load,
    }
}

/// A limit with two significant digits (7.5, 12, 0.85).
fn limit_digits(g: f64) -> String {
    if g >= 10.0 {
        format!("{g:.0}")
    } else if g >= 1.0 {
        format!("{g:.1}")
    } else {
        format!("{g:.2}")
    }
}

/// The hover: the load's direction and what sets the limit.
pub fn hover(r: &GReadout) -> String {
    let mut s = "Proper acceleration (what the crew and structure feel)".to_string();
    if let Some(t) = r.toward.filter(|_| r.load >= 0.005) {
        s += &format!(", {}", t.words());
    }
    match &r.limit {
        Some((l, by)) => s += &format!(".\nLimit {:.1} g along it, set by {by}: amber from 80 %, red at it.", l),
        None => s += ".",
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The direction (table): the largest body axis, its sign.
    #[test]
    fn direction_in_body_axes() {
        // (body-axis acceleration, direction)
        let table = [
            (DVec3::new(0.0, 0.0, 9.8), Some(Toward::Nose)),
            (DVec3::new(1.0, 2.0, -30.0), Some(Toward::Tail)),
            (DVec3::new(5.0, 1.0, 1.0), Some(Toward::Right)),
            (DVec3::new(-5.0, 1.0, 1.0), Some(Toward::Left)),
            (DVec3::new(0.0, -3.0, 1.0), Some(Toward::Top)),
            (DVec3::new(0.0, 3.0, 1.0), Some(Toward::Bottom)),
            (DVec3::ZERO, None),
        ];
        for (a, want) in table {
            assert_eq!(Toward::of(a), want, "{a}");
        }
    }

    /// The text and the limiter's name: a joint by name, the crew by axis.
    #[test]
    fn the_row_and_what_sets_the_limit() {
        let joint = |j: usize| (j == 2).then(|| "s1.tank to octaweb".to_string());
        let pad = readout(DVec3::new(0.0, 0.0, G0 * 1.02), None, joint);
        assert_eq!(text(&pad), "1.02 g ▲");
        let crew = GLimit { accel: 7.5 * G0, by: Limiter::Crew(0) };
        let r = readout(DVec3::new(0.0, 0.0, G0 * 3.0), Some(crew), joint);
        assert_eq!(text(&r), "3.00 / 7.5 g ▲");
        assert_eq!(r.limit.as_ref().unwrap().1, "crew, eyeballs in (+Ax)");
        assert!(hover(&r).contains("crew, eyeballs in"));
        let joint_limit = GLimit { accel: 5.1 * G0, by: Limiter::Joint(2) };
        assert_eq!(readout(DVec3::Z, Some(joint_limit), joint).limit.unwrap().1, "s1.tank to octaweb");
        assert_eq!(limiter(Limiter::Joint(7), joint), "joint 7");
        assert_eq!(limiter(Limiter::Crew(2), joint), "crew, sideways");
        let big = GLimit { accel: 12.4 * G0, by: Limiter::Crew(1) };
        assert_eq!(text(&readout(DVec3::ZERO, Some(big), joint)), "0.00 / 12 g");
    }
}
