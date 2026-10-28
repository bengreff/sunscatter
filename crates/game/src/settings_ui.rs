//! The settings screen (F3, or Settings in the pause menu) and the
//! performance overlay (F4). Tabs: Orbits, Controls, Interface; everything
//! is saved in `settings.ron`, and each tab can be reset. There are no
//! graphics settings: one look for everyone (`look`, D073); the Interface
//! tab measures what its parts cost (`bench`).

use crate::bench::{self, Bench};
use crate::interface::layout::{self as layout, InterfaceSettings};
use crate::interface::menus::MenuId;
use crate::persist::ControlsSettings;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Orbits,
    Controls,
    Interface,
}

impl SettingsTab {
    const ALL: [SettingsTab; 3] = [SettingsTab::Orbits, SettingsTab::Controls, SettingsTab::Interface];

    fn name(self) -> &'static str {
        match self {
            SettingsTab::Orbits => "Orbits",
            SettingsTab::Controls => "Controls",
            SettingsTab::Interface => "Interface",
        }
    }
}

#[derive(Resource, Default)]
pub struct SettingsUi {
    pub open: bool,
    pub tab: SettingsTab,
    pub overlay: bool,
    /// Recent frame times (s), newest last.
    frames: VecDeque<f64>,
}

/// Keeps the recent frame times (the performance overlay).
pub fn record_frames(time: Res<Time>, mut ui: ResMut<SettingsUi>) {
    ui.frames.push_back(time.delta_secs_f64());
    while ui.frames.len() > 240 {
        ui.frames.pop_front();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut ui: ResMut<SettingsUi>,
    mut controls: ResMut<ControlsSettings>,
    mut iface: ResMut<InterfaceSettings>,
    mut orbits: ResMut<crate::trajectory::settings::OrbitSettings>,
    sim: Res<crate::state::SimState>,
    mut bench: ResMut<Bench>,
    terrain: Res<crate::terrain::Terrain>,
    fps: Res<crate::hud::FpsMeter>,
    flight: Res<crate::interface::menus::FlightData>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if ui.overlay || bench.running() {
        let n = ui.frames.len().max(1) as f64;
        let avg = ui.frames.iter().sum::<f64>() / n;
        let worst = ui.frames.iter().copied().fold(0.0, f64::max);
        egui::Area::new("perf".into()).anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0]).show(ctx, |ui_| {
            egui::Frame::popup(ui_.style()).show(ui_, |ui_| {
                ui_.monospace(format!("{} fps  {:.2} ms (worst {:.1})", fps.text(), avg * 1e3, worst * 1e3));
                ui_.monospace(format!("terrain chunks: {}", terrain.drawn));
                // The old Utilities ▸ Diagnostics (D111): the anchor, the
                // fleet, the profiler's summary.
                let g = &flight.diagnostics;
                ui_.monospace(format!("anchor {} · {} vessels", g.anchor, g.vessels));
                if g.profile.is_empty() {
                    ui_.monospace("profiler off (SUNSCATTER_PROFILE=1)");
                }
                for (k, v) in &g.profile {
                    ui_.monospace(format!("{k}: {v}"));
                }
                if bench.running() {
                    let (d, t) = bench.progress();
                    ui_.monospace(format!("benchmark {d}/{t}…"));
                }
            });
        });
    }
    if !ui.open {
        return Ok(());
    }
    let mut open = true;
    egui::Window::new("Settings").open(&mut open).default_width(420.0).collapsible(false).show(ctx, |ui_| {
        ui_.horizontal(|ui_| {
            for t in SettingsTab::ALL {
                if ui_.selectable_label(ui.tab == t, t.name()).clicked() {
                    ui.tab = t;
                }
            }
        });
        ui_.separator();
        let tab = ui.tab;
        egui::ScrollArea::vertical().max_height(560.0).show(ui_, |ui_| match tab {
            // Each tab edits a copy, written back only if it changed: a
            // `&mut` through ResMut marks it changed every frame.
            SettingsTab::Orbits => {
                let eph = &sim.world.eph;
                let bodies: Vec<_> = eph.bodies().map(|n| (n, eph.node(n).name.clone())).collect();
                let mut o = orbits.clone();
                crate::trajectory::settings::settings_ui(ui_, &mut o, &bodies);
                orbits.set_if_neq(o);
            }
            SettingsTab::Controls => {
                let mut c = *controls;
                controls_tab(ui_, &mut c);
                controls.set_if_neq(c);
            }
            SettingsTab::Interface => {
                let mut i = iface.clone();
                interface_tab(ui_, &mut i);
                iface.set_if_neq(i);
                ui_.separator();
                performance(ui_, &mut ui, &mut bench);
            }
        });
    });
    ui.open = open;
    Ok(())
}

