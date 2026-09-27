//! Physical properties of bodies (shape, gravity field, rotation, atmosphere).
//! Motion lives in [`crate::ephem`]; this is everything else. The values come
//! from data files (`data/bodies/<body>/body.ron`, see [`data`]).

pub mod data;

pub use data::{default_bodies_dir, load_body, parse_body, BodyDef, DataError};

use crate::frame::{BodyFixed, Inertial, Vec3};
use crate::math;
use crate::terrain::{self, detail, Detail, Heightmap};
use crate::time::Epoch;
use glam::DVec3;
use std::sync::Arc;

/// IAU-style rotation model: pole right ascension/declination with linear
/// precession terms, and prime-meridian angle `W = w0 + w_rate * t`.
///
/// Body-fixed → inertial is `Rz(ra + 90°) · Rx(90° − dec) · Rz(W)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    /// Pole right ascension and declination at J2000 (rad).
    pub ra: f64,
    pub dec: f64,
    /// Linear pole drift (rad/s), from the IAU `T` terms.
    pub ra_rate: f64,
    pub dec_rate: f64,
    /// Prime meridian angle at J2000 (rad).
    pub w0: f64,
    /// Rotation rate (rad/s).
    pub w_rate: f64,
}

impl Rotation {
    pub fn angle(&self, t: Epoch) -> f64 {
        // Split to keep precision over centuries: whole days and the remainder.
        let s = t.whole_seconds();
        let days = s.div_euclid(86_400);
        let rem = (s.rem_euclid(86_400)) as f64 + t.fractional_second();
        let per_day = math::wrap_tau(self.w_rate * 86_400.0 * days as f64);
        math::wrap_tau(self.w0 + per_day + self.w_rate * rem)
    }

    /// Pole right ascension and declination at `t`.
    pub fn pole_radec(&self, t: Epoch) -> (f64, f64) {
        let dt = t.to_seconds_f64(); // centuries of drift: f64 seconds are ample here
        (self.ra + self.ra_rate * dt, self.dec + self.dec_rate * dt)
    }

    /// Unit pole vector at `t` (inertial axes).
    pub fn pole(&self, t: Epoch) -> DVec3 {
        let (ra, dec) = self.pole_radec(t);
        DVec3::new(math::cos(dec) * math::cos(ra), math::cos(dec) * math::sin(ra), math::sin(dec))
    }

    /// Angular velocity vector (inertial axes).
    pub fn omega(&self, t: Epoch) -> Vec3<Inertial> {
        Vec3::from_raw(self.pole(t) * self.w_rate)
    }

    pub fn to_inertial(&self, v: Vec3<BodyFixed>, t: Epoch) -> Vec3<Inertial> {
        let (ra, dec) = self.pole_radec(t);
        let x = math::rotate_z(v.raw(), self.angle(t));
        let x = math::rotate_x(x, math::PI / 2.0 - dec);
        Vec3::from_raw(math::rotate_z(x, ra + math::PI / 2.0))
    }

    pub fn to_fixed(&self, v: Vec3<Inertial>, t: Epoch) -> Vec3<BodyFixed> {
        let (ra, dec) = self.pole_radec(t);
        let x = math::rotate_z(v.raw(), -(ra + math::PI / 2.0));
        let x = math::rotate_x(x, -(math::PI / 2.0 - dec));
        Vec3::from_raw(math::rotate_z(x, -self.angle(t)))
    }
}

/// Exponential atmosphere (prototype model), and the air's properties the
/// aerodynamics and heating need (`sim::aero::air`, `sim::thermal`). The
/// air fields default to Earth air.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Atmosphere {
    /// Sea-level density (kg/m³).
    pub rho0: f64,
    /// Scale height (m).
    pub scale_height: f64,
    /// Altitude above which density is zero (m).
    pub top: f64,
    /// Sea-level pressure (Pa); falls with the same scale height (engines'
    /// back pressure). Zero if not given.
    #[serde(default)]
    pub p0: f64,
    /// Ratio of specific heats.
    #[serde(default = "air_defaults::gamma")]
    pub gamma: f64,
    /// Mean molar mass (kg/mol).
    #[serde(default = "air_defaults::molar_mass")]
    pub molar_mass: f64,
    /// Mean free path at `rho0` (m); λ ∝ 1/ρ.
    #[serde(default = "air_defaults::mean_free_path")]
    pub mean_free_path: f64,
    /// Sutton–Graves stagnation heating constant (SI, `thermal::sutton_graves`).
    #[serde(default = "air_defaults::sutton_graves_k")]
    pub sutton_graves_k: f64,
}

