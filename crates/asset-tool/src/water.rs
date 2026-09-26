//! Water masks: which parts of a body's surface are open sea (display only; the
//! physics treats the sea as solid ground at sea level, D037/D047).
//!
//! Method: at the source resolution, a cell is sea if it lies below sea level
//! **and** is connected (4-neighbour, longitude wraps) to the open ocean, seeded
//! by every cell deeper than [`SEED_DEPTH_M`]. Inland basins below sea level that
//! are not connected to the ocean (Dead Sea, Caspian, Qattara, Death Valley) stay
//! land, as do lakes above sea level. The binary mask is then area-averaged to the
//! output grid, so each output pixel holds the sea fraction of its area (0..=255),
//! which the renderer filters into smooth coastlines.
//!
//! Output: `water.png`, 8-bit grayscale, the grid convention of [`crate::grid`].

use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use crate::resample::resample_area;

/// Cells this far below sea level (m) seed the open ocean. Deeper than any lake
/// bed on Earth (Baikal's is about −1,190 m, the Caspian's about −1,050 m) and
/// shallower than every sea basin we want (the Black Sea reaches −2,200 m).
pub const SEED_DEPTH_M: f32 = 1500.0;

/// Sea cells (1) of a `w × h` height grid (m, row-major, row 0 north): below `sea`
/// and connected to a cell below `sea − seed_depth`. Columns wrap in longitude;
/// rows do not wrap (the poles are edges).
pub fn ocean_mask(heights: &[f32], w: usize, h: usize, sea: f32, seed_depth: f32) -> Vec<u8> {
    assert_eq!(heights.len(), w * h);
    let mut mask = vec![0u8; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for (k, &z) in heights.iter().enumerate() {
        if z < sea - seed_depth {
            mask[k] = 1;
            stack.push(k);
        }
    }
    while let Some(k) = stack.pop() {
        let (i, j) = (k % w, k / w);
        let left = j * w + (i + w - 1) % w;
        let right = j * w + (i + 1) % w;
        let up = (j > 0).then(|| k - w);
        let down = (j + 1 < h).then(|| k + w);
        for n in [Some(left), Some(right), up, down].into_iter().flatten() {
            if mask[n] == 0 && heights[n] < sea {
                mask[n] = 1;
                stack.push(n);
            }
        }
    }
    mask
}

/// Area-averages a 0/1 mask to `dst_w × dst_h` sea fractions, quantised to `u8`.
pub fn coverage(mask: &[u8], w: usize, h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
    let frac = resample_area(w, h, 1, dst_w, dst_h, |j, buf| {
        for (b, &m) in buf.iter_mut().zip(&mask[j * w..(j + 1) * w]) {
            *b = f32::from(m);
        }
    });
    frac.into_iter().map(|f| (f.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// Writes an 8-bit grayscale PNG with maximum deflate compression.
pub fn write_mask_png(path: &Path, w: usize, h: usize, data: &[u8]) {
    assert_eq!(data.len(), w * h);
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let enc =
        PngEncoder::new_with_quality(std::io::BufWriter::new(file), CompressionType::Level(9), FilterType::Adaptive);
    enc.write_image(data, w as u32, h as u32, ExtendedColorType::L8).expect("encode PNG");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses a map: `#` land (+100 m), `~` deep ocean (−4000 m), `.` shallow
    /// water or land just below sea level (−20 m).
    fn grid(rows: &[&str]) -> (Vec<f32>, usize, usize) {
        let w = rows[0].len();
        let heights = rows
            .iter()
            .flat_map(|r| {
                r.chars().map(|c| match c {
                    '#' => 100.0,
                    '~' => -4000.0,
                    '.' => -20.0,
                    _ => panic!("bad cell {c}"),
                })
            })
            .collect();
        (heights, w, rows.len())
    }

    fn render(mask: &[u8], w: usize) -> Vec<String> {
        mask.chunks(w).map(|r| r.iter().map(|&m| if m == 1 { 'W' } else { '-' }).collect()).collect()
    }

    #[test]
    fn inland_basin_below_sea_level_stays_land() {
        let (h, w, rows) = grid(&[
            "##########", //
            "#..####..#",
            "#..#~~#..#",
            "#..#~~#.##",
            "####..####",
            "..........",
        ]);
        // Both shallow basins below sea level are enclosed by land, so they stay
        // land. The deep centre seeds the ocean, which spreads through the
        // shallow cells touching it (row 4) to the shallow sea in row 5.
        let mask = ocean_mask(&h, w, rows, 0.0, SEED_DEPTH_M);
        assert_eq!(
            render(&mask, w),
            ["----------", "----------", "----WW----", "----WW----", "----WW----", "WWWWWWWWWW",]
        );
    }

    #[test]
    fn longitude_wraps_and_poles_do_not() {
        // Shallow water at the right edge connects to the deep cell at the left
        // edge across the date line. The shallow cell in the south row is
        // enclosed: rows do not wrap, so it does not reach the north row.
        let (h, w, rows) = grid(&[
            ".####", //
            "~###.", "#####", ".####",
        ]);
        let mask = ocean_mask(&h, w, rows, 0.0, SEED_DEPTH_M);
        assert_eq!(render(&mask, w), ["W----", "W---W", "-----", "-----"]);
    }

    #[test]
    fn coverage_is_the_sea_fraction() {
        let mask = [1u8, 1, 0, 0, 1, 0, 0, 0];
        assert_eq!(coverage(&mask, 4, 2, 2, 1), vec![191, 0]);
        assert_eq!(coverage(&mask, 4, 2, 4, 2), vec![255, 255, 0, 0, 255, 0, 0, 0]);
    }

    #[test]
    fn png_round_trip() {
        let dir = std::env::temp_dir().join(format!("asset-tool-water-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("w.png");
        let data = [0u8, 12, 255, 128, 7, 0, 255, 1];
        write_mask_png(&path, 4, 2, &data);
        let img = image::open(&path).unwrap().into_luma8();
        assert_eq!(img.dimensions(), (4, 2));
        assert_eq!(img.into_raw(), data);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
