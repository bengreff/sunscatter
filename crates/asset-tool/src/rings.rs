//! Saturn's rings' radial profile (agent SKY; docs/features/solar-visuals.md#rings):
//! `asset-tool rings`. The normal optical depth from the Voyager 2 photopolarimeter's
//! occultation of δ Scorpii (PDS Rings Node, VG_2801, `EASYDATA/KM010/PS1P01.TAB`: 10 km
//! bins from 72,000 to 142,500 km; Esposito et al. 1983, JGR 88, 8643), stored as a
//! 16-bit PNG one pixel high (τ × 10,000; negative noise clamped to 0). The PPS saturates
//! in the B ring's core (τ above ~2.5), where the ring is opaque anyway.

use std::path::{Path, PathBuf};

/// Stored units per unit of optical depth.
pub const TAU_SCALE: f64 = 10_000.0;

pub fn run() {
    let root = PathBuf::from(format!("{}/../../data", env!("CARGO_MANIFEST_DIR")));
    let src = root.join("external/rings/PS1P01.TAB");
    let text = std::fs::read_to_string(&src).unwrap_or_else(|e| panic!("read {}: {e}", src.display()));
    let rows = parse_profile(&text);
    let (first, last) = (rows[0].0, rows[rows.len() - 1].0);
    println!("  {} bins, {first} to {last} km", rows.len());
    let px: Vec<u16> = rows.iter().map(|r| encode_tau(r.1)).collect();
    let out = root.join("bodies/saturn/rings.png");
    write_png16(&out, &px);
    println!("  wrote {} ({} kB)", out.display(), std::fs::metadata(&out).map_or(0, |m| m.len() / 1000));
}

/// (radius km, normal optical depth) from the table's first and fourth columns.
pub fn parse_profile(text: &str) -> Vec<(f64, f64)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split(',').map(str::trim);
            let r = f.next()?.parse().ok()?;
            let tau = f.nth(2)?.parse().ok()?;
            Some((r, tau))
        })
        .collect()
}

/// The stored value of an optical depth (clamped to 0..6.5).
pub fn encode_tau(tau: f64) -> u16 {
    (tau.max(0.0) * TAU_SCALE).round().min(f64::from(u16::MAX)) as u16
}

fn write_png16(path: &Path, px: &[u16]) {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::{ExtendedColorType, ImageEncoder};
    let bytes: Vec<u8> = px.iter().flat_map(|v| v.to_be_bytes()).collect();
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    PngEncoder::new_with_quality(std::io::BufWriter::new(file), CompressionType::Best, FilterType::Adaptive)
        .write_image(&bytes, px.len() as u32, 1, ExtendedColorType::L16)
        .expect("encode PNG");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_parses_and_noise_clamps() {
        let text =
            " 72000.00, 55.469, 0.777,-0.0199,-0.0290,-0.0106\n100000.00, 1.0, 0.1, 2.5000, 2.4, 2.6\nbad line\n";
        let rows = parse_profile(text);
        assert_eq!(rows, vec![(72000.0, -0.0199), (100000.0, 2.5)]);
        assert_eq!(encode_tau(rows[0].1), 0);
        assert_eq!(encode_tau(2.5), 25_000);
        assert_eq!(encode_tau(9.0), u16::MAX);
    }
}
