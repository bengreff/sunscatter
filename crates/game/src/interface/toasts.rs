//! Notifications (D111): every message of the game (saves, loads, errors,
//! commands sent, the ships' events) goes into one bounded history, shown
//! in a scrollable window in the bottom-left corner under the left dock,
//! newest at the bottom, a new one bright for a few seconds. One about a
//! vessel ends with a link: a click switches to that vessel, or opens the
//! systems screen for the active vessel's own. The window
//! folds to its header; outside the flight view it shows only while a
//! message is new. The history and the window's rules are pure, tested
//! below; [`draw`] wires them up.

use super::{layout, theme};
use crate::commands::GameCommand;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::vessel::VesselId;
use std::collections::VecDeque;

/// How long a message stays new (s of real time).
pub const FRESH_SECONDS: f64 = 4.0;
/// How many messages the history keeps (the oldest go first).
pub const HISTORY: usize = 200;
/// The window's egui area (the dock ends above it).
pub const AREA: &str = "notifications";
/// The list's height before it scrolls (logical px, about six lines).
pub const LIST_HEIGHT: f32 = 110.0;

/// Where a message leads. (A link to a layer of the systems screen waits
/// for events that say which system they come from.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Vessel(VesselId),
}

impl Link {
    /// As shown after the message.
    pub fn label(self) -> String {
        match self {
            Link::Vessel(id) => crate::tracking::vessel_name(id),
        }
    }
}

/// One message.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    /// App time (s) it came.
    pub at: f64,
    pub link: Option<Link>,
}

/// The history, and whether the window is folded.
#[derive(Resource, Default)]
pub struct Toasts {
    items: VecDeque<Toast>,
    /// How many messages have come since the game started (the newest's
    /// number): an agent reads those after the last it saw ([`Toasts::since`]).
    count: u64,
    pub folded: bool,
}

impl Toasts {
    /// Adds a message without a link (logged too).
    pub fn push(&mut self, now: f64, text: impl Into<String>, error: bool) {
        self.push_linked(now, text, error, None);
    }

    /// Adds a message leading to `link` (logged too); the oldest goes when
    /// the history is full.
    pub fn push_linked(&mut self, now: f64, text: impl Into<String>, error: bool, link: Option<Link>) {
        let text = text.into();
        let to = link.map(|l| format!(" ({})", l.label())).unwrap_or_default();
        if error {
            warn!("{text}{to}");
        } else {
            info!("{text}{to}");
        }
        if self.items.len() == HISTORY {
            self.items.pop_front();
        }
        self.items.push_back(Toast { text, error, at: now, link });
        self.count += 1;
    }

    /// How many messages have come (the newest's number, from 1).
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The messages still kept that came after number `after`, oldest
    /// first, each with its number (the MCP `notifications` tool, D044).
    pub fn since(&self, after: u64) -> impl Iterator<Item = (u64, &Toast)> {
        let first = self.count - self.items.len() as u64 + 1;
        (first..).zip(self.items.iter()).filter(move |(n, _)| *n > after)
    }

    /// The history, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &Toast> {
        self.items.iter()
    }

    /// The newest message if it is still new at `now`.
    pub fn newest_fresh(&self, now: f64) -> Option<&Toast> {
        self.items.back().filter(|t| fresh(t, now))
    }
}

/// Whether `t` is still new at `now`.
pub fn fresh(t: &Toast, now: f64) -> bool {
    now - t.at < FRESH_SECONDS
}

/// What a click on a link does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    Switch(VesselId),
    /// The active vessel's systems screen.
    Systems,
}

/// What a click on `link` does while `active` is flown: another vessel is
/// switched to; the active vessel's own message opens its systems screen.
pub fn go(link: Link, active: VesselId) -> Go {
    match link {
        Link::Vessel(id) if id != active => Go::Switch(id),
        Link::Vessel(_) => Go::Systems,
    }
}

/// A link's label and what its hover says.
pub fn link_text(link: Link, active: VesselId) -> (String, String) {
    let label = format!("{} ›", link.label());
    let hover = match go(link, active) {
        Go::Switch(id) => format!("Switch to {}", crate::tracking::vessel_name(id)),
        Go::Systems => "Open the systems screen".to_string(),
    };
    (label, hover)
}

/// How the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Hidden,
    /// The header, and the newest message while it is new.
    Folded,
    /// The header and the scrolling history.
    Open,
}

