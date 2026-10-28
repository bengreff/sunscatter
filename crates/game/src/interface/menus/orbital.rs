//! The Orbital menu (key 1): the orbit in full, the target and the ship's
//! clock. The planned burns are the Maneuver menu's.

use super::data::{Elements, FlightData, TargetData};
use super::{MenuAction, Section, Tone};
use crate::format::{duration as fmt_duration, Known};
use crate::hud::clock_offset;
use crate::navball::ApsisReadout;
use sim::systems::Quantity::{Position, Velocity};

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(d), elements(d), target(&d.target, &d.known), clock(d)]
}

/// Ap and Pe from the predicted trajectory (D076), then the osculating
/// period, inclination and eccentricity.
fn summary(d: &FlightData) -> Section {
    let mut s = Section::summary();
    if d.landed {
        s.note(format!("on {}: no orbit", d.body), Tone::Dim);
        return s;
    }
    let k = &d.known;
    apsis(&mut s, "APOAPSIS", d.ap.as_ref(), k);
    apsis(&mut s, "PERIAPSIS", d.pe.as_ref(), k);
    if let Some(el) = &d.elements {
        if let Some(p) = el.period {
            s.row("PERIOD", k.or_dash(k.orbit(), || fmt_duration(p)));
        }
        s.row("INCLINATION", k.or_dash(k.orbit(), || format!("{:.2}°", el.inclination)));
        s.row("ECCENTRICITY", k.or_dash(k.orbit(), || format!("{:.5}", el.e)));
    }
    if d.ap.is_none() && d.elements.is_some_and(|e| e.e >= 1.0) {
        s.note(format!("escaping {}", d.body), Tone::Dim);
    }
    s
}

fn apsis(s: &mut Section, label: &str, a: Option<&ApsisReadout>, k: &Known) {
    if let Some(a) = a {
        let altitude = k.distance(Position, a.above_datum);
        let value = match &a.other_body {
            Some(b) => format!("{altitude} · {b}"),
            None => altitude,
        };
        s.row(label, value);
        s.row("  IN", k.or_dash(k.orbit(), || fmt_duration(a.in_s)));
    }
}

fn elements(d: &FlightData) -> Section {
    let mut s = Section::new("elements", "ELEMENTS · OSCULATING");
    let Some(Elements { a, lan, argp, true_anomaly, speed, .. }) = d.elements else { return s };
    let k = &d.known;
    let angle = |x: f64| k.or_dash(k.orbit(), || format!("{x:.2}°"));
    s.row("SEMI-MAJOR AXIS", k.distance(Position, a));
    s.row("ASC. NODE", angle(lan));
    s.row("ARG. PERIAPSIS", angle(argp));
    s.row("TRUE ANOMALY", angle(true_anomaly));
    s.row("ORBITAL SPEED", k.speed(Velocity, speed));
    s.note(format!("two-body, about {}'s equator; Ap and Pe above are the predicted path's", d.body), Tone::Dim);
    s
}

/// The target picker (always), then what is known about the target.
fn target(t: &TargetData, k: &Known) -> Section {
    let title = t.info.as_ref().map_or("TARGET".to_string(), |i| format!("TARGET · {}", i.name));
    let mut s = Section::new("target", title);
    let current = t.choices.iter().find(|c| c.1 == t.current).map_or("none".to_string(), |c| c.0.clone());
    let options = t.choices.iter().map(|(name, id)| (name.clone(), *id == t.current, MenuAction::SetTarget(*id)));
    s.items.push(super::Item::Choice { label: "TARGET".into(), current, options: options.collect() });
    let Some(i) = &t.info else { return s };
    s.row("DISTANCE", k.distance(Position, i.distance));
    s.row("REL. SPEED", k.speed(Velocity, i.rel_speed));
    if let Some(ca) = &i.approach {
        s.row(if ca.over_body { "PASS" } else { "CLOSEST" }, k.distance(Position, ca.distance));
        s.row("  IN", k.or_dash(k.orbit(), || fmt_duration(ca.in_s)));
        s.row("  AT", k.speed(Velocity, ca.speed));
    }
    if let Some(x) = i.rel_inclination {
        s.row("REL. INCLINATION", k.or_dash(k.orbit(), || format!("{x:.2}°")));
    }
    s
}

/// Each planned burn, and the planner (N).
/// The ship's own clock against the origin's (relativity).
fn clock(d: &FlightData) -> Section {
    let mut s = Section::new("clock", "SHIP CLOCK");
    s.row("PROPER − ORIGIN", clock_offset(d.craft.clock_offset));
    s.note("the ship's proper time minus the origin's clock, the time shown", Tone::Dim);
    s
}

#[cfg(test)]
mod tests {
    use super::super::data::{Approach, TargetInfo};
    use super::super::{find, Item};
    use super::*;
    use crate::navball::NavTarget;

