//! The part layer (D032, D033, D065, D083; `docs/technical/parts.md`): a
//! craft is a set of parts attached in a tree. **A foundation, not a part
//! catalog**: craft creation is the owner's to guide (interview
//! 2026-10-26), so every property here is a placeholder a real part
//! definition will replace.
//!
//! A [`Part`] owns primitives of the craft's geometry (each primitive
//! belongs to exactly one part), its dry mass, its engine's lump (the
//! thermal network's mount node), the parachutes, a control flag (a crew
//! cabin or probe core) and a load limit. Its tanks, thrusters, batteries,
//! cells and the rest of its systems are the ship definition's
//! ([`super::ship`]). The craft's aggregates (mass, centre of mass and
//! inertia, crew) are computed from its parts; cells and interior nodes
//! are tagged with the part they belong to ([`super::CraftDesign`]).
//!
//! A piece of a craft ([`CraftDef::subset`]) keeps its
//! parts' body axes, so a piece's geometry, mass and attitude are those it
//! had inside the craft.

use super::file::{CraftFile, EngineFile, GeometryFile, Mount, Tank};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// One part (placeholder properties).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    /// Its index in the craft it was designed in: pieces keep it, so a
    /// piece names its parts in the original craft ([`super::CraftId::piece`]).
    pub id: u32,
    pub name: String,
    /// Indices of the geometry's primitives it owns.
    pub primitives: Vec<u32>,
    /// The part it is attached to (an index into the craft's parts);
    /// `None` for a root. The attachments form a forest (one tree for a
    /// craft, more only transiently in a piece being split).
    pub parent: Option<u32>,
    /// kg, spread as a shell over its own primitives' surface.
    pub dry_mass: f64,
    /// Its engines' lump (`combine_engines`): where the thermal network
    /// puts the nozzle's mount node; the engines are the ship's.
    pub engine: Option<EngineFile>,
    /// Carries the craft's parachutes (`CraftFile::chute`).
    pub chute: bool,
    /// Specific heat of its interior (J/(kg·K)); the craft's if `None`.
    pub interior_specific_heat: Option<f64>,
    /// Limit of the interior nodes it fills (K); the craft's
    /// `internal_max_k` if `None`. Its skin's limit is its primitives'
    /// skin `max_k`.
    pub node_max_k: Option<f64>,
    /// A crew cabin or probe core: the craft can be commanded (D083).
    pub control: bool,
    /// Seats.
    pub crew: u32,
    /// The largest aerodynamic or contact force its cells may carry (N)
    /// before it tears off (`f64::INFINITY`: never).
    pub max_load: f64,
}

/// A craft's definition: its files and its parts; a craft of catalog
/// parts also its ship definition ([`super::ship`]: tanks, engines,
/// components, layers, sequence).
#[derive(Clone, Debug, PartialEq)]
pub struct CraftDef {
    pub spec: CraftFile,
    pub geometry: GeometryFile,
    pub parts: Vec<Part>,
    pub ship: Option<std::sync::Arc<super::ship::ShipDef>>,
}

impl CraftDef {
    /// The part owning each primitive.
    pub fn primitive_part(&self) -> Vec<u32> {
        let mut owner = vec![u32::MAX; self.geometry.primitives.len()];
        for (i, p) in self.parts.iter().enumerate() {
            for &q in &p.primitives {
                owner[q as usize] = i as u32;
            }
        }
        owner
    }

    /// The tanks (region, capacity): its ship's tanks and bottles
    /// ([`super::ship::ShipDef::regions`]).
    pub fn tanks(&self) -> Vec<(Tank, f64)> {
        self.ship.as_ref().map_or_else(Vec::new, |ship| ship.regions())
    }

    /// The piece made of `parts` (indices into this craft's parts, any
    /// order): their primitives (renumbered, in order), parts (in this
    /// craft's order, attachments to parts outside the piece cut), the
    /// ship's piece and the craft file's aggregates ([`aggregate`]). Same
    /// body axes.
    pub fn subset(&self, parts: &[u32]) -> CraftDef {
        let mut keep: Vec<u32> = parts.to_vec();
        keep.sort_unstable();
        keep.dedup();
        let mut local = vec![u32::MAX; self.parts.len()];
        for (k, &p) in keep.iter().enumerate() {
            local[p as usize] = k as u32;
        }
        let owner = self.primitive_part();
        let mut prim_local = vec![u32::MAX; owner.len()];
        let mut primitives = Vec::new();
        for (q, &o) in owner.iter().enumerate() {
            if o != u32::MAX && local[o as usize] != u32::MAX {
                prim_local[q] = primitives.len() as u32;
                primitives.push(self.geometry.primitives[q].clone());
            }
        }
        let new_parts: Vec<Part> = (keep.iter())
            .map(|&p| {
                let part = &self.parts[p as usize];
                Part {
                    primitives: part.primitives.iter().map(|&q| prim_local[q as usize]).collect(),
                    parent: part.parent.map(|up| local[up as usize]).filter(|&l| l != u32::MAX),
                    ..part.clone()
                }
            })
            .collect();
        let mut spec = self.spec.clone();
        aggregate(&mut spec, &new_parts);
        let ids: Vec<u32> = new_parts.iter().map(|p| p.id).collect();
        let ship = self.ship.as_ref().map(|s| std::sync::Arc::new(s.subset(&ids)));
        let geometry = GeometryFile { skin: self.geometry.skin, primitives };
        CraftDef { spec, geometry, parts: new_parts, ship }
    }

