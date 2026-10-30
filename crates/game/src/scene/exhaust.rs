//! Engine exhaust drawn from the exhaust seed (D109; technical/exhaust.md):
//! each running engine's plume from its state at the nozzle exit
//! (`sim::systems::exhaust`) and the air at the vessel, raymarched in a
//! cylinder around it (`exhaust.wgsl`, one draw per engine). Drawn when the
//! player's toggle is on (`persist::VisualSettings`), else the flame cone
//! (`parts::flame`).
//!
//! The plume, along its axis `x` from the exit (every rule here, mirrored
//! by the shader):
//! * **Shape** ([`boundary`]): from the exit radius toward the jet's fully
//!   expanded radius, leaving the lip at the jet's Prandtl–Meyer turning
//!   (narrowing when overexpanded, the wide fan in vacuum; turnings past
//!   [`MAX_TURN`], the vacuum fan's backflow, are left out), widened by
//!   the turbulent mixing layer with the air ([`mixing_rate`]): the nine
//!   jets of a cluster merge a few metres behind it.
//! * **Potential core** ([`potential_core`]): on the axis the gas keeps
//!   its exit state until the fan from the lip reaches it.
//! * **Across the jet** ([`profile`]): the gas fills the exit evenly, so
//!   the plume leaves the lip as wide as the bell; its even core shrinks to
//!   the axis over the potential core, with Simons' profile outside it.
//! * **Temperature** ([`temperature`]): isentropic cooling as the gas
//!   spreads (density `∝ (r₀/R)²`, `T ∝ ρ^(γ−1)`); reheating behind the
//!   shock cells (the normal-shock ratio at the jet's Mach, its strength
//!   from how far the jet is from matched and how much air confines it,
//!   decaying over [`CELLS`]); the fuel-rich gas afterburning in air
//!   toward [`AFTERBURN_TEMPERATURE`].
//! * **Light**: soot only (Rayleigh-limit absorption of the seed's soot,
//!   [`soot_extinction`]): its Planck emission ([`planck_rgb`], absolute,
//!   in nits) and the starlight it scatters at the craft; a plume without
//!   soot (hydrolox, hypergolic) is not drawn, as it is nearly invisible.
//!   The soot thins along the jet's core length ([`core_length`]).

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use sim::systems::exhaust::{ExitState, Jet};
use std::f64::consts::PI;

const SHADER: &str = "embedded://game/scene/exhaust.wgsl";

