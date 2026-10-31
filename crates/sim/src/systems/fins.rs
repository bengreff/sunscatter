//! Grid fins in flight (ship-model.md §5.4; falcon9.md §1.2): a deployed
//! lattice fin (its mechanism at its deployed angle, its actuator not lost)
//! gives the normal force of `aero::lattice` along its tangent, at the
//! middle of its span, for the flow's incidence on its cells plus its
//! deflection. SAS and the pilot command the deflections: their torque is
//! allocated over the fins by least squares, each fin within its range and
//! turning at its hydraulic actuator's rate (falcon9.md: ±20°, 30°/s, U).

use crate::aero::lattice;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::math;
use glam::DVec3;

/// A deployed lattice fin, body axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fin {
    /// Index in `ShipDef::surfaces`.
    pub surface: usize,
    /// Where its force acts (mid-span) and the way it pushes (its tangent).
    pub at: DVec3,
    pub tangent: DVec3,
    /// Cells' frontal area (m²), deflection range (rad) and rate (rad/s).
    pub area: f64,
    pub range: f64,
    pub rate: f64,
    /// It cannot move (its actuator lost).
    pub stuck: bool,
}

/// The flow at the fins (body axes): the direction the air moves, q, Mach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinFlow {
    pub dir: DVec3,
    pub q: f64,
    pub mach: f64,
}

/// The deployed lattice fins of a ship.
pub fn deployed(def: &ShipDef, state: &ShipState) -> Vec<Fin> {
    let mut out = Vec::new();
    for (i, s) in def.surfaces.iter().enumerate().filter(|(_, s)| s.item.lattice) {
        let Some(m) = def.mechanisms.iter().position(|m| m.part == s.part) else { continue };
        let (md, ms) = (&def.mechanisms[m].item, &state.mechanisms[m]);
        if ms.angle != md.deployed_deg {
            continue;
        }
        // Out from the body at its hinge, pushing along its tangent.
        let radial = DVec3::new(md.pos.x, md.pos.y, 0.0);
        if radial.length() == 0.0 {
            continue;
        }
        let radial = radial.normalize();
        out.push(Fin {
            surface: i,
            at: md.pos + radial * (0.5 * s.item.span),
            tangent: DVec3::Z.cross(radial),
            area: s.item.area,
            range: s.item.deflection_deg * math::PI / 180.0,
            rate: s.item.rate_dps * math::PI / 180.0,
            stuck: super::mechanisms::stuck(def, state, m),
        });
    }
    out
}

/// A fin's incidence (rad): the flow's angle to its cells' axis in the
/// plane of its tangent (either way through the cells).
pub fn incidence(fin: &Fin, flow: &FinFlow) -> f64 {
    math::atan2(flow.dir.dot(fin.tangent), flow.dir.z.abs())
}

/// Each fin's torque about `com` per radian of deflection (body axes,
/// N·m), linearised at the flow's incidence (the deflections' own
/// saturation is left to SAS's loop).
pub fn gains(fins: &[Fin], flow: &FinFlow, com: DVec3) -> Vec<DVec3> {
    (fins.iter())
        .map(|f| (f.at - com).cross(f.tangent) * (flow.q * f.area * lattice::slope(incidence(f, flow), flow.mach)))
        .collect()
}

/// The torque authority per body axis of `gains` over the fins' ranges.
pub fn authority(fins: &[Fin], gains: &[DVec3]) -> DVec3 {
    (fins.iter().zip(gains)).filter(|(f, _)| !f.stuck).map(|(f, g)| g.abs() * f.range).sum()
}

/// The deflections (rad) that best give torque `want` (least squares by
/// Gauss–Seidel sweeps, each fin within its range), from `now`, each
/// turning at most its rate × `dt`; a stuck fin stays.
pub fn allocate(fins: &[Fin], gains: &[DVec3], want: DVec3, now: &[f64], dt: f64) -> Vec<f64> {
    let mut d = vec![0.0; fins.len()];
    for _ in 0..40 {
        for i in 0..fins.len() {
            let g2 = gains[i].length_squared();
            if fins[i].stuck || g2 <= 0.0 {
                continue;
            }
            let rest: DVec3 = (0..fins.len()).filter(|&j| j != i).map(|j| gains[j] * d[j]).sum();
            d[i] = ((want - rest).dot(gains[i]) / g2).clamp(-fins[i].range, fins[i].range);
        }
    }
    (0..fins.len())
        .map(|i| {
            let n = now.get(i).copied().unwrap_or(0.0);
            if fins[i].stuck {
                n
            } else {
                n + (d[i] - n).clamp(-fins[i].rate * dt, fins[i].rate * dt)
            }
        })
        .collect()
}