    fn orbit() -> FlightData {
        let ap = ApsisReadout {
            altitude: 301_200.0,
            above_datum: 301_200.0,
            distance: 6_378_137.0 + 301_200.0,
            in_s: 725.0,
            other_body: None,
        };
        let pe = ApsisReadout {
            altitude: 280_000.0,
            above_datum: 280_000.0,
            distance: 6_378_137.0 + 280_000.0,
            in_s: 3_490.0,
            other_body: None,
        };
        FlightData {
            body: "Earth".into(),
            ap: Some(ap),
            pe: Some(pe),
            elements: Some(Elements {
                a: 6_668_000.0,
                e: 0.0016,
                inclination: 28.61,
                lan: 12.3,
                argp: 45.0,
                true_anomaly: 190.0,
                period: Some(5_420.0),
                speed: 7_730.0,
            }),
            ..FlightData::default()
        }
    }

    #[test]
    fn the_summary_reads_the_apsides_and_the_osculating_shape() {
        let s = sections(&orbit());
        let summary = find(&s, "summary").unwrap();
        assert_eq!(
            summary.rows(),
            [
                ("APOAPSIS", "301.2 km"),
                ("  IN", "12m 05s"),
                ("PERIAPSIS", "280.0 km"),
                ("  IN", "58m 10s"),
                ("PERIOD", "1h 30m"),
                ("INCLINATION", "28.61°"),
                ("ECCENTRICITY", "0.00160")
            ]
        );
        let el = find(&s, "elements").unwrap();
        assert!(el.title.contains("OSCULATING"), "labelled so");
        assert_eq!(el.value("SEMI-MAJOR AXIS"), Some("6668.0 km"));
    }

    #[test]
    fn on_the_ground_there_is_no_orbit() {
        let d = FlightData { landed: true, ..orbit() };
        let s = sections(&d);
        assert_eq!(find(&s, "summary").unwrap().items, [Item::Note("on Earth: no orbit".into(), Tone::Dim)]);
    }

    #[test]
    fn an_escape_has_no_apoapsis_and_says_so() {
        let mut d = orbit();
        d.ap = None;
        d.elements = d.elements.map(|e| Elements { e: 1.2, period: None, ..e });
        let s = sections(&d);
        let summary = find(&s, "summary").unwrap();
        assert_eq!(summary.value("APOAPSIS"), None);
        assert_eq!(summary.value("PERIOD"), None);
        assert!(summary.items.contains(&Item::Note("escaping Earth".into(), Tone::Dim)));
        // An apsis about another body names it.
        d.pe = Some(ApsisReadout {
            altitude: 110_000.0,
            above_datum: 110_000.0,
            distance: 6_378_137.0 + 110_000.0,
            in_s: 2.6e5,
            other_body: Some("Moon".into()),
        });
        assert_eq!(find(&sections(&d), "summary").unwrap().value("PERIAPSIS"), Some("110.0 km · Moon"));
    }

    #[test]
    fn the_target_section_has_the_picker_and_with_a_target_its_approach() {
        let moon = NavTarget::Body(sim::frame::NodeId(10));
        let choices = vec![("none".to_string(), None), ("Moon".to_string(), Some(moon))];
        // (target, rows)
        let none = TargetData { current: None, choices: choices.clone(), info: None };
        let s = target(&none, &Known::default());
        assert_eq!(s.title, "TARGET");
        assert!(s.rows().is_empty(), "no target: only the picker");
        let Some(Item::Choice { current, options, .. }) = s.items.first() else { panic!("the picker comes first") };
        assert_eq!(current, "none");
        assert_eq!(options.iter().filter(|o| o.1).count(), 1, "one selected");
        assert_eq!(options[1].2, MenuAction::SetTarget(Some(moon)));
        let info = TargetInfo {
            name: "Moon".into(),
            distance: 3.844e8,
            rel_speed: 1_020.0,
            approach: Some(Approach { distance: 110_000.0, in_s: 2.6e5, speed: 2_300.0, over_body: true }),
            rel_inclination: Some(5.14),
        };
        let with = TargetData { current: Some(moon), choices, info: Some(info) };
        let s = target(&with, &Known::default());
        assert_eq!(s.title, "TARGET · Moon");
        assert_eq!(
            s.rows(),
            [
                ("DISTANCE", "384.40 Mm"),
                ("REL. SPEED", "1020.0 m/s"),
                ("PASS", "110.0 km"),
                ("  IN", "3d 00h"),
                ("  AT", "2300.0 m/s"),
                ("REL. INCLINATION", "5.14°")
            ]
        );
        let mut vessel = with.info.clone().unwrap();
        vessel.approach = vessel.approach.map(|a| Approach { over_body: false, ..a });
        vessel.rel_inclination = None;
        let s = target(&TargetData { info: Some(vessel), ..with }, &Known::default());
        assert!(s.value("CLOSEST").is_some() && s.value("PASS").is_none() && s.value("REL. INCLINATION").is_none());
    }

    #[test]
    fn the_ship_clock_is_its_own_section() {
        let s = sections(&orbit());
        assert!(find(&s, "clock").is_some_and(|c| c.value("PROPER − ORIGIN").is_some()));
    }
}
