//! The Utilities menu (key U, right dock): a brief status line per system
//! the craft has, and a section per system. Power and life support arrive
//! with their systems; the detailed ship-systems screen is separate (D080).

use super::data::{CommsData, FlightData};
use super::landing::heat_tone;
use super::{Section, Tone};
use crate::format::{delay as fmt_delay, rate as fmt_rate};
use crate::hud::{clock_offset, temperature};

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(d), propulsion(d), thermal(d), comms(&d.comms), clock(d), diagnostics(d)]
}

fn summary(d: &FlightData) -> Section {
    let mut s = Section::summary();
    let c = &d.craft;
    // Before the first snapshot `full` is 0: no division by it.
    let fraction = if c.full > 0.0 { c.propellant / c.full } else { 0.0 };
    let tone = if c.debug {
        Tone::Normal
    } else if c.propellant <= 0.0 {
        Tone::Alert
    } else if fraction < 0.1 {
        Tone::Warn
    } else {
        Tone::Normal
    };
    s.toned("PROPELLANT", format!("{:.0} kg · {:.0} %", c.propellant, 100.0 * fraction), tone);
    s.row("ΔV (VAC)", format!("{:.0} m/s", c.dv_vac));
    let (skin, skin_limit) = c.skin;
    s.toned("SKIN MAX", temperature(skin, skin_limit), heat_tone(skin, skin_limit));
    let (node, node_limit) = c.interior;
    s.toned("INTERIOR MAX", temperature(node, node_limit), heat_tone(node, node_limit));
    let link = d.comms.link.as_ref();
    let comms = match link {
        None => "no signal".to_string(),
        Some(l) if l.delay == 0.0 => "aboard".to_string(),
        Some(l) => fmt_delay(l.delay),
    };
    s.toned("COMMS", format!("{}: {comms}", d.comms.home), if link.is_some() { Tone::Normal } else { Tone::Warn });
    if let Some(delay) = d.comms.probe {
        let text = delay.map_or("no signal: controls do not arrive".to_string(), |t| {
            format!("controls arrive after {}", fmt_delay(t))
        });
        s.toned("PROBE", text, if delay.is_some() { Tone::Normal } else { Tone::Warn });
    }
    if c.debug {
        s.toned("DEBUG MODE", "on", Tone::Warn);
    }
    s.note("power and life support are not modelled yet", Tone::Dim);
    s
}

fn propulsion(d: &FlightData) -> Section {
    let mut s = Section::new("propulsion", "PROPULSION");
    let c = &d.craft;
    s.row("TWR (VAC)", c.twr_vac.map_or("— (free space)".to_string(), |t| format!("{t:.2} in {}'s gravity", d.body)));
    s.row("MASS", format!("{:.2} t", c.mass / 1e3));
    s.row("THRUST (VAC)", format!("{:.0} kN", c.thrust_vac / 1e3));
    s.row("ISP", format!("{:.0} s vac · {:.0} s SL", c.isp_vac, c.isp_sl));
    let [pitch, yaw, roll] = c.rcs_torque;
    s.row("RCS PITCH / YAW", format!("{:.1} / {:.1} kN·m", pitch / 1e3, yaw / 1e3));
    s.row("RCS ROLL", format!("{:.1} kN·m", roll / 1e3));
    let [fp, fy, fr] = c.rcs_flow;
    s.row("RCS USE (FULL)", format!("{fp:.2} / {fy:.2} / {fr:.2} kg/s"));
    s
}

/// The parts nearest their limits, and the interior.
fn thermal(d: &FlightData) -> Section {
    let mut s = Section::new("thermal", "THERMAL");
    for (name, t, limit) in &d.craft.hottest {
        s.toned(name.to_uppercase(), temperature(*t, *limit), heat_tone(*t, *limit));
    }
    if !s.items.is_empty() {
        let (node, limit) = d.craft.interior;
        s.toned("INTERIOR", temperature(node, limit), heat_tone(node, limit));
    }
    s
}

fn comms(c: &CommsData) -> Section {
    let mut s = Section::new("comms", "COMMS");
    match &c.link {
        None => {
            s.toned(c.home.to_uppercase(), "no signal", Tone::Warn);
        }
        Some(l) if l.delay == 0.0 => {
            s.row(c.home.to_uppercase(), "aboard");
        }
        Some(l) => {
            s.row(c.home.to_uppercase(), fmt_delay(l.delay));
            s.row("  RATE", fmt_rate(l.rate));
            if !l.via.is_empty() {
                s.row("  VIA", l.via.join(", "));
            }
        }
    }
    s
}

fn clock(d: &FlightData) -> Section {
    let mut s = Section::new("clock", "CLOCK");
    s.row("SHIP CLOCK", clock_offset(d.craft.clock_offset));
    s.note("proper time minus the origin's clock, the time shown (relativity)", Tone::Dim);
    s
}