/// The performance overlay and the benchmark of the look's parts.
fn performance(ui_: &mut egui::Ui, ui: &mut SettingsUi, bench: &mut Bench) {
    ui_.checkbox(&mut ui.overlay, "Performance overlay (also in the Esc menu)");
    if !bench.running() && ui_.button("Measure the look and what each part costs, from this view").clicked() {
        bench.start("interactive");
    }
    if bench.results.is_empty() {
        return;
    }
    if ui_.button("Copy results as Markdown").clicked() {
        ui_.ctx().copy_text(bench::markdown(bench));
    }
    let base = bench::baseline(&bench.results).unwrap_or(0.0);
    egui::Grid::new("bench").striped(true).show(ui_, |ui_| {
        for r in &bench.results {
            ui_.monospace(&r.label);
            ui_.monospace(format!("{:.2} ms (p95 {:.2})", r.avg_ms, r.p95_ms));
            ui_.monospace(format!("{:+.2}", base - r.avg_ms));
            ui_.end_row();
        }
    });
}

fn controls_tab(ui: &mut egui::Ui, c: &mut ControlsSettings) {
    ui.add(egui::Slider::new(&mut c.wheel_zoom, 1.01..=1.3).text("zoom per wheel line"));
    // What a trackpad pixel zooms, as `camera::scroll_lines` applies it.
    let trackpad = format!(
        "trackpad zoom ({:.3} wheel lines per px)",
        crate::camera::scroll_lines(1.0, true, c.trackpad_lines_per_px)
    );
    ui.add(egui::Slider::new(&mut c.trackpad_lines_per_px, 0.02..=0.5).logarithmic(true).text(trackpad));
    ui.add(
        egui::Slider::new(&mut c.mouse_sensitivity, 0.001..=0.02).logarithmic(true).text("camera drag (rad per px)"),
    );
    ui.checkbox(&mut c.invert_y, "Invert vertical camera drag");
    if ui.button("Reset to defaults").clicked() {
        *c = ControlsSettings::default();
    }
    ui.separator();
    crate::interface::help::key_grid(ui);
}

fn interface_tab(ui: &mut egui::Ui, s: &mut InterfaceSettings) {
    ui.add(egui::Slider::new(&mut s.ui_scale, layout::UI_SCALE_RANGE).text("UI scale"));
    ui.add(egui::Slider::new(&mut s.navball_size, layout::NAVBALL_SIZE_RANGE).text("navball size (px)"));
    ui.checkbox(&mut s.show_fps, "Frame rate in the time panel");
    ui.separator();
    ui.label("Menus (the menu bar, or their keys; places are fixed):");
    for m in MenuId::ALL {
        let mut open = s.menus.is_open(m);
        if ui.checkbox(&mut open, m.with_key(m.title())).changed() {
            s.menus.set_open(m, open);
        }
    }
    ui.separator();
    ui.checkbox(&mut s.agent.enabled, "AI agent server (MCP, local only)");
    ui.horizontal(|ui| {
        ui.label("port");
        ui.add(egui::DragValue::new(&mut s.agent.port).range(1024..=65535));
    });
    if s.agent.enabled && !s.agent.token.is_empty() {
        ui.label(egui::RichText::new("Connect Claude Code with:").small());
        let mut cmd = crate::agent::connect_command(s.agent.port, &s.agent.token);
        ui.add(egui::TextEdit::multiline(&mut cmd).font(egui::TextStyle::Monospace).desired_rows(2));
        if ui.button("New token").clicked() {
            s.agent.token = crate::agent::new_token();
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Reset to defaults").clicked() {
            *s = InterfaceSettings::default();
        }
    });
}
