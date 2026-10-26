//! The Landing/Reentry menu (key L, left dock): the predicted impact and
//! the surface motion, the braking burn, the entry, the parachute and the
//! attitude. While it is open the impact marker is drawn
//! (`landing::impact_marker_shown`).

use super::data::{Brake, ChuteData, FlightData};
use super::{Section, Tone};
use crate::format::{distance as fmt_dist, duration as fmt_duration};

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(d), braking(d), reentry(d), parachute(d), attitude(d)]
}

/// The impact (`sim::landing`'s prediction), then the motion over the
/// ground now.
fn summary(d: &FlightData) -> Section {
    let mut s = Section::summary();
    match &d.impact {
        Some(h) => {
            let tone = if h.in_s < 60.0 { Tone::Warn } else { Tone::Normal };
            s.toned("IMPACT IN", fmt_duration(h.in_s), tone);
            s.row("IMPACT V/S", format!("{:+.1} m/s", h.v_vertical));
            s.row("IMPACT H. SPEED", format!("{:.1} m/s", h.v_horizontal));
            s.row("AT", format!("{:+.3}° {:+.3}°", h.lat, h.lon));
        }
        None if d.landed => {
            s.note("on the ground", Tone::Dim);
        }
        None => {
            s.note("no impact predicted", Tone::Dim);
        }
    }
    if let Some(m) = d.surface.filter(|_| !d.landed) {
        s.row("RADAR ALT", fmt_dist(m.radar));
        s.row("V/S", format!("{:+.1} m/s", m.v_vertical));
        s.row("H. SPEED", format!("{:.1} m/s", m.v_horizontal));
    }
    s
}

/// With a predicted impact: the full-thrust stopping burn.
fn braking(d: &FlightData) -> Section {
    let mut s = Section::new("braking", "BRAKING");
    let (Some(_), Some(brake)) = (&d.impact, d.braking) else { return s };
    if let Some(m) = d.surface {
        s.toned("LOCAL TWR", format!("{:.2}", m.twr), if m.twr < 1.0 { Tone::Warn } else { Tone::Normal });
    }
    match brake {
        Brake::Burn { in_s, ignite_height } => {
            let tone = if in_s < 10.0 { Tone::Warn } else { Tone::Normal };
            s.toned("BURN IN", crate::landing::burn_in(in_s), tone);
            s.row("IGNITE AT", fmt_dist(ignite_height));
        }
        Brake::CannotStop => {
            s.toned("BRAKE", "cannot stop", Tone::Alert);
        }
    }
    s
}

/// In an atmosphere, or with the next periapsis inside one.
fn reentry(d: &FlightData) -> Section {
    let mut s = Section::new("reentry", "REENTRY");
    match (&d.air, d.entry) {
        (Some(air), _) => {
            s.row("DYN. PRESSURE", format!("{:.2} kPa", air.q / 1e3));
            s.row("MACH", format!("{:.2}", air.mach));
            s.row("G-LOAD", format!("{:.2} g", d.g_load));
            if let Some((aoa, slip)) = air.aoa_slip {
                s.row("AoA", format!("{aoa:+.1}°"));
                s.row("SIDESLIP", format!("{slip:+.1}°"));
            }
        }
        (None, Some(e)) => {
            s.row("ENTRY INTERFACE", fmt_dist(e.top));
            if let Some(t) = e.in_s {
                s.row("  IN", fmt_duration(t));
            }
            if let Some(pe) = e.periapsis {
                s.row("PERIAPSIS", fmt_dist(pe));
            }
        }
        (None, None) => return s,
    }
    let (t, limit) = d.craft.skin;
    s.toned("SKIN MAX", crate::hud::temperature(t, limit), heat_tone(t, limit));
    s
}

/// How a temperature against its limit reads.
pub fn heat_tone(t: f64, limit: f64) -> Tone {
    if limit <= 0.0 {
        Tone::Normal
    } else if t >= limit {
        Tone::Alert
    } else if t >= 0.9 * limit {
        Tone::Warn
    } else {
        Tone::Normal
    }
}

/// Whether opening now is within the design opening q (D079), before the
/// command; nothing refuses an opening above it, the canopy may tear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opening {
    /// Not in an atmosphere: a drogue opened here meets the full entry q.
    NoAir,
    Safe,
    TooFast,
}

