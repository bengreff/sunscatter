//! Surface zones (D073, `docs/technical/crisp-surfaces.md`): the zone map
//! baked by `asset-tool zones` (`zones.png`, 0 sea, 1..=N the zone list),
//! drawn by `terrain.wgsl` so that coasts and zone edges are crisp at every
//! distance and each zone is one consistent procedural texture.
//!
//! Loaded once per body on a background task:
//! - the **zone map**, read by the shader with `textureLoad`, its sea pixels
//!   filled with the nearest land zone ([`ZoneMap::fill_sea`]): the coast is
//!   drawn from the water mask, so a land fragment near it always finds a
//!   land zone;
//! - the **far texture** (mipmapped, RGBA8 sRGB): rgb is what the near path
//!   draws without its pattern (each texel's zone colour × the satellite
//!   tint, [`tinted`]; lakes in the water colour), used once a pixel covers
//!   about a zone texel or more, so minification is filtered; alpha is the
//!   shallowness of the sea floor ([`shallowness`]);
//! - the **palette** uniform ([`palette`]).
//!
//! The pure parts of the shader are mirrored here and tested: the bilinear
//! weights of a gather, the zone pick from 4 indices ([`pick`]) and its
//! antialiased edge ([`edge_coverage`]), the coast step ([`coast_step`]) and
//! the octave rule ([`octave_weight`], [`octave_count`]).

use super::material::{linear_to_srgb, srgb_to_linear};
use crate::body_visual::{OceanDef, ZonesDef};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use sim::terrain::Heightmap;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Palette entries in the shader (index 0 and up to 15 zones).
pub const MAX_ZONES: usize = 16;

/// The warp's first octave: wavelength in zone texels, amplitude in zone
/// texels (halving per octave). Mirrored in `terrain.wgsl` (`WARP_AMP`).
pub const WARP_WAVELENGTH_TEXELS: f64 = 4.0;
#[cfg_attr(not(test), allow(dead_code))] // The shader's constants; a test pins them.
pub const WARP_AMP_TEXELS: f32 = 0.6;
/// Octave limits in `terrain.wgsl`: no warp octave under `WARP_MIN_M`, no
/// pattern octave under `TONE_MIN_M` (the body-fixed position is good to
/// ~0.5 m in f32; close up, the simple ground pattern takes over).
#[cfg_attr(not(test), allow(dead_code))]
pub const WARP_OCTAVES: usize = 10;
#[cfg_attr(not(test), allow(dead_code))]
pub const WARP_MIN_M: f32 = 100.0;
#[cfg_attr(not(test), allow(dead_code))]
pub const TONE_OCTAVES: usize = 6;
#[cfg_attr(not(test), allow(dead_code))]
pub const TONE_MIN_M: f32 = 40.0;

/// The warp's first wavelength (m) on a body of equatorial radius
/// `radius_m` with a zone map `width` texels around: 4 texels at the equator.
pub fn warp_wavelength(radius_m: f64, width: usize) -> f64 {
    WARP_WAVELENGTH_TEXELS * std::f64::consts::TAU * radius_m / width as f64
}

/// An octave is fully drawn at a wavelength of 3 pixel footprints or more
/// and gone at 2 (`terrain.wgsl`: `octave_weight`).
pub fn octave_weight(wavelength_m: f32, footprint_m: f32) -> f32 {
    (wavelength_m / footprint_m.max(1e-6) - 2.0).clamp(0.0, 1.0)
}

/// The simple ground pattern near the ground (`terrain.wgsl`, the look's
/// depth cue, D073): its octaves (wavelengths 8 detail units,
/// halving), and the broad patches' contrast in stops.
#[cfg_attr(not(test), allow(dead_code))] // The shader's constants; a test pins them.
pub const GROUND_OCTAVES: usize = 8;
#[cfg_attr(not(test), allow(dead_code))]
pub const GROUND_PATCH_STOPS: f32 = 0.35;

/// A ground-pattern octave: full at a wavelength of 8 pixel footprints or
/// more, gone at 4 (stricter than the zones' octaves: it is closer and
/// higher in contrast).
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
pub fn ground_octave_weight(period: f32, footprint: f32) -> f32 {
    (period / (4.0 * footprint.max(1e-9)) - 1.0).clamp(0.0, 1.0)
}

