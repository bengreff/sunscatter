//! Part cards (D111): one card per part with its live essentials on this
//! ship now and its commands (`content`, pure). Clicking a part's icon on
//! the staging bar opens its card stacked left of the bar (over whatever
//! menu is there); clicking the part on the ship (a click, not a camera
//! drag: `scene::pick`) opens it near the part, fixed on screen and
//! draggable. A line joins each card to its part (projected every frame)
//! or icon. A dash's tooltip, after hovering it 2 s, names the sensor that
//! would measure it (D106). Irreversible commands ask for a confirmation.
//! The card's "more" opens the systems screen at the part
//! (`SystemsScreen::open_at`): everything about it, by layer.

pub mod content;
pub mod inside;
pub mod place;

use crate::commands::GameCommand;
use crate::interface::{layout, stage_bar, theme};
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use content::{Card, Live, Value};
use sim::vessel::VesselId;

/// Where a card was opened from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum From {
    /// The staging bar's icon: stacked left of the bar.
    Bar,
    /// The part on the ship, clicked at this point (egui points).
    Ship(egui::Pos2),
}

/// An open card.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    pub vessel: VesselId,
    /// `Part::id`.
    pub part: u32,
    pub from: From,
    /// The command (index on the card) waiting for its confirmation.
    pub confirm: Option<usize>,
    /// Its own egui id (a card opened again starts where it is asked).
    pub serial: u64,
}

/// The open cards, the mouse press a click starts from, each card's part
/// on screen, and the UI zoom (to turn window points into egui's).
#[derive(Resource, Default)]
pub struct PartCards {
    pub open: Vec<Open>,
    serial: u64,
    press: Option<(f64, Vec2)>,
    /// Each ship card's part on screen (egui points), by serial.
    anchors: Vec<(u64, egui::Pos2)>,
    zoom: f32,
}

impl PartCards {
    /// A click on part `part` (on the ship or its staging bar icon): opens
    /// its card, or closes it when it is open (the owner, 2026-10-28).
    pub fn open(&mut self, vessel: VesselId, part: u32, from: From) {
        if let Some(k) = self.open.iter().position(|c| c.vessel == vessel && c.part == part) {
            self.open.remove(k);
            return;
        }
        self.serial += 1;
        let serial = self.serial;
        self.open.push(Open { vessel, part, from, confirm: None, serial });
    }
}

pub struct PartCardPlugin;

impl Plugin for PartCardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartCards>()
            .add_systems(Update, (place::click, place::anchors).chain().after(crate::state::advance))
            .add_systems(bevy_egui::EguiPrimaryContextPass, show.after(stage_bar::show));
    }
}

/// How long a dash is hovered before its tooltip names the sensor (s).
const DASH_HOVER_S: f64 = 2.0;
/// A card's width (logical px).
pub const CARD_WIDTH: f32 = 250.0;
/// A card's height before it has been drawn once (logical px).
const CARD_GUESS: f32 = 300.0;

/// Draws the active vessel's cards and their lines; sends their commands.
#[allow(clippy::too_many_arguments)]
pub fn show(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    station: Res<crate::tracking::TrackingStation>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    bar: Res<stage_bar::StageBar>,
    mut cards: ResMut<PartCards>,
    mut cmds: MessageWriter<GameCommand>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    cards.zoom = ctx.zoom_factor();
    let v = sim.ship();
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else {
        cards.open.clear();
        return Ok(());
    };
    // Cards of parts no longer aboard (another vessel's now) close.
    let id = v.id();
    cards.open.retain(|c| c.vessel == id && def.parts.iter().any(|p| p.part == c.part));
    if station.open || systems.open || cards.open.is_empty() {
        return Ok(());
    }
    let knowledge = v.knowledge();
    let p = {
        let (anchor, r, _) = v.state_at(&sim.world, sim.clock);
        sim::forces::ambient_pressure(&sim.world, &sim.world.snapshot(sim.clock), anchor, r)
    };
    let thrusts = v.engine_thrusts(sim.controls().throttle, p).unwrap_or_default();
    let kelvin = v.part_kelvin();
    let chute_parts: Vec<u32> = v.craft.design().parts.iter().filter(|p| p.chute).map(|p| p.id).collect();
    let words = content::chute_words(stage_bar::icons::chute_look(&v.chute));
    let chute = (!chute_parts.is_empty()).then_some((chute_parts.as_slice(), words));
    let live = Live { def: &def, state, knowledge: &knowledge, thrusts: &thrusts, kelvin: &kelvin, chute };
    let screen = ctx.content_rect();
    let right = screen.max.x - layout::MARGIN - stage_bar::WIDTH - layout::GAP;
    let top = screen.min.y + layout::MARGIN + 40.0;
    // The staging bar's cards in columns, each as tall as it was drawn last.
    let id_of = |serial: u64| egui::Id::new(("part_card", serial));
    let heights: Vec<f32> = (cards.open.iter().filter(|c| c.from == From::Bar))
        .map(|c| ctx.memory(|m| m.area_rect(id_of(c.serial))).map_or(CARD_GUESS, |r| r.height()))
        .collect();
    let mut corners = place::stack(&heights, top, screen.max.y - layout::MARGIN, right, CARD_WIDTH, layout::GAP).into_iter();
    let mut closed = Vec::new();
    let mut lines: Vec<(egui::Rect, egui::Pos2)> = Vec::new();
    for k in 0..cards.open.len() {
        let open = cards.open[k].clone();
        let Some(card) = content::card(&live, open.part) else { continue };
        let area = egui::Area::new(id_of(open.serial));
        let area = match open.from {
            From::Bar => area.fixed_pos(corners.next().unwrap_or(egui::pos2(right - CARD_WIDTH, top))).movable(false),
            From::Ship(at) => area.default_pos(place::beside(at, screen, CARD_WIDTH)).movable(true),
        };
        let shown = area.show(ctx, |ui| draw(ui, &card, &mut cards.open[k]));
        let rect = shown.response.rect;
        match shown.inner {
            Some(Click::Close) => closed.push(open.serial),
            Some(Click::Run(c)) => {
                cmds.write(GameCommand::Ship { vessel: id, commands: vec![c] });
            }
            Some(Click::More) => systems.open_at(None, Some(open.part)),
            None => {}
        }
        let target = match open.from {
            From::Bar => bar.icons.iter().find(|(p, _)| *p == open.part).map(|(_, r)| r.center()),
            From::Ship(_) => cards.anchors.iter().find(|(s, _)| *s == open.serial).map(|a| a.1),
        };
        if let Some(t) = target {
            lines.push((rect, t));
        }
    }
    cards.open.retain(|c| !closed.contains(&c.serial));
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("part_card_lines")));
    for (rect, t) in lines {
        painter.line_segment([rect.clamp(t), t], egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.7)));
        painter.circle_filled(t, 2.5, theme::ACCENT);
    }
    Ok(())
}

