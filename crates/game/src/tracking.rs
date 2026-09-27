//! Vessel switching and the tracking station (D050).
//!
//! `[` / `]` cycle the active vessel. F7 opens the tracking station, a full
//! screen (KSP-like): the object list on the left (vessels grouped by status
//! with track toggles, bodies as a star → planet → moon tree), details and
//! actions (Focus, Switch to, Delete with confirmation) for the selection,
//! and the map, where tracked vessels stay in map view whatever their size
//! (`map_view::Object::pinned`) and clicking an icon selects and focuses it.
//!
//! Vessels get stable ids here (a Vec parallel to `SimState::fleet`), so the
//! tracked set survives deleting other vessels. Only tracked vessels get map
//! orbit lines (the active vessel always does).

use crate::camera::{self, CameraRig, Focus};
use crate::commands::{GameCommand, InputContext, Keys};
use crate::format::{distance as fmt_dist, duration as fmt_duration};
use crate::state::{Prediction, SimState, WARP_LEVELS};
use bevy::prelude::*;
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
    pub selected: Option<Selection>,
    /// A vessel waiting for "delete for good?".
    confirm_delete: Option<usize>,
}

impl TrackingStation {
    /// On leaving, the flight camera follows the active vessel (after a
    /// switch).
    pub fn follow_active_on_leave(&mut self) {
        if let Some(saved) = self.saved.as_mut() {
            saved.0 = Focus::Ship;
        }
    }

    /// Forgets the selection and any pending delete (their indices may now
    /// name other vessels).
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.confirm_delete = None;
    }
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
    ctx: Res<InputContext>,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
    mut tracked: ResMut<Tracked>,
    mut ts: ResMut<TrackingStation>,
    mut commands: MessageWriter<GameCommand>,
) {
    tracked.sync(sim.fleet.len());
    if ctx.allows(Keys::Menus) && keys.just_pressed(KeyCode::F7) {
        ts.open = !ts.open;
    }
    if ctx.allows(Keys::Flight) {
        let n = sim.fleet.len();
        if keys.just_pressed(KeyCode::BracketRight) {
            commands.write(GameCommand::Switch((sim.active + 1) % n));
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            commands.write(GameCommand::Switch((sim.active + n - 1) % n));
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
        let (anchor, r, v) = vessel.state_at(&sim.world, sim.clock);
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
    Select(Selection),
    FocusVessel(usize),
    Switch(usize),
    AskDelete(usize),
    Delete(usize),
    Track(usize, bool),
    FocusBody(NodeId),
}

/// What is selected in the station's list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Vessel(usize),
    Body(NodeId),
}

/// The bodies as a tree for the station's list: each body after its display
/// primary, children ordered by distance from it, with their depth (star 0,
/// planets 1, moons 2).
pub fn body_tree(world: &sim::world::World, t: sim::time::Epoch) -> Vec<(NodeId, usize)> {
    let eph = &world.eph;
    let bodies: Vec<NodeId> = eph.bodies().filter(|&n| world.source(n).is_some_and(|s| s.physical.is_some())).collect();
    let snap = world.snapshot(t);
    let children = |p: Option<NodeId>| {
        let mut c: Vec<NodeId> = bodies.iter().copied().filter(|&n| crate::relations::primary(eph, n) == p).collect();
        c.sort_by(|&a, &b| {
            let d = |n: NodeId| p.map_or(0.0, |p| snap.relative_r(n, p).length());
            d(a).total_cmp(&d(b))
        });
        c
    };
    let mut out = Vec::new();
    let mut stack: Vec<(NodeId, usize)> = children(None).into_iter().rev().map(|n| (n, 0)).collect();
    while let Some((n, depth)) = stack.pop() {
        out.push((n, depth));
        stack.extend(children(Some(n)).into_iter().rev().map(|c| (c, depth + 1)));
    }
    out
}

/// A vessel's group in the station's list.
fn group(phase: &Phase) -> &'static str {
    match phase {
        Phase::Landed { .. } => "Landed",
        Phase::Crashed { .. } => "Debris",
        Phase::Powered { .. } | Phase::Coasting { .. } => "In flight",
    }
}