/// The ground pattern's brightness factor: 2^((fine + contrast × patch) ×
/// near), `fine` the octaves' sum (about −1..1), `patch` the broad patches
/// (−1..1), `near` 1 at the camera fading to 0 at 20 km.
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
pub fn ground_gain(fine: f32, patch: f32, near: f32) -> f32 {
    ((fine + GROUND_PATCH_STOPS * patch) * near).exp2()
}

/// How many octaves (wavelengths `first_m`, `first_m`/2, …, none under
/// `min_m`, at most `max`) are drawn at a pixel footprint.
#[cfg_attr(not(test), allow(dead_code))] // The shader's loop; tests pin it.
pub fn octave_count(first_m: f32, footprint_m: f32, min_m: f32, max: usize) -> usize {
    let mut lam = first_m;
    let mut n = 0;
    while n < max && lam >= min_m && octave_weight(lam, footprint_m) > 0.0 {
        n += 1;
        lam *= 0.5;
    }
    n
}

/// The four texels around texel coordinate `(tx, ty)` (texel centres at
/// half-integers) in a `w × h` grid, in gather order ((i0, j1), (i1, j1),
/// (i1, j0), (i0, j0)): longitude wraps, latitude clamps; and the fractions
/// between them. The shader reads them with `textureLoad`.
#[cfg_attr(not(test), allow(dead_code))] // The shader's addressing on the CPU; tests pin it.
pub fn texel_block(tx: f32, ty: f32, w: usize, h: usize) -> ([(usize, usize); 4], (f32, f32)) {
    let (x, y) = (tx - 0.5, ty - 0.5);
    let (i, j) = (x.floor(), y.floor());
    let (w, h) = (w as i64, h as i64);
    let x0 = (i as i64).rem_euclid(w);
    let x1 = (x0 + 1) % w;
    let y0 = (j as i64).clamp(0, h - 1);
    let y1 = (j as i64 + 1).clamp(0, h - 1);
    let p = |x: i64, y: i64| (x as usize, y as usize);
    ([p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], (x - i, y - j))
}

/// Bilinear weights of the four texels of [`texel_block`] (gather order
/// ((i0, j1), (i1, j1), (i1, j0), (i0, j0))), at fractions `(fx, fy)`, and
/// their derivatives with respect to `fx` and `fy`.
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
pub fn gather_weights(fx: f32, fy: f32) -> ([f32; 4], [f32; 4], [f32; 4]) {
    (
        [(1.0 - fx) * fy, fx * fy, fx * (1.0 - fy), (1.0 - fx) * (1.0 - fy)],
        [-fy, fy, 1.0 - fy, -(1.0 - fy)],
        [1.0 - fx, fx, -fx, -(1.0 - fx)],
    )
}

#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
fn dot4(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// The zone pick from the 4 zone indices around a point: each distinct zone's
/// bilinear weights summed. The largest sum wins (the first on ties); the
/// runner-up is the largest other zone (the winner itself if there is none).
/// `margin` is winner − runner-up, and `grad` its derivative with respect
/// to `(fx, fy)`, from which the shader antialiases the edge over a pixel.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub winner: u8,
    pub runner: u8,
    pub margin: f32,
    pub grad: [f32; 2],
}

#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
pub fn pick(ids: [u8; 4], fx: f32, fy: f32) -> Pick {
    let (w, dwx, dwy) = gather_weights(fx, fy);
    let ind = |z: u8| ids.map(|i| f32::from(u8::from(i == z)));
    let sums = ids.map(|z| dot4(w, ind(z)));
    let mut win = 0;
    for k in 1..4 {
        if sums[k] > sums[win] {
            win = k;
        }
    }
    let mut run: Option<usize> = None;
    for k in 0..4 {
        if ids[k] != ids[win] && run.is_none_or(|r| sums[k] > sums[r]) {
            run = Some(k);
        }
    }
    let Some(run) = run else {
        return Pick { winner: ids[win], runner: ids[win], margin: 1.0, grad: [0.0, 0.0] };
    };
    let (a, b) = (ind(ids[win]), ind(ids[run]));
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
    Pick { winner: ids[win], runner: ids[run], margin: sums[win] - sums[run], grad: [dot4(d, dwx), dot4(d, dwy)] }
}

