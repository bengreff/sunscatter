//! Tests of `zones`: the CPU mirrors of the shader's rules, the load-time
//! textures and the shipped data.

use super::*;

#[test]
fn octaves_fade_by_footprint() {
    // (wavelength, footprint, weight): full at 3 footprints, gone at 2.
    for (lam, foot, want) in [(300.0, 100.0, 1.0), (250.0, 100.0, 0.5), (200.0, 100.0, 0.0), (1e4, 1.0, 1.0)] {
        assert!((octave_weight(lam, foot) - want).abs() < 1e-6, "{lam} {foot}");
    }
    // (first wavelength, footprint, minimum, max, octaves)
    let cases = [
        (39_000.0, 220.0, 100.0, 10, 7),    // Earth from 300 km: 39 km .. 610 m
        (39_000.0, 0.01, 100.0, 10, 9),     // at the pad: down to the 100 m floor
        (39_000.0, 0.01, 10.0, 10, 10),     // capped
        (39_000.0, 20_000.0, 100.0, 10, 0), // far away: no warp
        (800.0, 2.0, 40.0, 6, 5),           // a forest pattern from 3 km
    ];
    for (first, foot, min, max, want) in cases {
        assert_eq!(octave_count(first, foot, min, max), want, "{first} {foot}");
    }
}

#[test]
fn the_shader_uses_these_constants() {
    let wgsl = include_str!("terrain.wgsl");
    for decl in [
        format!("const WARP_AMP: f32 = {WARP_AMP_TEXELS:?};"),
        format!("const WARP_OCTAVES: i32 = {WARP_OCTAVES};"),
        format!("const WARP_MIN_M: f32 = {WARP_MIN_M:?};"),
        format!("const TONE_OCTAVES: i32 = {TONE_OCTAVES};"),
        format!("const TONE_MIN_M: f32 = {TONE_MIN_M:?};"),
        format!("zone_color: array<vec4<f32>, {MAX_ZONES}>,"),
        format!("zone_pattern: array<vec4<f32>, {MAX_ZONES}>,"),
    ] {
        assert!(wgsl.contains(&decl), "terrain.wgsl lacks `{decl}`");
    }
    // Earth's zone map (4096 wide): the warp starts at ~39 km and moves a
    // lookup by at most 2 × 0.6 texels.
    assert!((warp_wavelength(6_378_137.0, 4096) - 39_136.0).abs() < 1.0);
    assert!(2.0 * WARP_AMP_TEXELS < 0.5 * WARP_WAVELENGTH_TEXELS as f32);
    // From 300 km (220 m pixels at nadir) the warp has 7 octaves, at the pad 9.
    let lam = warp_wavelength(6_378_137.0, 4096) as f32;
    assert_eq!(octave_count(lam, 220.0, WARP_MIN_M, WARP_OCTAVES), 7);
    assert_eq!(octave_count(lam, 0.01, WARP_MIN_M, WARP_OCTAVES), 9);
}

#[test]
fn texel_block_wraps_longitude_and_clamps_latitude() {
    // (texel coordinate, the 4 texels in gather order, fractions)
    let (w, h) = (8, 4);
    let cases = [
        ((3.25, 1.75), [(2, 2), (3, 2), (3, 1), (2, 1)], (0.75, 0.25)),
        // Exactly on a texel centre: that texel is (i0, j0) with weight 1.
        ((2.5, 2.5), [(2, 3), (3, 3), (3, 2), (2, 2)], (0.0, 0.0)),
        // Across the date line: the last and first columns.
        ((0.25, 1.5), [(7, 2), (0, 2), (0, 1), (7, 1)], (0.75, 0.0)),
        ((7.75, 1.5), [(7, 2), (0, 2), (0, 1), (7, 1)], (0.25, 0.0)),
        // At the poles the rows clamp.
        ((4.5, 0.25), [(4, 0), (5, 0), (5, 0), (4, 0)], (0.0, 0.75)),
        ((4.5, 3.75), [(4, 3), (5, 3), (5, 3), (4, 3)], (0.0, 0.25)),
    ];
    for ((tx, ty), texels, (fx, fy)) in cases {
        let (t, f) = texel_block(tx, ty, w, h);
        assert_eq!(t, texels, "{tx}, {ty}");
        assert!((f.0 - fx).abs() < 1e-6 && (f.1 - fy).abs() < 1e-6, "{tx}, {ty}: {f:?}");
    }
    // The bilinear weights of a block reproduce a texel's value at its centre.
    let grid: Vec<f32> = (0..w * h).map(|k| k as f32).collect();
    let (t, (fx, fy)) = texel_block(5.5, 1.5, w, h);
    let v = dot4(gather_weights(fx, fy).0, t.map(|(i, j)| grid[j * w + i]));
    assert_eq!(v, grid[w + 5]);
}

