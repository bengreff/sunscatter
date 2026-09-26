//! Physical properties of bodies (shape, gravity field, rotation, atmosphere).
//! Motion lives in [`crate::ephem`]; this is everything else.

use crate::frame::{BodyFixed, Inertial, Vec3};
use crate::math;
use crate::time::Epoch;
use glam::DVec3;

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

/// Exponential atmosphere (prototype model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    /// Sea-level density (kg/m³).
    pub rho0: f64,
    /// Scale height (m).
    pub scale_height: f64,
    /// Altitude above which density is zero (m).
    pub top: f64,
}

impl Atmosphere {
    pub fn density(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else {
            self.rho0 * math::exp(-altitude.max(0.0) / self.scale_height)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyPhysical {
    pub name: &'static str,
    pub radius_eq: f64,
    pub radius_polar: f64,
    /// Zonal harmonic J2 (dimensionless), referenced to `radius_eq`.
    pub j2: f64,
    pub rotation: Rotation,
    pub atmosphere: Option<Atmosphere>,
}

impl BodyPhysical {
    /// Height above the reference ellipsoid of a body-fixed point (m).
    /// Uses the geocentric-radius approximation (exact on the axes and equator;
    /// < 1 m off elsewhere for Earth's flattening at low altitude).
    pub fn altitude(&self, p: Vec3<BodyFixed>) -> f64 {
        let r = p.raw();
        let d = r.length();
        let sin_lat = r.z / d;
        let cos2 = 1.0 - sin_lat * sin_lat;
        let (a, b) = (self.radius_eq, self.radius_polar);
        let surface = (a * b) / (b * b * cos2 + a * a * sin_lat * sin_lat).sqrt();
        d - surface
    }

    /// Body-fixed position of a point at geodetic-ish latitude/longitude (rad)
    /// on the ellipsoid surface plus `height` along the radial direction.
    pub fn surface_point(&self, lat: f64, lon: f64, height: f64) -> Vec3<BodyFixed> {
        let dir = DVec3::new(math::cos(lat) * math::cos(lon), math::cos(lat) * math::sin(lon), math::sin(lat));
        let (a, b) = (self.radius_eq, self.radius_polar);
        let s = dir.z;
        let surface = (a * b) / (b * b * (1.0 - s * s) + a * a * s * s).sqrt();
        Vec3::from_raw(dir * (surface + height))
    }
}

const DEG: f64 = math::PI / 180.0;
/// Seconds per Julian century (IAU `T` unit).
const CENTURY: f64 = 36_525.0 * 86_400.0;

/// Earth: WGS84 shape, EGM J2, IAU 2015 rotation (no precession), exponential
/// atmosphere up to 150 km.
pub fn earth() -> BodyPhysical {
    BodyPhysical {
        name: "Earth",
        radius_eq: 6_378_137.0,
        radius_polar: 6_356_752.314_245,
        j2: 1.082_626_68e-3,
        rotation: Rotation {
            ra: 0.0,
            dec: 90.0 * DEG,
            ra_rate: -0.641 * DEG / CENTURY,
            dec_rate: -0.557 * DEG / CENTURY,
            w0: 190.147 * DEG,
            w_rate: 360.985_623_5 * DEG / 86_400.0,
        },
        atmosphere: Some(Atmosphere { rho0: 1.225, scale_height: 7_200.0, top: 150_000.0 }),
    }
}

/// Moon: sphere, J2 from GRAIL, IAU 2015 mean rotation (no librations).
pub fn moon() -> BodyPhysical {
    BodyPhysical {
        name: "Moon",
        radius_eq: 1_737_400.0,
        radius_polar: 1_737_400.0,
        j2: 2.033e-4,
        rotation: Rotation {
            ra: 269.9949 * DEG,
            dec: 66.5392 * DEG,
            ra_rate: 0.0031 * DEG / CENTURY,
            dec_rate: 0.0130 * DEG / CENTURY,
            w0: 38.3213 * DEG,
            w_rate: 13.176_358_15 * DEG / 86_400.0,
        },
        atmosphere: None,
    }
}

/// Sun (for rendering and gravity; no rotation modelled beyond IAU mean).
pub fn sun() -> BodyPhysical {
    BodyPhysical {
        name: "Sun",
        radius_eq: 695_700_000.0,
        radius_polar: 695_700_000.0,
        j2: 0.0,
        rotation: Rotation {
            ra: 286.13 * DEG,
            dec: 63.87 * DEG,
            ra_rate: 0.0,
            dec_rate: 0.0,
            w0: 84.176 * DEG,
            w_rate: 14.1844 * DEG / 86_400.0,
        },
        atmosphere: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
