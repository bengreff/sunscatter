//! Orbit-line settings (D056): how long vessel and body lines are, and which
//! bodies' lines are shown. Saved in `settings.ron` (`orbits`). The rules
//! that turn them into limits are pure functions with table tests.

use super::line::Limits;
use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};
use sim::frame::NodeId;
use std::collections::BTreeSet;

const DAY: f64 = 86_400.0;

/// How long a vessel's line is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VesselLine {
    /// One revolution about the dominant body.
    #[default]
    OneRevolution,
    TwoRevolutions,
    /// Until impact, escape or the time cap.
    UntilImpactOrEscape,
    /// A fixed time ahead (`fixed_time_days`).
    FixedTime,
}

impl VesselLine {
    pub const ALL: [VesselLine; 4] =
        [VesselLine::OneRevolution, VesselLine::TwoRevolutions, VesselLine::UntilImpactOrEscape, VesselLine::FixedTime];

    pub fn name(self) -> &'static str {
        match self {
            VesselLine::OneRevolution => "1 revolution",
            VesselLine::TwoRevolutions => "2 revolutions",
            VesselLine::UntilImpactOrEscape => "until impact/escape",
            VesselLine::FixedTime => "fixed time",
        }
    }
}

/// How long a body's line is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyLine {
    #[default]
    OneRevolution,
    Off,
}

/// Orbit-line settings.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrbitSettings {
    pub vessel_line: VesselLine,
    /// Line length for [`VesselLine::FixedTime`] (days).
    pub fixed_time_days: f64,
    /// No vessel line is longer than this (days).
    pub vessel_time_cap_days: f64,
    pub body_line: BodyLine,
    /// Bodies whose lines are hidden, by name.
    pub hidden_bodies: BTreeSet<String>,
    /// Show only lines of bodies in the current system (those whose
    /// dominant body is the active vessel's).
    pub only_current_system: bool,
}

impl Default for OrbitSettings {
    fn default() -> Self {
        OrbitSettings {
            vessel_line: VesselLine::OneRevolution,
            fixed_time_days: 1.0,
            vessel_time_cap_days: 365.25,
            body_line: BodyLine::OneRevolution,
            hidden_bodies: BTreeSet::new(),
            only_current_system: false,
        }
    }
}

impl OrbitSettings {
    /// Limits of a vessel's line starting at local time `t_start`.
    pub fn vessel_limits(&self, t_start: f64, escape_radius: f64) -> Limits {
        let cap = self.vessel_time_cap_days.max(0.0) * DAY;
        let (revolutions, span) = match self.vessel_line {
            VesselLine::OneRevolution => (Some(1.0), cap),
            VesselLine::TwoRevolutions => (Some(2.0), cap),
            VesselLine::UntilImpactOrEscape => (None, cap),
            VesselLine::FixedTime => (None, cap.min(self.fixed_time_days.max(0.0) * DAY)),
        };
        Limits { revolutions, t_cap: t_start + span, escape_radius }
    }

    /// Limits of a body's line (one revolution; the ephemeris span caps it).
    pub fn body_limits(&self, escape_radius: f64) -> Option<Limits> {
        match self.body_line {
            BodyLine::OneRevolution => Some(Limits { revolutions: Some(1.0), t_cap: f64::INFINITY, escape_radius }),
            BodyLine::Off => None,
        }
    }

    /// Whether a body's line is shown: `dominant` is the body's dominant
    /// body, `current_system` the active vessel's.
    pub fn body_line_shown(&self, name: &str, dominant: NodeId, current_system: Option<NodeId>) -> bool {
        self.body_line != BodyLine::Off
            && !self.hidden_bodies.contains(name)
            && (!self.only_current_system || current_system.is_none_or(|c| c == dominant))
    }
}

