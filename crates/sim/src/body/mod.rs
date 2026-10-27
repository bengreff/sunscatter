//! Physical properties of bodies (shape, gravity field, rotation, atmosphere).
//! Motion lives in [`crate::ephem`]; this is everything else. The values come
//! from data files (`data/bodies/<body>/body.ron`, see [`data`]).

pub mod atmosphere;
pub mod data;

pub use atmosphere::{AirState, Atmosphere, AtmosphereTable};
pub use data::{default_bodies_dir, load_body, parse_body, BodyDef, DataError};

use crate::frame::{BodyFixed, Inertial, Vec3};
use crate::math;
use crate::terrain::{self, detail, Detail, Heightmap};
use crate::time::Epoch;
use glam::DVec3;
use std::sync::Arc;

/// Most periodic terms a [`Rotation`] carries.
pub const MAX_LIBRATIONS: usize = 10;

/// One periodic term of an IAU rotation model, on the angle
/// `θ = phase + rate · t` (t seconds from J2000): the pole's right
/// ascension gains `ra · sin θ`, its declination `dec · cos θ`, the prime
/// meridian `w · sin θ` (rad, rad/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Libration {
    pub phase: f64,
    pub rate: f64,
    pub ra: f64,
    pub dec: f64,
    pub w: f64,
}

/// IAU-style rotation model: pole right ascension/declination with linear
/// precession terms and periodic terms ([`Libration`]), and prime-meridian
/// angle `W = w0 + w_rate * t` plus its periodic terms.
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
    /// Periodic terms (the first `n_librations`).
    pub librations: [Libration; MAX_LIBRATIONS],
    pub n_librations: usize,
}

impl Rotation {
    /// No periodic terms.
    pub const NO_LIBRATIONS: [Libration; MAX_LIBRATIONS] =
        [Libration { phase: 0.0, rate: 0.0, ra: 0.0, dec: 0.0, w: 0.0 }; MAX_LIBRATIONS];

    fn terms(&self) -> &[Libration] {
        &self.librations[..self.n_librations]
    }

    pub fn angle(&self, t: Epoch) -> f64 {
        // Split to keep precision over centuries: whole days and the remainder.
        let s = t.whole_seconds();
        let days = s.div_euclid(86_400);
        let rem = (s.rem_euclid(86_400)) as f64 + t.fractional_second();
        let per_day = math::wrap_tau(self.w_rate * 86_400.0 * days as f64);
        if self.n_librations == 0 {
            return math::wrap_tau(self.w0 + per_day + self.w_rate * rem);
        }
        let dt = t.to_seconds_f64();
        let lib: f64 = self.terms().iter().filter(|l| l.w != 0.0).map(|l| l.w * math::sin(l.phase + l.rate * dt)).sum();
        math::wrap_tau(self.w0 + per_day + self.w_rate * rem + lib)
    }

    /// Pole right ascension and declination at `t`.
    pub fn pole_radec(&self, t: Epoch) -> (f64, f64) {
        let dt = t.to_seconds_f64(); // centuries of drift: f64 seconds are ample here
        let (mut ra, mut dec) = (self.ra + self.ra_rate * dt, self.dec + self.dec_rate * dt);
        for l in self.terms() {
            let th = l.phase + l.rate * dt;
            if l.ra != 0.0 {
                ra += l.ra * math::sin(th);
            }
            if l.dec != 0.0 {
                dec += l.dec * math::cos(th);
            }
        }
        (ra, dec)
    }

    /// Unit pole vector at `t` (inertial axes).
    pub fn pole(&self, t: Epoch) -> DVec3 {
        let (ra, dec) = self.pole_radec(t);
        let cos_dec = math::cos(dec);
        DVec3::new(cos_dec * math::cos(ra), cos_dec * math::sin(ra), math::sin(dec))
    }

