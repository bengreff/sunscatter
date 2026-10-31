//! The staging bar's icons (D111): one simple monochrome glyph per thing a
//! stage's command acts on, denoting only its type (engine, decoupler,
//! clamp, leg, grid fin, RCS block, valve, parachute, servo, power switch,
//! throttle), coloured by its state now (idle, armed, running, failed,
//! spent). Which icons a command has and how each looks are pure rules
//! with table tests (rule 8); [`paint`] draws a glyph with egui's painter.

use crate::interface::theme;
use bevy_egui::egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Shape, Stroke};
use sim::craft::assembly::JointKind;
use sim::craft::command::{matches, ShipCommand};
use sim::craft::ship::ShipDef;
use sim::craft::state::{EngineStatus, ShipState};
use sim::systems::command::can_ignite;

/// What an icon shows: the type of what the command acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Engine,
    Decoupler,
    Clamp,
    Leg,
    GridFin,
    Rcs,
    Valve,
    Parachute,
    /// Any other mechanism (a solar panel's or nosecone's hinge).
    Servo,
    Power,
    /// Engines' throttle or selection (what the pilot's throttle drives).
    Throttle,
}

/// The state an icon is coloured by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Ready, waiting for its command.
    Idle,
    /// Commanded and waiting (a release armed, a start waiting for settled
    /// propellant, a mechanism on its way, parachutes armed).
    Armed,
    /// Doing its job (burning, deployed, enabled, open, on).
    Running,
    Failed,
    /// Done for good (separated, released, out of ignitions or propellant).
    Spent,
}

/// One icon: its glyph, state, the part it opens a card for (`Part::id`),
/// and what hovering it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub kind: Kind,
    pub look: Look,
    pub part: u32,
    pub id: String,
}

/// The icons of command `c`: one per engine, valve, mechanism, joint,
/// thruster block or component it acts on, in the ship definition's order;
/// for the parachutes one per part carrying them (`chute`: those parts and
/// the canopies' look now).
pub fn icons(def: &ShipDef, state: &ShipState, c: &ShipCommand, chute: (&[u32], Look)) -> Vec<Icon> {
    let icon = |kind, look, part, id: &str| Icon { kind, look, part, id: id.to_string() };
    let lost = |id: &str| state.out(id);
    match c {
        ShipCommand::Ignite(s) | ShipCommand::Shutdown(s) | ShipCommand::Throttle(s, _) | ShipCommand::Select(s) => {
            let kind = if matches!(c, ShipCommand::Ignite(_) | ShipCommand::Shutdown(_)) {
                Kind::Engine
            } else {
                Kind::Throttle
            };
            let look = |i: usize| match kind {
                Kind::Engine => engine_look(def, state, i),
                _ if state.engines[i].selected => Look::Running,
                _ => Look::Idle,
            };
            (def.engines_of(s).into_iter())
                .map(|i| icon(kind, look(i), def.engines[i].part, &def.engines[i].id))
                .collect()
        }
        ShipCommand::Open(s) | ShipCommand::Close(s) => (def.valves.iter().enumerate())
            .filter(|(_, v)| matches(s, &v.id))
            .map(|(i, v)| {
                let st = &state.valves[i];
                let look = if st.stuck || lost(&v.id) {
                    Look::Failed
                } else if st.open {
                    Look::Running
                } else {
                    Look::Idle
                };
                icon(Kind::Valve, look, v.part, &v.id)
            })
            .collect(),
        ShipCommand::Deploy(s) | ShipCommand::Retract(s) | ShipCommand::Angle(s, _) => (def.mechanisms.iter())
            .enumerate()
            .filter(|(_, m)| matches(s, &m.id))
            .map(|(i, m)| {
                let lattice = def.surfaces.iter().any(|f| f.part == m.part && f.item.lattice);
                let kind = match (m.item.foot.is_some(), lattice) {
                    (true, _) => Kind::Leg,
                    (false, true) => Kind::GridFin,
                    _ => Kind::Servo,
                };
                let st = &state.mechanisms[i];
                let look = if lost(&m.id) {
                    Look::Failed
                } else if (st.angle - st.target).abs() > 0.5 {
                    Look::Armed
                } else if (st.angle - m.item.stowed_deg).abs() > 0.5 {
                    Look::Running
                } else {
                    Look::Idle
                };
                icon(kind, look, m.part, &m.id)
            })
            .collect(),
        ShipCommand::Separate(name) => (def.joints.iter().zip(&state.joints))
            .filter(|(d, _)| d.name == *name)
            .map(|(d, s)| joint_icon(d, s.intact, state))
            .collect(),
        ShipCommand::ReleaseClamps => (def.joints.iter().zip(&state.joints))
            .filter(|(d, _)| d.kind == JointKind::HoldDown)
            .map(|(d, s)| joint_icon(d, s.intact, state))
            .collect(),
        ShipCommand::ArmChutes => chute.0.iter().map(|&p| icon(Kind::Parachute, chute.1, p, "parachutes")).collect(),
        ShipCommand::Rcs(s, _) => (def.thrusters.iter().enumerate())
            .filter(|(_, t)| matches(s, &t.id))
            .map(|(i, t)| {
                let look = match (lost(&t.id), state.thrusters[i].enabled) {
                    (true, _) => Look::Failed,
                    (false, true) => Look::Running,
                    (false, false) => Look::Idle,
                };
                icon(Kind::Rcs, look, t.part, &t.id)
            })
            .collect(),
        ShipCommand::Power(s, _) => (def.components.iter().enumerate())
            .filter(|(_, comp)| matches(s, &comp.id))
            .map(|(i, comp)| {
                let on = state.power.get(i).is_some_and(|p| p.on);
                let look = match (lost(&comp.id) && on, on) {
                    (true, _) => Look::Failed,
                    (false, true) => Look::Running,
                    (false, false) => Look::Idle,
                };
                icon(Kind::Power, look, comp.part, &comp.id)
            })
            .collect(),
        ShipCommand::Sequence(_) => Vec::new(),
    }
}