/// Length of a gradient given with respect to texel fractions, in units per
/// pixel: `jx`, `jy` are the texel coordinates' derivatives along the
/// screen's x and y.
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule on the CPU; tests pin it.
pub fn px_gradient(grad: [f32; 2], jx: [f32; 2], jy: [f32; 2]) -> f32 {
    let gx = grad[0] * jx[0] + grad[1] * jx[1];
    let gy = grad[0] * jy[0] + grad[1] * jy[1];
    (gx * gx + gy * gy).sqrt()
}

/// The winner's coverage at a zone edge: 0.5 on the edge, 1 half a pixel
/// into the winner's side (`grad_px`: the margin's change per pixel).
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule; tests pin it.
pub fn edge_coverage(margin: f32, grad_px: f32) -> f32 {
    (0.5 + margin / grad_px.max(1e-6)).clamp(0.5, 1.0)
}

/// The crisp coastline: the bilinear sea fraction `raw` thresholded at 0.5,
/// antialiased over one pixel (`grad_px`: its change per pixel).
#[cfg_attr(not(test), allow(dead_code))] // The shader's rule; tests pin it.
pub fn coast_step(raw: f32, grad_px: f32) -> f32 {
    (0.5 + (raw - 0.5) / grad_px.max(1e-6)).clamp(0.0, 1.0)
}

/// How much of a shallow bottom's brightness shows through `depth_m` of
/// water (1 at the shore): exp(−depth / e-folding depth).
pub fn shallowness(depth_m: f32, e_fold_m: f32) -> f32 {
    (-depth_m.max(0.0) / e_fold_m.max(1e-3)).exp()
}

/// A zone's drawn colour with the satellite's regional variation kept:
/// `color × clamp((satellite / source)^tint, 0.5, 2)` per channel (linear
/// light). Only a ratio is taken, so it never shows as blur.
pub fn tinted(color: Vec3, source: Vec3, satellite: Vec3, tint: f32) -> Vec3 {
    let r = (satellite.max(Vec3::splat(1e-5)) / source.max(Vec3::splat(1e-5))).powf(tint);
    color * r.clamp(Vec3::splat(0.5), Vec3::splat(2.0))
}

/// The shader's palette: per zone index, rgb its drawn colour (linear: the
/// zone's physical albedo, water zones the open-water albedo) and w 1 for
/// water; x, y its pattern's scale (m) and contrast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub color: [Vec4; MAX_ZONES],
    pub pattern: [Vec4; MAX_ZONES],
    /// Linear source colours (for the tint), and the tint exponent.
    pub source: [Vec3; MAX_ZONES],
    pub tint: f32,
}

fn lin(c: [u8; 3]) -> Vec3 {
    Vec3::from_array(c.map(|v| srgb_to_linear(f32::from(v) / 255.0)))
}

/// Packs a body's zones. Index 0 (sea left unfilled, only on a body with no
/// land) gets `fallback`; zones past [`MAX_ZONES`] − 1 are dropped (a test
/// keeps the shipped lists under it).
pub fn palette(def: &ZonesDef, ocean: Option<&OceanDef>, fallback: [f32; 3]) -> Palette {
    let mut p = Palette {
        color: [Vec4::ZERO; MAX_ZONES],
        pattern: [Vec4::new(1000.0, 0.0, 0.0, 0.0); MAX_ZONES],
        source: [Vec3::ONE; MAX_ZONES],
        tint: def.tint,
    };
    p.color[0] = Vec3::from_array(fallback).extend(0.0);
    for (k, z) in def.list.iter().enumerate().take(MAX_ZONES - 1) {
        let i = k + 1;
        let drawn = match (z.water, ocean, z.albedo) {
            (true, Some(o), _) => Vec3::from_array(o.albedo),
            (_, _, Some(a)) => Vec3::from_array(a),
            _ => lin(z.source),
        };
        p.color[i] = drawn.extend(if z.water { 1.0 } else { 0.0 });
        p.pattern[i] = Vec4::new(z.pattern.scale_m, z.pattern.contrast, 0.0, 0.0);
        p.source[i] = lin(z.source);
    }
    p
}