/// How the window shows: never with no message; in the flight view folded
/// or open as the player left it; elsewhere (the tracking station, the
/// systems screen) folded, and only while a message is new.
pub fn shown(flight: bool, folded: bool, empty: bool, fresh: bool) -> Shown {
    match (empty, flight, folded, fresh) {
        (true, ..) => Shown::Hidden,
        (false, false, _, false) => Shown::Hidden,
        (false, false, _, true) | (false, true, true, _) => Shown::Folded,
        (false, true, false, _) => Shown::Open,
    }
}

/// The window, bottom left, and what its links ask.
pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    sim: Res<crate::state::SimState>,
    station: Res<crate::tracking::TrackingStation>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    mut toasts: ResMut<Toasts>,
    mut cmds: MessageWriter<GameCommand>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    let flight = !station.open && !systems.open;
    let how = shown(flight, toasts.folded, toasts.items.is_empty(), toasts.newest_fresh(now).is_some());
    if how == Shown::Hidden {
        return Ok(());
    }
    let active = sim.ship().id();
    let clicked = window(ctx, &mut toasts, how, now, active);
    match clicked.map(|l| go(l, active)) {
        Some(Go::Switch(id)) => {
            cmds.write(GameCommand::Switch(id));
        }
        Some(Go::Systems) => systems.open_at(None, None),
        None => {}
    }
    Ok(())
}

/// Draws the window as `how` says; returns the link clicked.
pub fn window(ctx: &egui::Context, toasts: &mut Toasts, how: Shown, now: f64, active: VesselId) -> Option<Link> {
    let mut clicked = None;
    egui::Area::new(egui::Id::new(AREA))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_BOTTOM, [layout::MARGIN, -layout::MARGIN])
        .movable(false)
        .show(ctx, |ui| {
            // An area lays out within its remembered size: room for the
            // list to grow to its height.
            ui.set_max_height(LIST_HEIGHT + 60.0);
            theme::panel_frame().inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                ui.set_width(layout::DOCK_WIDTH - 2.0 * 6.0 - 2.0);
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
                header(ui, toasts, how);
                let list: Vec<&Toast> = match how {
                    Shown::Open => toasts.history().collect(),
                    _ => toasts.newest_fresh(now).into_iter().collect(),
                };
                let scroll = egui::ScrollArea::vertical().max_height(LIST_HEIGHT).auto_shrink([false, true]);
                scroll.stick_to_bottom(true).show(ui, |ui| {
                    for t in list {
                        if let Some(link) = line(ui, t, now, active) {
                            clicked = Some(link);
                        }
                    }
                });
            });
        });
    clicked
}

/// The window's title, the count and the fold switch (the window grows
/// upwards: ⏶ shows the history, ⏷ folds it; the proportional
/// font has no ▲ ▼, the icon font has these).
fn header(ui: &mut egui::Ui, toasts: &mut Toasts, how: Shown) {
    ui.horizontal(|ui| {
        let count = toasts.items.len();
        ui.label(egui::RichText::new(format!("NOTIFICATIONS  {count}")).small().color(theme::DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (glyph, tip) = if how == Shown::Open { ("⏷", "Fold") } else { ("⏶", "Show the history") };
            let b = ui.add(egui::Button::new(egui::RichText::new(glyph).small().color(theme::DIM)).frame(false));
            if b.on_hover_text(tip).clicked() {
                toasts.folded = how == Shown::Open;
            }
        });
    });
}

