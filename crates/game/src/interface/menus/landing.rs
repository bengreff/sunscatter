//! The Landing/Reentry menu (key 2): the predicted impact and the surface
//! motion, the braking burn, the entry, the parts nearest their heat
//! limits, the parachute and the attitude. While it is open the impact marker is drawn
//! (`landing::impact_marker_shown`).

use super::data::{Brake, ChuteData, FlightData};
use super::{Section, Tone};
use crate::format::{distance as fmt_dist, duration as fmt_duration, Known, DASH};
use crate::hud::temperature;
use sim::systems::Quantity::{Acceleration, Altitude, Attitude, Position, Velocity};

/// A signed speed as the craft knows it (`+12.3 m/s`).
fn signed(k: &Known, v: f64) -> String {
    let text = k.speed(Velocity, v);
    if text == DASH || v < 0.0 {
        text
    } else {
        format!("+{text}")
    }
}

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(d), braking(d), reentry(d), thermal(d), parachute(d), attitude(d)]
}

/// The impact (`sim::landing`'s prediction), then the motion over the
/// ground now.
fn summary(d: &FlightData) -> Section {
    let mut s = Section::summary();
    match &d.impact {
        Some(h) => {
            let k = &d.known;
            let tone = if h.in_s < 60.0 { Tone::Warn } else { Tone::Normal };
            s.toned("IMPACT IN", k.or_dash(k.orbit(), || fmt_duration(h.in_s)), tone);
            s.row("IMPACT V/S", signed(k, h.v_vertical));
            s.row("IMPACT H. SPEED", k.speed(Velocity, h.v_horizontal));
            s.row("AT", k.or_dash(k.orbit(), || format!("{:+.3}° {:+.3}°", h.lat, h.lon)));
        }
        None if d.landed => {
            s.note("on the ground", Tone::Dim);
        }
        None => {
            s.note("no impact predicted", Tone::Dim);
        }
    }
    if let Some(m) = d.surface.filter(|_| !d.landed) {
        s.row("RADAR ALT", d.known.distance(Altitude, m.radar));
        s.row("V/S", signed(&d.known, m.v_vertical));
        s.row("H. SPEED", d.known.speed(Velocity, m.v_horizontal));
    }
    s
}

/// With a predicted impact: the full-thrust stopping burn.
fn braking(d: &FlightData) -> Section {
    let mut s = Section::new("braking", "BRAKING");
    let (Some(_), Some(brake)) = (&d.impact, d.braking) else { return s };
    // The burn needs the orbit and the mass (the propellant aboard).
    let k = &d.known;
    let known = k.orbit() && k.propellant;
    if let Some(m) = d.surface {
        let tone = if m.twr.value < 1.0 { Tone::Warn } else { Tone::Normal };
        s.toned("LOCAL TWR", crate::format::ratio(Some(m.twr)), tone);
    }
    match brake {
        Brake::Burn { in_s, ignite_height } => {
            let tone = if in_s < 10.0 { Tone::Warn } else { Tone::Normal };
            s.toned("BURN IN", k.or_dash(known, || crate::landing::burn_in(in_s)), tone);
            s.row("IGNITE AT", k.or_dash(known, || fmt_dist(ignite_height)));
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
            let k = &d.known;
            let moving = k.knows(Velocity);
            s.row("DYN. PRESSURE", k.or_dash(moving, || format!("{:.2} kPa", air.q / 1e3)));
            s.row("MACH", k.or_dash(moving, || format!("{:.2}", air.mach)));
            s.row("G-LOAD", k.or_dash(k.knows(Acceleration), || format!("{:.2} g", d.g_load)));
            if let Some((aoa, slip)) = air.aoa_slip {
                let known = moving && k.knows(Attitude);
                s.row("AoA", k.or_dash(known, || format!("{aoa:+.1}°")));
                s.row("SIDESLIP", k.or_dash(known, || format!("{slip:+.1}°")));
            }
        }
        (None, Some(e)) => {
            let k = &d.known;
            s.row("ENTRY INTERFACE", fmt_dist(e.top));
            if let Some(t) = e.in_s {
                s.row("  IN", k.or_dash(k.orbit(), || fmt_duration(t)));
            }
            if let Some(pe) = e.periapsis {
                s.row("PERIAPSIS", k.distance(Position, pe));
            }
        }
        (None, None) => return s,
    }
    let (t, limit) = d.craft.skin;
    let k = &d.known;
    s.toned("SKIN MAX", k.or_dash(k.temperatures, || crate::hud::temperature(t, limit)), heat_tone(t, limit));
    for (label, value) in shock_rows(d.craft.shock.as_ref(), d.craft.peak_heating) {
        s.row(label, k.or_dash(k.temperatures, || value));
    }
    s
}

