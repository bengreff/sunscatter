//! Named craters from the IAU Gazetteer of Planetary Nomenclature (USGS),
//! for placing the real large craters of bodies without a DEM
//! ([`crate::relief`]). Source: the GIS downloads' centre-point shapefiles,
//! `https://asc-planetarynames-data.s3.us-west-2.amazonaws.com/<TARGET>_nomenclature_center_pts.zip`;
//! only the attribute table (`.dbf`) is read: name, diameter (km), centre
//! latitude and longitude (planetocentric, degrees east, 0..360), type.

use std::path::Path;
use std::process::Command;

use crate::source::{fetch, EXTERNAL};

const URL: &str = "https://asc-planetarynames-data.s3.us-west-2.amazonaws.com/";

/// A named feature.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub name: String,
    pub diameter_km: f64,
    pub lat: f64,
    /// Degrees east, −180..180.
    pub lon: f64,
    pub kind: String,
}

/// Parses a dBASE III table into records of (field name, value) strings.
pub fn parse_dbf(b: &[u8]) -> Vec<Vec<(String, String)>> {
    let n = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let header = usize::from(u16::from_le_bytes([b[8], b[9]]));
    let record = usize::from(u16::from_le_bytes([b[10], b[11]]));
    let mut fields = Vec::new();
    let mut o = 32;
    while b[o] != 0x0D {
        let name: String = b[o..o + 11].iter().take_while(|&&c| c != 0).map(|&c| char::from(c)).collect();
        fields.push((name, usize::from(b[o + 16])));
        o += 32;
    }
    (0..n)
        .map(|k| {
            let r = &b[header + k * record + 1..header + (k + 1) * record];
            let mut q = 0;
            fields
                .iter()
                .map(|(name, len)| {
                    let v: String = r[q..q + len].iter().map(|&c| char::from(c)).collect();
                    q += len;
                    (name.clone(), v.trim().to_string())
                })
                .collect()
        })
        .collect()
}

/// Features from a parsed table.
pub fn features(records: &[Vec<(String, String)>]) -> Vec<Feature> {
    let get =
        |r: &[(String, String)], k: &str| r.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    let num = |r: &[(String, String)], k: &str| get(r, k).parse::<f64>().unwrap_or(f64::NAN);
    records
        .iter()
        .map(|r| {
            let lon = num(r, "center_lon");
            Feature {
                name: get(r, "name"),
                diameter_km: num(r, "diameter"),
                lat: num(r, "center_lat"),
                lon: if lon > 180.0 { lon - 360.0 } else { lon },
                kind: get(r, "type"),
            }
        })
        .filter(|f| f.diameter_km.is_finite() && f.lat.is_finite() && f.lon.is_finite())
        .collect()
}

/// The named craters of `target` (e.g. `"CALLISTO"`).
pub fn craters(target: &str) -> Vec<Feature> {
    named(target, "Crater")
}

/// The named features of `target` whose type starts with `kind` (e.g.
/// `"Mons"`, `"Patera"`), downloaded and unpacked into
/// `data/external/gazetteer/` once.
pub fn named(target: &str, kind: &str) -> Vec<Feature> {
    let stem = format!("{target}_nomenclature_center_pts");
    let dir = Path::new(EXTERNAL).join("gazetteer");
    std::fs::create_dir_all(&dir).expect("create the gazetteer directory");
    let zip = fetch(&format!("{URL}{stem}.zip"), &format!("gazetteer/{stem}.zip"));
    let dbf = dir.join(format!("{stem}.dbf"));
    if !dbf.exists() {
        let status =
            Command::new("unzip").arg("-o").arg("-q").arg(&zip).arg("-d").arg(&dir).status().expect("run unzip");
        assert!(status.success(), "unzip {}", zip.display());
    }
    let bytes = std::fs::read(&dbf).unwrap_or_else(|e| panic!("{}: {e}", dbf.display()));
    let all = features(&parse_dbf(&bytes));
    all.into_iter().filter(|f| f.kind.starts_with(kind)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-field, two-record dBASE table.
    fn table() -> Vec<u8> {
        let mut b = vec![0u8; 32];
        b[0] = 3;
        b[4] = 2;
        let fields: [(&str, u8, u8); 4] =
            [("name", b'C', 8), ("diameter", b'N', 6), ("center_lon", b'N', 6), ("center_lat", b'N', 6)];
        let header = 32 + 32 * fields.len() + 1;
        let record = 1 + fields.iter().map(|f| usize::from(f.2)).sum::<usize>();
        b[8..10].copy_from_slice(&(header as u16).to_le_bytes());
        b[10..12].copy_from_slice(&(record as u16).to_le_bytes());
        for (name, t, len) in fields {
            let mut d = vec![0u8; 32];
            d[..name.len()].copy_from_slice(name.as_bytes());
            d[11] = t;
            d[16] = len;
            b.extend(d);
        }
        b.push(0x0D);
        for rec in [["Valhalla", "3000", "304", "14.7"], ["Burr", "75.4", "225.5", "42.7"]] {
            b.push(b' ');
            for (v, (_, _, len)) in rec.iter().zip(fields) {
                let mut f = v.as_bytes().to_vec();
                f.resize(usize::from(len), b' ');
                b.extend(f);
            }
        }
        b
    }

    #[test]
    fn dbf_records_parse_and_longitudes_wrap() {
        let f = features(&parse_dbf(&table()));
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].name, "Valhalla");
        assert_eq!(f[0].diameter_km, 3000.0);
        assert_eq!(f[0].lon, -56.0);
        assert_eq!(f[1].lat, 42.7);
        assert!((f[1].lon + 134.5).abs() < 1e-12);
    }
}
