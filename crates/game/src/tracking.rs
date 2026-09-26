//! Vessel switching and the tracking station (D050).
//!
//! `[` / `]` cycle the active vessel. F7 opens the tracking station: the
//! camera pulled out to show the Moon's orbit, where tracked vessels stay in
//! map view whatever their size (`map_view::Object::pinned`). It lists vessels (orbit about the nearest body,
//! phase) with Track, Focus, Switch to and Delete, and bodies with Focus.
//!
//! Vessels get stable ids here (a Vec parallel to `SimState::fleet`), so the
//! tracked set survives deleting other vessels. Only tracked vessels get map
//! orbit lines (the active vessel always does).

use crate::camera::{self, CameraRig, Focus};
use crate::hud::fmt_dist;
use crate::map::fmt_duration;
use crate::state::{Prediction, SimState, WARP_LEVELS};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};
use sim::frame::NodeId;
use sim::vessel::{Controls, Phase};
use std::collections::HashSet;

/// Camera distance when the tracking station opens (m): Earth with the
/// whole of the Moon's orbit in view, far enough out that the Moon is in map
/// view (its sprite under 1 px, D054) so its orbit line shows.
const STATION_DISTANCE: f64 = 2.5e9;

/// Stable vessel ids (parallel to `SimState::fleet`) and the tracked set.
#[derive(Resource)]
pub struct Tracked {
    ids: Vec<u64>,
    next: u64,
    hidden: HashSet<u64>,
}

impl Default for Tracked {
    fn default() -> Self {
        Tracked { ids: Vec::new(), next: 1, hidden: HashSet::new() }
    }
}

impl Tracked {
    /// Follows the fleet's length: vessels are appended at the end and
    /// truncated from the end everywhere except [`delete_vessel`].
    pub fn sync(&mut self, fleet_len: usize) {
        self.ids.truncate(fleet_len);
        while self.ids.len() < fleet_len {
            self.ids.push(self.next);
            self.next += 1;
        }
    }

    /// Forgets every id (after loading a save); all vessels are tracked.
    pub fn reset(&mut self, fleet_len: usize) {
        *self = Tracked::default();
        self.sync(fleet_len);
    }

    pub fn id(&self, i: usize) -> Option<u64> {
        self.ids.get(i).copied()
    }

    pub fn name(&self, i: usize) -> String {
        self.id(i).map_or_else(|| format!("Vessel #{}", i + 1), |id| format!("Vessel {id}"))
    }

    /// Whether vessel `i` gets a map orbit line (vessels are tracked by default).
    pub fn is_tracked(&self, i: usize) -> bool {
        self.id(i).is_none_or(|id| !self.hidden.contains(&id))
    }

    pub fn set_tracked(&mut self, i: usize, tracked: bool) {
        if let Some(id) = self.id(i) {
            if tracked {
                self.hidden.remove(&id);
            } else {
                self.hidden.insert(id);
            }
        }
    }

    fn remove(&mut self, i: usize) {
        if i < self.ids.len() {
            let id = self.ids.remove(i);
            self.hidden.remove(&id);
        }
    }
}

/// The tracking station screen. Set `open`; the camera follows on the next
/// input stage.
#[derive(Resource, Default)]
pub struct TrackingStation {
    pub open: bool,
    applied: bool,
    /// The flight camera to restore on leaving.
    saved: Option<(Focus, f64, f64, f64)>,
}

/// Makes vessel `i` active. Its controls start from throttle 0 with SAS on;
/// the camera follows it.
pub fn switch_to(sim: &mut SimState, rig: &mut CameraRig, pred: &mut Prediction, i: usize) {
    if i >= sim.fleet.len() || i == sim.active {
        return;
    }
    sim.active = i;
    sim.controls = Controls { sas: true, ..Default::default() };
    *pred = Prediction::default();
    rig.focus = Focus::Ship;
}

/// Removes a non-active vessel ("delete debris"), keeping the active index,
/// ids and camera focus pointing at the same vessels.
pub fn delete_vessel(sim: &mut SimState, tracked: &mut Tracked, rig: &mut CameraRig, i: usize) -> bool {
    if i >= sim.fleet.len() || i == sim.active {
        return false;
    }
    sim.fleet.remove(i);
    tracked.remove(i);
    if sim.active > i {
        sim.active -= 1;
    }
    if let Focus::Vessel(j) = rig.focus {
        rig.focus = match j.cmp(&i) {
            std::cmp::Ordering::Less => Focus::Vessel(j),
            std::cmp::Ordering::Equal => Focus::Ship,
            std::cmp::Ordering::Greater => Focus::Vessel(j - 1),
        };
    }
    true
}