/// The shock layer's rows (D109): its temperature where the gas has an
/// equilibrium model, the stagnation heating now, the peak so far.
pub fn shock_rows(shock: Option<&sim::vessel::ShockLayer>, peak: f64) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    if let Some(s) = shock {
        if s.temperature > 0.0 {
            rows.push(("SHOCK LAYER", format!("{:.0} K", s.temperature)));
        }
        rows.push(("HEATING", format!("{:.1} W/cm²", (s.convective + s.radiative) / 1e4)));
    }
    if peak > 0.0 {
        rows.push(("  PEAK", format!("{:.1} W/cm²", peak / 1e4)));
    }
    rows
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

/// The measured parts nearest their limits (each by its skin cell or
/// interior node nearest its own), and how many nothing measures: the
/// systems screen's rule (`menus::hottest`, interface-map.md finding 13).
fn thermal(d: &FlightData) -> Section {
    let mut s = Section::new("thermal", "THERMAL");
    let h = &d.craft.hottest;
    for (name, t, limit) in &h.parts {
        s.toned(name.to_uppercase(), temperature(*t, *limit), heat_tone(*t, *limit));
    }
    if h.unmeasured > 0 {
        s.toned("NOT MEASURED", format!("{} parts", h.unmeasured), Tone::Dim);
    }
    s
}

/// How a part's state reads in a menu.
fn part_tone(t: crate::staging::PartTone) -> Tone {
    match t {
        crate::staging::PartTone::Idle => Tone::Dim,
        crate::staging::PartTone::Working => Tone::Good,
        crate::staging::PartTone::Failed => Tone::Alert,
    }
}