#[test]
fn ground_pattern_is_visible_close_and_fades_by_footprint() {
    // (wavelength, footprint, weight): full at 8 pixels, gone at 4.
    for (period, foot, want) in
        [(8.0, 1.0, 1.0), (6.0, 1.0, 0.5), (4.0, 1.0, 0.0), (2.5, 0.01, 1.0), (320.0, 100.0, 0.0)]
    {
        assert!((ground_octave_weight(period, foot) - want).abs() < 1e-6, "{period} {foot}");
    }
    // At the pad camera (~60 m off, ~1 mrad pixels, 40 m detail units: a
    // footprint of ~0.002 units) all 8 octaves are drawn, 320 m to 2.5 m.
    let foot = 60.0 * 1.2e-3 / 40.0;
    let periods: Vec<f32> = (0..GROUND_OCTAVES).map(|k| 8.0 / 2f32.powi(k as i32)).collect();
    assert!(periods.iter().all(|&p| ground_octave_weight(p, foot) == 1.0));
    assert_eq!(periods.last().map(|p| p * 40.0), Some(2.5));
    // The brightness: a broad patch alone is ±0.35 stops; nothing far away.
    assert!((ground_gain(0.0, 1.0, 1.0) - 2f32.powf(0.35)).abs() < 1e-6);
    assert!((ground_gain(0.0, -1.0, 1.0) - 2f32.powf(-0.35)).abs() < 1e-6);
    assert!((ground_gain(0.5, 1.0, 0.0) - 1.0).abs() < 1e-6);
    let wgsl = include_str!("terrain.wgsl");
    for decl in [
        format!("const GROUND_OCTAVES: i32 = {GROUND_OCTAVES};"),
        format!("const GROUND_PATCH_STOPS: f32 = {GROUND_PATCH_STOPS:?};"),
    ] {
        assert!(wgsl.contains(&decl), "terrain.wgsl lacks `{decl}`");
    }
}

