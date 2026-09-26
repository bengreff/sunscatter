//! Messages: one queue of short toasts on the right edge (saves, loads,
//! errors), replacing ad-hoc labels. The queue is pure data.

use super::theme;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::VecDeque;

/// How long a toast stays (s), and how many show at once.
pub const TOAST_SECONDS: f64 = 4.0;
pub const MAX_TOASTS: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub text: String,
    pub error: bool,
    /// App time (s) when it disappears.
    pub until: f64,
}

#[derive(Resource, Default)]
pub struct Toasts {
    items: VecDeque<Toast>,
}

impl Toasts {
    /// Adds a message (logged too); the oldest goes when the queue is full.
    pub fn push(&mut self, now: f64, text: impl Into<String>, error: bool) {
        let text = text.into();
        if error {
            warn!("{text}");
        } else {
            info!("{text}");
        }
        if self.items.len() == MAX_TOASTS {
            self.items.pop_front();
        }
        self.items.push_back(Toast { text, error, until: now + TOAST_SECONDS });
    }

    /// The messages still showing at `now` (expired ones are dropped).
    pub fn live(&mut self, now: f64) -> impl Iterator<Item = &Toast> {
        self.items.retain(|t| t.until > now);
        self.items.iter()
    }
}

pub fn draw(mut contexts: EguiContexts, time: Res<Time>, mut toasts: ResMut<Toasts>) -> Result {
    let ctx = contexts.ctx_mut()?;
    let now = time.elapsed_secs_f64();
    let live: Vec<Toast> = toasts.live(now).cloned().collect();
    if live.is_empty() {
        return Ok(());
    }
    egui::Area::new("toasts".into()).anchor(egui::Align2::RIGHT_TOP, [-10.0, 90.0]).show(ctx, |ui| {
        for t in &live {
            theme::panel_frame().show(ui, |ui| {
                let color = if t.error { theme::WARN } else { theme::TEXT };
                ui.label(egui::RichText::new(&t.text).color(color));
            });
            ui.add_space(4.0);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toasts_expire_and_the_queue_is_bounded() {
        let mut q = Toasts::default();
        q.push(0.0, "saved", false);
        q.push(1.0, "failed", true);
        assert_eq!(q.live(2.0).count(), 2);
        assert_eq!(q.live(TOAST_SECONDS + 0.5).map(|t| t.text.as_str()).collect::<Vec<_>>(), ["failed"]);
        assert_eq!(q.live(10.0).count(), 0);
        for k in 0..8 {
            q.push(20.0, format!("m{k}"), false);
        }
        let texts: Vec<String> = q.live(20.0).map(|t| t.text.clone()).collect();
        assert_eq!(texts, ["m3", "m4", "m5", "m6", "m7"], "oldest dropped first");
    }
}