/// What the old Debug panel showed.
fn diagnostics(d: &FlightData) -> Section {
    let mut s = Section::new("diagnostics", "DIAGNOSTICS");
    let g = &d.diagnostics;
    if g.anchor.is_empty() {
        return s;
    }
    s.row("ANCHOR", &g.anchor);
    s.row("PLOT FRAME", format!("{} (Tab)", g.plot_frame));
    s.row("FPS", &g.fps);
    s.row("VESSELS", g.vessels.to_string());
    if g.profile.is_empty() {
        s.note("profiler off (SUNSCATTER_PROFILE=1)", Tone::Dim);
    }
    for (k, v) in &g.profile {
        s.row(k, v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::data::CraftData;
    use super::super::{find, Item, MenuId};
    use super::*;
    use crate::comms::Signal;

    fn ship() -> FlightData {
        let craft = CraftData {
            propellant: 12_000.0,
            full: 16_000.0,
            dv_vac: 3_512.4,
            twr_vac: Some(1.53),
            mass: 16_000.0,
            thrust_vac: 300e3,
            isp_vac: 320.0,
            isp_sl: 290.0,
            rcs_torque: [8_000.0, 8_000.0, 3_700.0],
            rcs_flow: [1.2, 1.2, 1.25],
            skin: (300.0, 1_100.0),
            interior: (295.0, 400.0),
            hottest: vec![("engine bell".into(), 1_500.0, 1_640.0), ("body".into(), 310.0, 1_100.0)],
            ..CraftData::default()
        };
        let link = Signal { delay: 0.0213, rate: 4.26e6, via: vec!["Canberra".into()] };
        let comms = CommsData { home: "Houston".into(), link: Some(link), probe: None };
        FlightData { body: "Earth".into(), craft, comms, ..FlightData::default() }
    }

    #[test]
    fn the_summary_is_a_line_per_system() {
        let s = MenuId::Utilities.sections(&ship());
        let summary = find(&s, "summary").unwrap();
        assert_eq!(
            summary.rows(),
            [
                ("PROPELLANT", "12000 kg · 75 %"),
                ("ΔV (VAC)", "3512 m/s"),
                ("SKIN MAX", "300 K / 1100 K"),
                ("INTERIOR MAX", "295 K / 400 K"),
                ("COMMS", "Houston: 21 ms")
            ]
        );
        assert!(matches!(summary.items.last(), Some(Item::Note(n, Tone::Dim)) if n.contains("not modelled")));
    }

    #[test]
    fn warnings_for_low_propellant_heat_lost_signal_and_probes() {
        // (propellant kg, debug, tone)
        let table = [(12_000.0, false, Tone::Normal), (800.0, false, Tone::Warn), (0.0, false, Tone::Alert)];
        for (kg, debug, want) in table {
            let mut d = ship();
            d.craft.propellant = kg;
            d.craft.debug = debug;
            assert_eq!(summary(&d).tone("PROPELLANT"), Some(want), "{kg}");
        }
        let mut d = ship();
        d.craft.propellant = 0.0;
        d.craft.debug = true;
        let s = summary(&d);
        assert_eq!(s.tone("PROPELLANT"), Some(Tone::Normal), "debug mode: infinite propellant");
        assert_eq!(s.tone("DEBUG MODE"), Some(Tone::Warn));
        let mut d = ship();
        d.comms.link = None;
        d.comms.probe = Some(None);
        let s = summary(&d);
        assert_eq!((s.value("COMMS"), s.tone("COMMS")), (Some("Houston: no signal"), Some(Tone::Warn)));
        assert_eq!(s.value("PROBE"), Some("no signal: controls do not arrive"));
        d.comms.probe = Some(Some(1.28));
        assert_eq!(summary(&d).value("PROBE"), Some("controls arrive after 1.28 s"));
        let thermal = thermal(&ship());
        assert_eq!(thermal.rows()[0], ("ENGINE BELL", "1500 K / 1640 K !"));
        assert_eq!(thermal.tone("ENGINE BELL"), Some(Tone::Warn));
    }

    #[test]
    fn sections_for_propulsion_comms_and_the_clock() {
        let s = MenuId::Utilities.sections(&ship());
        let p = find(&s, "propulsion").unwrap();
        assert_eq!(p.value("TWR (VAC)"), Some("1.53 in Earth's gravity"));
        assert_eq!(p.value("RCS PITCH / YAW"), Some("8.0 / 8.0 kN·m"));
        assert_eq!(p.value("ISP"), Some("320 s vac · 290 s SL"));
        let c = find(&s, "comms").unwrap();
        assert_eq!(c.rows(), [("HOUSTON", "21 ms"), ("  RATE", "4.3 Mbit/s"), ("  VIA", "Canberra")]);
        assert!(find(&s, "clock").is_some());
        assert!(find(&s, "diagnostics").is_none(), "nothing gathered, nothing shown");
    }
}