    /// Checks the parts against the geometry: each primitive owned by
    /// exactly one part, parents in range and acyclic, positive masses,
    /// at most one nozzle per part.
    pub fn validate(&self) -> Result<(), String> {
        if self.parts.is_empty() {
            return Err("a craft has at least one part".into());
        }
        let n = self.geometry.primitives.len();
        let mut owned = vec![0u32; n];
        for (i, p) in self.parts.iter().enumerate() {
            let what = format!("part {i} ({:?})", p.name);
            if p.primitives.is_empty() {
                return Err(format!("{what} owns no primitive"));
            }
            for &q in &p.primitives {
                *owned.get_mut(q as usize).ok_or(format!("{what}: primitive {q} out of range"))? += 1;
            }
            if !(p.dry_mass.is_finite() && p.dry_mass > 0.0) {
                return Err(format!("{what}: dry_mass must be positive"));
            }
            if p.max_load.is_nan() || p.max_load <= 0.0 {
                return Err(format!("{what}: max_load must be positive"));
            }
            let nozzles = p.primitives.iter().filter(|&&q| self.geometry.primitives[q as usize].nozzle).count();
            if nozzles > 1 {
                return Err(format!("{what}: at most one nozzle per part"));
            }
            let (mut at, mut steps) = (i, 0);
            while let Some(up) = self.parts[at].parent {
                at = up as usize;
                steps += 1;
                if at >= self.parts.len() || steps > self.parts.len() {
                    return Err(format!("{what}: parent out of range or a cycle"));
                }
            }
        }
        match owned.iter().position(|&c| c != 1) {
            Some(q) => Err(format!("primitive {q} is owned by {} parts", owned[q])),
            None => Ok(()),
        }
    }
}

/// The engine of several engines firing together: summed thrust, the Isp
/// of the summed flow, the thrust-weighted mount and direction, the
/// highest minimum throttle, the smallest gimbal range, and the nozzle
/// share that gives the summed wall heat. One engine is itself; none a
/// zero-thrust stand-in (no engine, [`has_engine`]).
pub fn combine_engines(engines: &[EngineFile]) -> EngineFile {
    match engines {
        [] => EngineFile {
            thrust_vac: 0.0,
            isp_vac: 1.0,
            isp_sl: 1.0,
            min_throttle: 1.0,
            gimbal_deg: 0.0,
            mount: Mount { pos: DVec3::ZERO, dir: DVec3::Z },
            nozzle_heat_fraction: 0.0,
        },
        [one] => *one,
        _ => {
            let thrust: f64 = engines.iter().map(|e| e.thrust_vac).sum();
            let flow = |isp: fn(&EngineFile) -> f64| engines.iter().map(|e| e.thrust_vac / isp(e)).sum::<f64>();
            let (isp_vac, isp_sl) = (thrust / flow(|e| e.isp_vac), thrust / flow(|e| e.isp_sl));
            let pos = engines.iter().map(|e| e.mount.pos * e.thrust_vac).sum::<DVec3>() / thrust;
            let dir = engines.iter().map(|e| e.mount.dir.normalize() * e.thrust_vac).sum::<DVec3>().normalize();
            // Wall heat f·½·F·vₑ per engine, vₑ ∝ Isp.
            let heat: f64 = engines.iter().map(|e| e.nozzle_heat_fraction * e.thrust_vac * e.isp_vac).sum();
            EngineFile {
                thrust_vac: thrust,
                isp_vac,
                isp_sl,
                min_throttle: engines.iter().map(|e| e.min_throttle).fold(0.0, f64::max),
                gimbal_deg: engines.iter().map(|e| e.gimbal_deg).fold(f64::MAX, f64::min),
                mount: Mount { pos, dir },
                nozzle_heat_fraction: heat / (thrust * isp_vac),
            }
        }
    }
}

/// Whether an engine (from [`combine_engines`]) is a real one.
pub fn has_engine(e: &EngineFile) -> bool {
    e.thrust_vac > 0.0
}