/// A zone index grid in the asset-tool grid convention (row 0 north,
/// column 0 at −180°).
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneMap {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl ZoneMap {
    pub fn load(path: &Path) -> Option<Self> {
        match image::open(path) {
            Ok(i) => {
                let i = i.into_luma8();
                Some(Self { w: i.width() as usize, h: i.height() as usize, data: i.into_raw() })
            }
            Err(e) => {
                error!("zone map {}: {e}", path.display());
                None
            }
        }
    }

    /// Gives every sea pixel (0) the nearest land zone that is not water
    /// (`water[z]`), by breadth-first search over the sea (4-neighbour,
    /// longitude wraps). Sea reachable from no such zone stays 0.
    pub fn fill_sea(&mut self, water: &[bool]) {
        let (w, h) = (self.w, self.h);
        let mut queue: VecDeque<usize> = (0..w * h)
            .filter(|&k| {
                let z = usize::from(self.data[k]);
                z != 0 && !water.get(z).copied().unwrap_or(false)
            })
            .collect();
        while let Some(k) = queue.pop_front() {
            let (i, j) = (k % w, k / w);
            let z = self.data[k];
            let mut visit = |n: usize, data: &mut Vec<u8>| {
                if data[n] == 0 {
                    data[n] = z;
                    queue.push_back(n);
                }
            };
            visit(j * w + (i + w - 1) % w, &mut self.data);
            visit(j * w + (i + 1) % w, &mut self.data);
            if j > 0 {
                visit(k - w, &mut self.data);
            }
            if j + 1 < h {
                visit(k + w, &mut self.data);
            }
        }
    }

    /// The zone map texture: R8, one level (the shader reads it with `textureLoad`).
    pub fn to_image(&self) -> Image {
        let mut image = Image::new(
            Extent3d { width: self.w as u32, height: self.h as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            self.data.clone(),
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Nearest,
            min_filter: ImageFilterMode::Nearest,
            mipmap_filter: ImageFilterMode::Nearest,
            ..default()
        });
        image
    }
}

/// Source pixel range `[a, b)` covered by destination pixel `k` (at least one).
fn block(k: usize, n_dst: usize, n_src: usize) -> (usize, usize) {
    let a = k * n_src / n_dst;
    (a, ((k + 1) * n_src / n_dst).max(a + 1).min(n_src))
}

/// Mean linear colour of the land part of each zone texel (`w × h`): the
/// colour map's pixels where the sea mask (same or other size) is under
/// half. `None` where a texel has no land pixel.
pub fn land_colour(
    rgb: &[u8],
    cw: usize,
    ch: usize,
    sea: Option<(&[u8], usize, usize)>,
    w: usize,
    h: usize,
) -> Vec<Option<Vec3>> {
    let lut: Vec<f32> = (0..256).map(|v| srgb_to_linear(v as f32 / 255.0)).collect();
    let is_sea = |i: usize, j: usize| {
        sea.is_some_and(|(m, mw, mh)| m[(j * mh / ch).min(mh - 1) * mw + (i * mw / cw).min(mw - 1)] >= 128)
    };
    let mut out = Vec::with_capacity(w * h);
    for zj in 0..h {
        let (j0, j1) = block(zj, h, ch);
        for zi in 0..w {
            let (i0, i1) = block(zi, w, cw);
            let (mut acc, mut n) = (Vec3::ZERO, 0.0f32);
            for j in j0..j1 {
                for i in i0..i1 {
                    if !is_sea(i, j) {
                        let p = &rgb[(j * cw + i) * 3..(j * cw + i) * 3 + 3];
                        acc += Vec3::new(lut[usize::from(p[0])], lut[usize::from(p[1])], lut[usize::from(p[2])]);
                        n += 1.0;
                    }
                }
            }
            out.push((n > 0.0).then(|| acc / n));
        }
    }
    out
}

/// Mean [`shallowness`] of each zone texel (`w × h`) from a heightmap (sea
/// level `sea`; land counts as the shore, 1).
pub fn shallow_grid(hm: &Heightmap, sea: f64, e_fold_m: f32, w: usize, h: usize) -> Vec<f32> {
    let (hw, hh) = (hm.width(), hm.height());
    let data = hm.data();
    let mut out = Vec::with_capacity(w * h);
    for zj in 0..h {
        let (j0, j1) = block(zj, h, hh);
        for zi in 0..w {
            let (i0, i1) = block(zi, w, hw);
            let mut acc = 0.0f32;
            for j in j0..j1 {
                for i in i0..i1 {
                    acc += shallowness((sea - f64::from(data[j * hw + i])) as f32, e_fold_m);
                }
            }
            out.push(acc / ((j1 - j0) * (i1 - i0)) as f32);
        }
    }
    out
}

/// The far texture's base level (linear rgb, alpha): land texels their
/// zone's tinted colour, water zones the water colour, sea texels (0 in
/// `raw`) their filled zone's untinted colour; alpha the shallowness (1
/// without a sea floor).
pub fn far_texels(
    raw: &ZoneMap,
    filled: &ZoneMap,
    pal: &Palette,
    sat: Option<&[Option<Vec3>]>,
    shallow: Option<&[f32]>,
) -> Vec<Vec4> {
    (0..raw.data.len())
        .map(|k| {
            let z = usize::from(filled.data[k]).min(MAX_ZONES - 1);
            let color = pal.color[z].truncate();
            let land = raw.data[k] != 0 && pal.color[z].w == 0.0;
            let rgb = match sat.and_then(|s| s[k]) {
                Some(s) if land => tinted(color, pal.source[z], s, pal.tint),
                _ => color,
            };
            rgb.extend(shallow.map_or(1.0, |s| s[k]))
        })
        .collect()
}

/// A mipmapped RGBA8 sRGB texture (rgb averaged in linear light) with the
/// colour map's sampler.
pub fn far_image(base: Vec<Vec4>, w: usize, h: usize) -> Image {
    let (w0, h0) = (w as u32, h as u32);
    let (mut level, mut w, mut h) = (base, w, h);
    let mut bytes = Vec::new();
    let mut mips = 0;
    loop {
        bytes.extend(level.iter().flat_map(|c| {
            let s = |x: f32| (linear_to_srgb(x.clamp(0.0, 1.0)) * 255.0 + 0.5) as u8;
            [s(c.x), s(c.y), s(c.z), (c.w.clamp(0.0, 1.0) * 255.0 + 0.5) as u8]
        }));
        mips += 1;
        if w == 1 && h == 1 {
            break;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let at = |x: usize, y: usize| level[y.min(h - 1) * w + x.min(w - 1)];
        level = (0..nw * nh)
            .map(|k| {
                let (x, y) = ((k % nw) * 2, (k / nw) * 2);
                (at(x, y) + at(x + 1, y) + at(x, y + 1) + at(x + 1, y + 1)) * 0.25
            })
            .collect();
        (w, h) = (nw, nh);
    }
    let mut image = Image::new_uninit(
        Extent3d { width: w0, height: h0, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(bytes);
    image.texture_descriptor.mip_level_count = mips;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    image
}

/// What a body's zones need, gathered on the main thread.
pub struct ZoneInputs {
    pub dir: PathBuf,
    pub def: ZonesDef,
    pub color_map: Option<String>,
    pub ocean: Option<OceanDef>,
    pub heights: Option<Arc<Heightmap>>,
    pub sea_level: Option<f64>,
    pub base_color: [f32; 3],
}

/// A body's loaded zones.
pub struct ZoneTextures {
    pub map: Image,
    pub far: Image,
    pub palette: Palette,
    /// Zone map width (texels around the equator).
    pub width: usize,
}

/// Loads a body's zones (on a background task; ~1 s for Earth).
pub fn load(inp: ZoneInputs) -> Option<ZoneTextures> {
    let raw = ZoneMap::load(&inp.dir.join(&inp.def.map))?;
    let pal = palette(&inp.def, inp.ocean.as_ref(), inp.base_color);
    let water: Vec<bool> = pal.color.iter().map(|c| c.w > 0.0).collect();
    let mut filled = raw.clone();
    filled.fill_sea(&water);
    let mask = inp.ocean.as_ref().and_then(|o| o.mask.as_ref()).and_then(|m| match image::open(inp.dir.join(m)) {
        Ok(i) => Some(i.into_luma8()),
        Err(e) => {
            error!("water mask {m}: {e}");
            None
        }
    });
    let sat = inp.color_map.as_ref().and_then(|c| match image::open(inp.dir.join(c)) {
        Ok(img) => {
            let img = img.into_rgb8();
            let (cw, ch) = (img.width() as usize, img.height() as usize);
            let sea = mask.as_ref().map(|m| (m.as_raw().as_slice(), m.width() as usize, m.height() as usize));
            Some(land_colour(img.as_raw(), cw, ch, sea, raw.w, raw.h))
        }
        Err(e) => {
            error!("colour map {c}: {e}");
            None
        }
    });
    let shallow = match (&inp.heights, inp.sea_level, inp.ocean.as_ref().and_then(|o| o.shallow_depth_m)) {
        (Some(hm), Some(sea), Some(e)) => Some(shallow_grid(hm, sea, e, raw.w, raw.h)),
        _ => None,
    };
    let base = far_texels(&raw, &filled, &pal, sat.as_deref(), shallow.as_deref());
    filled.mark_edges(&edge_mask(&raw, EDGE_RADIUS));
    Some(ZoneTextures { map: filled.to_image(), far: far_image(base, raw.w, raw.h), palette: pal, width: raw.w })
}

/// The zone map's flag for texels near an edge (its top bit; indices stay
/// under 128): only there can the warp change what is drawn.
pub const EDGE_BIT: u8 = 0x80;

/// How far (zone texels) the warp and the shader's 2×2 lookup reach from a
/// point: the warp's octaves sum to 2 × `WARP_AMP_TEXELS` (1.2), the
/// gather one more, and one for the texel the test is made in.
pub const EDGE_RADIUS: usize = 4;

/// Texels within `radius` (square, longitude wrapping, latitude clamped)
/// of a texel whose raw value differs from a neighbour's: a zone edge, a
/// lake's or the coast (the raw map keeps the sea as 0). Elsewhere every
/// lookup the warp can reach sees one zone and one side of the coast, so
/// the shader skips the warp there (the zones' warp took ~10 ms of a 23 ms
/// frame at 3024×1964 from 185 km, 2026-10-26, nearly all of it where it
/// changed nothing).
pub fn edge_mask(raw: &ZoneMap, radius: usize) -> Vec<bool> {
    let (w, h) = (raw.w, raw.h);
    let at = |i: usize, j: usize| raw.data[j * w + i];
    // Texels with a 4-neighbour of another value.
    let mut seed = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            let v = at(i, j);
            let east = at((i + 1) % w, j);
            let south = if j + 1 < h { at(i, j + 1) } else { v };
            if v != east {
                seed[j * w + i] = true;
                seed[j * w + (i + 1) % w] = true;
            }
            if v != south {
                seed[j * w + i] = true;
                seed[(j + 1) * w + i] = true;
            }
        }
    }
    // Dilated by `radius`: along longitude (wrapping), then latitude.
    let mut rows = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if seed[j * w + i] {
                for d in 0..=2 * radius {
                    rows[j * w + (i + w * (radius + 1) + d - radius) % w] = true;
                }
            }
        }
    }
    let mut out = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if rows[j * w + i] {
                for jj in j.saturating_sub(radius)..=(j + radius).min(h - 1) {
                    out[jj * w + i] = true;
                }
            }
        }
    }
    out
}

impl ZoneMap {
    /// Sets [`EDGE_BIT`] on the texels of `edges`.
    pub fn mark_edges(&mut self, edges: &[bool]) {
        for (v, &e) in self.data.iter_mut().zip(edges) {
            if e {
                *v |= EDGE_BIT;
            }
        }
    }
}

#[cfg(test)]
#[path = "zones_tests.rs"]
mod tests;
