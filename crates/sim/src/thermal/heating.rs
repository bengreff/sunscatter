//! Heat into the skin cells: the air's over the whole body, and sunlight
//! (realism-1 §5b, D085). Until 2026-10-26 the stagnation flux was spread
//! over the cells as f·sin^1.5 θ (a Lees-type falloff), at least 3 %; the
//! boundary layer of every cell replaces it.
//!
//! * **Stagnation flux:** Sutton & Graves, "A General Stagnation-Point
//!   Convective Heating Equation for Arbitrary Gas Mixtures", NASA TR R-376
//!   (1971): q̇ = k·√(ρ/Rn)·V³ (SI; k = 1.7415e-4 Earth air, 1.9027e-4
//!   Mars), cold fully catalytic wall; stated accuracy ≈ 10 % for blunt
//!   bodies. Both k checked against NASA's TFAWS 2012 aerothermodynamics
//!   course; West & Brandis (AIAA 2020, NTRS 20200002354, Eq. 2) give
//!   1.83e-4 for 97 % CO₂ / 3 % N₂, 4 % lower and inside the stated 10 %. The nose radius Rn is the bake's per-direction estimate
//!   ([`crate::aero::DirSums::nose_radius`]).
//! * **Radiative stagnation flux** (shock-layer radiation, [`tauber_sutton`]):
//!   Tauber & Sutton, "Stagnation-Point Radiative Heating Relations for
//!   Earth and Mars Entries", J. Spacecraft and Rockets 28(1), 1991:
//!   q̇ = C·Rn^a·ρ^b·f(V) (W/cm², Rn m, ρ kg/m³, V m/s). Earth: C =
//!   4.736e4, b = 1.22, a = 1.072e6·V^−1.88·ρ^−0.325, at most 1 for Rn ≤
//!   1 m, 0.6 to 2 m, 0.5 beyond; f(V) tabulated 9–16 km/s (fitted for ρ
//!   6.66e-5–6.31e-4, Rn 0.3–3 m, 54–72 km); Mars (97 % CO₂): C =
//!   2.35e4, a = 0.526, b = 1.19, f(V) 6–9 km/s. Stated accuracy ±20–30 %
//!   in range (equilibrium, no radiative cooling; NTRS 19910048758, the
//!   abstract). The paper itself is not public: the Mars constants are
//!   checked against West & Brandis (above; fitted 6.5–9 km/s, ρ 1e-4–1e-3,
//!   Rn 1–23 m) and its f(V) against Lu & Saikia's plot (IntechOpen 2020,
//!   fig. 1, all 17 points within 0.2); the validity ranges against Dec &
//!   Braun (NTRS 20060004824, eq. 2); the Earth C, b, f(V) table and the
//!   cap on a only against independent open-source implementations (AMAT,
//!   citing Brandis & Johnston 2014, for C, b and the whole-km f(V);
//!   polmes/aether and mitchwall85/physics_utils for all 19 f(V) points;
//!   traj_calc, physics_utils and Flight-Heating for the cap), all
//!   agreeing.
//!   Added to the exposed cells as q̇·f·sin θ.
//! * **Over the whole body** ([`cell_heat_flow`], D085): each cell's own
//!   boundary layer (`aero::boundary`: the tangent-wedge edge state,
//!   laminar and turbulent, transition by Re_θ, the skin's temperature)
//!   gives its laminar and turbulent acreage heating by Colburn's
//!   analogy. Laminar, a cell takes the smaller of its own plate value
//!   and the larger of the blunt-nose law q̇_stag·(1 − T_w/T₀)·f·sin θ
//!   (the cosine of the body angle from the stagnation point: "a
//!   theoretical basis to the sonic point, reasonable to about 70°",
//!   Tauber, NASA TP-2914, 1989, fig. 5) and a plate along the flow at its
//!   run length (at most q̇_stag): a nose takes its stagnation law, flanks
//!   far downstream their own boundary layer, surfaces along the flow a
//!   plate's; cells facing the flow beyond the sonic point (sin θ > 0.7:
//!   a fin's leading edge, a step) are stagnation regions of their own and
//!   take the nose law (at the craft's nose radius: sharper edges heat
//!   more); turbulent, never less than
//!   laminar; across transition (1−γ)·laminar + γ·turbulent; on the lee
//!   (from 6° leeward) and where separated at least [`LEEWARD_FRACTION`]
//!   of the stagnation flux (windward cells have their own layer: a sharp
//!   nose's huge stagnation flux must not set its flanks'). Hot walls: the stagnation flux × (1 − T_w/T₀) (T₀ the total
//!   temperature), the layers' (T_aw − T_w): negative (cooling) above the
//!   recovery temperature. Rarefied: by Wilmoth's function, the stagnation
//!   flux capped at and bridged to the free-molecular energy flux ½ρV³
//!   (C_H = 1 at full accommodation, Tauber fig. 8; Sutton–Graves passes it
//!   at Kn ≳ 1) in the Knudsen number on the nose radius, and each cell to
//!   its free-molecular ½ρV³·f·sin θ in the Knudsen number on the body's
//!   diameter, as the forces.
//! * **Sunlight:** a flux vector S (W/m², the direction light travels, body
//!   axes); a cell facing it absorbs ε·|S|·cos·A, shadowed by the craft
//!   itself through the same exposure bake (parallel light is a flow
//!   direction). The absorptivity is the emissivity (grey skin) until the
//!   craft data carries a solar absorptivity.