/// The full-screen tracking station: the object list on the left (vessels
/// grouped by status, bodies as a tree), the selection's details and
/// actions, and the map. No flight HUD.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
    mut tracked: ResMut<Tracked>,
    mut ts: ResMut<TrackingStation>,
    mut commands: MessageWriter<GameCommand>,
) -> Result {
    if !ts.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut actions = Vec::new();
    let dim = crate::interface::theme::DIM;
    let accent = crate::interface::theme::ACCENT;
    let height = ctx.content_rect().height() - 16.0;
    egui::Window::new("tracking_station")
        .title_bar(false)
        .resizable(false)
        .movable(false)
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .fixed_size([330.0, height])
        .frame(crate::interface::theme::panel_frame().corner_radius(0))
        .show(ctx, |ui| {
            // The list runs the full height of the screen.
            ui.set_min_height(height - 16.0);
            ui.label(egui::RichText::new("TRACKING STATION").size(18.0).color(accent));
            let (y, mo, d, h, mi, _) = sim.clock.to_calendar();
            ui.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02} TDB   warp {}x", WARP_LEVELS[sim.warp]));
            if ui.button("Back to flight (F7)").clicked() {
                actions.push(Action::Close);
            }
            ui.separator();
            egui::ScrollArea::vertical().max_height(ui.available_height() * 0.62).show(ui, |ui| {
                for g in ["In flight", "Landed", "Debris"] {
                    let members: Vec<usize> =
                        (0..sim.fleet.len()).filter(|&i| group(&sim.fleet[i].phase) == g).collect();
                    if members.is_empty() {
                        continue;
                    }
                    ui.label(egui::RichText::new(format!("{g} ({})", members.len())).small().color(dim));
                    for i in members {
                        ui.horizontal(|ui| {
                            let mut on = tracked.is_tracked(i);
                            if ui.checkbox(&mut on, "").on_hover_text("Track: orbit line and icon on the map").changed()
                            {
                                actions.push(Action::Track(i, on));
                            }
                            let name =
                                if i == sim.active { format!("{} (active)", tracked.name(i)) } else { tracked.name(i) };
                            if ui.selectable_label(ts.selected == Some(Selection::Vessel(i)), name).clicked() {
                                actions.push(Action::Select(Selection::Vessel(i)));
                            }
                        });
                    }
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Bodies").small().color(dim));
                for (node, depth) in body_tree(&sim.world, sim.clock) {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0 * depth as f32);
                        let name = &sim.world.eph.node(node).name;
                        if ui.selectable_label(ts.selected == Some(Selection::Body(node)), name).clicked() {
                            actions.push(Action::Select(Selection::Body(node)));
                            actions.push(Action::FocusBody(node));
                        }
                    });
                }
            });
            ui.separator();
            match ts.selected {
                Some(Selection::Vessel(i)) if i < sim.fleet.len() => {
                    vessel_details(ui, &sim, &tracked, &ts, i, &mut actions)
                }
                Some(Selection::Body(node)) => {
                    ui.strong(&sim.world.eph.node(node).name);
                    if let Some((p, o)) = crate::relations::body_orbit(&sim.world.eph, sim.clock, node) {
                        ui.monospace(format!("orbits {}", sim.world.eph.node(p).name));
                        ui.monospace(format!("a {}", fmt_dist(o.elements.a)));
                        if let Some(t) = o.period() {
                            ui.monospace(format!("period {}", fmt_duration(t)));
                        }
                    }
                    if ui.button("Focus").clicked() {
                        actions.push(Action::FocusBody(node));
                    }
                }
                _ => {
                    ui.label(egui::RichText::new("Select a vessel or body, or click its icon on the map.").color(dim));
                }
            }
        });
    for a in actions {
        match a {
            Action::Close => ts.open = false,
            Action::Select(s) => {
                ts.selected = Some(s);
                ts.confirm_delete = None;
            }
            Action::FocusVessel(i) => focus_vessel(&sim, &mut rig, i),
            Action::Switch(i) => {
                commands.write(GameCommand::Switch(i));
                ts.open = false;
            }
            Action::AskDelete(i) => ts.confirm_delete = Some(i),
            Action::Delete(i) => {
                commands.write(GameCommand::Delete(i));
            }
            Action::Track(i, on) => tracked.set_tracked(i, on),
            Action::FocusBody(node) => camera::focus_body(&mut rig, &sim, node),
        }
    }
    Ok(())
}

