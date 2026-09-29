//! The network's fixed parts in a world at one instant: occluding bodies
//! and ground sites, positioned relative to an anchor through the frame tree.

use super::{CommsData, Node, Occluder};
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;

/// Every body with physical data as an occluder (its reference ellipsoid,
/// pole at `t`), in the order of `World::surfaces`, relative to `anchor` at
/// `t`.
pub fn occluders(world: &World, t: Epoch, anchor: NodeId) -> Vec<(NodeId, Occluder)> {
    let snap = world.snapshot(t);
    world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            let centre = snap.relative_r(s.node, anchor);
            let pole = p.rotation.pole(t);
            Some((s.node, Occluder { centre, radius: p.radius_eq, polar_radius: p.radius_polar, pole }))
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
            Some(Node {
                pos: bodies[k].1.centre + rel,
                antenna: site.antenna,
                ground: Some((k, up)),
                min_elevation_deg: site.min_elevation(&data.link),
                wired: true,
            })
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

    /// The sites and vessels in flight at (lat, lon, height) (deg, deg, m)
    /// above Earth at the sol epoch: the data, the nodes (sites first, then
    /// the vessels) and the graph.
    fn earth_network(vessels: &[(f64, f64, f64)]) -> (CommsData, Vec<Node>, Vec<Occluder>, Graph) {
        let w = world();
        let data = CommsData::load(&CommsData::default_path()).unwrap();
        let earth = w.find("Earth").unwrap();
        let t = crate::sol::sol_epoch();
        let bodies = occluders(&w, t, earth.node);
        let mut nodes: Vec<Node> = site_nodes(&w, &data, t, &bodies).into_iter().map(|n| n.unwrap()).collect();
        let p = earth.physical.as_ref().unwrap();
        let rad = crate::math::PI / 180.0;
        for &(lat, lon, h) in vessels {
            let fixed = p.surface_point(lat * rad, lon * rad, h);
            nodes.push(Node {
                pos: p.rotation.to_inertial(fixed, t).raw(),
                antenna: Some(crate::comms::Antenna { gain_dbi: 20.0, power_w: 20.0 }),
                ground: None,
                min_elevation_deg: 0.0,
                wired: false,
            });
        }
        let occ: Vec<Occluder> = bodies.iter().map(|b| b.1).collect();
        let g = Graph::new(&nodes, &occ, &data.link);
        (data, nodes, occ, g)
    }

    #[test]
    fn houston_reaches_a_geostationary_ship_over_the_americas_through_a_station_there() {
        // A ship 36,000 km above the ground point at 100° W, 0° N.
        let (data, nodes, _, g) = earth_network(&[(0.0, -100.0, 3.6e7)]);
        let hq = data.site("Houston").unwrap();
        let path = best_path(&g, hq, nodes.len() - 1).expect("a path");
        let relay = &data.sites[path.nodes[1]];
        assert!(relay.lon_deg < -60.0 && relay.lon_deg > -120.0, "{}", relay.name);
        assert!(path.delay > 0.12 && path.delay < 0.14, "{}", path.delay);
        // Canberra is on the other side of the planet.
        assert!(!g.linked(data.site("Canberra").unwrap(), nodes.len() - 1));
    }

    #[test]
    fn a_vessel_just_off_the_pad_is_heard_through_the_pad_radio() {
        // At liftoff (2 m above the ground) and 100 m above the pad, in
        // flight (no umbilical).
        let w = world();
        let p = w.find("Earth").unwrap().physical.clone().unwrap();
        let rad = crate::math::PI / 180.0;
        let ground = p.surface_height_latlon(28.6082 * rad, -80.66 * rad);
        let (data, nodes, _, g) = earth_network(&[(28.6082, -80.66, ground + 2.0), (28.6082, -80.66, ground + 100.0)]);
        let n = nodes.len();
        for ship in [n - 2, n - 1] {
            assert!(g.linked(data.site("Kennedy pad").unwrap(), ship), "{ship}");
            let path = best_path(&g, data.site("Houston").unwrap(), ship).expect("a path home");
            assert!(path.delay < 0.02, "{}", path.delay);
        }
        // Merritt Island (11.5 km away) has it below its 0.5° mask.
        assert!(!g.linked(data.site("Merritt Island").unwrap(), n - 1));
    }

    #[test]
    fn at_one_and_a_half_kilometres_merritt_island_has_it() {
        let (data, nodes, _, g) = earth_network(&[(28.6082, -80.66, 1500.0)]);
        assert!(g.linked(data.site("Merritt Island").unwrap(), nodes.len() - 1));
    }

    #[test]
    fn the_range_stations_follow_a_due_east_ascent() {
        // Points along a due-east ascent from the Cape (lat, lon, height): at
        // least one Eastern Range station sees each.
        let track = [(28.5, -78.0, 60e3), (27.5, -72.0, 120e3), (25.5, -65.0, 170e3), (22.0, -57.0, 200e3)];
        let (data, nodes, _, g) = earth_network(&track);
        let range = ["Merritt Island", "Jonathan Dickinson", "Bermuda", "Antigua"].map(|n| data.site(n).unwrap());
        for (k, point) in track.iter().enumerate() {
            let ship = nodes.len() - track.len() + k;
            assert!(range.iter().any(|&s| g.linked(s, ship)), "{point:?}");
        }
    }

    #[test]
    fn the_earth_blocks_vessels_on_its_far_side_even_low_ones() {
        // 0: a ship in LEO over the Cape's antipode; 1, 2: low vessels (2 km,
        // inside the equatorial-radius sphere at these latitudes) far apart.
        let (data, nodes, occ, g) = earth_network(&[(-28.6, 99.34, 4e5), (45.0, 0.0, 2e3), (45.0, 90.0, 2e3)]);
        let n = nodes.len();
        for site in ["Kennedy pad", "Merritt Island", "Goldstone", "Bermuda"] {
            assert!(!g.linked(data.site(site).unwrap(), n - 3), "{site}");
        }
        assert!(!crate::comms::clear_line_of_sight(nodes[n - 2].pos, nodes[n - 1].pos, &occ));
        // Madrid (40° N, 4° W) is over the horizon of the low vessel at 90° E.
        let madrid = nodes[data.site("Madrid").unwrap()].pos;
        assert!(!crate::comms::clear_line_of_sight(madrid, nodes[n - 1].pos, &occ));
        // ... and two low vessels a few kilometres apart see each other.
        let (_, near, occ, _) = earth_network(&[(45.0, 0.0, 2e3), (45.0, 0.05, 2e3)]);
        assert!(crate::comms::clear_line_of_sight(near[near.len() - 2].pos, near[near.len() - 1].pos, &occ));
    }
}