/// Earth air: the defaults of [`Atmosphere`]'s air fields.
mod air_defaults {
    pub fn gamma() -> f64 {
        crate::aero::air::EARTH_AIR_GAMMA
    }
    pub fn molar_mass() -> f64 {
        crate::aero::air::EARTH_AIR_MOLAR_MASS
    }
    pub fn mean_free_path() -> f64 {
        crate::aero::air::EARTH_MEAN_FREE_PATH_SL
    }
    pub fn sutton_graves_k() -> f64 {
        crate::thermal::heating::SUTTON_GRAVES_EARTH
    }
}

impl Atmosphere {
    /// Ambient pressure (Pa) at `altitude`.
    pub fn pressure(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else {
            self.p0 * math::exp(-altitude.max(0.0) / self.scale_height)
        }
    }

    pub fn density(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else {
            self.rho0 * math::exp(-altitude.max(0.0) / self.scale_height)
        }
    }

    /// Temperature (K) of the isothermal atmosphere this scale height
    /// implies, for surface gravity `g0` (m/s²).
    pub fn temperature(&self, g0: f64) -> f64 {
        crate::aero::air::isothermal_temperature(self.scale_height, g0, self.molar_mass)
    }

    /// Mean free path (m) at density `rho` (infinite in vacuum).
    pub fn mean_free_path_at(&self, rho: f64) -> f64 {
        crate::aero::air::mean_free_path(self.mean_free_path, self.rho0, rho)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyPhysical {
    pub name: String,
    pub radius_eq: f64,
    pub radius_polar: f64,
    /// Zonal harmonic J2 (dimensionless), referenced to `radius_eq`.
    pub j2: f64,
    pub rotation: Rotation,
    pub atmosphere: Option<Atmosphere>,
    /// Has a surface that vessels can land on or hit.
    pub solid: bool,
    /// Height of a solid ocean surface above the reference (m), if any
    /// (D037: oceans are solid at sea level).
    pub sea_level: Option<f64>,
    /// Terrain heights relative to the reference ellipsoid (D047), shared
    /// between clones of the world. `None`: the smooth ellipsoid.
    pub terrain: Option<Arc<Heightmap>>,
    /// Procedural detail below the heightmap's resolution (D059). `None`: none.
    pub detail: Option<Detail>,
    /// Rails warp is not allowed below this altitude above the reference
    /// (m; D062), chosen by [`rails_floor_rule`]. 0 for bodies without one.
    pub rails_floor: f64,
    /// Radiated power (W): non-zero for stars (sunlight on vessels).
    pub luminosity: f64,
}

/// Margin above the highest terrain for the rails floor of an airless body (m).
pub const RAILS_FLOOR_MARGIN: f64 = 2_000.0;

/// The rails-warp floor rule (D062): the top of the atmosphere, or on an
/// airless body the highest terrain plus a margin, rounded up to a whole km.
pub fn rails_floor_rule(atmosphere_top: Option<f64>, max_terrain: f64) -> f64 {
    let floor = atmosphere_top.unwrap_or(max_terrain.max(0.0) + RAILS_FLOOR_MARGIN);
    (floor / 1000.0).ceil() * 1000.0
}

impl BodyPhysical {
    /// Terrain height (m) above the reference ellipsoid in a body-fixed
    /// direction (any length > 0): the heightmap (bicubic) plus procedural
    /// detail, **not** raised to sea level. This is the one surface (D047,
    /// D059); renderers that draw the sea themselves use it, everything else
    /// uses [`Self::surface_height`].
    pub fn terrain_height(&self, dir: DVec3) -> f64 {
        let (lat, lon) = terrain::lat_lon(dir);
        self.terrain_at(dir.normalize(), lat, lon)
    }

    /// [`Self::terrain_height`] at unit direction `dir` whose latitude and longitude are `lat`, `lon`.
    fn terrain_at(&self, dir: DVec3, lat: f64, lon: f64) -> f64 {
        let base = self.terrain.as_ref().map_or(0.0, |t| t.sample(lat, lon));
        match &self.detail {
            Some(d) => base + d.height(dir, lat, lon, self.radius_eq, detail::coast_weight(base, self.sea_level)),
            None => base,
        }
    }

    /// Height (m) of the solid surface above the reference ellipsoid at
    /// latitude/longitude (rad): the terrain, raised to sea level where the
    /// body has an ocean (oceans are solid, D037).
    pub fn surface_height_latlon(&self, lat: f64, lon: f64) -> f64 {
        let h = if self.detail.is_some() {
            let dir = DVec3::new(math::cos(lat) * math::cos(lon), math::cos(lat) * math::sin(lon), math::sin(lat));
            self.terrain_at(dir, lat, lon)
        } else {
            self.terrain.as_ref().map_or(0.0, |t| t.sample(lat, lon))
        };
        self.sea_level.map_or(h, |sea| h.max(sea))
    }

    /// [`Self::surface_height_latlon`] in the direction of a body-fixed vector.
    pub fn surface_height(&self, dir: Vec3<BodyFixed>) -> f64 {
        let h = self.terrain_height(dir.raw());
        self.sea_level.map_or(h, |sea| h.max(sea))
    }

    /// Height of a body-fixed point above the solid surface (terrain or sea) (m).
    /// This is what contact and landing use; [`Self::altitude`] (above the
    /// ellipsoid) is what the atmosphere uses.
    pub fn altitude_above_surface(&self, p: Vec3<BodyFixed>) -> f64 {
        self.altitude(p) - self.surface_height(p)
    }

    /// Body-fixed point `height` above the solid surface at latitude/longitude (rad).
    pub fn ground_point(&self, lat: f64, lon: f64, height: f64) -> Vec3<BodyFixed> {
        self.surface_point(lat, lon, self.surface_height_latlon(lat, lon) + height)
    }

    /// Height above the reference ellipsoid of a body-fixed point (m).
    /// Uses the geocentric-radius approximation (exact on the axes and equator;
    /// < 1 m off elsewhere for Earth's flattening at low altitude).
    pub fn altitude(&self, p: Vec3<BodyFixed>) -> f64 {
        let r = p.raw();
        let d = r.length();
        d - self.ellipsoid_radius(r.z / d)
    }

    /// Radius of the reference ellipsoid (m) in a direction whose sine of
    /// geocentric latitude is `sin_lat`.
    pub fn ellipsoid_radius(&self, sin_lat: f64) -> f64 {
        let (a, b) = (self.radius_eq, self.radius_polar);
        let s2 = sin_lat * sin_lat;
        (a * b) / (b * b * (1.0 - s2) + a * a * s2).sqrt()
    }

    /// Body-fixed position of a point at geodetic-ish latitude/longitude (rad)
    /// on the ellipsoid surface plus `height` along the radial direction.
    pub fn surface_point(&self, lat: f64, lon: f64, height: f64) -> Vec3<BodyFixed> {
        let dir = DVec3::new(math::cos(lat) * math::cos(lon), math::cos(lat) * math::sin(lon), math::sin(lat));
        Vec3::from_raw(dir * (self.ellipsoid_radius(dir.z) + height))
    }
}

/// Earth from the default data directory (panics if the data is missing).
pub fn earth() -> BodyPhysical {
    data::load_default("earth").physical
}

/// The Moon from the default data directory (panics if the data is missing).
pub fn moon() -> BodyPhysical {
    data::load_default("moon").physical
}

/// The Sun from the default data directory (panics if the data is missing).
pub fn sun() -> BodyPhysical {
    data::load_default("sun").physical
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEG: f64 = math::PI / 180.0;

    #[test]
    fn rotation_round_trips_and_spins_at_sidereal_rate() {
        let e = earth();
        let t = Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0);
        let p: Vec3<BodyFixed> = Vec3::new(6.4e6, 1.0e5, 2.0e6);
        let back = e.rotation.to_fixed(e.rotation.to_inertial(p, t), t);
        assert!((back - p).length() < 1e-6);
        // One sidereal day later the body-fixed point is back where it was
        // (checked without pole precession, which moves it ~2.5 m/day).
        let spin_only = Rotation { ra_rate: 0.0, dec_rate: 0.0, ..e.rotation };
        let sidereal = math::TAU / spin_only.w_rate;
        let a = spin_only.to_inertial(p, t);
        let b = spin_only.to_inertial(p, t.add_seconds(sidereal));
        assert!((a - b).length() < 1e-3, "{}", (a - b).length());
        assert!((sidereal - 86_164.09).abs() < 0.1);
    }

    #[test]
    fn moon_pole_points_near_ecliptic_north() {
        // The Moon's pole is ~1.5° from the ecliptic pole (obliquity 23.44°).
        let ecl_pole = DVec3::new(0.0, -math::sin(23.439 * DEG), math::cos(23.439 * DEG));
        let angle = math::acos(moon().rotation.pole(Epoch::J2000).dot(ecl_pole)) / DEG;
        assert!(angle < 2.0, "{angle}");
    }

    #[test]
    fn altitude_of_surface_points_is_zero() {
        let e = earth();
        for lat in [-80.0, -30.0, 0.0, 28.5, 60.0, 89.0] {
            let p = e.surface_point(lat * DEG, 1.0, 0.0);
            assert!(e.altitude(p).abs() < 1e-6, "lat {lat}: {}", e.altitude(p));
        }
    }
}