/// The widest lip turning drawn (rad, 75°): past it the vacuum fan turns
/// back toward the craft, which a radius along the axis cannot show.
pub const MAX_TURN: f64 = 75.0 * PI / 180.0;
/// The widest expanded radius drawn, in exit radii (the vacuum jet's is
/// unbounded).
pub const MAX_SPREAD: f64 = 1e3;
/// Shock cells fade over about this many cells (a sea-level jet shows 5–10
/// diamonds in photographs; an approximation).
pub const CELLS: f64 = 6.0;
/// Temperature (K) of the fuel-rich exhaust burning in air: a placeholder
/// for the CO/H₂–air mixing-layer flame (not from a source).
pub const AFTERBURN_TEMPERATURE: f64 = 2_400.0;
/// The unburned share at which the afterburning is near full (placeholder).
pub const UNBURNED_SCALE: f64 = 0.1;
/// The ambient pressure (Pa) that scales the afterburning,
/// `1 − exp(−pₐ/P)`: near full to 20 km (5.5 kPa), half at 30 km, a tenth
/// at 45 km, gone by 60 km: kerolox plumes' afterburning ceases at 40–50 km
/// (CFD of LOX/kerosene plumes against altitude, "Study on the Influence of
/// Flight Altitude on the Rocket Plume Radiation Enhancement Effect Caused
/// by Afterburning", 2021). It once used the exit pressure (`pₐ/pₑ`), which
/// put out the Merlin's afterburning by 15 km.
pub const AFTERBURN_PRESSURE: f64 = 1_400.0;
/// Soot: material density (kg/m³), the absorption function E(m) of its
/// refractive index, the wavelength of its extinction (m) and its
/// single-scattering albedo (small particles; placeholders from the soot
/// literature's usual figures, not checked against a source).
pub const SOOT_DENSITY: f64 = 1_800.0;
pub const SOOT_E_M: f64 = 0.26;
pub const SOOT_WAVELENGTH: f64 = 550e-9;
pub const SOOT_ALBEDO: f64 = 0.2;
/// The plume is drawn out to where its optical depth across falls below
/// this, at most [`MAX_LENGTH`] exit diameters.
pub const TAU_FLOOR: f64 = 0.01;
/// The mixing layer: incompressible growth `dδ/dx` of a shear layer's
/// vorticity thickness for a jet into still air with the density ratio 1
/// (Brown & Roshko 1974, ~0.17), and the compressibility factor at
/// convective Mach above ~1 (Papamoschou & Roshko 1988: 0.2–0.3); the
/// rocket jet's is ~2.4 at sea level.
pub const MIXING_LAYER: f64 = 0.17;
pub const COMPRESSIBILITY: f64 = 0.25;
/// The air the jet mixes with: its molar mass (g/mol) and a temperature
/// (K) for the stratosphere and below (216–288 K; it enters as a square
/// root).
pub const AIR_MOLAR_MASS: f64 = 28.96;
pub const AIR_TEMPERATURE: f64 = 250.0;
pub const MAX_LENGTH: f64 = 150.0;

/// Whether plumes are drawn: the player's toggle, or always while the
/// benchmark measures the look (so its "exhaust" row is the plumes' cost),
/// unless the benchmark has the part off.
pub fn drawn(setting: bool, benchmarking: bool, part: bool) -> bool {
    (setting || benchmarking) && part
}

/// One engine's plume, what the shader draws (lengths in m, temperatures
/// in K).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plume {
    /// Exit radius; the boundary's slope at the lip and its rise to the
    /// fully expanded radius (signed: negative when overexpanded).
    pub r0: f64,
    pub slope: f64,
    pub spread: f64,
    /// The mixing layer's widening of the boundary (m per m, [`mixing_rate`]).
    pub mixing: f64,
    /// Shock-cell length and the peak temperature rise behind the shocks
    /// (share of the local temperature).
    pub cell: f64,
    pub shock: f64,
    /// The potential core's length ([`potential_core`]): the axis keeps
    /// the exit state this far.
    pub potential: f64,
    /// Exit temperature and the cooling exponent `2(γ − 1)`.
    pub temperature: f64,
    pub cooling: f64,
    /// Weight of the afterburning (0..1).
    pub burn: f64,
    /// Soot extinction at the exit (1/m), the radial profile's exponent
    /// (Simons, `2/(γ − 1)`) and the core length over which soot thins.
    pub kappa: f64,
    pub profile: f64,
    pub core: f64,
    /// The drawn length and the boundary's radius there.
    pub length: f64,
    pub width: f64,
}

/// The boundary's radius at `x`: `r₀ + x·k·Δ/(Δ + x·k)`, slope `k` at the
/// lip, tending to `r₀ + Δ`; continuous through matched (`k`, `Δ` → 0);
/// plus the mixing layer's growth `x·m`.
pub fn boundary(p: &Plume, x: f64) -> f64 {
    let (k, d) = (p.slope, p.spread);
    let den = d + x * k;
    let guard = if den < 0.0 { -1e-9 } else { 1e-9 } * p.r0.max(1e-3);
    p.r0 + x * k * d / (den + guard) + x * p.mixing
}