/// Keys (F7, `[`, `]`), id bookkeeping, and entering/leaving the station.
#[allow(clippy::too_many_arguments)]
pub fn update(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Res<EguiWantsInput>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut pred: ResMut<Prediction>,
    mut tracked: ResMut<Tracked>,
    mut ts: ResMut<TrackingStation>,
) {
    tracked.sync(sim.fleet.len());
    if !egui.wants_any_keyboard_input() {
        if keys.just_pressed(KeyCode::F7) {
            ts.open = !ts.open;
        }
        let n = sim.fleet.len();
        if keys.just_pressed(KeyCode::BracketRight) {
            let i = (sim.active + 1) % n;
            switch_to(&mut sim, &mut rig, &mut pred, i);
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            let i = (sim.active + n - 1) % n;
            switch_to(&mut sim, &mut rig, &mut pred, i);
        }
    }
    if let Focus::Vessel(i) = rig.focus {
        if i >= sim.fleet.len() {
            rig.focus = Focus::Ship;
        }
    }
    if ts.open != ts.applied {
        ts.applied = ts.open;
        if ts.open {
            ts.saved = Some((rig.focus, rig.yaw, rig.pitch, rig.distance));
            if let Some(earth) = sim.world.find("Earth").map(|s| s.node) {
                camera::focus_body(&mut rig, &sim, earth);
                rig.distance = STATION_DISTANCE;
                rig.pitch = 1.1;
            }
        } else if let Some((focus, yaw, pitch, distance)) = ts.saved.take() {
            let focus = if matches!(focus, Focus::Vessel(_)) { Focus::Ship } else { focus };
            (rig.focus, rig.yaw, rig.pitch, rig.distance) = (focus, yaw, pitch, distance);
        }
    }
}

/// Orbit summary of a vessel about its nearest body.
pub struct VesselInfo {
    pub primary: Option<NodeId>,
    pub status: String,
    /// Periapsis and apoapsis altitudes above the equatorial radius (m);
    /// apoapsis is infinite on escape.
    pub apsides: Option<(f64, f64)>,
    pub period: Option<f64>,
}

pub fn vessel_info(sim: &SimState, i: usize) -> VesselInfo {
    let vessel = &sim.fleet[i];
    let primary = camera::nearest_body_to(sim, i);
    let name = |n: NodeId| sim.world.eph.node(n).name.clone();
    let status = match &vessel.phase {
        Phase::Landed { body, .. } => format!("landed on {}", name(*body)),
        Phase::Crashed { body, .. } => format!("crashed on {}", name(*body)),
        Phase::Powered { .. } => "powered".into(),
        Phase::Coasting { .. } => "coasting".into(),
    };
    let mut info = VesselInfo { primary, status, apsides: None, period: None };
    let flying = matches!(vessel.phase, Phase::Powered { .. } | Phase::Coasting { .. });
    if let (true, Some(body)) = (flying, primary.and_then(|p| sim.world.source(p))) {
        let (anchor, r, v) = vessel.state(&sim.world);
        if let Some(o) = crate::relations::orbit_about(&sim.world, sim.clock, anchor, r, v, body.node) {
            let radius = body.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            info.apsides = Some((o.elements.periapsis() - radius, o.elements.apoapsis() - radius));
            info.period = o.period();
        }
    }
    info
}

enum Action {
    Close,
    FocusVessel(usize),
    Switch(usize),
    Delete(usize),
    Track(usize, bool),
    FocusBody(NodeId),
}

/// The tracking station window.
pub fn draw(
    mut contexts: EguiContexts,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut pred: ResMut<Prediction>,
    mut tracked: ResMut<Tracked>,
    mut ts: ResMut<TrackingStation>,
) -> Result {
    if !ts.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut actions = Vec::new();
    egui::Area::new("tracking_title".into()).anchor(egui::Align2::CENTER_TOP, [0.0, 10.0]).show(ctx, |ui| {
        ui.label(egui::RichText::new("TRACKING STATION").size(18.0).color(egui::Color32::from_rgb(170, 200, 230)));
    });
    egui::Window::new("Tracking station (F7)")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 50.0])
        .default_width(560.0)
        .resizable(false)
        .show(ctx, |ui| {
            let (y, mo, d, h, mi, _) = sim.clock.to_calendar();
            ui.horizontal(|ui| {
                ui.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02} TDB   warp {}x", WARP_LEVELS[sim.warp]));
                if ui.button("Back to flight").clicked() {
                    actions.push(Action::Close);
                }
            });
            ui.separator();
            ui.strong("Vessels");
            egui::Grid::new("ts_vessels").striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                for label in ["track", "vessel", "about", "Pe / Ap", "status", ""] {
                    ui.label(egui::RichText::new(label).weak());
                }
                ui.end_row();
                for i in 0..sim.fleet.len() {
                    vessel_row(ui, &sim, &tracked, i, &mut actions);
                }
            });
            ui.separator();
            ui.strong("Bodies");
            ui.horizontal_wrapped(|ui| {
                for node in sim.world.eph.bodies() {
                    if sim.world.source(node).and_then(|s| s.physical.as_ref()).is_some()
                        && ui.button(&sim.world.eph.node(node).name).on_hover_text("Focus").clicked()
                    {
                        actions.push(Action::FocusBody(node));
                    }
                }
            });
        });
    for a in actions {
        match a {
            Action::Close => ts.open = false,
            Action::FocusVessel(i) => {
                let primary = camera::nearest_body_to(&sim, i);
                let (anchor, r, _) = sim.fleet[i].state(&sim.world);
                let from =
                    primary.map_or(1.0e7, |p| (r - sim.world.snapshot(sim.clock).relative(p, anchor).r).length());
                rig.focus = Focus::Vessel(i);
                rig.distance = 3.0 * from;
            }
            Action::Switch(i) => {
                switch_to(&mut sim, &mut rig, &mut pred, i);
                ts.open = false;
                // Leaving restores the flight camera: follow the new vessel.
                if let Some(saved) = ts.saved.as_mut() {
                    saved.0 = Focus::Ship;
                }
            }
            Action::Delete(i) => {
                delete_vessel(&mut sim, &mut tracked, &mut rig, i);
            }
            Action::Track(i, on) => tracked.set_tracked(i, on),
            Action::FocusBody(node) => camera::focus_body(&mut rig, &sim, node),
        }
    }
    Ok(())
}

