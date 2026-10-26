//! Painting the ball itself: an egui mesh shaded per vertex from the pure
//! projection in [`super::rules`], the horizon grid and heading labels,
//! the velocity, orbit, target and maneuver markers, and the fixed ship
//! reticle.

use super::rules::{self, Ship};
use super::NavState;
use bevy_egui::egui;
use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke};
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, TAU};

const BRIGHT: Color32 = Color32::from_rgb(225, 235, 245);
const ACCENT: Color32 = Color32::from_rgb(255, 170, 40);
const PROGRADE: Color32 = Color32::from_rgb(235, 220, 40);
const NORMAL: Color32 = Color32::from_rgb(205, 90, 235);
const RADIAL: Color32 = Color32::from_rgb(70, 205, 235);
const TARGET: Color32 = Color32::from_rgb(245, 95, 165);
const MANEUVER: Color32 = Color32::from_rgb(70, 140, 255);

pub fn paint_ball(painter: &egui::Painter, rect: Rect, s: &NavState) {
    // Whole pixels, so the rim stroke and the disc's edge cannot drift apart.
    let centre = rect.center().round();
    let radius = (rect.width() * 0.5 - 2.0).round();
    let at = |x: f64, y: f64| centre + vec2(x as f32, -y as f32) * radius;
    // Shaded disc: rings of vertices, colour and limb darkening per vertex.
    const RINGS: usize = 24;
    const SEGS: usize = 72;
    let mut mesh = egui::Mesh::default();
    let vertex = |mesh: &mut egui::Mesh, x: f64, y: f64| {
        let d = rules::ball_unproject(&s.ship, x, y);
        let z = (1.0 - x * x - y * y).max(0.0).sqrt() as f32;
        let shade = 0.5 + 0.5 * z;
        let [r, g, b] = rules::ball_colour(d.dot(s.local.up)).map(|c| (c * shade) as u8);
        mesh.colored_vertex(at(x, y), Color32::from_rgb(r, g, b));
    };
    vertex(&mut mesh, 0.0, 0.0);
    for i in 1..=RINGS {
        // Denser rings towards the rim, where the ball curves away.
        let rho = (i as f64 / RINGS as f64 * FRAC_PI_2).sin();
        for j in 0..SEGS {
            let a = j as f64 / SEGS as f64 * TAU;
            vertex(&mut mesh, rho * a.cos(), rho * a.sin());
        }
    }
    let idx = |i: usize, j: usize| if i == 0 { 0 } else { (1 + (i - 1) * SEGS + j % SEGS) as u32 };
    for i in 0..RINGS {
        for j in 0..SEGS {
            if i == 0 {
                mesh.add_triangle(0, idx(1, j), idx(1, j + 1));
            } else {
                mesh.add_triangle(idx(i, j), idx(i + 1, j), idx(i + 1, j + 1));
                mesh.add_triangle(idx(i, j), idx(i + 1, j + 1), idx(i, j + 1));
            }
        }
    }
    painter.add(Shape::mesh(mesh));
    paint_grid(painter, &at, s);
    painter.circle_stroke(centre, radius, Stroke::new(2.0, Color32::from_rgb(60, 70, 80)));
    paint_markers(painter, &at, radius, s);
    // Fixed ship reticle.
    let w = [(-0.42, 0.0), (-0.16, 0.0), (-0.08, -0.1), (0.0, 0.0), (0.08, -0.1), (0.16, 0.0), (0.42, 0.0)];
    let pts: Vec<Pos2> = w.iter().map(|&(x, y)| at(x, y)).collect();
    painter.add(Shape::line(pts.clone(), Stroke::new(4.0, Color32::from_black_alpha(160))));
    painter.add(Shape::line(pts, Stroke::new(2.0, ACCENT)));
    painter.circle_filled(centre, 2.0, ACCENT);
}

/// Draws a curve on the ball, only where it faces the viewer.
fn curve(
    painter: &egui::Painter,
    at: &impl Fn(f64, f64) -> Pos2,
    ship: &Ship,
    dirs: impl Iterator<Item = DVec3>,
    stroke: Stroke,
) {
    let mut run = Vec::new();
    for d in dirs {
        let (x, y, z) = rules::ball_project(ship, d);
        if rules::faces_viewer(z) {
            run.push(at(x, y));
        } else if run.len() > 1 {
            painter.add(Shape::line(std::mem::take(&mut run), stroke));
        } else {
            run.clear();
        }
    }
    if run.len() > 1 {
        painter.add(Shape::line(run, stroke));
    }
}

fn paint_grid(painter: &egui::Painter, at: &impl Fn(f64, f64) -> Pos2, s: &NavState) {
    let deg = |x: f64| x.to_radians();
    let faint = Stroke::new(1.0, Color32::from_white_alpha(45));
    let major = Stroke::new(1.0, Color32::from_white_alpha(90));
    for el in (-80..=80).step_by(10).filter(|&e| e != 0) {
        let stroke = if el % 30 == 0 { major } else { faint };
        curve(painter, at, &s.ship, (0..=120).map(|k| s.local.direction(deg(el as f64), deg(k as f64 * 3.0))), stroke);
    }
    for az in (0..360).step_by(30) {
        let stroke = if az % 90 == 0 { major } else { faint };
        curve(painter, at, &s.ship, (-29..=29).map(|k| s.local.direction(deg(k as f64 * 3.0), deg(az as f64))), stroke);
    }
    curve(
        painter,
        at,
        &s.ship,
        (0..=180).map(|k| s.local.direction(0.0, deg(k as f64 * 2.0))),
        Stroke::new(2.0, BRIGHT),
    );
    // Heading labels along the horizon, pitch labels along the heading meridian.
    let label = |d: DVec3, text: String, colour: Color32| {
        let (x, y, z) = rules::ball_project(&s.ship, d);
        if z > 0.25 {
            painter.text(at(x, y), Align2::CENTER_CENTER, text, FontId::monospace(10.0), colour);
        }
    };
    for az in (0..360).step_by(30) {
        let text = match az {
            0 => "N".to_string(),
            90 => "E".to_string(),
            180 => "S".to_string(),
            270 => "W".to_string(),
            _ => (az / 10).to_string(),
        };
        label(s.local.direction(deg(4.0), deg(az as f64)), text, BRIGHT);
    }
    let hdg = deg(s.angles.heading);
    for el in [-60, -30, 30, 60] {
        label(s.local.direction(deg(el as f64), hdg + deg(8.0)), el.to_string(), Color32::from_white_alpha(170));
    }
}