use crate::aero::{self, friction, AeroBake, Flow};
use crate::craft::Cell;
use glam::DVec3;

/// Sutton–Graves constant for Earth air (SI: kg^½·m⁻¹ units, q̇ in W/m²).
pub const SUTTON_GRAVES_EARTH: f64 = 1.7415e-4;
/// Sutton–Graves constant for Mars (CO₂/N₂).
pub const SUTTON_GRAVES_MARS: f64 = 1.9027e-4;
/// Tauber & Sutton's radiative heating relation for one atmosphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TauberSutton {
    Earth,
    Mars,
}

/// Earth f(V) (m/s, W/cm² units of the relation). The points 9, 10, …, 16
/// km/s as reproduced by Brandis & Johnston, "Characterization of
/// Stagnation-Point Heat Flux for Earth Entry", AIAA 2014-2374.
const EARTH_F: [(f64, f64); 19] = [
    (9000.0, 1.5),
    (9250.0, 4.3),
    (9500.0, 9.7),
    (9750.0, 19.5),
    (10000.0, 35.0),
    (10250.0, 55.0),
    (10500.0, 81.0),
    (10750.0, 115.0),
    (11000.0, 151.0),
    (11500.0, 238.0),
    (12000.0, 359.0),
    (12500.0, 495.0),
    (13000.0, 660.0),
    (13500.0, 850.0),
    (14000.0, 1065.0),
    (14500.0, 1313.0),
    (15000.0, 1550.0),
    (15500.0, 1780.0),
    (16000.0, 2040.0),
];

/// Mars (97 % CO₂, 3 % N₂) f(V).
const MARS_F: [(f64, f64); 17] = [
    (6000.0, 0.2),
    (6150.0, 1.0),
    (6300.0, 1.95),
    (6500.0, 3.42),
    (6700.0, 5.1),
    (6900.0, 7.1),
    (7000.0, 8.1),
    (7200.0, 10.2),
    (7400.0, 12.5),
    (7600.0, 14.8),
    (7800.0, 17.1),
    (8000.0, 19.2),
    (8200.0, 21.4),
    (8400.0, 24.1),
    (8600.0, 26.0),
    (8800.0, 28.9),
    (9000.0, 32.8),
];

/// f(V) of a table: zero below its first speed, linear between entries,
/// the last interval continued above.
fn velocity_function(table: &[(f64, f64)], v: f64) -> f64 {
    if v < table[0].0 {
        return 0.0;
    }
    let k = table.partition_point(|e| e.0 <= v).clamp(1, table.len() - 1);
    let (a, b) = (table[k - 1], table[k]);
    a.1 + (b.1 - a.1) * (v - a.0) / (b.0 - a.0)
}

/// Stagnation-point radiative heat flux (W/m²) by Tauber–Sutton at density
/// `rho` (kg/m³), nose radius `nose_radius` (m) and airspeed `speed` (m/s).
pub fn tauber_sutton(model: TauberSutton, rho: f64, nose_radius: f64, speed: f64) -> f64 {
    if rho <= 0.0 || nose_radius <= 0.0 {
        return 0.0;
    }
    let pow = |x: f64, e: f64| crate::math::exp(e * crate::math::ln(x));
    let (c, a, b, f) = match model {
        TauberSutton::Earth => {
            let cap = if nose_radius <= 1.0 {
                1.0
            } else if nose_radius <= 2.0 {
                0.6
            } else {
                0.5
            };
            let a = (1.072e6 * pow(speed, -1.88) * pow(rho, -0.325)).min(cap);
            (4.736e4, a, 1.22, velocity_function(&EARTH_F, speed))
        }
        TauberSutton::Mars => (2.35e4, 0.526, 1.19, velocity_function(&MARS_F, speed)),
    };
    if f <= 0.0 {
        return 0.0;
    }
    c * pow(nose_radius, a) * pow(rho, b) * f * 1e4
}