/// The fins' force (body axes, N) and torque about `com` at `deflections`.
pub fn forces(fins: &[Fin], flow: &FinFlow, com: DVec3, deflections: &[f64]) -> (DVec3, DVec3) {
    let (mut force, mut torque) = (DVec3::ZERO, DVec3::ZERO);
    for (f, &d) in fins.iter().zip(deflections) {
        let n = f.tangent * (flow.q * f.area * lattice::normal(incidence(f, flow) + d, flow.mach));
        force += n;
        torque += (f.at - com).cross(n);
    }
    (force, torque)
}

/// The deflections the ship's fins hold now (aligned with `fins`).
pub fn held(state: &ShipState, fins: &[Fin]) -> Vec<f64> {
    fins.iter().map(|f| state.fins.get(f.surface).copied().unwrap_or(0.0)).collect()
}

/// Stores the fins' deflections.
pub fn hold(def: &ShipDef, state: &mut ShipState, fins: &[Fin], deflections: &[f64]) {
    state.fins.resize(def.surfaces.len(), 0.0);
    for (f, &d) in fins.iter().zip(deflections) {
        state.fins[f.surface] = d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn falcon9() -> (ShipDef, ShipState) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut state = ShipState::initial(&def);
        for (d, s) in def.mechanisms.iter().zip(state.mechanisms.iter_mut()) {
            if d.id.starts_with("s1.gridfin") {
                (s.angle, s.target) = (d.item.deployed_deg, d.item.deployed_deg);
            }
        }
        ((*def).clone(), state)
    }

    /// The booster falling engines first at Mach 0.5 with q 20 kPa: its four
    /// fins, trailing, give roll and pitch/yaw authority; the allocation
    /// gives a pitch torque without roll; with no deflection and the flow
    /// along the axis the fins push nothing; a crossflow pushes them with it
    /// (they stabilise the stage, trailing).
    #[test]
    fn the_booster_steers_on_its_grid_fins() {
        let (def, state) = falcon9();
        let fins = deployed(&def, &state);
        assert_eq!(fins.len(), 4);
        let com = DVec3::new(0.0, 0.0, -45.0);
        let flow = FinFlow { dir: DVec3::Z, q: 20e3, mach: 0.5 };
        let zero = vec![0.0; 4];
        let g = gains(&fins, &flow, com);
        let auth = authority(&fins, &g);
        println!("authority {auth:?} N·m");
        assert!(auth.x > 1e6 && auth.y > 1e6 && auth.z > 1e5, "{auth}");
        assert_eq!(forces(&fins, &flow, com, &zero).0, DVec3::ZERO);
        let want = DVec3::new(0.3 * auth.x, 0.0, 0.0);
        let mut d = zero.clone();
        for _ in 0..50 {
            d = allocate(&fins, &g, want, &d, 0.02);
        }
        let (_, t) = forces(&fins, &flow, com, &d);
        // Within the lattice's saturation at 6° (tanh: 5 %) and the
        // allocation's sweeps; SAS's loop closes the rest.
        assert!((t.x - want.x).abs() < 0.15 * want.x && t.z.abs() < 0.05 * want.x, "{t}");
        // Rate: 30°/s, so 0.6° in a tick.
        let first = allocate(&fins, &g, want * 3.0, &zero, 0.02);
        assert!(first.iter().all(|x| x.abs() <= 0.6f64.to_radians() + 1e-12));
        // A crossflow along +X pushes the trailing fins along +X.
        let side = FinFlow { dir: DVec3::new(0.1, 0.0, 1.0).normalize(), ..flow };
        assert!(forces(&fins, &side, com, &zero).0.x > 0.0);
    }
}