/// What a click on a card asks.
enum Click {
    Close,
    Run(sim::craft::command::ShipCommand),
    /// The systems screen at this part.
    More,
}

/// One card's contents (see the module).
fn draw(ui: &mut egui::Ui, card: &Card, open: &mut Open) -> Option<Click> {
    let mut click = None;
    theme::panel_frame().show(ui, |ui| {
        ui.set_width(CARD_WIDTH - 18.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&card.title).strong().color(theme::ACCENT));
            ui.label(egui::RichText::new(&card.entry).small().color(theme::DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("×").frame(false)).clicked() {
                    click = Some(Click::Close);
                }
            });
        });
        for s in &card.sections {
            ui.label(egui::RichText::new(&s.heading).small().color(theme::MUTED));
            for l in &s.lines {
                line(ui, &l.label, &l.value, l.level);
            }
        }
        let more = ui.add(egui::Button::new(egui::RichText::new("more ›").small().color(theme::ACCENT)).frame(false));
        if more.on_hover_text("Everything about this part: the systems screen (Tab)").clicked() {
            click = Some(Click::More);
        }
        if card.commands.is_empty() {
            return;
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            for (k, c) in card.commands.iter().enumerate() {
                if ui.small_button(&c.label).clicked() {
                    if c.confirm {
                        open.confirm = Some(k);
                    } else {
                        click = Some(Click::Run(c.command.clone()));
                    }
                }
            }
        });
        let Some(c) = open.confirm.and_then(|k| card.commands.get(k)) else { return };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}: cannot be undone", c.label)).small().color(theme::WARN));
            if ui.small_button("Confirm").clicked() {
                click = Some(Click::Run(c.command.clone()));
                open.confirm = None;
            }
            if ui.small_button("Cancel").clicked() {
                open.confirm = None;
            }
        });
    });
    click
}

/// A label and its value; an unknown value is a dash whose tooltip, after
/// [`DASH_HOVER_S`] of hovering, names the sensor that would measure it.
fn line(ui: &mut egui::Ui, label: &str, value: &Value, level: crate::format::Level) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).small().color(theme::DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match value {
            Value::Shown(text) => {
                ui.label(egui::RichText::new(text).small().monospace().color(theme::level_colour(level)));
            }
            Value::Unknown(sensor) => {
                let dash = ui.add(
                    egui::Label::new(egui::RichText::new(crate::format::DASH).small()).sense(egui::Sense::hover()),
                );
                let now = ui.ctx().input(|i| i.time);
                let key = dash.id.with("hover_since");
                let since = ui.data_mut(|d| {
                    if dash.hovered() {
                        *d.get_temp_mut_or(key, now)
                    } else {
                        d.remove::<f64>(key);
                        now
                    }
                });
                if dash.hovered() && now - since >= DASH_HOVER_S {
                    dash.show_tooltip_text(format!("Measured by {sensor}"));
                } else if dash.hovered() {
                    ui.ctx().request_repaint_after_secs((DASH_HOVER_S - (now - since)) as f32);
                }
            }
        });
    });
}

#[cfg(test)]
mod toggle_tests {
    use super::*;

    /// (clicks on parts, cards open after them): a second click on a part
    /// closes its card; others stay.
    #[test]
    fn a_click_on_an_open_cards_part_closes_it() {
        let table: [(&[u32], &[u32]); 4] = [(&[3], &[3]), (&[3, 3], &[]), (&[3, 5], &[3, 5]), (&[3, 5, 3], &[5])];
        for (clicks, want) in table {
            let mut cards = PartCards::default();
            for p in clicks {
                cards.open(VesselId(1), *p, From::Bar);
            }
            let open: Vec<u32> = cards.open.iter().map(|c| c.part).collect();
            assert_eq!(open, want, "{clicks:?}");
        }
    }
}