pub fn opening(q: Option<f64>, design_q: f64) -> Opening {
    match q {
        None => Opening::NoAir,
        Some(q) if q <= design_q => Opening::Safe,
        Some(_) => Opening::TooFast,
    }
}

/// The canopies in their sequence (the Staging menu's parts), and before
/// the command the q against the design opening q.
fn parachute(d: &FlightData) -> Section {
    let mut s = Section::new("parachute", "PARACHUTE");
    let ChuteData { commanded, q, design_q, .. } = d.chute;
    let parts = d.stages.iter().find(|st| st.number == 0).map(|st| st.parts.as_slice()).unwrap_or_default();
    for p in parts {
        s.toned(p.name, &p.state, super::staging::part_tone(p.tone));
    }
    let q_text = q.map_or("—".to_string(), |q| format!("{:.1}", q / 1e3));
    s.row("q / DESIGN q", format!("{q_text} / {:.1} kPa", design_q / 1e3));
    if !commanded && !d.landed {
        match opening(q, design_q) {
            Opening::NoAir => s.note("no air: deploy after entry (P or Space)", Tone::Dim),
            Opening::Safe => s.note("safe to deploy (P or Space)", Tone::Good),
            Opening::TooFast => s.note("too fast: the canopy may tear", Tone::Warn),
        };
    }
    s
}