/// How fast the jet of exit state `s` widens in air at `ambient` Pa by
/// its turbulent mixing layer (m per m of the boundary, one side):
/// Papamoschou & Roshko's `½·C·Φ·(1 + √s)` for a jet into still air, `s`
/// the air's density over the jet's at the same pressure,
/// `(M_air/M)·(Tⱼ/T_air)`, `Tⱼ` the jet's expanded to the ambient. Half
/// the layer's growth widens the boundary (the layer straddles the lip
/// line). In vacuum (`s` → 0) it falls to `½·C·Φ`, small beside the
/// lip's fan.
pub fn mixing_rate(s: &ExitState, ambient: f64) -> f64 {
    let g = s.gamma;
    let ratio = (ambient.max(0.0) / s.pressure.max(1e-12)).min(1e6);
    let tj = s.temperature * ratio.powf((g - 1.0) / g);
    let density = AIR_MOLAR_MASS / s.molar_mass * tj / AIR_TEMPERATURE;
    0.5 * MIXING_LAYER * COMPRESSIBILITY * (1.0 + density.sqrt())
}

/// The normal-shock static temperature ratio at Mach `m` (≥ 1).
pub fn normal_shock_temperature(m: f64, gamma: f64) -> f64 {
    let m2 = m * m;
    (2.0 * gamma * m2 - (gamma - 1.0)) * ((gamma - 1.0) * m2 + 2.0) / ((gamma + 1.0) * (gamma + 1.0) * m2)
}

/// Strength of the shock cells (0..1): none when matched, none without air
/// to confine the jet.
pub fn shock_strength(pressure_ratio: f64) -> f64 {
    pressure_ratio.max(1e-12).ln().abs().tanh() * (1.0 - (-1.0 / pressure_ratio.max(1e-12)).exp())
}

/// The jet's core length (m): `Dⱼ·(4.2 + 1.1·Mⱼ²)` (supersonic jets' core
/// length, Lau, Morris & Fisher 1979; cited from memory, not checked).
pub fn core_length(jet_radius: f64, jet_mach: f64) -> f64 {
    2.0 * jet_radius * (4.2 + 1.1 * jet_mach * jet_mach)
}

/// Soot's extinction (1/m) at gas density `rho` with soot mass fraction
/// `soot`: Rayleigh-limit absorption `6π·E(m)·f_v/λ`.
pub fn soot_extinction(rho: f64, soot: f64) -> f64 {
    6.0 * PI * SOOT_E_M * soot * rho / SOOT_DENSITY / SOOT_WAVELENGTH
}

/// The potential core's length (m) of a jet leaving exit radius `r0` at
/// Mach `mach`: the first characteristic from the lip (at the Mach angle,
/// `tan μ = 1/√(M² − 1)`) reaches the axis at `r₀·√(M² − 1)`; upstream of
/// it the axis has not felt the lip, whatever the air outside (method of
/// characteristics; Anderson, *Modern Compressible Flow*, ch. 11).
pub fn potential_core(r0: f64, mach: f64) -> f64 {
    r0 * (mach * mach - 1.0).max(0.0).sqrt()
}

/// The gas's spreading on the axis at `x`, `r₀/R` with `R` the boundary
/// a potential core downstream: 1 in the core, then falling as the
/// boundary widens.
pub fn spread_ratio(p: &Plume, x: f64) -> f64 {
    p.r0 / boundary(p, (x - p.potential).max(0.0))
}

/// Temperature on the axis at `x` (the shader's, mirrored for its tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn temperature(p: &Plume, x: f64) -> f64 {
    let iso = p.temperature * spread_ratio(p, x).powf(p.cooling);
    let cells = 0.5 * (1.0 - (2.0 * PI * x / p.cell).cos()) * (-x / (CELLS * p.cell)).exp();
    let shocked = iso * (1.0 + p.shock * cells);
    shocked + (AFTERBURN_TEMPERATURE - shocked) * p.burn * (1.0 - (-x / p.cell).exp())
}