/// The craft's canopies in their sequence, and before the command the q
/// against the design opening q.
fn parachute(d: &FlightData) -> Section {
    let mut s = Section::new("parachute", "PARACHUTE");
    let ChuteData { commanded, q, design_q, .. } = d.chute;
    for p in &d.canopies {
        s.toned(p.name.clone(), &p.state, part_tone(p.tone));
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
        let k = &d.known;
        let known = k.knows(Attitude);
        s.row("HEADING", k.or_dash(known, || format!("{heading:05.1}°")));
        s.row("PITCH", k.or_dash(known, || format!("{pitch:+.1}°")));
        s.row("ROLL", k.or_dash(known, || format!("{roll:+.1}°")));
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
            surface: Some(Surface {
                radar: 2_850.0,
                v_vertical: -152.3,
                v_horizontal: 40.0,
                twr: sim::systems::Reading { value: 1.4, precision: 0.0 },
            }),
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
    fn the_shock_layer_rows() {
        // (shock, peak) → rows.
        assert!(shock_rows(None, 0.0).is_empty());
        assert_eq!(shock_rows(None, 2.5e6), vec![("  PEAK", "250.0 W/cm²".to_string())]);
        let s =
            sim::vessel::ShockLayer { temperature: 10_571.4, convective: 1e6, radiative: 2e5, ..Default::default() };
        let rows = shock_rows(Some(&s), 1.5e6);
        assert_eq!(rows[0], ("SHOCK LAYER", "10571 K".to_string()));
        assert_eq!(rows[1], ("HEATING", "120.0 W/cm²".to_string()));
        // CO₂ has no equilibrium model: no temperature, the heating still.
        let co2 = sim::vessel::ShockLayer { temperature: 0.0, ..s };
        assert_eq!(shock_rows(Some(&co2), 0.0).len(), 1);
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
        let spec = sim::craft::small::rocket_ref().spec.chute.clone();
        let chute = sim::chute::ChuteState::default();
        let mut d = FlightData { canopies: crate::staging::canopies(&spec, &chute), ..FlightData::default() };
        d.chute = ChuteData { commanded: false, state: chute, q: Some(9_000.0), design_q: 5_500.0 };
        let s = shown(&d);
        let p = find(&s, "parachute").unwrap();
        assert_eq!(p.rows(), [("DROGUES", "stowed"), ("MAINS", "stowed"), ("q / DESIGN q", "9.0 / 5.5 kPa")]);
        assert!(p.items.contains(&Item::Note("too fast: the canopy may tear".into(), Tone::Warn)));
        d.chute.commanded = true;
        let s = shown(&d);
        assert!(!find(&s, "parachute").unwrap().items.iter().any(|i| matches!(i, Item::Note(..))), "armed: no advice");
    }

    /// What the craft cannot measure reads as a dash (D106); what it can,
    /// to the digits its precision gives.
    #[test]
    fn readouts_follow_what_the_craft_knows() {
        let mut k = sim::systems::Knowledge::default();
        let blind = FlightData { known: Known::from(k.clone()), attitude: Some([1.0, 2.0, 3.0]), ..falling() };
        let s = shown(&blind);
        let summary = find(&s, "summary").unwrap();
        assert!(summary.rows().iter().all(|(_, v)| *v == DASH), "{:?}", summary.rows());
        assert!(find(&s, "attitude").unwrap().rows().iter().all(|(_, v)| *v == DASH));
        assert_eq!(find(&s, "braking").unwrap().value("BURN IN"), Some(DASH));
        // A GNSS fix (3 m, 0.1 m/s): the impact and the motion, not the attitude.
        for (q, p) in sim::systems::data::provides("position", 3.0) {
            k.learn(q, p);
        }
        let fixed = FlightData { known: Known::from(k), ..blind };
        let s = shown(&fixed);
        let summary = find(&s, "summary").unwrap();
        assert_eq!(summary.value("IMPACT V/S"), Some("-160.0 m/s"));
        assert_eq!(summary.value("RADAR ALT"), Some("2850 m"), "altitude to 4.5 m");
        assert!(find(&s, "attitude").unwrap().rows().iter().all(|(_, v)| *v == DASH));
        assert_eq!(find(&s, "braking").unwrap().value("BURN IN"), Some(DASH), "the propellant is not known");
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

    /// The measured parts against their limits (amber near one), then how
    /// many nothing measures.
    #[test]
    fn thermal_lists_the_measured_parts_and_counts_the_rest() {
        use super::super::hottest::Hottest;
        let mut d = FlightData::default();
        assert!(thermal(&d).items.is_empty());
        let parts: Vec<(String, f64, f64)> =
            vec![("engine bell".into(), 1_500.0, 1_640.0), ("body".into(), 310.0, 1_100.0)];
        // (how many of `parts` listed, not measured, rows)
        type Rows = &'static [(&'static str, &'static str)];
        let table: [(usize, usize, Rows); 3] = [
            (2, 0, &[("ENGINE BELL", "1500 K / 1640 K !"), ("BODY", "310 K / 1100 K")]),
            (2, 3, &[("ENGINE BELL", "1500 K / 1640 K !"), ("BODY", "310 K / 1100 K"), ("NOT MEASURED", "3 parts")]),
            (0, 12, &[("NOT MEASURED", "12 parts")]),
        ];
        for (n, unmeasured, rows) in table {
            d.craft.hottest = Hottest { parts: parts[..n].to_vec(), unmeasured };
            let t = thermal(&d);
            assert_eq!(t.rows(), rows, "{unmeasured}");
        }
        assert_eq!(thermal(&d).tone("NOT MEASURED"), Some(Tone::Dim));
    }
}