#[test]
fn gather_weights_are_bilinear() {
    for (fx, fy) in [(0.0, 0.0), (0.25, 0.75), (1.0, 0.5)] {
        let (w, dx, dy) = gather_weights(fx, fy);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(dx.iter().sum::<f32>().abs() < 1e-6 && dy.iter().sum::<f32>().abs() < 1e-6);
    }
    // At (0, 0) all weight is on (i0, j0), the gather's 4th texel.
    assert_eq!(gather_weights(0.0, 0.0).0, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(gather_weights(1.0, 1.0).0, [0.0, 1.0, 0.0, 0.0]);
}

#[test]
fn zone_pick_from_four_texels() {
    // (gather ids, fractions, winner, runner-up, margin, gradient)
    let cases = [
        // One zone: no edge.
        ([3, 3, 3, 3], (0.4, 0.7), 3, 3, 1.0, [0.0, 0.0]),
        // Left column (i0) zone 2, right column (i1) zone 5: the edge is fx = 0.5.
        ([2, 5, 5, 2], (0.3, 0.5), 2, 5, 0.4, [-2.0, 0.0]),
        ([2, 5, 5, 2], (0.8, 0.1), 5, 2, 0.6, [2.0, 0.0]),
        // On the edge: the first wins at zero margin.
        ([2, 5, 5, 2], (0.5, 0.5), 2, 5, 0.0, [-2.0, 0.0]),
        // Three zones: (i0, j0) = 4 alone, the rest split; 7 holds the
        // bottom row (j1) and wins at fy = 0.75.
        ([7, 7, 1, 4], (0.5, 0.75), 7, 1, 0.625, [-0.25, 1.5]),
        // The same zone on a diagonal sums its two corners.
        ([6, 2, 6, 2], (0.5, 0.5), 6, 2, 0.0, [0.0, 0.0]),
    ];
    for (ids, (fx, fy), winner, runner, margin, grad) in cases {
        let p = pick(ids, fx, fy);
        assert_eq!((p.winner, p.runner), (winner, runner), "{ids:?} at {fx}, {fy}");
        assert!((p.margin - margin).abs() < 1e-6, "{ids:?}: margin {}", p.margin);
        assert!((p.grad[0] - grad[0]).abs() < 1e-6 && (p.grad[1] - grad[1]).abs() < 1e-6, "{ids:?}: {:?}", p.grad);
    }
}

#[test]
fn edges_and_coasts_are_one_pixel_wide() {
    // A zone edge (margin changing 0.2 per pixel): coverage from 0.5 on
    // the edge to 1 at half a pixel.
    for (m, want) in [(0.0, 0.5), (0.05, 0.75), (0.1, 1.0), (0.5, 1.0)] {
        assert!((edge_coverage(m, 0.2) - want).abs() < 1e-6, "{m}");
    }
    // The coast: sea fraction 0.5 is the coastline; 0.1 per pixel.
    for (raw, want) in [(0.5, 0.5), (0.55, 1.0), (0.45, 0.0), (0.525, 0.75), (0.0, 0.0), (1.0, 1.0)] {
        assert!((coast_step(raw, 0.1) - want).abs() < 1e-6, "{raw}");
    }
    // Flat (no gradient): a hard step, still 0.5 exactly on the line.
    assert_eq!((coast_step(0.6, 0.0), coast_step(0.4, 0.0), coast_step(0.5, 0.0)), (1.0, 0.0, 0.5));
    // The pixel gradient: texel fractions changing 0.5 per pixel along x.
    assert!((px_gradient([2.0, 0.0], [0.5, 0.0], [0.0, 0.5]) - 1.0).abs() < 1e-6);
    assert!((px_gradient([1.0, 1.0], [1.0, 0.0], [0.0, 1.0]) - 2f32.sqrt()).abs() < 1e-6);
}

#[test]
fn shallowness_and_tint() {
    assert_eq!(shallowness(0.0, 10.0), 1.0);
    assert_eq!(shallowness(-50.0, 10.0), 1.0, "land is the shore");
    assert!((shallowness(10.0, 10.0) - (-1.0f32).exp()).abs() < 1e-6);
    assert!(shallowness(200.0, 10.0) < 1e-8);
    let (c, s) = (Vec3::new(0.1, 0.2, 0.3), Vec3::new(0.1, 0.1, 0.1));
    // Satellite equal to the source: the zone colour.
    assert!((tinted(c, s, s, 0.3) - c).length() < 1e-6);
    // Twice as bright, tint 0.5: × √2; clamped to 0.5 .. 2.
    assert!((tinted(c, s, s * 2.0, 0.5) - c * 2f32.sqrt()).length() < 1e-6);
    assert!((tinted(c, s, s * 100.0, 1.0) - c * 2.0).length() < 1e-6);
    assert!((tinted(c, s, Vec3::ZERO, 1.0) - c * 0.5).length() < 1e-6);
    // No tint: flat zone colours.
    assert!((tinted(c, s, s * 3.0, 0.0) - c).length() < 1e-6);
}

fn zones_def() -> ZonesDef {
    ron::from_str(
        r#"(map: "zones.png", tint: 0.4, list: [
            (name: "forest", source: (31, 43, 15), pattern: (scale_m: 800.0, contrast: 0.25)),
            (name: "desert", source: (185, 156, 117), albedo: Some((0.40, 0.30, 0.20))),
            (name: "lake", source: (3, 6, 8), water: true),
        ])"#,
    )
    .unwrap()
}