/// The soot's share across the jet at `rho` from the axis at `x` (0..1,
/// times the axis's [`extinction`]). Within an even core it is 1. The core
/// is the exit's radius at the lip and shrinks to the axis over the
/// potential core, as the mixing layer from the lip grows. Outside it,
/// out to the boundary, Simons' `cos(π/2·s)^(2/(γ−1))` applies. That is a
/// far-field profile: applied across the exit too, it drew the jet a third
/// as wide as the bell (the owner's "gap/thickness discrepancy",
/// 2026-10-29). The shader's, mirrored for its tests.
#[cfg_attr(not(test), allow(dead_code))]
pub fn profile(p: &Plume, x: f64, rho: f64) -> f64 {
    let r = boundary(p, x);
    let core = (p.r0 * (1.0 - x / p.potential.max(1e-6)).clamp(0.0, 1.0)).min(r);
    if rho <= core {
        return 1.0;
    }
    if rho >= r {
        return 0.0;
    }
    (0.5 * PI * (rho - core) / (r - core)).cos().powf(p.profile)
}

/// Soot extinction (1/m) on the axis at `x`.
pub fn extinction(p: &Plume, x: f64) -> f64 {
    p.kappa * spread_ratio(p, x).powi(2) * (-x / p.core).exp()
}

/// The afterburning's weight (0..1) in air at `ambient` Pa of exhaust
/// with `unburned` share of its fuel left: none in vacuum or without fuel.
pub fn afterburning(ambient: f64, unburned: f64) -> f64 {
    (1.0 - (-ambient.max(0.0) / AFTERBURN_PRESSURE).exp()) * (1.0 - (-unburned / UNBURNED_SCALE).exp())
}

/// The plume of exit state `s` and its jet `j` in air at `ambient` Pa.
pub fn plume(s: &ExitState, j: &Jet, ambient: f64) -> Plume {
    let g = s.gamma;
    let r0 = s.radius;
    let rj = j.radius.min(r0 * MAX_SPREAD);
    let cell = j.cell.min(core_length(rj, j.mach)).max(r0);
    let burn = afterburning(ambient, s.composition.unburned);
    let mut p = Plume {
        r0,
        slope: j.turn.clamp(-MAX_TURN, MAX_TURN).tan(),
        spread: rj - r0,
        mixing: mixing_rate(s, ambient),
        cell,
        shock: shock_strength(j.pressure_ratio) * (normal_shock_temperature(j.mach, g) - 1.0),
        potential: potential_core(r0, s.mach),
        temperature: s.temperature,
        cooling: 2.0 * (g - 1.0),
        burn,
        kappa: soot_extinction(s.density, s.composition.soot),
        profile: 2.0 / (g - 1.0),
        core: core_length(rj, j.mach),
        length: 0.0,
        width: r0,
    };
    // Out to where the soot is too thin to see (geometric steps).
    let max = MAX_LENGTH * 2.0 * r0;
    let mut x = 0.25 * r0;
    while x < max && 2.0 * boundary(&p, x) * extinction(&p, x) > TAU_FLOOR {
        x *= 1.25;
    }
    p.length = if p.kappa > 0.0 { x.min(max) } else { 0.0 };
    p.width = (0..=8).map(|i| boundary(&p, p.length * f64::from(i) / 8.0)).fold(r0, f64::max);
    p
}

/// Blackbody colour at `t` K as linear RGB in nits: Planck at three
/// wavelengths (610, 550, 465 nm), balanced to white at 6,504 K and scaled
/// so the luminance at platinum's freezing point (2,042 K) is the old
/// candle's 60 cd/cm² (6.0e5 nits).
pub fn planck_rgb(t: f64) -> [f64; 3] {
    let shape = |t: f64| {
        PLANCK_WAVELENGTHS.map(|l| {
            let a = C2 / l;
            ((a / WHITE).exp() - 1.0) / ((a / t.max(1.0)).min(700.0).exp() - 1.0)
        })
    };
    let lum = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let scale = PLATINUM_NITS / lum(shape(PLATINUM));
    shape(t).map(|c| c * scale)
}

