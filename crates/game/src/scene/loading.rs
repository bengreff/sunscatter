//! The loading screen: at start and after every replacement of the game
//! (a new flight, a load, a revert: `GameCommand`'s replace path), a
//! full-screen view with a line naming what is loading, the clock held
//! (the simulation stage does not run, so the state flown is the state loaded),
//! until what the first view needs is ready: the terrain chunks around the
//! camera at their wanted level, the bodies' surface maps, the render
//! pipelines still compiling (Bevy's pipeline cache, counted in the render
//! world). The craft's meshes are built at once when the ship is drawn
//! (`scene::ships`); their pipelines are among those counted.
//!
//! The rule ([`ready`]) is a pure function of counts with a table test;
//! the systems only gather the counts and draw.

use crate::commands::GameCommand;
use crate::terrain::Terrain;
use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Frames drawn before the scene can be judged ready (the camera placed,
/// the first chunks asked for).
pub const MIN_FRAMES: u32 = 5;
/// Frames in a row with nothing loading before the scene is ready: chunks
/// installed one frame ask for their children the next.
pub const QUIET_FRAMES: u32 = 10;
/// The longest the loading screen stays (s), whatever is still loading.
pub const MAX_SECONDS: f64 = 60.0;

/// What is still loading, and for how long.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// Terrain chunks being built, surface maps loading, render pipelines
    /// compiling.
    pub chunks: usize,
    pub maps: usize,
    pub pipelines: usize,
    /// Frames since the loading began and frames in a row with nothing
    /// loading.
    pub frames: u32,
    pub quiet: u32,
    /// Seconds since the loading began (wall clock).
    pub seconds: f64,
}

impl Counts {
    /// Nothing loading this frame.
    pub fn idle(&self) -> bool {
        self.chunks == 0 && self.maps == 0 && self.pipelines == 0
    }

    /// The counts after one more frame with `chunks`, `maps` and
    /// `pipelines` loading, `dt` seconds later.
    pub fn next(&self, chunks: usize, maps: usize, pipelines: usize, dt: f64) -> Counts {
        let mut c = Counts { chunks, maps, pipelines, frames: self.frames + 1, seconds: self.seconds + dt, ..*self };
        c.quiet = if c.idle() { self.quiet + 1 } else { 0 };
        c
    }
}

/// The first view is ready: enough frames, then nothing loading for
/// [`QUIET_FRAMES`] in a row; or [`MAX_SECONDS`] passed.
pub fn ready(c: &Counts) -> bool {
    c.seconds >= MAX_SECONDS || (c.frames >= MIN_FRAMES && c.quiet >= QUIET_FRAMES)
}

/// The progress line: what is loading now.
pub fn progress(c: &Counts) -> String {
    let mut parts = Vec::new();
    if c.chunks > 0 {
        parts.push(format!("terrain: {} tiles", c.chunks));
    }
    if c.maps > 0 {
        parts.push(format!("surface maps: {}", c.maps));
    }
    if c.pipelines > 0 {
        parts.push(format!("shaders: {} compiling", c.pipelines));
    }
    if parts.is_empty() {
        "settling the view".into()
    } else {
        parts.join(" · ")
    }
}

/// Whether a command replaces the game (the loading screen follows it).
pub fn replaces(c: &GameCommand) -> bool {
    matches!(c, GameCommand::Restore(_) | GameCommand::Reset | GameCommand::NewFlight(_))
}

/// The loading screen's state: on at start.
#[derive(Resource, Debug)]
pub struct Loading {
    pub active: bool,
    pub counts: Counts,
}

impl Default for Loading {
    fn default() -> Self {
        Loading { active: true, counts: Counts::default() }
    }
}

/// The run condition of what waits for the scene (the clock, the demo).
pub fn idle(loading: Res<Loading>) -> bool {
    !loading.active
}

/// Render pipelines waiting to compile, written in the render world.
#[derive(Resource, Clone, Default)]
pub struct Pipelines(Arc<AtomicUsize>);

fn count_pipelines(cache: Res<PipelineCache>, shared: Res<Pipelines>) {
    shared.0.store(cache.waiting_pipelines().count(), Ordering::Relaxed);
}