/// A separation joint's or the pad clamps' icon, on the child's part.
fn joint_icon(d: &sim::craft::ship::JointDef, intact: bool, state: &ShipState) -> Icon {
    let clamp = d.kind == JointKind::HoldDown;
    let look = match (intact, clamp && state.release_armed) {
        (false, _) => Look::Spent,
        (true, true) => Look::Armed,
        (true, false) => Look::Idle,
    };
    Icon { kind: if clamp { Kind::Clamp } else { Kind::Decoupler }, look, part: d.child, id: d.name.clone() }
}

/// How engine `i` looks: failed, burning (starting, running, tailing off),
/// waiting for its propellant to start, out of ignitions or propellant,
/// else ready.
pub fn engine_look(def: &ShipDef, state: &ShipState, i: usize) -> Look {
    let (d, e) = (&def.engines[i], &state.engines[i]);
    let settling = state.engines.get(i).is_some_and(|r| r.settling.is_some());
    let fed = d.feeds.iter().all(|l| l.iter().any(|&t| state.tanks[t as usize].liquid > 0.0));
    match e.status {
        EngineStatus::Failed => Look::Failed,
        EngineStatus::Starting | EngineStatus::Running | EngineStatus::ShuttingDown => Look::Running,
        EngineStatus::Off if state.out(&d.owner) => Look::Failed,
        EngineStatus::Off if settling => Look::Armed,
        EngineStatus::Off if !fed || !can_ignite(d.data.igniter, e.ignitions, state.clamped) => Look::Spent,
        EngineStatus::Off => Look::Idle,
    }
}

/// The parachutes' look from their canopies (`sim::chute`): torn, open,
/// armed, spent (every canopy cut away), else stowed.
pub fn chute_look(c: &sim::chute::ChuteState) -> Look {
    use sim::chute::CanopyState::{Failed, Open, Released};
    let canopies = [c.drogue, c.main];
    if canopies.contains(&Failed) {
        Look::Failed
    } else if canopies.iter().any(|s| matches!(s, Open { .. })) {
        Look::Running
    } else if c.armed && c.main == Released {
        Look::Spent
    } else if c.armed {
        Look::Armed
    } else {
        Look::Idle
    }
}

/// An icon's colour: monochrome glyphs tinted by state.
pub fn colour(look: Look) -> Color32 {
    match look {
        Look::Idle => theme::MUTED,
        Look::Armed => theme::ACCENT,
        Look::Running => theme::GOOD,
        Look::Failed => theme::ALERT,
        Look::Spent => Color32::from_rgb(90, 100, 112),
    }
}