/// The Orbits tab of the settings screen. `bodies` lists the bodies that
/// can have lines (id, name).
#[allow(dead_code)] // embedded by the settings screen (`settings_ui`)
pub fn settings_ui(ui: &mut egui::Ui, s: &mut OrbitSettings, bodies: &[(NodeId, String)]) {
    ui.heading("Vessel lines");
    egui::ComboBox::from_label("Length").selected_text(s.vessel_line.name()).show_ui(ui, |ui| {
        for v in VesselLine::ALL {
            ui.selectable_value(&mut s.vessel_line, v, v.name());
        }
    });
    if s.vessel_line == VesselLine::FixedTime {
        ui.add(egui::Slider::new(&mut s.fixed_time_days, 0.01..=365.25).logarithmic(true).text("days ahead"));
    }
    ui.add(egui::Slider::new(&mut s.vessel_time_cap_days, 1.0..=3650.0).logarithmic(true).text("time cap (days)"));
    ui.separator();
    ui.heading("Body lines");
    let mut on = s.body_line == BodyLine::OneRevolution;
    if ui.checkbox(&mut on, "Show body orbits (1 revolution)").changed() {
        s.body_line = if on { BodyLine::OneRevolution } else { BodyLine::Off };
    }
    ui.add_enabled_ui(on, |ui| {
        ui.checkbox(&mut s.only_current_system, "Only orbits in the current system");
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for (_, name) in bodies {
                let mut shown = !s.hidden_bodies.contains(name);
                if ui.checkbox(&mut shown, name.as_str()).changed() {
                    if shown {
                        s.hidden_bodies.remove(name);
                    } else {
                        s.hidden_bodies.insert(name.clone());
                    }
                }
            }
        });
    });
    ui.separator();
    if ui.button("Reset to defaults").clicked() {
        *s = OrbitSettings::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vessel_limits_follow_the_setting() {
        let cap = 365.25 * DAY;
        let cases = [
            (VesselLine::OneRevolution, Some(1.0), cap),
            (VesselLine::TwoRevolutions, Some(2.0), cap),
            (VesselLine::UntilImpactOrEscape, None, cap),
            (VesselLine::FixedTime, None, 3.0 * DAY),
        ];
        for (vessel_line, revs, span) in cases {
            let s = OrbitSettings { vessel_line, fixed_time_days: 3.0, ..OrbitSettings::default() };
            let l = s.vessel_limits(100.0, 7.0);
            assert_eq!(l, Limits { revolutions: revs, t_cap: 100.0 + span, escape_radius: 7.0 }, "{vessel_line:?}");
        }
        // A fixed time beyond the cap is capped.
        let s =
            OrbitSettings { vessel_line: VesselLine::FixedTime, fixed_time_days: 900.0, ..OrbitSettings::default() };
        assert_eq!(s.vessel_limits(0.0, 1.0).t_cap, cap);
    }

    #[test]
    fn body_lines_shown_by_setting_hidden_list_and_system() {
        let (sun, earth) = (NodeId(1), NodeId(3));
        let hidden = OrbitSettings { hidden_bodies: ["Mars".to_string()].into(), ..OrbitSettings::default() };
        let system = OrbitSettings { only_current_system: true, ..OrbitSettings::default() };
        let off = OrbitSettings { body_line: BodyLine::Off, ..OrbitSettings::default() };
        // (settings, body, its dominant body, current system, shown)
        let cases = [
            (&OrbitSettings::default(), "Mars", sun, Some(earth), true),
            (&hidden, "Mars", sun, Some(sun), false),
            (&hidden, "Venus", sun, Some(sun), true),
            (&system, "Moon", earth, Some(earth), true),
            (&system, "Mars", sun, Some(earth), false),
            (&system, "Mars", sun, Some(sun), true),
            (&system, "Mars", sun, None, true),
            (&off, "Moon", earth, Some(earth), false),
        ];
        for (s, name, dominant, current, want) in cases {
            assert_eq!(s.body_line_shown(name, dominant, current), want, "{name} {s:?}");
        }
        assert!(off.body_limits(1.0).is_none());
        assert_eq!(OrbitSettings::default().body_limits(1.0).map(|l| l.revolutions), Some(Some(1.0)));
    }

    #[test]
    fn settings_round_trip_and_fill_defaults() {
        let s = OrbitSettings {
            vessel_line: VesselLine::FixedTime,
            fixed_time_days: 2.5,
            vessel_time_cap_days: 30.0,
            body_line: BodyLine::Off,
            hidden_bodies: ["Moon".to_string(), "Pluto".to_string()].into(),
            only_current_system: true,
        };
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<OrbitSettings>(&text).unwrap(), s);
        let partial: OrbitSettings = ron::from_str("(vessel_line: TwoRevolutions)").unwrap();
        assert_eq!(partial, OrbitSettings { vessel_line: VesselLine::TwoRevolutions, ..OrbitSettings::default() });
    }
}
