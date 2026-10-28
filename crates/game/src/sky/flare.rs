//! A basic sun flare, painted additively over the 3D view (behind the UI):
//! two soft glows for each star drawn as a light (`lighting::star_slots`),
//! as strong as the star's direct light at the camera against the light the
//! exposure is set for (D087), hidden by the bodies in front of its disc
//! (the one eclipse rule) and by craft ([`craft_cover`], D110), dimmed and
//! reddened by the air along the line of sight (`haze::air_along`, from
//! inside an atmosphere or through a limb; [`flare`]). Soft glows only
//! (D058): a streak and ghosts popped in and out as the Sun crossed a limb.

use crate::camera::MainCamera;
use crate::lighting::{rules::luminance, LightScene};
use crate::look::Parts;
use crate::map;
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use std::sync::Arc;

/// Below this strength no flare is drawn.
const MIN_STRENGTH: f32 = 0.01;

/// One star's flare: its strength (0..1) and colour (the brightest channel 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flare {
    pub strength: f32,
    pub rgb: [f32; 3],
}

/// The flare of a star whose disc is `visible` (the unhidden fraction), its
/// direct beam passing `transmittance` per channel through the air, giving
/// `lux` at the camera above the air where the exposure is set for
/// `exposure_lux`, in the light's colour `tint`, at `offset` (its distance
/// from the screen's centre over the screen's width): strongest on screen,
/// fading out from 0.6 to 1.2 widths off centre. `None` when too faint.
pub fn flare(
    visible: f64,
    transmittance: [f64; 3],
    tint: [f64; 3],
    lux: f64,
    exposure_lux: f64,
    offset: f64,
) -> Option<Flare> {
    if exposure_lux <= 0.0 || !lux.is_finite() {
        return None;
    }
    let light = [0, 1, 2].map(|c| tint[c] * transmittance[c]);
    let share = (visible * luminance(light) / luminance(tint).max(1e-9) * lux / exposure_lux).clamp(0.0, 1.0);
    let on_screen = (1.0 - (offset - 0.6).max(0.0) / 0.6).clamp(0.0, 1.0);
    let strength = (share * on_screen) as f32;
    let peak = light.into_iter().fold(0.0, f64::max);
    if strength < MIN_STRENGTH || peak <= 0.0 {
        return None;
    }
    Some(Flare { strength, rgb: light.map(|c| (c / peak) as f32) })
}

/// Directions across a star's disc of angular radius `a` around `axis`
/// (unit): its centre and six around it at 0.6 a, each standing for a
/// seventh of the disc.
pub fn disc_rays(axis: Vec3, a: f32) -> [Vec3; 7] {
    let (e1, e2) = axis.any_orthonormal_pair();
    let t = (0.6 * a).tan();
    let mut rays = [axis; 7];
    for (k, ray) in rays.iter_mut().enumerate().skip(1) {
        let phi = k as f32 * std::f32::consts::TAU / 6.0;
        *ray = (axis + (e1 * phi.cos() + e2 * phi.sin()) * t).normalize();
    }
    rays
}

/// A craft as the flare sees it: its bounding sphere (camera-relative
/// centre, radius) and its parts' meshes with their transforms.
pub struct CraftShape<'a> {
    pub centre: Vec3,
    pub radius: f32,
    pub parts: Vec<(&'a sim::craft::RenderMesh, Transform)>,
}

/// The share of `rays` from the camera (the origin) that craft hide: a ray
/// is tested against a craft's parts (`systems_screen::rules::pick`, the
/// one ray-against-parts rule) only when it passes the craft's sphere in
/// front of the camera.
pub fn craft_cover(rays: &[Vec3], crafts: &[CraftShape]) -> f64 {
    if rays.is_empty() {
        return 0.0;
    }
    let near = |ray: Vec3, c: &CraftShape| {
        let along = ray.dot(c.centre);
        along > -c.radius && (c.centre - ray * along).length() <= c.radius
    };
    let hidden = (rays.iter())
        .filter(|&&ray| {
            crafts
                .iter()
                .any(|c| near(ray, c) && crate::systems_screen::rules::pick(Vec3::ZERO, ray, &c.parts).is_some())
        })
        .count();
    hidden as f64 / rays.len() as f64
}

