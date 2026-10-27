//! A basic sun flare, painted additively over the 3D view (behind the UI):
//! two soft glows for each star drawn as a light (`lighting::star_slots`),
//! as strong as the star's direct light at the camera against the light the
//! exposure is set for (D087), hidden by the bodies in front of its disc
//! (the one eclipse rule), dimmed and reddened by the air along the line of
//! sight when the camera is in an atmosphere ([`flare`]). Soft glows only
//! (D058): a streak and ghosts popped in and out as the Sun crossed a limb.

use crate::camera::{CameraRig, MainCamera};
use crate::lighting::{rules::luminance, LightScene};
use crate::look::Parts;
use crate::map;
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;

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
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    scene: Res<LightScene>,
    parts: Res<Parts>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
) -> Result {
    if !parts.flare || scene.slots.is_empty() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let exposure_lux = crate::lighting::exposure_lux(&sim, &rig, &defs, &scene);
    let air = crate::lighting::air_at_camera(&sim, &rig, &defs);
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
        let visible = crate::lighting::eclipse_factor(DVec3::ZERO, star.centre, star.radius, &occluders);
        let to_star = star.centre.normalize_or(DVec3::Z);
        let transmittance =
            air.map_or([1.0; 3], |(a, r, h, up)| crate::lighting::haze::direct_transmittance(a, r, h, up.dot(to_star)));
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