/// Heating of leeward and shadowed cells as a fraction of the stagnation flux.
pub const LEEWARD_FRACTION: f64 = 0.03;

/// Stagnation-point convective heat flux (W/m²) at density `rho` (kg/m³),
/// nose radius `nose_radius` (m) and airspeed `speed` (m/s).
pub fn sutton_graves(k: f64, rho: f64, nose_radius: f64, speed: f64) -> f64 {
    if rho <= 0.0 || nose_radius <= 0.0 {
        return 0.0;
    }
    k * (rho / nose_radius).sqrt() * speed * speed * speed
}

/// Sunlight absorbed by each cell (W), added to `out`.
fn sunlight(bake: &AeroBake, cells: &[Cell], sun: DVec3, scratch: &mut [f64], out: &mut [f64]) {
    let flux = sun.length();
    if flux > 0.0 {
        let l = sun / flux;
        bake.exposure_at(l, scratch);
        for ((o, c), &f) in out.iter_mut().zip(cells).zip(scratch.iter()) {
            let cos = -c.normal.dot(l);
            if cos > 0.0 {
                *o += c.emissivity * flux * cos * f * c.area;
            }
        }
    }
}

/// Free-molecular heating by air arriving along `dir` (unit, body axes)
/// with energy flux `flux` (W/m² facing it, ½ρV³ at full accommodation):
/// each exposed cell takes `weight`·flux·cos θ·f·A (f its exposure, the
/// craft shading itself), added to `out` (`scratch` a per-cell buffer).
/// The rarefied end of [`cell_heat_flow`], for coasts in thin air.
pub fn free_molecular_heat(
    bake: &AeroBake,
    cells: &[Cell],
    (dir, flux): (DVec3, f64),
    weight: f64,
    scratch: &mut [f64],
    out: &mut [f64],
) {
    bake.exposure_at(dir, scratch);
    for ((h, c), &f) in out.iter_mut().zip(cells).zip(scratch.iter()) {
        let cos = -c.normal.dot(dir);
        if cos > 0.0 {
            *h += flux * cos * f * c.area * weight;
        }
    }
}

/// The air side of [`cell_heat_flow`].
#[derive(Clone, Copy, Debug)]
pub struct FlowHeat<'a> {
    /// The free stream in body axes, as for the forces (its airspeed sets
    /// the density and the gas constant: R = V²/(γ·M²·T)).
    pub flow: &'a Flow,
    /// Sutton–Graves constant of the air (cold-wall stagnation heating).
    pub sutton_graves_k: f64,
    /// Shock-layer radiation, where the atmosphere has it.
    pub radiative: Option<TauberSutton>,
    /// Each cell's skin temperature (K).
    pub wall: &'a [f64],
}

/// Cells facing the flow more steeply than this (sin θ, the sonic point of
/// a blunt body near 45°; Tauber, TP-2914, fig. 5) are in a subsonic
/// stagnation region of their own, wherever they are on the craft (a fin's
/// leading edge, a step): the nose law, not the layer from the front;
/// blended over ±[`STAGNATION_BLEND`]/2 around it.
const STAGNATION_SIN: f64 = 0.7;
const STAGNATION_BLEND: f64 = 0.2;

/// The leeward floor fades in over sin θ from 0 to −this (6°).
const LEE_BLEND: f64 = 0.1;

/// Earth air's specific gas constant (J/(kg·K)), for a flow without an
/// airspeed.
const EARTH_AIR_R: f64 = aero::air::GAS_CONSTANT / aero::air::EARTH_AIR_MOLAR_MASS;

/// Power absorbed by each cell (W) into `out` from the air over the whole
/// body (`air`, if any; see the module notes) and the sunlight `sun` (W/m²,
/// body axes); `scratch` is a per-cell buffer.
pub fn cell_heat_flow(
    bake: &AeroBake,
    cells: &[Cell],
    air: Option<&FlowHeat>,
    sun: DVec3,
    scratch: &mut [f64],
    out: &mut [f64],
) {
    out.fill(0.0);
    if let Some(air) = air {
        let layer = friction::FlowLayer::new(bake, air.flow, Some(air.wall), true);
        convective(bake, cells, air, &layer, scratch, out);
    }
    sunlight(bake, cells, sun, scratch, out);
}

/// [`cell_heat_flow`] with the boundary layer already computed for the
/// air's flow and walls (`layer`, with its heat fluxes: a live tick's, shared
/// with its forces); the same bits.
pub fn cell_heat_layer(
    bake: &AeroBake,
    cells: &[Cell],
    air: &FlowHeat,
    layer: &friction::FlowLayer,
    sun: DVec3,
    scratch: &mut [f64],
    out: &mut [f64],
) {
    out.fill(0.0);
    convective(bake, cells, air, layer, scratch, out);
    sunlight(bake, cells, sun, scratch, out);
}

