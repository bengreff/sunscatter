//! The Comms menu (key 6): the link home (its delay, rate and the stations
//! it goes through) and, for a probe, when the controls arrive (D111: the
//! old Utilities menu's comms lines).

use super::data::{CommsData, FlightData};
use super::{Section, Tone};
use crate::format::{delay as fmt_delay, rate as fmt_rate};

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(&d.comms)]
}

fn summary(c: &CommsData) -> Section {
    let mut s = Section::summary();
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
    if let Some(delay) = c.probe {
        let text = delay.map_or("no signal: controls do not arrive".to_string(), |t| {
            format!("controls arrive after {}", fmt_delay(t))
        });
        s.toned("PROBE", text, if delay.is_some() { Tone::Normal } else { Tone::Warn });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::Signal;

    /// The link (table): relayed, aboard, lost; a probe's control delay.
    #[test]
    fn the_link_home_and_the_probe() {
        let link = Signal { delay: 0.0213, rate: 4.26e6, via: vec!["Canberra".into()] };
        let mut c = CommsData { home: "Houston".into(), link: Some(link), probe: None };
        assert_eq!(summary(&c).rows(), [("HOUSTON", "21 ms"), ("  RATE", "4.3 Mbit/s"), ("  VIA", "Canberra")]);
        c.link = None;
        c.probe = Some(None);
        let s = summary(&c);
        assert_eq!((s.value("HOUSTON"), s.tone("HOUSTON")), (Some("no signal"), Some(Tone::Warn)));
        assert_eq!(s.value("PROBE"), Some("no signal: controls do not arrive"));
        c.probe = Some(Some(1.28));
        assert_eq!(summary(&c).value("PROBE"), Some("controls arrive after 1.28 s"));
    }
}
