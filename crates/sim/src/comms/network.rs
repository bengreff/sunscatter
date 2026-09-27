//! The relay network: nodes (sites and vessels), usable links, and the
//! least-delay path between two nodes.

use super::link::{above_horizon, clear_line_of_sight, rate_bps, Occluder};
use super::{Antenna, LinkParams, C};
use glam::DVec3;

/// A network node at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// Position in the common frame.
    pub pos: DVec3,
    /// Without an antenna a node only uses the ground network.
    pub antenna: Option<Antenna>,
    /// For a ground site: the occluder (index) it stands on, and local up.
    pub ground: Option<(usize, DVec3)>,
}

/// A usable link.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge {
    to: usize,
    delay: f64,
    rate: f64,
}

/// Every usable link between the nodes, at one instant.
#[derive(Clone, Debug)]
pub struct Graph {
    edges: Vec<Vec<Edge>>,
}

/// A chain of links: nodes from sender to receiver, one-way delay (s) and
/// rate (bit/s, the slowest link; infinite on the ground network only).
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub nodes: Vec<usize>,
    pub delay: f64,
    pub rate: f64,
}

impl Graph {
    pub fn new(nodes: &[Node], occluders: &[Occluder], p: &LinkParams) -> Self {
        let mut edges = vec![Vec::new(); nodes.len()];
        for i in 0..nodes.len() {
            for j in i + 1..nodes.len() {
                if let Some((delay, rate)) = link(&nodes[i], &nodes[j], occluders, p) {
                    edges[i].push(Edge { to: j, delay, rate });
                    edges[j].push(Edge { to: i, delay, rate });
                }
            }
        }
        Graph { edges }
    }

    /// Whether `a` and `b` are directly linked.
    pub fn linked(&self, a: usize, b: usize) -> bool {
        self.edges[a].iter().any(|e| e.to == b)
    }
}

/// The link between two nodes, if usable: (delay, rate).
fn link(a: &Node, b: &Node, occluders: &[Occluder], p: &LinkParams) -> Option<(f64, f64)> {
    // Two sites on one body: the ground network, along the great circle.
    if let (Some((ba, ua)), Some((bb, ub))) = (a.ground, b.ground) {
        if ba == bb {
            let o = occluders[ba];
            let angle = crate::math::acos(ua.dot(ub).clamp(-1.0, 1.0));
            return Some((o.radius * angle / (p.ground_speed_factor * C), f64::INFINITY));
        }
    }
    let (aa, ab) = (a.antenna?, b.antenna?);
    for (from, to) in [(a, b), (b, a)] {
        if let Some((_, up)) = from.ground {
            if !above_horizon(from.pos, up, to.pos, p.min_elevation_deg) {
                return None;
            }
        }
    }
    if !clear_line_of_sight(a.pos, b.pos, occluders) {
        return None;
    }
    let d = (a.pos - b.pos).length();
    let rate = rate_bps(p, aa, ab, d);
    (rate >= p.min_rate_bps).then_some((d / C, rate))
}