/// One message: its text, bright while new, then its link (a click
/// returns it).
fn line(ui: &mut egui::Ui, t: &Toast, now: f64, active: VesselId) -> Option<Link> {
    let new = fresh(t, now);
    let colour = match (t.error, new) {
        (true, _) => theme::WARN,
        (false, true) => theme::TEXT,
        (false, false) => theme::DIM,
    };
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(&t.text).small().color(colour));
        if let Some(link) = t.link {
            let (label, hover) = link_text(link, active);
            let text = egui::RichText::new(label).small().color(theme::ACCENT);
            if ui.add(egui::Button::new(text).frame(false)).on_hover_text(hover).clicked() {
                clicked = Some(link);
            }
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_is_bounded_and_the_newest_is_new_for_a_while() {
        let mut q = Toasts::default();
        q.push(0.0, "saved", false);
        q.push_linked(1.0, "Stage separation", false, Some(Link::Vessel(VesselId(2))));
        assert_eq!(q.history().count(), 2);
        assert_eq!(q.newest_fresh(2.0).map(|t| t.link), Some(Some(Link::Vessel(VesselId(2)))));
        assert_eq!(q.newest_fresh(1.0 + FRESH_SECONDS + 0.1), None);
        assert_eq!(q.history().count(), 2, "kept after it is no longer new");
        for k in 0..HISTORY + 3 {
            q.push(20.0, format!("m{k}"), false);
        }
        let texts: Vec<&str> = q.history().map(|t| t.text.as_str()).collect();
        assert_eq!(texts.len(), HISTORY);
        assert_eq!((texts[0], texts[HISTORY - 1]), ("m3", "m202"), "oldest dropped first");
    }

    /// Messages are numbered as they come; an agent reads those after the
    /// last it saw, the oldest dropped from the history no longer there.
    #[test]
    fn messages_are_read_after_the_last_seen() {
        let mut q = Toasts::default();
        assert_eq!((q.count(), q.since(0).count()), (0, 0));
        for k in 1..=HISTORY + 2 {
            q.push(0.0, format!("m{k}"), false);
        }
        let read = |after: u64| q.since(after).map(|(n, t)| (n, t.text.clone())).collect::<Vec<_>>();
        // (after, the first and last numbers read, the first's text)
        let table = [(0, Some((3, 202)), "m3"), (200, Some((201, 202)), "m201"), (202, None, "")];
        for (after, numbers, first) in table {
            let got = read(after);
            let ends = got.first().zip(got.last()).map(|(a, b)| (a.0, b.0));
            assert_eq!(ends, numbers, "after {after}");
            assert!(got.windows(2).all(|w| w[1].0 == w[0].0 + 1), "after {after}: consecutive");
            assert_eq!(got.first().map_or("", |g| g.1.as_str()), first, "after {after}");
        }
        assert_eq!(q.count(), 202);
    }

    /// A link to another vessel switches to it; to the active vessel, opens
    /// the systems screen.
    #[test]
    fn links_switch_vessels_or_open_the_systems_screen() {
        let active = VesselId(1);
        // (link, go, label, hover)
        let table = [
            (Link::Vessel(VesselId(2)), Go::Switch(VesselId(2)), "Vessel 2 ›", "Switch to Vessel 2"),
            (Link::Vessel(active), Go::Systems, "Vessel 1 ›", "Open the systems screen"),
        ];
        for (link, want, label, hover) in table {
            assert_eq!(go(link, active), want, "{link:?}");
            assert_eq!(link_text(link, active), (label.to_string(), hover.to_string()), "{link:?}");
        }
    }

    /// Open, the list grows message by message to its height, then
    /// scrolls (an egui area kept it at the height it first had, a few
    /// lines); folded, the header and the newest message alone.
    #[test]
    fn the_window_grows_to_its_list_height() {
        use egui::{Pos2, RawInput, Rect};
        let pass = |ctx: &egui::Context, q: &mut Toasts, how: Shown| {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 900.0))),
                ..Default::default()
            };
            ctx.begin_pass(input);
            window(ctx, q, how, 1.0, VesselId(1));
            ctx.end_pass().drop_without_applying_deltas();
        };
        let rect = |ctx: &egui::Context| ctx.memory(|m| m.area_rect(egui::Id::new(AREA))).expect("drawn");
        let (ctx, mut q) = (egui::Context::default(), Toasts::default());
        for k in 0..30 {
            q.push_linked(0.0, format!("event {k}"), false, Some(Link::Vessel(VesselId(1))));
            pass(&ctx, &mut q, Shown::Open);
            pass(&ctx, &mut q, Shown::Open);
        }
        let open = rect(&ctx);
        assert!((open.bottom() - (900.0 - layout::MARGIN)).abs() < 1.0, "{open:?}");
        assert!(open.height() > LIST_HEIGHT && open.height() < LIST_HEIGHT + 50.0, "{open:?}");
        for _ in 0..3 {
            pass(&ctx, &mut q, Shown::Folded);
        }
        assert!(rect(&ctx).height() < 70.0, "{:?}", rect(&ctx));
    }

    #[test]
    fn the_window_shows_in_flight_and_elsewhere_only_while_a_message_is_new() {
        use Shown::*;
        // (flight, folded, empty, fresh, shown)
        let table = [
            (true, false, true, false, Hidden),
            (true, false, false, false, Open),
            (true, true, false, false, Folded),
            (true, true, false, true, Folded),
            (false, false, false, false, Hidden),
            (false, false, false, true, Folded),
            (false, true, true, false, Hidden),
        ];
        for (flight, folded, empty, fresh, want) in table {
            assert_eq!(shown(flight, folded, empty, fresh), want, "{flight} {folded} {empty} {fresh}");
        }
    }
}