/// Writes the parts' aggregates into a craft file: dry mass, the engines'
/// lump (the thermal network's mount node), the seats.
pub fn aggregate(spec: &mut CraftFile, parts: &[Part]) {
    spec.dry_mass = parts.iter().map(|p| p.dry_mass).sum();
    let engines: Vec<EngineFile> = parts.iter().filter_map(|p| p.engine).collect();
    spec.engine = combine_engines(&engines);
    spec.crew = parts.iter().map(|p| p.crew).sum();
}

/// The connected groups of parts once the attachments of `cut` (part
/// indices: each loses the link to its parent) are broken and the parts
/// of `removed` are taken out (all their links): each group sorted, the
/// groups ordered by their lowest part.
pub fn components(parts: &[Part], cut: &[u32], removed: &[u32]) -> Vec<Vec<u32>> {
    let n = parts.len();
    // Union-find over the attachments that hold.
    let mut root: Vec<u32> = (0..n as u32).collect();
    fn find(root: &mut [u32], mut i: u32) -> u32 {
        while root[i as usize] != i {
            root[i as usize] = root[root[i as usize] as usize];
            i = root[i as usize];
        }
        i
    }
    let gone = |i: u32| removed.contains(&i);
    for (i, p) in parts.iter().enumerate() {
        let i = i as u32;
        let Some(up) = p.parent else { continue };
        if gone(i) || gone(up) || cut.contains(&i) {
            continue;
        }
        let (a, b) = (find(&mut root, i), find(&mut root, up));
        if a != b {
            root[a.max(b) as usize] = a.min(b);
        }
    }
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut slot = vec![usize::MAX; n];
    for i in 0..n as u32 {
        if gone(i) {
            continue;
        }
        let r = find(&mut root, i) as usize;
        if slot[r] == usize::MAX {
            slot[r] = groups.len();
            groups.push(Vec::new());
        }
        groups[slot[r]].push(i);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(n: usize) -> Vec<Part> {
        let base = crate::craft::small::rocket_ref().parts[0].clone();
        (0..n)
            .map(|i| Part {
                id: i as u32,
                primitives: vec![i as u32],
                parent: i.checked_sub(1).map(|j| j as u32 / 2),
                ..base.clone()
            })
            .collect()
    }

    /// A binary tree of 7 parts (0; 1, 2; 3, 4 under 1; 5, 6 under 2).
    #[test]
    fn components_after_cuts_and_removals() {
        let parts = chain(7);
        // (cut, removed, groups)
        type Case<'a> = (&'a [u32], &'a [u32], Vec<Vec<u32>>);
        let cases: [Case; 6] = [
            (&[], &[], vec![vec![0, 1, 2, 3, 4, 5, 6]]),
            (&[1], &[], vec![vec![0, 2, 5, 6], vec![1, 3, 4]]),
            (&[3, 6], &[], vec![vec![0, 1, 2, 4, 5], vec![3], vec![6]]),
            (&[], &[1], vec![vec![0, 2, 5, 6], vec![3], vec![4]]),
            (&[], &[0], vec![vec![1, 3, 4], vec![2, 5, 6]]),
            (&[0], &[], vec![vec![0, 1, 2, 3, 4, 5, 6]]),
        ];
        for (cut, removed, want) in cases {
            assert_eq!(components(&parts, cut, removed), want, "cut {cut:?} removed {removed:?}");
        }
    }

    /// Two engines: thrust adds, the Isp is that of the summed flow, the
    /// mount is thrust-weighted, the wall heat adds.
    #[test]
    fn engines_combine_by_flow() {
        let a = crate::craft::engine::tests::test_file();
        let b = EngineFile {
            thrust_vac: 100e3,
            isp_vac: 300.0,
            isp_sl: 250.0,
            min_throttle: 0.4,
            gimbal_deg: 2.0,
            mount: Mount { pos: DVec3::new(2.0, 0.0, -1.0), dir: DVec3::Z },
            nozzle_heat_fraction: 1e-3,
        };
        let e = combine_engines(&[a, b]);
        assert_eq!(e.thrust_vac, 400e3);
        let flow = 300e3 / 320.0 + 100e3 / 300.0;
        assert!((e.isp_vac - 400e3 / flow).abs() < 1e-9);
        assert_eq!((e.min_throttle, e.gimbal_deg), (0.4, 2.0));
        assert!((e.mount.pos - (a.mount.pos * 0.75 + b.mount.pos * 0.25)).length() < 1e-12);
        let heat = |e: &EngineFile| e.nozzle_heat_fraction * e.thrust_vac * e.isp_vac;
        assert!((heat(&e) / (heat(&a) + heat(&b)) - 1.0).abs() < 1e-12);
        assert!(!has_engine(&combine_engines(&[])) && has_engine(&e));
    }
}