/// The least-delay path from `from` to `to` (Dijkstra; the graph is tiny).
pub fn best_path(g: &Graph, from: usize, to: usize) -> Option<Path> {
    let n = g.edges.len();
    let mut delay = vec![f64::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut done = vec![false; n];
    delay[from] = 0.0;
    loop {
        // The nearest unfinished node (ties: lowest index, deterministic).
        let next = (0..n).filter(|&i| !done[i] && delay[i].is_finite()).min_by(|&a, &b| delay[a].total_cmp(&delay[b]));
        let Some(u) = next else { break };
        if u == to {
            break;
        }
        done[u] = true;
        for e in &g.edges[u] {
            let d = delay[u] + e.delay;
            if d < delay[e.to] {
                delay[e.to] = d;
                prev[e.to] = u;
            }
        }
    }
    if !delay[to].is_finite() {
        return None;
    }
    let mut nodes = vec![to];
    while *nodes.last().expect("non-empty") != from {
        nodes.push(prev[*nodes.last().expect("non-empty")]);
    }
    nodes.reverse();
    let rate = nodes
        .windows(2)
        .map(|w| g.edges[w[0]].iter().find(|e| e.to == w[1]).map_or(0.0, |e| e.rate))
        .fold(f64::INFINITY, f64::min);
    Some(Path { nodes, delay: delay[to], rate })
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6.371e6;

    fn params() -> LinkParams {
        LinkParams {
            frequency_hz: 8.4e9,
            bandwidth_hz: 1.0e6,
            noise_temperature_k: 50.0,
            min_rate_bps: 10.0,
            min_elevation_deg: 6.0,
            ground_speed_factor: 0.7,
        }
    }

    fn site(lon_deg: f64, antenna: bool) -> Node {
        let a = lon_deg.to_radians();
        let up = DVec3::new(a.cos(), a.sin(), 0.0);
        Node {
            pos: up * R,
            antenna: antenna.then_some(Antenna { gain_dbi: 74.0, power_w: 2e4 }),
            ground: Some((0, up)),
        }
    }

    fn craft(pos: DVec3) -> Node {
        Node { pos, antenna: Some(Antenna { gain_dbi: 20.0, power_w: 20.0 }), ground: None }
    }

    #[test]
    fn mission_control_reaches_a_ship_through_the_station_that_sees_it() {
        let earth = Occluder { centre: DVec3::ZERO, radius: R };
        // 0: mission control (no antenna) at lon 0; 1: a station at 90°;
        // 2: a station at 180°; 3: a ship in LEO above lon 180°.
        let nodes = [site(0.0, false), site(90.0, true), site(180.0, true), craft(DVec3::new(-(R + 4e5), 0.0, 0.0))];
        let g = Graph::new(&nodes, &[earth], &params());
        assert!(!g.linked(1, 3), "the station at 90° has the ship below its horizon");
        let path = best_path(&g, 0, 3).expect("a path");
        assert_eq!(path.nodes, vec![0, 2, 3]);
        // Half the equator by ground at 0.7 c, then 400 km up.
        let expected = R * std::f64::consts::PI / (0.7 * C) + 4e5 / C;
        assert!((path.delay - expected).abs() < 1e-9, "{} vs {expected}", path.delay);
        assert!(path.rate.is_finite() && path.rate > 1e5);
    }

    #[test]
    fn a_ship_behind_the_moon_has_no_path() {
        let earth = Occluder { centre: DVec3::ZERO, radius: R };
        let moon = Occluder { centre: DVec3::new(3.84e8, 0.0, 0.0), radius: 1.737e6 };
        let nodes = [site(0.0, true), craft(DVec3::new(3.84e8 + 2e6, 0.0, 0.0))];
        let g = Graph::new(&nodes, &[earth, moon], &params());
        assert!(best_path(&g, 0, 1).is_none());
        // Moved off the far side: visible, ~1.26 s from the surface site.
        let ship = DVec3::new(3.84e8, 3e6, 0.0);
        let nodes = [site(0.0, true), craft(ship)];
        let g = Graph::new(&nodes, &[earth, moon], &params());
        let p = best_path(&g, 0, 1).expect("visible");
        assert!((p.delay - (ship - DVec3::X * R).length() / C).abs() < 1e-12, "{}", p.delay);
    }

    #[test]
    fn a_relay_ship_carries_the_signal_around_the_body() {
        let moon = Occluder { centre: DVec3::ZERO, radius: 1.737e6 };
        // A lander on the far side (ground on body 0) and a relay above it
        // that also sees a distant station (a ship far away on the other side).
        let up = DVec3::X;
        let lander =
            Node { pos: up * 1.737e6, antenna: Some(Antenna { gain_dbi: 20.0, power_w: 20.0 }), ground: Some((0, up)) };
        let relay = craft(DVec3::new(1.737e6 + 5e6, 0.0, 1e7));
        let far = craft(DVec3::new(-3e8, 0.0, 1e8));
        let g = Graph::new(&[lander, relay, far], &[moon], &params());
        assert!(!g.linked(0, 2));
        assert_eq!(best_path(&g, 0, 2).expect("relayed").nodes, vec![0, 1, 2]);
    }
}
