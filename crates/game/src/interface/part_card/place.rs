//! Where cards and parts are on screen: the click on the ship that opens a
//! part's card (`scene::pick`: the ray from the camera against the parts
//! as drawn, mechanisms turned), each ship card's part projected every
//! frame for its line, and where a card opened at a click goes
//! ([`beside`], pure, table-tested).

use super::{From, PartCards};
use crate::interface::{layout, stage_bar};
use crate::scene::{parts, pick, ships::ShipModels, ships::ShipVisual};
use crate::state::SimState;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;
use bevy_egui::input::EguiWantsInput;
use sim::craft::RenderMesh;
use std::sync::Arc;

/// Whether a card's row puts its value on a line of its own under its
/// label: when the label (`label` wide), a gap and the value (`value` wide)
/// do not fit the row's `width` (a long reading ran over its label, e.g.
/// the capsule's "pressure (highest)", interface-map.md finding 15).
pub fn value_below(label: f32, value: f32, width: f32) -> bool {
    label + layout::GAP + value > width
}

/// Where a card opened by a click at `at` goes (egui points): beside the
/// click, right of it unless that runs into the staging bar, and on screen.
pub fn beside(at: egui::Pos2, screen: egui::Rect, width: f32) -> egui::Pos2 {
    let room = screen.max.x - layout::MARGIN - stage_bar::WIDTH - layout::GAP;
    let x = if at.x + 24.0 + width <= room { at.x + 24.0 } else { at.x - 24.0 - width };
    let y = (at.y - 40.0).min(screen.max.y - 260.0).max(screen.min.y + layout::MARGIN);
    egui::pos2(x.max(screen.min.x + layout::MARGIN), y)
}

/// Where the cards opened from the staging bar go (their top-left
/// corners): stacked down from `top`, each as tall as `heights` says (last
/// frame's), `gap` apart, their right edge at `right`; a card that would
/// pass `bottom` starts a new column to the left, so none is pushed back
/// over another.
pub fn stack(heights: &[f32], top: f32, bottom: f32, right: f32, width: f32, gap: f32) -> Vec<egui::Pos2> {
    let (mut x, mut y) = (right - width, top);
    let mut out = Vec::with_capacity(heights.len());
    for &h in heights {
        if y > top && y + h > bottom {
            (x, y) = (x - width - gap, top);
        }
        out.push(egui::pos2(x, y));
        y += h + gap;
    }
    out
}

/// The active vessel's parts as drawn: (part id, mesh, transform in the
/// body axes: a mechanism's hinge at its angle now).
fn placed(v: &sim::vessel::Vessel, models: &mut ShipModels) -> Vec<(u32, Arc<RenderMesh>, Transform)> {
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { return Vec::new() };
    let craft = sim::craft::craft_of(&v.craft.id);
    let ids: Vec<u32> = craft.parts.iter().map(|p| p.id).collect();
    let on = parts::mechanisms(&def, &ids);
    let deployed = v.craft.id.deployed();
    (0..craft.parts.len())
        .map(|k| {
            let hinge = on[k].and_then(|m| {
                let (d, s) = (def.mechanisms.get(m)?, state.mechanisms.get(m)?);
                let built = parts::built_angle(&d.item, deployed.contains(&d.part));
                Some(parts::hinge(d.item.pos, d.item.axis, built, s.angle))
            });
            (ids[k], models.part_mesh(&craft, k), hinge.unwrap_or(Transform::IDENTITY))
        })
        .collect()
}

/// A left click on the active ship (not a camera drag, not over the
/// interface) opens the card of the part under it, beside the click.
#[allow(clippy::too_many_arguments)]
pub fn click(
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    egui_input: Res<EguiWantsInput>,
    window: Query<&Window, With<PrimaryWindow>>,
    sim: Res<SimState>,
    screens: (Res<crate::tracking::TrackingStation>, Res<crate::systems_screen::SystemsScreen>),
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ships: Query<(&ShipVisual, &GlobalTransform)>,
    mut models: ResMut<ShipModels>,
    mut cards: ResMut<PartCards>,
) {
    if screens.0.open || screens.1.open {
        cards.press = None;
        return;
    }
    let Ok(window) = window.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let now = time.elapsed_secs_f64();
    if buttons.just_pressed(MouseButton::Left) {
        cards.press = (!egui_input.wants_any_pointer_input()).then_some((now, cursor));
    }
    if !buttons.just_released(MouseButton::Left) {
        return;
    }
    if !cards.press.take().is_some_and(|(t0, p0)| pick::is_click(t0, p0, now, cursor)) {
        return;
    }
    let v = sim.ship();
    let (Ok((cam, cam_t)), Some((_, ship_t))) = (camera.single(), ships.iter().find(|(s, _)| s.index == sim.active))
    else {
        return;
    };
    let Ok(ray) = cam.viewport_to_world(cam_t, cursor) else { return };
    let placed = placed(v, &mut models);
    let meshes: Vec<(&RenderMesh, Transform)> = placed.iter().map(|(_, m, t)| (m.as_ref(), *t)).collect();
    if let Some((k, _)) = pick::ray_hit(ray.origin, ray.direction.as_vec3(), ship_t.compute_transform(), &meshes) {
        let zoom = if cards.zoom > 0.0 { cards.zoom } else { 1.0 };
        cards.open(v.id(), placed[k].0, From::Ship(egui::pos2(cursor.x / zoom, cursor.y / zoom)));
    }
}