/// Paints a glyph of `kind` in `rect` (square) with `colour`.
pub fn paint(painter: &Painter, rect: Rect, kind: Kind, colour: Color32) {
    let s = Stroke::new(1.3, colour);
    let (c, r) = (rect.center(), rect.width().min(rect.height()) * 0.42);
    // Points in the glyph's unit square [-1, 1]² (y down).
    let p = |x: f32, y: f32| -> Pos2 { c + vec2(x, y) * r };
    let line = |pts: &[Pos2]| Shape::line(pts.to_vec(), s);
    let shapes: Vec<Shape> = match kind {
        Kind::Engine => vec![
            Shape::closed_line(vec![p(-0.35, -0.9), p(0.35, -0.9), p(0.35, -0.35), p(-0.35, -0.35)], s),
            Shape::closed_line(vec![p(-0.2, -0.35), p(0.2, -0.35), p(0.75, 0.9), p(-0.75, 0.9)], s),
        ],
        Kind::Decoupler => vec![
            Shape::closed_line(vec![p(-0.9, -0.75), p(0.9, -0.75), p(0.9, -0.2), p(-0.9, -0.2)], s),
            Shape::closed_line(vec![p(-0.9, 0.2), p(0.9, 0.2), p(0.9, 0.75), p(-0.9, 0.75)], s),
            Shape::circle_filled(p(-0.45, 0.0), r * 0.12, colour),
            Shape::circle_filled(p(0.45, 0.0), r * 0.12, colour),
        ],
        Kind::Clamp => vec![
            line(&[p(-0.9, -0.9), p(-0.9, 0.9), p(-0.2, 0.9), p(-0.2, 0.2)]),
            line(&[p(0.9, -0.9), p(0.9, 0.9), p(0.2, 0.9), p(0.2, 0.2)]),
            line(&[p(-0.5, -0.4), p(0.5, -0.4)]),
        ],
        Kind::Leg => vec![line(&[p(-0.2, -0.9), p(-0.75, 0.75)]), line(&[p(-1.0, 0.75), p(-0.5, 0.75)]), {
            line(&[p(0.2, -0.9), p(0.75, 0.75), p(1.0, 0.75)])
        }],
        Kind::GridFin => {
            let mut v = vec![Shape::closed_line(vec![p(-0.8, -0.8), p(0.8, -0.8), p(0.8, 0.8), p(-0.8, 0.8)], s)];
            for x in [-0.27, 0.27] {
                v.push(line(&[p(x, -0.8), p(x, 0.8)]));
                v.push(line(&[p(-0.8, x), p(0.8, x)]));
            }
            v
        }
        Kind::Rcs => {
            let mut v = vec![Shape::circle_stroke(c, r * 0.35, s)];
            for (x, y) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)] {
                v.push(line(&[p(x * 0.5, y * 0.5), p(x * 0.95, y * 0.95)]));
            }
            v
        }
        Kind::Valve => vec![
            Shape::closed_line(vec![p(-0.9, -0.6), p(0.0, 0.0), p(-0.9, 0.6)], s),
            Shape::closed_line(vec![p(0.9, -0.6), p(0.0, 0.0), p(0.9, 0.6)], s),
            line(&[p(0.0, 0.0), p(0.0, -0.8)]),
        ],
        Kind::Parachute => {
            let dome: Vec<Pos2> = (0..=8)
                .map(|k| {
                    let a = std::f32::consts::PI * (1.0 + k as f32 / 8.0);
                    p(0.9 * a.cos(), -0.2 + 0.7 * a.sin())
                })
                .collect();
            vec![line(&dome), line(&[p(-0.9, -0.2), p(0.0, 0.9), p(0.9, -0.2)]), line(&[p(0.0, -0.2), p(0.0, 0.9)])]
        }
        Kind::Servo => {
            let arc: Vec<Pos2> = (0..=8)
                .map(|k| {
                    let a = std::f32::consts::PI * (0.75 + 1.25 * k as f32 / 8.0);
                    p(0.75 * a.cos(), 0.75 * a.sin())
                })
                .collect();
            let end = *arc.last().unwrap();
            vec![Shape::circle_filled(c, r * 0.18, colour), line(&arc), line(&[end, end + vec2(-0.35, -0.1) * r])]
        }
        Kind::Power => vec![line(&[p(0.25, -0.95), p(-0.4, 0.1), p(0.3, 0.1), p(-0.25, 0.95)])],
        Kind::Throttle => vec![
            line(&[p(-0.9, 0.6), p(0.9, 0.6)]),
            line(&[p(-0.9, -0.6), p(0.9, -0.6)]),
            Shape::circle_filled(p(0.3, 0.6), r * 0.22, colour),
            Shape::circle_filled(p(-0.4, -0.6), r * 0.22, colour),
        ],
    };
    for shape in shapes {
        painter.add(shape);
    }
}

/// An icon's square on the bar (logical px).
pub const SIZE: f32 = 14.0;

/// The square for icon `k` of a row starting at `origin`, `per_row` to a row.
pub fn cell(origin: Pos2, k: usize, per_row: usize) -> Rect {
    let (col, row) = (k % per_row.max(1), k / per_row.max(1));
    Rect::from_min_size(
        pos2(origin.x + col as f32 * (SIZE + 2.0), origin.y + row as f32 * (SIZE + 2.0)),
        vec2(SIZE, SIZE),
    )
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