#[derive(Clone, Copy)]
enum Glyph {
    Prograde,
    Retrograde,
    Normal,
    Antinormal,
    RadialOut,
    RadialIn,
    Target,
    AntiTarget,
    /// The next planned burn's direction.
    Maneuver,
}

fn paint_markers(painter: &egui::Painter, at: &impl Fn(f64, f64) -> Pos2, radius: f32, s: &NavState) {
    let m = &s.markers;
    let list = [
        (m.prograde, Glyph::Prograde, Glyph::Retrograde),
        (m.normal, Glyph::Normal, Glyph::Antinormal),
        (m.radial_out, Glyph::RadialOut, Glyph::RadialIn),
        (m.target, Glyph::Target, Glyph::AntiTarget),
    ];
    let size = radius * 0.085;
    if let Some((d, _)) = s.maneuver {
        let (x, y, z) = rules::ball_project(&s.ship, d);
        if rules::faces_viewer(z) {
            glyph_shapes(painter, at(x, y), size, Glyph::Maneuver, Stroke::new(4.0, Color32::from_black_alpha(150)));
            glyph_shapes(painter, at(x, y), size, Glyph::Maneuver, Stroke::new(1.8, MANEUVER));
        }
    }
    for (dir, pos_glyph, neg_glyph) in list {
        let Some(d) = dir else { continue };
        for (d, glyph) in [(d, pos_glyph), (-d, neg_glyph)] {
            let (x, y, z) = rules::ball_project(&s.ship, d);
            if z > 0.0 {
                // A dark outline first, so the glyph reads on sky and ground.
                glyph_shapes(painter, at(x, y), size, glyph, Stroke::new(4.0, Color32::from_black_alpha(150)));
                let colour = match glyph {
                    Glyph::Prograde | Glyph::Retrograde => PROGRADE,
                    Glyph::Normal | Glyph::Antinormal => NORMAL,
                    Glyph::RadialOut | Glyph::RadialIn => RADIAL,
                    Glyph::Target | Glyph::AntiTarget => TARGET,
                    Glyph::Maneuver => MANEUVER,
                };
                glyph_shapes(painter, at(x, y), size, glyph, Stroke::new(1.8, colour));
            }
        }
    }
}

fn glyph_shapes(painter: &egui::Painter, p: Pos2, s: f32, glyph: Glyph, stroke: Stroke) {
    let seg = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([p + vec2(a.0, a.1) * s, p + vec2(b.0, b.1) * s], stroke);
    };
    let ring = |r: f32| painter.circle_stroke(p, r * s, stroke);
    match glyph {
        Glyph::Prograde => {
            ring(0.7);
            seg((-0.7, 0.0), (-1.4, 0.0));
            seg((0.7, 0.0), (1.4, 0.0));
            seg((0.0, -0.7), (0.0, -1.4));
        }
        Glyph::Retrograde => {
            ring(0.7);
            seg((-0.5, -0.5), (0.5, 0.5));
            seg((-0.5, 0.5), (0.5, -0.5));
            seg((0.0, 0.7), (0.0, 1.4));
        }
        Glyph::Normal | Glyph::Antinormal => {
            let sign = if matches!(glyph, Glyph::Normal) { 1.0 } else { -1.0 };
            let tri = [(0.0, -sign), (0.9, 0.6 * sign), (-0.9, 0.6 * sign), (0.0, -sign)];
            let pts: Vec<Pos2> = tri.iter().map(|&(x, y)| p + vec2(x, y) * s).collect();
            painter.add(Shape::line(pts, stroke));
            if sign < 0.0 {
                painter.circle_filled(p, 0.2 * s, stroke.color);
            }
        }
        Glyph::RadialOut | Glyph::RadialIn => {
            ring(0.6);
            let (a, b) = if matches!(glyph, Glyph::RadialOut) { (0.6, 1.2) } else { (0.15, 0.45) };
            let k = std::f32::consts::FRAC_1_SQRT_2;
            for (dx, dy) in [(k, k), (-k, k), (k, -k), (-k, -k)] {
                seg((dx * a, dy * a), (dx * b, dy * b));
            }
        }
        Glyph::Target => {
            ring(0.8);
            painter.circle_filled(p, 0.2 * s, stroke.color);
        }
        Glyph::AntiTarget => {
            seg((-0.7, -0.7), (0.7, 0.7));
            seg((-0.7, 0.7), (0.7, -0.7));
        }
        Glyph::Maneuver => {
            // A ring with three ticks pointing in.
            ring(1.0);
            seg((0.0, -1.0), (0.0, -0.5));
            seg((-0.87, 0.5), (-0.43, 0.25));
            seg((0.87, 0.5), (0.43, 0.25));
        }
    }
}