/// The smaller and the larger in magnitude (the signs agree but where the
/// wall is near the recovery temperature, where both are small).
fn least(a: f64, b: f64) -> f64 {
    if a.abs() <= b.abs() {
        a
    } else {
        b
    }
}
fn most(a: f64, b: f64) -> f64 {
    if a.abs() >= b.abs() {
        a
    } else {
        b
    }
}

/// The air's heat into each cell (W), added to `out`, from the boundary
/// layer `layer` (`cont` a per-cell buffer).
fn convective(
    bake: &AeroBake,
    cells: &[Cell],
    air: &FlowHeat,
    layer: &friction::FlowLayer,
    cont: &mut [f64],
    out: &mut [f64],
) {
    let flow = air.flow;
    if flow.q <= 0.0 || flow.dir.length_squared() == 0.0 || flow.temperature <= 0.0 {
        return;
    }
    let d = flow.dir.normalize();
    let (g, t, m) = (flow.gamma, flow.temperature, flow.mach);
    let r_gas = if flow.speed > 0.0 && m > 0.0 { flow.speed * flow.speed / (g * m * m * t) } else { EARTH_AIR_R };
    let v = if flow.speed > 0.0 { flow.speed } else { m * (g * r_gas * t).sqrt() };
    let rho = 2.0 * flow.q / (v * v);
    let cp = g * r_gas / (g - 1.0);
    let t0 = t * (1.0 + 0.5 * (g - 1.0) * m * m);
    let rn = bake.nose_radius_at(d);
    // The continuum stagnation flux never exceeds the free-molecular
    // energy flux (C_H ≤ 1; Sutton–Graves passes it at Kn ≳ 1).
    // The stagnation point is rarefied on the nose's scale (λ/Rn, Tauber
    // fig. 8: its C_H to 1), the acreage on the body's (λ/D_b, as the
    // forces): a sharp tip goes free molecular long before its flanks.
    let q_fm = 0.5 * rho * v * v * v;
    let nose_bridge = if rn > 0.0 { aero::bridge(flow.knudsen * bake.length / rn) } else { 1.0 };
    let q_conv = q_fm + (sutton_graves(air.sutton_graves_k, rho, rn, v).min(q_fm) - q_fm) * (1.0 - nose_bridge);
    let q_rad = air.radiative.map_or(0.0, |model| tauber_sutton(model, rho, rn, v));
    let b = aero::bridge(flow.knudsen);
    let scale = rho * v * cp;
    let exposure = bake.exposure_weights(d);
    // The attached cells' continuum heating (NaN: separated).
    for (i, (c, layer)) in cont.iter_mut().zip(&layer.layers).enumerate() {
        let Some((s, layer)) = layer else {
            *c = f64::NAN;
            continue;
        };
        let s = *s;
        let hot = 1.0 - air.wall[i] / t0;
        let nose = q_conv * hot * exposure.at(i) * s.max(0.0);
        let aligned = least(scale * layer.heat_aligned, q_conv * hot);
        let from_front = least(scale * layer.heat_laminar, most(nose, aligned));
        let x = ((s - STAGNATION_SIN) / STAGNATION_BLEND + 0.5).clamp(0.0, 1.0);
        let local = x * x * (3.0 - 2.0 * x);
        let lam = local * nose + (1.0 - local) * from_front;
        let turb = most(scale * layer.heat_turbulent, lam);
        let gamma = layer.intermittency;
        *c = (1.0 - gamma) * lam + gamma * turb;
    }
    for (i, (o, c)) in out.iter_mut().zip(cells).enumerate() {
        let hot = 1.0 - air.wall[i] / t0;
        let s = -c.normal.dot(d);
        // The floor holds on the lee (fully from 6° leeward) and where
        // separated; windward cells have their own layer.
        let lee = (-s / LEE_BLEND).clamp(0.0, 1.0);
        let floor = LEEWARD_FRACTION * q_conv * hot * lee * lee * (3.0 - 2.0 * lee);
        let q = if cont[i].is_nan() { LEEWARD_FRACTION * q_conv * hot } else { most(cont[i], floor) };
        let lit = exposure.at(i) * s.max(0.0);
        let rarefied = q_fm * hot * lit;
        *o += ((1.0 - b) * q + b * rarefied + q_rad * lit) * c.area;
    }
}

#[cfg(test)]
#[path = "heating_tests.rs"]
mod tests;