/// The craft near the camera as [`CraftShape`]s (their part meshes kept by
/// `ShipModels`).
fn craft_shapes<'a>(
    sim: &SimState,
    ships: &Query<(&crate::scene::ships::ShipVisual, &Transform)>,
    meshes: &'a [(Arc<sim::craft::RenderMesh>, usize)],
) -> Vec<CraftShape<'a>> {
    let mut out: Vec<CraftShape> = Vec::new();
    for (ship, t) in ships {
        if sim.fleet.get(ship.index).is_none() {
            continue;
        }
        let (centre, radius) = ship.bound();
        let parts = (meshes.iter()).filter(|(_, i)| *i == ship.index).map(|(m, _)| (m.as_ref(), *t)).collect();
        out.push(CraftShape { centre: t.transform_point(centre), radius, parts });
    }
    out
}

/// Additive colour (premultiplied with zero alpha).
fn add(rgb: [f32; 3], k: f32) -> egui::Color32 {
    let to = |x: f32| (x * k * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(to(rgb[0]), to(rgb[1]), to(rgb[2]), 0)
}

/// A radial gradient disc as a triangle fan.
fn glow(painter: &egui::Painter, c: egui::Pos2, radius: f32, rgb: [f32; 3], k: f32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, add(rgb, k));
    let n = 48;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(c + egui::vec2(a.cos(), a.sin()) * radius, add(rgb, 0.0));
    }
    for i in 1..=n as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    rig: Res<crate::camera::CameraRig>,
    defs: Res<BodyDefs>,
    scene: Res<LightScene>,
    parts: Res<Parts>,
    mut models: ResMut<crate::scene::ships::ShipModels>,
    ships: Query<(&crate::scene::ships::ShipVisual, &Transform)>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
) -> Result {
    if !parts.flare || scene.slots.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let exposure_lux = crate::lighting::exposure_lux(&sim, &rig, &defs, &scene);
    let airs = scene.airs(&defs);
    // Every drawn craft's part meshes, by fleet slot.
    let meshes: Vec<(Arc<sim::craft::RenderMesh>, usize)> = (ships.iter())
        .filter_map(|(ship, _)| Some((ship.index, sim.fleet.get(ship.index)?)))
        .flat_map(|(i, v)| models.meshes_of(&sim::craft::craft_of(&v.craft.id)).into_iter().map(move |m| (m, i)))
        .collect();
    let crafts = craft_shapes(&sim, &ships, &meshes);
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    for &k in &scene.slots {
        let star = scene.stars[k];
        let Some(s) = view.project(star.centre) else { continue };
        // Every other body and star can hide its disc.
        let occluders: Vec<(DVec3, f64)> = scene
            .bodies
            .iter()
            .map(|b| (b.1.centre, b.1.radius))
            .chain(scene.stars.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, o)| (o.centre, o.radius)))
            .collect();
        let to_star = star.centre.normalize_or(DVec3::Z);
        let a = (star.radius / star.centre.length().max(star.radius)).asin() as f32;
        let hidden = craft_cover(&disc_rays(to_star.as_vec3(), a), &crafts);
        let visible =
            crate::lighting::eclipse_factor(DVec3::ZERO, star.centre, star.radius, &occluders) * (1.0 - hidden);
        let transmittance = crate::lighting::haze::air_along(DVec3::ZERO, to_star, &airs);
        let p = egui::pos2(s.x, s.y);
        let offset = f64::from((p - screen.center()).length() / screen.width().max(1.0));
        let lux = star.lux_at(DVec3::ZERO);
        let Some(f) = flare(visible, transmittance, star.tint, lux, exposure_lux, offset) else { continue };
        // The outer glow warm, the inner one nearly the light's colour.
        let warm = [f.rgb[0], f.rgb[1] * 0.92, f.rgb[2] * 0.8];
        glow(&painter, p, 260.0, warm, 0.22 * f.strength);
        glow(&painter, p, 70.0, [f.rgb[0], f.rgb[1] * 0.97, f.rgb[2] * 0.9], 0.55 * f.strength);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flare_for_each_star_as_strong_as_its_light_at_the_camera() {
        let white = [1.0; 3];
        let clear = [1.0; 3];
        // (case, visible, transmittance, tint, lux, exposure lux, offset, strength or none)
        type Case = (&'static str, f64, [f64; 3], [f64; 3], f64, f64, f64, Option<f32>);
        let cases: [Case; 9] = [
            ("the Sun in space, on screen", 1.0, clear, white, 1.3e5, 1.3e5, 0.1, Some(1.0)),
            ("half its disc behind the Moon", 0.5, clear, white, 1.3e5, 1.3e5, 0.1, Some(0.5)),
            ("behind Earth", 0.0, clear, white, 1.3e5, 1.3e5, 0.1, None),
            ("a second star with a tenth of the light", 1.0, clear, white, 1.3e4, 1.43e5, 0.1, Some(0.0909)),
            ("a star with a thousandth: too faint", 1.0, clear, white, 130.0, 1.3e5, 0.1, None),
            ("at the screen's edge", 1.0, clear, white, 1.3e5, 1.3e5, 0.9, Some(0.5)),
            ("well off screen", 1.0, clear, white, 1.3e5, 1.3e5, 1.3, None),
            ("under Venus's clouds: no direct beam", 1.0, [1e-12; 3], white, 2.6e5, 9.0e3, 0.1, None),
            ("no light at the camera", 1.0, clear, white, 0.0, 0.0, 0.1, None),
        ];
        for (case, vis, t, tint, lux, e, off, want) in cases {
            let got = flare(vis, t, tint, lux, e, off).map(|f| f.strength);
            match (got, want) {
                (Some(g), Some(w)) => assert!((g - w).abs() < 1e-3, "{case}: {g}"),
                (g, w) => assert_eq!(g, w, "{case}"),
            }
        }
    }

    #[test]
    fn craft_in_front_of_the_sun_hide_its_flare() {
        // A 10 m square plate 100 m ahead, facing the camera.
        let plate = sim::craft::RenderMesh {
            positions: vec![[-5.0, -5.0, 0.0], [5.0, -5.0, 0.0], [5.0, 5.0, 0.0], [-5.0, 5.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let at = |x: f32| Transform::from_xyz(x, 0.0, -100.0);
        let craft = |x: f32| CraftShape { centre: at(x).translation, radius: 7.1, parts: vec![(&plate, at(x))] };
        let sun = Vec3::NEG_Z;
        // The Sun's disc seen from Earth: 0.27° in radius.
        let rays = disc_rays(sun, 0.27f32.to_radians());
        // (case, craft, share hidden)
        let cases: [(&str, Vec<CraftShape>, f64); 3] = [
            ("no craft", vec![], 0.0),
            ("the plate in front", vec![craft(0.0)], 1.0),
            ("beside the line", vec![craft(20.0)], 0.0),
        ];
        for (case, crafts, want) in cases {
            assert!((craft_cover(&rays, &crafts) - want).abs() < 1e-9, "{case}: {}", craft_cover(&rays, &crafts));
        }
        // Its edge across the disc: part of it.
        let edge = craft_cover(&rays, &[craft(4.9)]);
        assert!(edge > 0.0 && edge < 1.0, "{edge}");
        // Behind the camera, nothing hides it.
        let behind = CraftShape {
            centre: Vec3::new(0.0, 0.0, 100.0),
            radius: 7.1,
            parts: vec![(&plate, Transform::from_xyz(0.0, 0.0, 100.0))],
        };
        assert_eq!(craft_cover(&rays, &[behind]), 0.0);
    }

    #[test]
    fn a_low_sun_flares_dimmer_and_redder() {
        let sunset = [0.5, 0.25, 0.08];
        let f = flare(1.0, sunset, [1.0; 3], 1.3e5, 1.3e5, 0.0).expect("a flare");
        assert!((f.strength - luminance(sunset) as f32).abs() < 1e-6);
        assert_eq!(f.rgb[0], 1.0);
        assert!(f.rgb[0] > f.rgb[1] && f.rgb[1] > f.rgb[2], "{:?}", f.rgb);
        // A red dwarf's flare in its own colour.
        let red = flare(1.0, [1.0; 3], [1.6, 0.9, 0.4], 1.3e5, 1.3e5, 0.0).expect("a flare");
        assert!(red.rgb[0] == 1.0 && red.rgb[2] < 0.3, "{:?}", red.rgb);
    }
}