/// The centre of a mesh's bounds.
fn centre(m: &RenderMesh) -> Vec3 {
    let (lo, hi) = (m.positions.iter().map(|p| Vec3::from_array(*p)))
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if lo.x <= hi.x {
        (lo + hi) / 2.0
    } else {
        Vec3::ZERO
    }
}

/// Each ship card's part on screen now (egui points), for its line.
pub fn anchors(
    sim: Res<SimState>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    ships: Query<(&ShipVisual, &GlobalTransform)>,
    mut models: ResMut<ShipModels>,
    mut cards: ResMut<PartCards>,
) {
    cards.anchors.clear();
    if !cards.open.iter().any(|c| matches!(c.from, From::Ship(_))) {
        return;
    }
    let (Ok((cam, cam_t)), Some((_, ship_t))) = (camera.single(), ships.iter().find(|(s, _)| s.index == sim.active))
    else {
        return;
    };
    let placed = placed(sim.ship(), &mut models);
    let zoom = if cards.zoom > 0.0 { cards.zoom } else { 1.0 };
    let found: Vec<(u64, egui::Pos2)> = (cards.open.iter())
        .filter(|c| matches!(c.from, From::Ship(_)))
        .filter_map(|c| {
            let (_, mesh, t) = placed.iter().find(|(id, _, _)| *id == c.part)?;
            let at = ship_t.transform_point(t.transform_point(centre(mesh)));
            let s = cam.world_to_viewport(cam_t, at).ok()?;
            Some((c.serial, egui::pos2(s.x / zoom, s.y / zoom)))
        })
        .collect();
    cards.anchors = found;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row's value below its label when the two do not fit (table:
    /// label, value, row width).
    #[test]
    fn a_long_value_goes_below_its_label() {
        let table = [(60.0, 80.0, 230.0, false), (120.0, 102.0, 230.0, false), (120.0, 110.0, 230.0, true)];
        for (label, value, width, below) in table {
            assert_eq!(value_below(label, value, width), below, "{label} {value} {width}");
        }
    }

    #[test]
    fn a_card_opens_beside_the_click_and_on_screen() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
        let w = 250.0;
        // (click, card's corner)
        let table = [
            (egui::pos2(400.0, 300.0), egui::pos2(424.0, 260.0)),
            (egui::pos2(1300.0, 300.0), egui::pos2(1026.0, 260.0)),
            (egui::pos2(400.0, 10.0), egui::pos2(424.0, layout::MARGIN)),
            (egui::pos2(400.0, 890.0), egui::pos2(424.0, 640.0)),
            (egui::pos2(100.0, 300.0), egui::pos2(124.0, 260.0)),
        ];
        for (at, want) in table {
            assert_eq!(beside(at, screen, w), want, "{at:?}");
        }
    }

    /// Cards from the staging bar (table): stacked down, a new column to
    /// the left when the next would pass the bottom (a card taller than the
    /// whole column still starts at the top).
    #[test]
    fn bar_cards_stack_in_columns_without_overlapping() {
        let p = egui::pos2;
        // (heights) → corners; top 50, bottom 900, right 1400, width 250, gap 6
        let table: [(&[f32], Vec<egui::Pos2>); 4] = [
            (&[], vec![]),
            (&[300.0, 200.0], vec![p(1150.0, 50.0), p(1150.0, 356.0)]),
            (&[300.0, 300.0, 470.0], vec![p(1150.0, 50.0), p(1150.0, 356.0), p(894.0, 50.0)]),
            (&[1000.0, 100.0], vec![p(1150.0, 50.0), p(894.0, 50.0)]),
        ];
        for (heights, want) in table {
            assert_eq!(stack(heights, 50.0, 900.0, 1400.0, 250.0, 6.0), want, "{heights:?}");
        }
    }

    #[test]
    fn a_mesh_centre_is_its_bounds_centre() {
        let m = RenderMesh { positions: vec![[0.0, 0.0, 0.0], [2.0, 4.0, -6.0]], normals: vec![], indices: vec![] };
        assert_eq!(centre(&m), Vec3::new(1.0, 2.0, -3.0));
        assert_eq!(centre(&RenderMesh { positions: vec![], normals: vec![], indices: vec![] }), Vec3::ZERO);
    }
}