/// The plume's bound (the unit cylinder: axis +Y, centred, radius and
/// height 1) at nozzle `n` (craft axes): from the exit down the exhaust,
/// as long and wide as the drawn plume.
pub fn bound(n: &super::parts::Nozzle, p: &Plume) -> Transform {
    let centre = n.exit + n.exhaust * (p.length / 2.0);
    Transform {
        translation: centre.as_vec3(),
        rotation: Quat::from_rotation_arc(Vec3::Y, n.exhaust.as_vec3()),
        scale: Vec3::new(p.width as f32, p.length.max(1e-3) as f32, p.width as f32),
    }
}

/// The second radiation constant (m·K), the three wavelengths (m), the
/// white point and platinum's freezing point (K) and its luminance (nits).
pub const C2: f64 = 0.014_387_77;
pub const PLANCK_WAVELENGTHS: [f64; 3] = [610e-9, 550e-9, 465e-9];
pub const WHITE: f64 = 6_504.0;
pub const PLATINUM: f64 = 2_042.0;
pub const PLATINUM_NITS: f64 = 6.0e5;

/// The plume's uniform (`exhaust.wgsl`'s `PlumeParams`); positions
/// camera-relative.
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct PlumeParams {
    /// xyz: the exit's centre; w: the exit radius.
    pub origin: Vec4,
    /// xyz: the exhaust's direction; w: the drawn length.
    pub axis: Vec4,
    /// x slope, y spread, z cell length, w drawn width.
    pub shape: Vec4,
    /// x exit temperature, y cooling exponent, z shock, w afterburning.
    pub heat: Vec4,
    /// x extinction at the exit, y profile exponent, z core length, w albedo.
    pub soot: Vec4,
    /// rgb: the starlight at the craft (lux); w: the afterburning temperature.
    pub light: Vec4,
    /// rgb: the blackbody scale per channel (nits), w: [`CELLS`].
    pub planck: Vec4,
    /// x: the potential core's length; y: the mixing layer's widening; zw unused.
    pub core: Vec4,
}

impl PlumeParams {
    /// The uniform of plume `p` at exit `origin` along `axis` (camera-relative)
    /// lit by `starlight` (rgb lux).
    pub fn new(p: &Plume, origin: Vec3, axis: Vec3, starlight: [f32; 3]) -> Self {
        // planck_rgb(t)ᵢ = cᵢ/(exp(aᵢ/t) − 1), aᵢ = C2/λᵢ: the shader's cᵢ.
        let white: [f64; 3] = PLANCK_WAVELENGTHS.map(|l| (C2 / l / WHITE).exp() - 1.0);
        let unit = planck_rgb(WHITE);
        let f = |v: f64| v as f32;
        PlumeParams {
            origin: origin.extend(f(p.r0)),
            axis: axis.extend(f(p.length)),
            shape: Vec4::new(f(p.slope), f(p.spread), f(p.cell), f(p.width)),
            heat: Vec4::new(f(p.temperature), f(p.cooling), f(p.shock), f(p.burn)),
            soot: Vec4::new(f(p.kappa), f(p.profile), f(p.core), f(SOOT_ALBEDO)),
            light: Vec3::from_array(starlight).extend(f(AFTERBURN_TEMPERATURE)),
            planck: Vec4::new(f(unit[0] * white[0]), f(unit[1] * white[1]), f(unit[2] * white[2]), f(CELLS)),
            core: Vec4::new(f(p.potential), f(p.mixing), 0.0, 0.0),
        }
    }
}

/// A plume's material: one per engine, its uniform rewritten each frame.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct PlumeMaterial {
    #[uniform(0)]
    pub params: PlumeParams,
}

impl Material for PlumeMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Both faces: the back face draws only when the camera is inside.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The plume material and its shader.
pub struct ExhaustPlugin;

impl Plugin for ExhaustPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "exhaust.wgsl");
        app.add_plugins(MaterialPlugin::<PlumeMaterial>::default());
    }
}

#[cfg(test)]
#[path = "exhaust_tests.rs"]
mod tests;