#[test]
fn palette_packing() {
    let ocean = OceanDef {
        roughness: 0.1,
        albedo: [0.003, 0.008, 0.03],
        mask: None,
        shallow_albedo: None,
        shallow_depth_m: None,
    };
    let p = palette(&zones_def(), Some(&ocean), [0.5, 0.5, 0.5]);
    assert_eq!(p.color[0], Vec4::new(0.5, 0.5, 0.5, 0.0));
    let forest = lin([31, 43, 15]);
    assert_eq!(p.color[1], forest.extend(0.0));
    assert_eq!(p.pattern[1], Vec4::new(800.0, 0.25, 0.0, 0.0));
    // A physical albedo is drawn as given (linear); no pattern by default (contrast 0).
    assert_eq!(p.color[2], Vec4::new(0.40, 0.30, 0.20, 0.0));
    assert_eq!(p.source[2], lin([185, 156, 117]));
    assert_eq!(p.pattern[2].y, 0.0);
    // Water zones: the open-water albedo, flagged.
    assert_eq!(p.color[3], Vec4::new(0.003, 0.008, 0.03, 1.0));
    assert_eq!(p.tint, 0.4);
    // Unused entries are zero.
    assert!(p.color[4..].iter().all(|c| *c == Vec4::ZERO));
}

#[test]
fn sea_fills_with_the_nearest_land_zone() {
    // 0 sea, 1 and 2 land, 3 a lake (water): lakes never spread.
    let mut m = ZoneMap { w: 6, h: 3, data: vec![0, 0, 1, 0, 0, 2, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };
    m.fill_sea(&[false, false, false, true]);
    assert_eq!(m.data, vec![2, 1, 1, 1, 2, 2, 2, 3, 1, 1, 2, 2, 2, 1, 1, 1, 2, 2]);
    // Without land, the sea stays 0.
    let mut sea = ZoneMap { w: 3, h: 1, data: vec![0, 3, 0] };
    sea.fill_sea(&[false, false, false, true]);
    assert_eq!(sea.data, vec![0, 3, 0]);
}

#[test]
fn far_texels_and_mips() {
    let p = palette(&zones_def(), None, [0.0; 3]);
    let raw = ZoneMap { w: 4, h: 2, data: vec![1, 2, 0, 3, 1, 1, 2, 2] };
    let mut filled = raw.clone();
    filled.fill_sea(&[false, false, false, true]);
    let sat = vec![Some(p.source[1] * 2.0); 8];
    let shallow = vec![1.0, 1.0, 0.25, 1.0, 1.0, 1.0, 1.0, 1.0];
    let t = far_texels(&raw, &filled, &p, Some(&sat), Some(&shallow));
    // Land: tinted (tint 0.4); sea: the filled zone untinted, with its shallowness.
    assert!((t[0].truncate() - tinted(p.color[1].truncate(), p.source[1], sat[0].unwrap(), 0.4)).length() < 1e-6);
    assert_eq!(t[2], p.color[usize::from(filled.data[2])].truncate().extend(0.25));
    // A lake: its colour, untinted.
    assert_eq!(t[3].truncate(), p.color[3].truncate());
    let img = far_image(t, 4, 2);
    assert_eq!(img.texture_descriptor.mip_level_count, 3);
    assert_eq!(img.data.as_ref().unwrap().len(), (8 + 2 + 1) * 4);
}

#[test]
fn land_colour_skips_the_sea() {
    // A 4×2 colour map, 2×1 zone texels: the right texel's sea pixels are left out.
    let rgb: Vec<u8> =
        [[255, 255, 255], [0, 0, 0], [255, 0, 0], [0, 0, 255]].iter().cycle().take(8).flatten().copied().collect();
    let sea = [0u8, 0, 0, 255, 0, 0, 255, 255];
    let c = land_colour(&rgb, 4, 2, Some((&sea, 4, 2)), 2, 1);
    assert!((c[0].unwrap() - Vec3::splat(0.5)).length() < 1e-6);
    assert!((c[1].unwrap() - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
    let all_sea = [255u8; 8];
    assert_eq!(land_colour(&rgb, 4, 2, Some((&all_sea, 4, 2)), 2, 1), vec![None, None]);
}

#[test]
fn shipped_zone_maps_index_their_lists() {
    for name in ["Earth", "Moon"] {
        let def = crate::body_visual::load(name).unwrap();
        let z = def.zones.unwrap();
        let m = ZoneMap::load(&crate::body_visual::body_dir(name).join(&z.map)).unwrap();
        assert_eq!(m.w, 2 * m.h, "{name}");
        let max = m.data.iter().copied().max().unwrap();
        assert!(usize::from(max) <= z.list.len(), "{name}: index {max} of {} zones", z.list.len());
        // Every zone is used.
        for k in 1..=z.list.len() as u8 {
            assert!(m.data.contains(&k), "{name}: zone {k} unused");
        }
    }
}
