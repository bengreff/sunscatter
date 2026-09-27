//! Communication (D067, D068): ground sites, the link model, light-time and
//! relay paths.
//!
//! Everything here is a pure function of positions and data. Positions are
//! passed in one common inertial frame (the caller forms them through the
//! frame tree, rule 3). Nothing here moves anything: a link's delay and rate
//! only decide what a control location sees and when its commands arrive.
//!
//! Model:
//! - A link exists when the line of sight is clear (bodies are spheres; a
//!   ground site also needs its partner above `min_elevation_deg`) and the
//!   Shannon rate of the link budget is at least `min_rate_bps`.
//! - Sites on one body are joined by a ground network at
//!   `ground_speed_factor` × c along the great circle, with no rate limit.
//! - Light time per hop solves the light-time equation (`light`).
//! - A path is the least-delay chain of usable links (`network`).

mod light;
mod link;
mod network;
mod world;

pub use light::light_time;
pub use link::{clear_line_of_sight, rate_bps, Occluder};
pub use network::{best_path, Graph, Node, Path};
pub use world::{occluders, site_nodes};

use crate::body::BodyPhysical;
use crate::frame::{BodyFixed, Vec3};
use crate::math;
use crate::time::Epoch;
use glam::DVec3;
use serde::Deserialize;
use std::path::{Path as FsPath, PathBuf};

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;
/// Boltzmann's constant (J/K).
pub const K_B: f64 = 1.380_649e-23;

/// A transmitter/receiver: gain (dBi, the same both ways) and transmit power.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Antenna {
    pub gain_dbi: f64,
    pub power_w: f64,
}

/// The link model's constants.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct LinkParams {
    pub frequency_hz: f64,
    pub bandwidth_hz: f64,
    pub noise_temperature_k: f64,
    pub min_rate_bps: f64,
    pub min_elevation_deg: f64,
    pub ground_speed_factor: f64,
    /// A vessel landed within this distance of a launch site is on the
    /// ground network through the pad's umbilical (m).
    pub umbilical_range_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum SiteKind {
    /// A control location (D067).
    MissionControl,
    /// A relay on the ground.
    GroundStation,
    /// A launch pad: wired to the ground network, with an umbilical to a
    /// vessel standing on it.
    LaunchSite,
}

/// A site on a body's surface.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Site {
    pub name: String,
    pub body: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    pub kind: SiteKind,
    pub antenna: Option<Antenna>,
}

impl Site {
    /// Body-fixed position on `body`.
    pub fn fixed(&self, body: &BodyPhysical) -> Vec3<BodyFixed> {
        let rad = math::PI / 180.0;
        body.surface_point(self.lat_deg * rad, self.lon_deg * rad, self.height_m)
    }

    /// Position relative to the body's centre in inertial axes at `t`, and
    /// the local up (unit).
    pub fn inertial(&self, body: &BodyPhysical, t: Epoch) -> (DVec3, DVec3) {
        let p = body.rotation.to_inertial(self.fixed(body), t).raw();
        (p, p.normalize())
    }
}

/// The comms data file.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CommsData {
    pub link: LinkParams,
    pub sites: Vec<Site>,
}

impl CommsData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let data: CommsData = ron::from_str(text).map_err(|e| e.to_string())?;
        let l = &data.link;
        let positive = [l.frequency_hz, l.bandwidth_hz, l.noise_temperature_k, l.min_rate_bps, l.ground_speed_factor];
        if positive.iter().any(|x| !(x.is_finite() && *x > 0.0)) || l.ground_speed_factor > 1.0 {
            return Err("link parameters must be positive (ground speed factor at most 1)".into());
        }
        for s in &data.sites {
            if !(s.lat_deg.abs() <= 90.0 && s.lon_deg.abs() <= 360.0 && s.height_m.is_finite()) {
                return Err(format!("site {}: bad position", s.name));
            }
        }
        Ok(data)
    }

    pub fn load(path: &FsPath) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// `data/comms.ron` of this source tree.
    pub fn default_path() -> PathBuf {
        FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../data/comms.ron")
    }

    pub fn site(&self, name: &str) -> Option<usize> {
        self.sites.iter().position(|s| s.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_loads_with_mission_control_and_the_dsn() {
        let d = CommsData::load(&CommsData::default_path()).expect("comms.ron");
        let hq = d.site("Houston").expect("Houston");
        assert_eq!(d.sites[hq].kind, SiteKind::MissionControl);
        for dsn in ["Goldstone", "Madrid", "Canberra"] {
            let s = &d.sites[d.site(dsn).expect(dsn)];
            assert_eq!(s.kind, SiteKind::GroundStation);
            assert!(s.antenna.is_some());
        }
    }

    #[test]
    fn bad_link_parameters_are_rejected() {
        let text = std::fs::read_to_string(CommsData::default_path()).unwrap();
        assert!(CommsData::parse(&text.replace("bandwidth_hz: 1.0e6", "bandwidth_hz: -1.0")).is_err());
        assert!(CommsData::parse(&text.replace("ground_speed_factor: 0.7", "ground_speed_factor: 1.5")).is_err());
    }
}