fn vessel_row(ui: &mut egui::Ui, sim: &SimState, tracked: &Tracked, i: usize, actions: &mut Vec<Action>) {
    let active = i == sim.active;
    let info = vessel_info(sim, i);
    let mut on = tracked.is_tracked(i);
    if ui.checkbox(&mut on, "").on_hover_text("Orbit line in map mode").changed() {
        actions.push(Action::Track(i, on));
    }
    let name = tracked.name(i);
    if active {
        ui.label(egui::RichText::new(format!("{name} (active)")).color(egui::Color32::from_rgb(255, 215, 80)));
    } else {
        ui.label(name);
    }
    ui.monospace(info.primary.map_or_else(|| "-".into(), |p| sim.world.eph.node(p).name.clone()));
    match info.apsides {
        Some((pe, ap)) => {
            let ap = if ap.is_finite() { fmt_dist(ap) } else { "escape".into() };
            let text = ui.monospace(format!("{} / {}", fmt_dist(pe), ap));
            if let Some(p) = info.period {
                text.on_hover_text(format!("period {}", fmt_duration(p)));
            }
        }
        None => {
            ui.monospace("-");
        }
    }
    ui.monospace(info.status);
    ui.horizontal(|ui| {
        if ui.small_button("Focus").clicked() {
            actions.push(Action::FocusVessel(i));
        }
        if ui.add_enabled(!active, egui::Button::new("Switch to").small()).clicked() {
            actions.push(Action::Switch(i));
        }
        let delete = ui.add_enabled(!active, egui::Button::new("Delete").small());
        if delete.on_hover_text("Delete this vessel (debris)").clicked() {
            actions.push(Action::Delete(i));
        }
    });
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_across_deletion() {
        let mut t = Tracked::default();
        t.sync(3);
        assert_eq!((t.id(0), t.id(1), t.id(2)), (Some(1), Some(2), Some(3)));
        t.set_tracked(2, false);
        t.remove(1);
        assert_eq!((t.id(0), t.id(1)), (Some(1), Some(3)));
        assert!(t.is_tracked(0) && !t.is_tracked(1));
        t.sync(3);
        assert_eq!(t.id(2), Some(4), "new vessels never reuse an id");
        t.reset(2);
        assert_eq!((t.id(0), t.id(1)), (Some(1), Some(2)));
        assert!(t.is_tracked(1));
    }

    #[test]
    fn switching_and_deleting_keep_indices_consistent() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(3);
        let (mut rig, mut pred, mut tracked) = (CameraRig::default(), Prediction::default(), Tracked::default());
        tracked.sync(sim.fleet.len());
        sim.controls.throttle = 0.7;
        switch_to(&mut sim, &mut rig, &mut pred, 2);
        assert_eq!(sim.active, 2);
        assert_eq!(sim.controls.throttle, 0.0);
        assert!(sim.controls.sas);
        rig.focus = Focus::Vessel(3);
        assert!(!delete_vessel(&mut sim, &mut tracked, &mut rig, 2), "the active vessel cannot be deleted");
        assert!(delete_vessel(&mut sim, &mut tracked, &mut rig, 0));
        assert_eq!(sim.fleet.len(), 3);
        assert_eq!(sim.active, 1);
        assert_eq!(rig.focus, Focus::Vessel(2));
        assert_eq!((tracked.id(0), tracked.id(1), tracked.id(2)), (Some(2), Some(3), Some(4)));
        let info = vessel_info(&sim, 1);
        let (pe, ap) = info.apsides.expect("coasting in orbit");
        assert!(info.status == "coasting" && pe > 300_000.0 && ap < 500_000.0, "{pe} {ap}");
    }
}
