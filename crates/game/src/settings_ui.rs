//! The settings screen (F3, or Settings in the pause menu) and the
//! performance overlay (F4). Tabs: Graphics, Orbits, Controls, Interface;
//! everything is saved in `settings.ron`, and each tab can be reset.

use crate::bench::{self, Bench};
use crate::interface::layout::{self as layout, InterfaceSettings, PanelId};
use crate::persist::ControlsSettings;
use crate::settings::{AtmosphereQuality, GraphicsSettings, MsaaLevel, Tier};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Orbits,
    Controls,
    Interface,
}

impl SettingsTab {
    const ALL: [SettingsTab; 4] =
        [SettingsTab::Graphics, SettingsTab::Orbits, SettingsTab::Controls, SettingsTab::Interface];

    fn name(self) -> &'static str {
        match self {
            SettingsTab::Graphics => "Graphics",
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

pub fn toggle(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut ui: ResMut<SettingsUi>) {
    // F3 opens Settings on the Graphics tab.
    if keys.just_pressed(KeyCode::F3) {
        ui.open = !ui.open;
        ui.tab = SettingsTab::Graphics;
    }
    if keys.just_pressed(KeyCode::F4) {
        ui.overlay = !ui.overlay;
    }
    ui.frames.push_back(time.delta_secs_f64());
    while ui.frames.len() > 240 {
        ui.frames.pop_front();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut ui: ResMut<SettingsUi>,
    mut settings: ResMut<GraphicsSettings>,
    mut controls: ResMut<ControlsSettings>,
    mut iface: ResMut<InterfaceSettings>,
    mut orbits: ResMut<crate::trajectory::settings::OrbitSettings>,
    sim: Res<crate::state::SimState>,
    mut bench: ResMut<Bench>,
    terrain: Res<crate::terrain::Terrain>,
    fps: Res<crate::hud::FpsMeter>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if ui.overlay || bench.running() {
        let n = ui.frames.len().max(1) as f64;
        let avg = ui.frames.iter().sum::<f64>() / n;
        let worst = ui.frames.iter().copied().fold(0.0, f64::max);
        let tier = settings.tier.map_or("Custom", Tier::name);
        egui::Area::new("perf".into()).anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0]).show(ctx, |ui_| {
            egui::Frame::popup(ui_.style()).show(ui_, |ui_| {
                ui_.monospace(format!("{} fps  {:.2} ms (worst {:.1})", fps.text(), avg * 1e3, worst * 1e3));
                ui_.monospace(format!("graphics: {tier}   terrain chunks: {}", terrain.drawn));
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
            // `&mut` through ResMut marks it changed every frame, which
            // re-applied graphics settings every frame while open.
            SettingsTab::Graphics => {
                let mut g = *settings;
                graphics_tab(ui_, &mut ui, &mut g, &mut bench);
                settings.set_if_neq(g);
            }
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
            }
        });
    });
    ui.open = open;
    Ok(())
}

fn graphics_tab(ui_: &mut egui::Ui, ui: &mut SettingsUi, settings: &mut GraphicsSettings, bench: &mut Bench) {
    ui_.horizontal(|ui_| {
        for t in Tier::ALL {
            if ui_.selectable_label(settings.tier == Some(t), t.name()).clicked() {
                *settings = settings.with_preset(t);
            }
        }
    });
    let mut s = *settings;
    ui_.separator();
    ui_.checkbox(&mut s.terrain, "Terrain elevation (LOD)");
    ui_.add(egui::Slider::new(&mut s.terrain_error_px, 0.5..=16.0).text("terrain error (px)").logarithmic(true));
    egui::ComboBox::from_label("Colour texture size").selected_text(format!("{}", s.texture_size)).show_ui(
        ui_,
        |ui_| {
            for size in [1024, 2048, 4096, 8192] {
                ui_.selectable_value(&mut s.texture_size, size, format!("{size}"));
            }
        },
    );
    ui_.checkbox(&mut s.detail, "Surface detail layer");
    egui::ComboBox::from_label("Atmosphere").selected_text(format!("{:?}", s.atmosphere)).show_ui(ui_, |ui_| {
        for a in [AtmosphereQuality::Off, AtmosphereQuality::Lut, AtmosphereQuality::Raymarched] {
            ui_.selectable_value(&mut s.atmosphere, a, format!("{a:?}"));
        }
    });
    ui_.add(egui::Slider::new(&mut s.haze, 0.0..=2.0).text("haze (1 = physical)"));
    ui_.checkbox(&mut s.ocean_glint, "Ocean sun glint");
    ui_.add(egui::Slider::new(&mut s.star_magnitude, 0.0..=8.0).text("faintest star (mag, 0 = none)"));
    ui_.checkbox(&mut s.bloom, "Bloom");
    ui_.checkbox(&mut s.flare, "Sun flare");
    ui_.checkbox(&mut s.shadows, "Shadows");
    ui_.checkbox(&mut s.earthshine, "Planetshine (Earthshine, moonlight)");
    egui::ComboBox::from_label("MSAA").selected_text(format!("{:?}", s.msaa)).show_ui(ui_, |ui_| {
        for m in [MsaaLevel::Off, MsaaLevel::X2, MsaaLevel::X4] {
            ui_.selectable_value(&mut s.msaa, m, format!("{m:?}"));
        }
    });
    if s != *settings {
        s.tier = s.matching_tier();
        *settings = s;
    }
    ui_.separator();
    ui_.checkbox(&mut ui.overlay, "Performance overlay (F4)");
    if !bench.running() && ui_.button("Measure all tiers and features from this view").clicked() {
        bench.start("interactive", *settings);
    }
    if !bench.results.is_empty() {
        if ui_.button("Copy results as Markdown").clicked() {
            ui_.ctx().copy_text(bench::markdown(bench));
        }
        let base = bench::baseline(&bench.results).unwrap_or(0.0);
        egui::Grid::new("bench").striped(true).show(ui_, |ui_| {
            for r in &bench.results {
                ui_.monospace(&r.label);
                ui_.monospace(format!("{:.2} ms (p95 {:.2})", r.avg_ms, r.p95_ms));
                ui_.monospace(format!("{:+.2}", r.avg_ms - base));
                ui_.end_row();
            }
        });
    }
    if ui_.button("Reset to defaults").clicked() {
        *settings = GraphicsSettings::default();
    }
}

fn controls_tab(ui: &mut egui::Ui, c: &mut ControlsSettings) {
    ui.add(egui::Slider::new(&mut c.wheel_zoom, 1.01..=1.3).text("zoom per wheel line"));
    ui.add(
        egui::Slider::new(&mut c.trackpad_lines_per_px, 0.02..=0.5)
            .logarithmic(true)
            .text("trackpad zoom (lines per px)"),
    );
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
    ui.checkbox(&mut s.show_fps, "Frame rate in the Time panel");
    ui.separator();
    ui.label("Panels (drag any panel by its background to move it):");
    for id in PanelId::ALL {
        let mut visible = s.panel(id).visible;
        if ui.checkbox(&mut visible, id.title()).changed() {
            s.set_visible(id, visible);
        }
    }
    ui.horizontal(|ui| {
        if ui.button("Reset layout").clicked() {
            s.reset_layout();
        }
        if ui.button("Reset to defaults").clicked() {
            let generation = s.layout_generation.wrapping_add(1);
            *s = InterfaceSettings { layout_generation: generation, ..InterfaceSettings::default() };
        }
    });
}
