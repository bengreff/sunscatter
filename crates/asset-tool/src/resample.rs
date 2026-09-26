//! Area-average (box filter) resampling of full-globe equirectangular grids.
//!
//! Both source and destination are pixel-area registered over the same extent
//! (−180°..180°, 90°..−90°), so output pixel `k` of an axis with scale
//! `s = n_src / n_dst` covers source coordinates `[k·s, (k+1)·s)` and every source
//! pixel it overlaps contributes in proportion to the overlap. This is an exact area
//! average in grid space; the cos(latitude) variation inside one output pixel is
//! ignored (it is at most a fraction of a percent at our resolutions except within a
//! pixel or two of the poles).
//!
//! The vertical pass streams source rows, so memory is one output image plus one row.

/// For each destination index, the list of `(source index, weight)` it averages.
/// Weights of one destination index sum to 1.
pub fn axis_weights(n_src: usize, n_dst: usize) -> Vec<Vec<(usize, f32)>> {
    assert!(n_src > 0 && n_dst > 0);
    let s = n_src as f64 / n_dst as f64;
    (0..n_dst)
        .map(|k| {
            let a = k as f64 * s;
            let b = (k + 1) as f64 * s;
            let first = a.floor() as usize;
            let last = (b.ceil() as usize).min(n_src);
            (first..last)
                .filter_map(|i| {
                    let overlap = (b.min(i as f64 + 1.0) - a.max(i as f64)).max(0.0);
                    (overlap > 1e-12).then_some((i, (overlap / s) as f32))
                })
                .collect()
        })
        .collect()
}

/// Resamples a `src_w × src_h` image with `channels` interleaved channels to
/// `dst_w × dst_h`. `row(j, buf)` must fill `buf` (length `src_w · channels`) with
/// source row `j`; rows are requested once each, in order.
pub fn resample_area(
    src_w: usize,
    src_h: usize,
    channels: usize,
    dst_w: usize,
    dst_h: usize,
    mut row: impl FnMut(usize, &mut [f32]),
) -> Vec<f32> {
    let wx = axis_weights(src_w, dst_w);
    let wy = axis_weights(src_h, dst_h);
    // Invert the vertical weights: for each source row, which output rows it feeds.
    let mut feeds: Vec<Vec<(usize, f32)>> = vec![Vec::new(); src_h];
    for (k, list) in wy.iter().enumerate() {
        for &(j, w) in list {
            feeds[j].push((k, w));
        }
    }
    let mut out = vec![0.0f32; dst_w * dst_h * channels];
    let mut src_row = vec![0.0f32; src_w * channels];
    let mut hrow = vec![0.0f32; dst_w * channels];
    for (j, targets) in feeds.iter().enumerate() {
        if targets.is_empty() {
            continue;
        }
        row(j, &mut src_row);
        for (k, list) in wx.iter().enumerate() {
            for c in 0..channels {
                let mut acc = 0.0f32;
                for &(i, w) in list {
                    acc += src_row[i * channels + c] * w;
                }
                hrow[k * channels + c] = acc;
            }
        }
        for &(k, w) in targets {
            let dst = &mut out[k * dst_w * channels..(k + 1) * dst_w * channels];
            for (d, h) in dst.iter_mut().zip(&hrow) {
                *d += h * w;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one_and_cover_source() {
        for (n_src, n_dst) in [(21600, 8192), (10, 3), (7, 7), (3, 10), (11520, 4096)] {
            let w = axis_weights(n_src, n_dst);
            let mut per_src = vec![0.0f64; n_src];
            for list in &w {
                let sum: f32 = list.iter().map(|&(_, x)| x).sum();
                assert!((sum - 1.0).abs() < 1e-5, "{n_src}->{n_dst}: {sum}");
                for &(i, x) in list {
                    per_src[i] += f64::from(x);
                }
            }
            // Every source pixel contributes n_dst/n_src in total (area conservation).
            let expect = n_dst as f64 / n_src as f64;
            assert!(per_src.iter().all(|&t| (t - expect).abs() < 1e-4));
        }
    }

    #[test]
    fn identity_when_sizes_match() {
        let src: Vec<f32> = (0..12).map(|v| v as f32).collect();
        let out = resample_area(4, 3, 1, 4, 3, |j, buf| buf.copy_from_slice(&src[j * 4..(j + 1) * 4]));
        assert_eq!(out, src);
    }

    #[test]
    fn integer_factor_is_block_mean() {
        // 4×2 → 2×1: each output is the mean of a 2×2 block.
        let src = [1.0f32, 3.0, 10.0, 20.0, 5.0, 7.0, 30.0, 40.0];
        let out = resample_area(4, 2, 1, 2, 1, |j, buf| buf.copy_from_slice(&src[j * 4..(j + 1) * 4]));
        assert_eq!(out, vec![4.0, 25.0]);
    }

    #[test]
    fn mean_is_preserved_for_fractional_factor() {
        let (w, h) = (27, 13);
        let src: Vec<f32> = (0..w * h * 2).map(|v| ((v * 37) % 101) as f32).collect();
        let out = resample_area(w, h, 2, 10, 5, |j, buf| buf.copy_from_slice(&src[j * w * 2..(j + 1) * w * 2]));
        for c in 0..2 {
            let m_src: f32 = src.iter().skip(c).step_by(2).sum::<f32>() / (w * h) as f32;
            let m_dst: f32 = out.iter().skip(c).step_by(2).sum::<f32>() / 50.0;
            assert!((m_src - m_dst).abs() < 1e-3, "{m_src} vs {m_dst}");
        }
    }
}