/// Focus the station camera on vessel `i`, from a few times its distance
/// to the body it orbits.
pub fn focus_vessel(sim: &SimState, rig: &mut CameraRig, i: usize) {
    let primary = camera::nearest_body_to(sim, i);
    let (anchor, r, _) = sim.fleet[i].state_at(&sim.world, sim.clock);
    let from = primary.map_or(1.0e7, |p| (r - sim.world.snapshot(sim.clock).relative(p, anchor).r).length());
    rig.focus = Focus::Vessel(i);
    rig.distance = 3.0 * from;
}

fn vessel_details(
    ui: &mut egui::Ui,
    sim: &SimState,
    tracked: &Tracked,
    ts: &TrackingStation,
    i: usize,
    actions: &mut Vec<Action>,
) {
    let active = i == sim.active;
    let info = vessel_info(sim, i);
    ui.strong(tracked.name(i));
    ui.monospace(format!("status {}", info.status));
    ui.monospace(format!("about  {}", info.primary.map_or_else(|| "-".into(), |p| sim.world.eph.node(p).name.clone())));
    if let Some((pe, ap)) = info.apsides {
        let ap = if ap.is_finite() { fmt_dist(ap) } else { "escape".into() };
        ui.monospace(format!("Pe {}   Ap {}", fmt_dist(pe), ap));
    }
    if let Some(p) = info.period {
        ui.monospace(format!("period {}", fmt_duration(p)));
    }
    ui.horizontal(|ui| {
        if ui.button("Focus").clicked() {
            actions.push(Action::FocusVessel(i));
        }
        if ui.add_enabled(!active, egui::Button::new("Switch to")).clicked() {
            actions.push(Action::Switch(i));
        }
        if ui.add_enabled(!active, egui::Button::new("Delete")).on_hover_text("Delete this vessel").clicked() {
            actions.push(Action::AskDelete(i));
        }
    });
    if ts.confirm_delete == Some(i) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Delete it for good?").color(crate::interface::theme::WARN));
            if ui.button("Delete").clicked() {
                actions.push(Action::Delete(i));
            }
            if ui.button("Cancel").clicked() {
                actions.push(Action::Select(Selection::Vessel(i)));
            }
        });
    }
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
    fn bodies_form_a_star_planet_moon_tree() {
        let sim = SimState::new();
        let tree = body_tree(&sim.world, sim.clock);
        let name = |k: usize| sim.world.eph.node(tree[k].0).name.as_str();
        assert_eq!((name(0), tree[0].1), ("Sun", 0));
        let earth = tree.iter().position(|(n, _)| sim.world.eph.node(*n).name == "Earth").expect("Earth");
        assert_eq!(tree[earth].1, 1);
        assert_eq!((name(earth + 1), tree[earth + 1].1), ("Moon", 2), "the Moon right under Earth");
        // Planets in order of distance from the Sun.
        let planets: Vec<&str> =
            tree.iter().filter(|(_, d)| *d == 1).map(|(n, _)| sim.world.eph.node(*n).name.as_str()).collect();
        assert_eq!(&planets[..4], &["Mercury", "Venus", "Earth", "Mars"]);
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