    /// Angular velocity vector (inertial axes): the spin about the pole,
    /// and with periodic terms also their rates (the spin's `Ẇ` and the
    /// pole's motion, `α̇` about ICRF Z and `−δ̇` about the node line).
    /// Without them, the spin alone (the linear pole drift, ~1e-12 rad/s,
    /// is left out).
    pub fn omega(&self, t: Epoch) -> Vec3<Inertial> {
        let pole = self.pole(t);
        if self.n_librations == 0 {
            return Vec3::from_raw(pole * self.w_rate);
        }
        let dt = t.to_seconds_f64();
        let (mut w_dot, mut ra_dot, mut dec_dot) = (self.w_rate, self.ra_rate, self.dec_rate);
        for l in self.terms() {
            let th = l.phase + l.rate * dt;
            let (s, c) = (math::sin(th), math::cos(th));
            w_dot += l.w * l.rate * c;
            ra_dot += l.ra * l.rate * c;
            dec_dot -= l.dec * l.rate * s;
        }
        let (ra, _) = self.pole_radec(t);
        let node = DVec3::new(-math::sin(ra), math::cos(ra), 0.0);
        Vec3::from_raw(DVec3::Z * ra_dot - node * dec_dot + pole * w_dot)
    }

    pub fn to_inertial(&self, v: Vec3<BodyFixed>, t: Epoch) -> Vec3<Inertial> {
        let (ra, dec) = self.pole_radec(t);
        let x = math::rotate_z(v.raw(), self.angle(t));
        let x = math::rotate_x(x, math::PI / 2.0 - dec);
        Vec3::from_raw(math::rotate_z(x, ra + math::PI / 2.0))
    }

    /// The body-fixed → inertial rotation at `t` as a matrix (its transpose
    /// is the inverse): [`Self::to_inertial`]'s three rotations, each angle's
    /// sine and cosine computed once (a gravity field rotates a position in
    /// and its acceleration back out).
    pub fn matrix(&self, t: Epoch) -> glam::DMat3 {
        use glam::DMat3;
        let (ra, dec) = self.pole_radec(t);
        let (sa, ca) = (math::sin(ra + math::PI / 2.0), math::cos(ra + math::PI / 2.0));
        let (sb, cb) = (math::sin(math::PI / 2.0 - dec), math::cos(math::PI / 2.0 - dec));
        let w = self.angle(t);
        let (sw, cw) = (math::sin(w), math::cos(w));
        let rz_a = DMat3::from_cols(DVec3::new(ca, sa, 0.0), DVec3::new(-sa, ca, 0.0), DVec3::Z);
        let rx_b = DMat3::from_cols(DVec3::X, DVec3::new(0.0, cb, sb), DVec3::new(0.0, -sb, cb));
        let rz_w = DMat3::from_cols(DVec3::new(cw, sw, 0.0), DVec3::new(-sw, cw, 0.0), DVec3::Z);
        rz_a * rx_b * rz_w
    }

    pub fn to_fixed(&self, v: Vec3<Inertial>, t: Epoch) -> Vec3<BodyFixed> {
        let (ra, dec) = self.pole_radec(t);
        let x = math::rotate_z(v.raw(), -(ra + math::PI / 2.0));
        let x = math::rotate_x(x, -(math::PI / 2.0 - dec));
        Vec3::from_raw(math::rotate_z(x, -self.angle(t)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyPhysical {
    pub name: String,
    pub radius_eq: f64,
    pub radius_polar: f64,
    /// Zonal harmonic J2 (dimensionless), referenced to `radius_eq`: the
    /// ephemeris generator's figure (`gen::Oblate`). Vessels feel
    /// [`Self::gravity`].
    pub j2: f64,
    /// The gravity field beyond the point mass that vessels feel (D090):
    /// the body's `gravity.ron` if it has one, else the degree-2 zonal field
    /// of `j2` (`None` without either). Shared between clones of the world.
    pub gravity: Option<Arc<crate::gravity::Field>>,
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

    /// The angular velocity is the rotation's derivative: a body-fixed
    /// point moves at `ω × r` (the Moon, with its periodic terms).
    #[test]
    fn omega_is_the_rate_of_the_rotation() {
        let m = moon();
        assert!(m.rotation.n_librations > 0);
        let p: Vec3<BodyFixed> = Vec3::new(1.2e6, -0.8e6, 0.9e6);
        for k in 0..5 {
            let t = Epoch::from_calendar(2030 + 10 * k, 3, 1, 0, 0, 0.0);
            let h = 10.0;
            let ahead = m.rotation.to_inertial(p, t.add_seconds(h)).raw();
            let behind = m.rotation.to_inertial(p, t.add_seconds(-h)).raw();
            let numeric = (ahead - behind) / (2.0 * h);
            let r = m.rotation.to_inertial(p, t).raw();
            let analytic = m.rotation.omega(t).raw().cross(r);
            assert!((numeric - analytic).length() < 1e-6 * analytic.length(), "{numeric} vs {analytic}");
        }
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