/// Starts the loading after a replacement; gathers the counts; ends it
/// when [`ready`].
pub fn track(
    mut loading: ResMut<Loading>,
    mut commands: MessageReader<GameCommand>,
    terrain: Res<Terrain>,
    pipelines: Res<Pipelines>,
    time: Res<Time<Real>>,
) {
    if commands.read().any(replaces) {
        *loading = Loading::default();
    }
    if !loading.active {
        return;
    }
    let (chunks, maps) = terrain.loading();
    let waiting = pipelines.0.load(Ordering::Relaxed);
    loading.counts = loading.counts.next(chunks, maps, waiting, time.delta_secs_f64());
    if ready(&loading.counts) {
        loading.active = false;
        let c = loading.counts;
        info!("loading done: {} frames, {:.1} s", c.frames, c.seconds);
    }
}

/// The full-screen loading view in the interface's theme.
pub fn draw(mut contexts: EguiContexts, loading: Res<Loading>) -> Result {
    if !loading.active {
        return Ok(());
    }
    use crate::interface::theme;
    let ctx = contexts.ctx_mut()?;
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("loading")).order(egui::Order::Foreground).fixed_pos(egui::Pos2::ZERO).show(
        ctx,
        |ui| {
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_rgb(7, 10, 15));
            ui.allocate_rect(screen, egui::Sense::click_and_drag());
            let centre = screen.center();
            let painter = ui.painter();
            painter.text(
                centre - egui::vec2(0.0, 18.0),
                egui::Align2::CENTER_CENTER,
                "LOADING",
                egui::FontId::monospace(22.0),
                theme::ACCENT,
            );
            painter.text(
                centre + egui::vec2(0.0, 14.0),
                egui::Align2::CENTER_CENTER,
                progress(&loading.counts),
                egui::FontId::proportional(14.0),
                theme::DIM,
            );
        },
    );
    Ok(())
}

/// The loading screen's resources and systems.
pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        let shared = Pipelines::default();
        app.init_resource::<Loading>()
            .insert_resource(shared.clone())
            .configure_sets(Update, crate::Stage::Simulate.run_if(idle))
            .add_systems(Update, track.before(crate::Stage::Simulate))
            .add_systems(EguiPrimaryContextPass, draw.after(crate::interface::toasts::draw));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.insert_resource(shared).add_systems(Render, count_pipelines.in_set(RenderSystems::Cleanup));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(chunks: usize, maps: usize, pipelines: usize, frames: u32, quiet: u32, seconds: f64) -> Counts {
        Counts { chunks, maps, pipelines, frames, quiet, seconds }
    }

    #[test]
    fn ready_once_nothing_has_loaded_for_a_while_or_the_cap_passed() {
        for (c, want) in [
            (counts(0, 0, 0, 0, 0, 0.0), false),                           // the first frame
            (counts(0, 0, 0, MIN_FRAMES - 1, MIN_FRAMES - 1, 0.1), false), // too early to judge
            (counts(0, 0, 0, 40, QUIET_FRAMES, 1.0), true),
            (counts(0, 0, 0, 40, QUIET_FRAMES - 1, 1.0), false), // just went quiet
            (counts(12, 0, 0, 40, 0, 2.0), false),               // terrain building
            (counts(0, 1, 0, 40, 0, 2.0), false),                // a surface map
            (counts(0, 0, 3, 40, 0, 2.0), false),                // shaders compiling
            (counts(12, 1, 3, 4000, 0, MAX_SECONDS), true),      // the cap
        ] {
            assert_eq!(ready(&c), want, "{c:?}");
        }
    }

    #[test]
    fn quiet_frames_count_only_in_a_row() {
        let mut c = Counts::default();
        for _ in 0..QUIET_FRAMES + MIN_FRAMES {
            c = c.next(0, 0, 0, 0.016);
        }
        assert!(ready(&c));
        // One frame of new work starts the count again.
        let busy = c.next(4, 0, 0, 0.016);
        assert_eq!(busy.quiet, 0);
        assert!(!ready(&busy));
        assert_eq!(busy.frames, c.frames + 1);
    }

    #[test]
    fn the_line_names_what_is_loading() {
        assert_eq!(
            progress(&counts(12, 1, 3, 9, 0, 0.5)),
            "terrain: 12 tiles · surface maps: 1 · shaders: 3 compiling"
        );
        assert_eq!(progress(&counts(0, 0, 0, 9, 2, 0.5)), "settling the view");
    }

    #[test]
    fn replacements_show_it_other_commands_do_not() {
        assert!(replaces(&GameCommand::Reset));
        assert!(replaces(&GameCommand::NewFlight("falcon9-crew".into())));
        assert!(!replaces(&GameCommand::PauseTime));
        assert!(!replaces(&GameCommand::SetWarp(2)));
    }
}
