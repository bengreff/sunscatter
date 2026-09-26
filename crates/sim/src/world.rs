//! The simulated world: the ephemeris plus, for each gravity source, the
//! physical data the ship dynamics need (J2, surface, atmosphere, anchoring).
//! The physical data comes from `data/bodies/<body>/body.ron` ([`crate::body::data`]).

use crate::body::data::{body_dir_name, default_bodies_dir, load_body, BODY_FILE};
use crate::body::{BodyPhysical, DataError};
use crate::ephem::{Ephemeris, NodeKind, Snapshot};
use crate::frame::NodeId;
use crate::time::Epoch;
use std::path::Path;
use std::sync::Arc;

/// Distances (m) at which a body becomes / stops being the preferred anchor.
/// Pure precision policy: any value is *correct* (invariance tests), these
/// just keep offsets small near each body.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnchorZone {
    pub enter: f64,
    pub exit: f64,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub node: NodeId,
    pub name: String,
    pub gm: f64,
    /// Physical body data (surface, J2, rotation, atmosphere), if modelled.
    pub physical: Option<BodyPhysical>,
    /// The source's 1PN field acts on vessels (the Sun).
    pub relativistic: bool,
    pub anchor_zone: Option<AnchorZone>,
}

#[derive(Clone, Debug)]
pub struct World {
    pub eph: Arc<Ephemeris>,
    pub sources: Vec<Source>,
    /// Gravity sources whose acceleration at the vessel is below this (m/s²)
    /// at the start of a segment are not simulated for that segment (D023).
    pub cutoff: f64,
    /// Sources with anchor zones, smallest zone first (anchor policy order).
    pub anchor_order: Vec<usize>,
}

impl World {
    /// The Solar System world from the bodies in this source tree's
    /// `data/bodies` (panics if that data is missing or invalid).
    pub fn sol(eph: Arc<Ephemeris>) -> Self {
        Self::from_dir(eph, &default_bodies_dir()).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Builds the world from body data in `dir`: every body node of the
    /// ephemeris named `N` takes its physical data from `dir/<n>/body.ron`
    /// (lower-case name) if that file exists, and is a point mass otherwise.
    pub fn from_dir(eph: Arc<Ephemeris>, dir: &Path) -> Result<Self, DataError> {
        let mut sources = Vec::new();
        for id in eph.bodies() {
            let n = eph.node(id);
            let body_dir = dir.join(body_dir_name(&n.name));
            let def = if body_dir.join(BODY_FILE).exists() {
                let mut def = load_body(&body_dir)?;
                def.load_terrain()?;
                Some(def)
            } else {
                None
            };
            if let Some(d) = &def {
                if d.physical.name != n.name {
                    let message = format!("name {:?} does not match ephemeris node {:?}", d.physical.name, n.name);
                    return Err(DataError { path: body_dir.join(BODY_FILE), message });
                }
            }
            let (physical, relativistic, anchor_zone) =
                def.map_or((None, false, None), |d| (Some(d.physical), d.relativistic, d.anchor_zone));
            sources.push(Source { node: id, name: n.name.clone(), gm: n.gm, physical, relativistic, anchor_zone });
        }
        debug_assert!(eph.nodes().iter().any(|n| n.kind == NodeKind::Barycenter));
        let mut anchor_order: Vec<usize> = (0..sources.len()).filter(|&i| sources[i].anchor_zone.is_some()).collect();
        anchor_order.sort_by(|&a, &b| {
            let zone = |i: usize| sources[i].anchor_zone.map_or(f64::INFINITY, |z| z.enter);
            zone(a).total_cmp(&zone(b))
        });
        Ok(World { eph, sources, cutoff: 1e-11, anchor_order })
    }

    pub fn source(&self, node: NodeId) -> Option<&Source> {
        self.sources.iter().find(|s| s.node == node)
    }

    pub fn find(&self, name: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.name == name)
    }

    pub fn snapshot(&self, t: Epoch) -> Snapshot<'_> {
        self.eph.snapshot(t)
    }

    /// Sources with a solid surface (candidates for landing and impact).
    pub fn surfaces(&self) -> impl Iterator<Item = &Source> {
        self.sources.iter().filter(|s| s.physical.as_ref().is_some_and(|p| p.solid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sol::EPHEMERIS_PATH;

    fn eph() -> Arc<Ephemeris> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(EPHEMERIS_PATH);
        Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap())
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sunscatter-world-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mars")).unwrap();
        dir
    }

    const ROCK: &str = r#"(physical: (name: "NAME", radius_eq: 3396190.0, radius_polar: 3376200.0,
        rotation: (ra_deg: 317.68, dec_deg: 52.89, w0_deg: 176.63, w_rate_deg_per_day: 350.89), solid: true))"#;

    #[test]
    fn adding_a_body_needs_only_data() {
        let dir = temp_dir("mars");
        std::fs::write(dir.join("mars/body.ron"), ROCK.replace("NAME", "Mars")).unwrap();
        let w = World::from_dir(eph(), &dir).unwrap();
        let mars = w.find("Mars").unwrap().physical.as_ref().unwrap();
        assert_eq!(mars.radius_polar, 3_376_200.0);
        assert!(w.find("Earth").unwrap().physical.is_none(), "no Earth data in this directory");
        assert_eq!(w.surfaces().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Mars"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_name_that_does_not_match_the_ephemeris_is_an_error() {
        let dir = temp_dir("badname");
        std::fs::write(dir.join("mars/body.ron"), ROCK.replace("NAME", "Marz")).unwrap();
        let e = World::from_dir(eph(), &dir).unwrap_err();
        assert!(e.message.contains("Marz"), "{e}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sol_world_has_the_shipped_bodies() {
        let w = World::sol(eph());
        let solid: Vec<&str> = w.surfaces().map(|s| s.name.as_str()).collect();
        assert_eq!(solid, ["Earth", "Moon"]);
        assert!(w.find("Sun").unwrap().relativistic);
        let order: Vec<&str> = w.anchor_order.iter().map(|&i| w.sources[i].name.as_str()).collect();
        assert_eq!(order, ["Moon", "Earth"]);
    }
}