fn attitude(d: &FlightData) -> Section {
    let mut s = Section::new("attitude", "ATTITUDE");
    if let Some([heading, pitch, roll]) = d.attitude {
        s.row("HEADING", format!("{heading:05.1}°"));
        s.row("PITCH", format!("{pitch:+.1}°"));
        s.row("ROLL", format!("{roll:+.1}°"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::data::{AirData, Entry, ImpactData, Surface};
    use super::super::{find, Item};
    use super::*;

    fn falling() -> FlightData {
        FlightData {
            body: "Earth".into(),
            surface: Some(Surface { radar: 2_850.0, v_vertical: -152.3, v_horizontal: 40.0, twr: 1.4 }),
            impact: Some(ImpactData {
                body: "Earth".into(),
                in_s: 22.0,
                v_vertical: -160.0,
                v_horizontal: 38.0,
                lat: 28.6,
                lon: -80.7,
            }),
            braking: Some(Brake::Burn { in_s: 8.0, ignite_height: 1_200.0 }),
            ..FlightData::default()
        }
    }

    /// As drawn (empty sections dropped).
    fn shown(d: &FlightData) -> Vec<Section> {
        super::super::MenuId::Landing.sections(d)
    }

    fn ids(d: &FlightData) -> Vec<&'static str> {
        shown(d).iter().map(|s| s.id).collect()
    }

    #[test]
    fn with_an_impact_the_summary_and_braking_show_it() {
        let d = falling();
        let s = shown(&d);
        assert_eq!(
            find(&s, "summary").unwrap().rows(),
            [
                ("IMPACT IN", "22s"),
                ("IMPACT V/S", "-160.0 m/s"),
                ("IMPACT H. SPEED", "38.0 m/s"),
                ("AT", "+28.600° -80.700°"),
                ("RADAR ALT", "2850 m"),
                ("V/S", "-152.3 m/s"),
                ("H. SPEED", "40.0 m/s")
            ]
        );
        let b = find(&s, "braking").unwrap();
        assert_eq!(b.rows(), [("LOCAL TWR", "1.40"), ("BURN IN", "8s"), ("IGNITE AT", "1200 m")]);
        assert_eq!(b.tone("BURN IN"), Some(Tone::Warn));
        let stop = FlightData { braking: Some(Brake::CannotStop), ..d };
        assert_eq!(find(&shown(&stop), "braking").unwrap().tone("BRAKE"), Some(Tone::Alert));
    }

    #[test]
    fn without_an_impact_there_is_no_braking_and_the_summary_says_so() {
        let orbit = FlightData::default();
        let s = shown(&orbit);
        assert!(find(&s, "summary").unwrap().items.contains(&Item::Note("no impact predicted".into(), Tone::Dim)));
        assert!(!ids(&orbit).contains(&"braking"));
        let landed = FlightData { landed: true, surface: falling().surface, ..FlightData::default() };
        let summary = shown(&landed).into_iter().next().unwrap();
        assert_eq!(summary.items, [Item::Note("on the ground".into(), Tone::Dim)], "no motion rows on the ground");
        // A braking solution without an impact (stale) is not shown.
        let stale = FlightData { impact: None, ..falling() };
        assert!(!ids(&stale).contains(&"braking"));
    }

    #[test]
    fn reentry_shows_in_the_air_or_with_the_periapsis_inside_it() {
        let mut d = FlightData::default();
        d.craft.skin = (1_000.0, 1_100.0);
        assert!(!ids(&d).contains(&"reentry"), "in orbit above the air");
        d.entry = Some(Entry { top: 100_000.0, in_s: None, periapsis: Some(45_000.0) });
        let s = shown(&d);
        let r = find(&s, "reentry").unwrap();
        assert_eq!(r.rows()[..2], [("ENTRY INTERFACE", "100.0 km"), ("PERIAPSIS", "45.0 km")]);
        // A deorbit ending on the ground: no periapsis, the marked entry.
        d.entry = Some(Entry { top: 100_000.0, in_s: Some(312.0), periapsis: None });
        let s = shown(&d);
        let r = find(&s, "reentry").unwrap();
        assert_eq!(r.rows()[..2], [("ENTRY INTERFACE", "100.0 km"), ("  IN", "5m 12s")]);
        assert_eq!(r.tone("SKIN MAX"), Some(Tone::Warn), "within 10 % of the limit");
        d.air = Some(AirData { q: 12_340.0, mach: 6.2, aoa_slip: Some((3.0, -0.4)) });
        d.g_load = 2.5;
        let s = shown(&d);
        let r = find(&s, "reentry").unwrap();
        assert_eq!(
            r.rows()[..5],
            [
                ("DYN. PRESSURE", "12.34 kPa"),
                ("MACH", "6.20"),
                ("G-LOAD", "2.50 g"),
                ("AoA", "+3.0°"),
                ("SIDESLIP", "-0.4°")
            ]
        );
        assert_eq!(heat_tone(1_200.0, 1_100.0), Tone::Alert);
        assert_eq!(heat_tone(500.0, 1_100.0), Tone::Normal);
        assert_eq!(heat_tone(0.0, 0.0), Tone::Normal, "no limit known yet");
    }

    #[test]
    fn the_parachute_says_whether_it_is_safe_to_deploy() {
        // (q Pa, design q Pa, expected)
        let table = [
            (None, 5_500.0, Opening::NoAir),
            (Some(3_210.0), 5_500.0, Opening::Safe),
            (Some(5_500.0), 5_500.0, Opening::Safe),
            (Some(9_000.0), 5_500.0, Opening::TooFast),
        ];
        for (q, design, want) in table {
            assert_eq!(opening(q, design), want, "{q:?}");
        }
        let spec = sim::craft::test_craft().params().chute;
        let controls = sim::vessel::Controls::default();
        let chute = sim::chute::ChuteState::default();
        let inputs = crate::staging::StageInputs {
            controls: &controls,
            chute: &chute,
            spec: &spec,
            engine_running: false,
            fuelled: true,
        };
        let mut d = FlightData { stages: crate::staging::stack(&inputs), ..FlightData::default() };
        d.chute = ChuteData { commanded: false, state: chute, q: Some(9_000.0), design_q: 5_500.0 };
        let s = shown(&d);
        let p = find(&s, "parachute").unwrap();
        assert_eq!(p.rows(), [("DROGUES", "stowed"), ("MAINS", "stowed"), ("q / DESIGN q", "9.0 / 5.5 kPa")]);
        assert!(p.items.contains(&Item::Note("too fast: the canopy may tear".into(), Tone::Warn)));
        d.chute.commanded = true;
        let s = shown(&d);
        assert!(!find(&s, "parachute").unwrap().items.iter().any(|i| matches!(i, Item::Note(..))), "armed: no advice");
    }

    #[test]
    fn attitude_is_heading_pitch_and_roll() {
        let d = FlightData { attitude: Some([7.31, 89.9, -3.0]), ..FlightData::default() };
        let s = shown(&d);
        assert_eq!(
            find(&s, "attitude").unwrap().rows(),
            [("HEADING", "007.3°"), ("PITCH", "+89.9°"), ("ROLL", "-3.0°")]
        );
    }
}
