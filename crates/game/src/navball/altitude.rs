//! What "altitude" means in the flight view (D111): the navball's switch
//! between the datum and the terrain, the one rule every altitude above the
//! surface in the cluster and the menus goes through ([`altitude`]).
//!
//! The datum ([`datum`]): a body's sea; on a body with a solid surface and
//! no sea its lowest ground (the heightmap's lowest point); on a body
//! without a surface (a giant) its reference radius, which for the giants
//! is the 1-bar level (IAU). Terrain: the ground or sea under the craft.
//! Where the ground under a point is not known (an apsis ahead), its
//! altitude is above the datum.

use serde::{Deserialize, Serialize};

/// The switch's two settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AltitudeRef {
    /// Above sea level (or the body's datum).
    #[default]
    Datum,
    /// Above the ground (or sea) under the craft.
    Terrain,
}

impl AltitudeRef {
    pub fn other(self) -> AltitudeRef {
        match self {
            AltitudeRef::Datum => AltitudeRef::Terrain,
            AltitudeRef::Terrain => AltitudeRef::Datum,
        }
    }
}

/// A body's datum: its height above the reference ellipsoid (m) and its
/// short name (`SEA`, `LOW` for the lowest ground, `1 BAR`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Datum {
    pub height: f64,
    pub name: &'static str,
}

/// The datum of a body with `sea_level` (m above its reference), a solid
/// surface or none, and its lowest ground (m above its reference).
pub fn datum(sea_level: Option<f64>, solid: bool, lowest_ground: f64) -> Datum {
    match (sea_level, solid) {
        (Some(height), _) => Datum { height, name: "SEA" },
        (None, true) => Datum { height: lowest_ground, name: "LOW" },
        (None, false) => Datum { height: 0.0, name: "1 BAR" },
    }
}

/// The datum of `p`: its sea, its heightmap's lowest point (procedural
/// detail aside), or its reference radius (D111). Not cheap on a large
/// heightmap: the navball keeps it per body.
pub fn body_datum(p: &sim::body::BodyPhysical) -> Datum {
    let lowest = p.terrain.as_ref().map_or(0.0, |t| f64::from(t.data().iter().copied().min().unwrap_or(0)) * t.scale());
    datum(p.sea_level, p.solid, lowest)
}

/// An altitude as the switch reads it: a point `above_reference` (m above
/// the body's reference ellipsoid) over ground `ground` (m above the
/// reference, sea included; `None`: not known), above the datum or the
/// ground (the datum where the ground is not known).
pub fn altitude(r: AltitudeRef, above_reference: f64, datum: Datum, ground: Option<f64>) -> f64 {
    match (r, ground) {
        (AltitudeRef::Terrain, Some(g)) => above_reference - g,
        _ => above_reference - datum.height,
    }
}

/// The readout's label: `ALT SEA`, `ALT LOW`, `ALT 1 BAR` or `ALT TER`.
pub fn label(r: AltitudeRef, datum: Datum) -> String {
    match r {
        AltitudeRef::Datum => format!("ALT {}", datum.name),
        AltitudeRef::Terrain => "ALT TER".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The datum per body (table): Earth's sea, the Moon's lowest ground,
    /// Jupiter's 1-bar reference.
    #[test]
    fn the_datum_of_a_body() {
        assert_eq!(datum(Some(0.0), true, -10_900.0), Datum { height: 0.0, name: "SEA" });
        assert_eq!(datum(None, true, -9_100.0), Datum { height: -9_100.0, name: "LOW" });
        assert_eq!(datum(None, false, 0.0), Datum { height: 0.0, name: "1 BAR" });
    }

    /// The switch (table): above the datum, above the ground, and above
    /// the datum where the ground is not known.
    #[test]
    fn altitude_follows_the_switch() {
        let sea = datum(Some(0.0), true, -10_900.0);
        let low = datum(None, true, -9_100.0);
        // (switch, above the reference, datum, ground, altitude)
        let table = [
            (AltitudeRef::Datum, 1_500.0, sea, Some(1_200.0), 1_500.0),
            (AltitudeRef::Terrain, 1_500.0, sea, Some(1_200.0), 300.0),
            (AltitudeRef::Terrain, 1_500.0, sea, Some(0.0), 1_500.0),
            (AltitudeRef::Datum, 1_500.0, low, Some(-2_000.0), 10_600.0),
            (AltitudeRef::Terrain, 1_500.0, low, Some(-2_000.0), 3_500.0),
            (AltitudeRef::Terrain, 100e3, low, None, 109_100.0),
        ];
        for (r, h, d, g, want) in table {
            assert_eq!(altitude(r, h, d, g), want, "{r:?} {h} {d:?} {g:?}");
        }
        assert_eq!(label(AltitudeRef::Datum, sea), "ALT SEA");
        assert_eq!(label(AltitudeRef::Terrain, low), "ALT TER");
        assert_eq!(AltitudeRef::Datum.other(), AltitudeRef::Terrain);
    }
}
