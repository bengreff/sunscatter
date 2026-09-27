//! The network's fixed parts in a world at one instant: occluding bodies
//! and ground sites, positioned relative to an anchor through the frame tree.

use super::{CommsData, Node, Occluder};
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;

/// Every body with physical data as an occluder (radius: its equatorial
/// radius), in the order of `World::surfaces`, relative to `anchor` at `t`.
pub fn occluders(world: &World, t: Epoch, anchor: NodeId) -> Vec<(NodeId, Occluder)> {
    let snap = world.snapshot(t);
    world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            Some((s.node, Occluder { centre: snap.relative_r(s.node, anchor), radius: p.radius_eq }))
        })
        .collect()
}

/// The data's sites as network nodes (in `data.sites` order), relative to
/// `anchor` at `t`; `bodies` is the output of [`occluders`]. A site on a
/// body the world does not model is left out (`None`).
pub fn site_nodes(world: &World, data: &CommsData, t: Epoch, bodies: &[(NodeId, Occluder)]) -> Vec<Option<Node>> {
    data.sites
        .iter()
        .map(|site| {
            let src = world.find(&site.body)?;
            let p = src.physical.as_ref()?;
            let k = bodies.iter().position(|(n, _)| *n == src.node)?;
            let (rel, up) = site.inertial(p, t);
            Some(Node { pos: bodies[k].1.centre + rel, antenna: site.antenna, ground: Some((k, up)), wired: true })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::{best_path, Graph};

    fn world() -> World {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(crate::sol::EPHEMERIS_PATH);
        let eph = crate::ephem::Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap();
        World::sol(std::sync::Arc::new(eph))
    }

    #[test]
    fn houston_reaches_a_geostationary_ship_over_the_americas_through_goldstone() {
        let w = world();
        let data = CommsData::load(&CommsData::default_path()).unwrap();
        let earth = w.find("Earth").unwrap();
        let t = crate::sol::sol_epoch();
        let bodies = occluders(&w, t, earth.node);
        let mut nodes: Vec<Node> = site_nodes(&w, &data, t, &bodies).into_iter().map(|n| n.unwrap()).collect();
        // A ship 36,000 km above the ground point at 100° W, 0° N.
        let p = earth.physical.as_ref().unwrap();
        let rad = crate::math::PI / 180.0;
        let fixed = p.surface_point(0.0, -100.0 * rad, 3.6e7);
        let ship = p.rotation.to_inertial(fixed, t).raw();
        nodes.push(Node {
            pos: ship,
            antenna: Some(crate::comms::Antenna { gain_dbi: 20.0, power_w: 20.0 }),
            ground: None,
            wired: false,
        });
        let g = Graph::new(&nodes, &bodies.iter().map(|b| b.1).collect::<Vec<_>>(), &data.link);
        let hq = data.site("Houston").unwrap();
        let path = best_path(&g, hq, nodes.len() - 1).expect("a path");
        assert_eq!(data.sites[path.nodes[1]].name, "Goldstone");
        assert!(path.delay > 0.12 && path.delay < 0.14, "{}", path.delay);
        // Canberra is on the other side of the planet.
        assert!(!g.linked(data.site("Canberra").unwrap(), nodes.len() - 1));
    }
}
