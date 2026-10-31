//! Plumbing (D104; ship-model.md §5.2; falcon9.md §3): a lumped network of
//! tanks, pressurant bottles and their lines.
//!
//! * **Tank**: liquid mass and temperature (density from the fluid's table),
//!   the ullage's pressurant (moles, temperature) over the liquid; its
//!   pressure is the pressurant's ideal-gas pressure in the ullage plus the
//!   liquid's vapour pressure. The liquid warms from the air (a film
//!   coefficient from falcon9.md §3.6's 2 kW/m² on a frosted LOX tank on the
//!   pad, U) and from sunlight on its wetted wall (the wall's solar
//!   absorptivity); a tank inside a component (Dragon's) is held by its
//!   heaters. Above the relief pressure (the tank's MEOP) the ullage vents;
//!   a liquid at saturation boils off what it is heated by (Q/L), vented.
//! * **Pressurisation** (falcon9.md §3.3): each tank's ullage is held at its
//!   loading pressure by its controller from the bottles plumbed to it, as
//!   far as the lines' choked flow lets it (Cd 0.8, U); the helium enters
//!   heated (250 K into LOX, 300 K into RP-1, U) while a pump-fed engine the
//!   tank feeds is lit, else at the bottle's temperature. A bottle blows down
//!   isentropically and exchanges heat with the liquid it sits in (τ 60 s, U).
//!   The ullage expands adiabatically as the liquid drains, mixes with what
//!   enters and relaxes towards the liquid (τ 300 s, U).
//! * **Feed** (falcon9.md §3.4): inlet pressure = ullage + ρ·a·h (the head
//!   of the proper acceleration along the tank's axis) − the line's loss,
//!   0.05 MPa per feed at full flow (∝ ṁ², U); NPSP = inlet − vapour pressure.
//! * **Settling** (falcon9.md §3.5): the liquid's bulk moves along the tank's
//!   axis under the proper acceleration there (a slug between the outlet and
//!   the far dome, stopping at either) while the Bond number ρ·a·R²/σ is
//!   above 1; below it surface tension holds the liquid, which disperses
//!   towards the tank's middle over the capillary time √(ρR³/σ). The outlet
//!   is covered while the bulk is within 5 cm + 10 % of the liquid's height
//!   of it (U: a sump's allowance; generous, as nothing fails when unsure).
//!   Settling after a coast takes √(2L/a): the stage's nitrogen thrusters
//!   firing aft, as on the real stage.

use crate::catalog::Cycle;
use crate::craft::mass::Slug;
use crate::craft::ship::{ShipDef, TankDef};
use crate::craft::state::{EngineStatus, ShipState, TankState};
use crate::fluids::{fluids, Liquid, R_GAS};
use crate::math;
use glam::DVec3;

/// Air film coefficient on a tank's wetted wall (W/(m²·K)): falcon9.md
/// §3.6's 2 kW/m² on a frosted LOX tank on the pad (U) over its 222 K.
pub const H_AIR: f64 = 9.0;
/// Bond number below which surface tension holds the liquid.
pub const BOND_INERTIAL: f64 = 1.0;
/// Line loss at an engine's full flow (Pa, falcon9.md §3.4, U).
pub const LINE_LOSS: f64 = 5.0e4;
/// Heated pressurant's inlet temperatures (K, falcon9.md §3.3, U).
pub const HEATED_CRYO_K: f64 = 250.0;
pub const HEATED_K: f64 = 300.0;
/// Discharge coefficient of the pressurant lines' choked flow (U).
const CD: f64 = 0.8;
/// A bottle's and an ullage's thermal time constants (s, U).
const TAU_BOTTLE: f64 = 60.0;
const TAU_ULLAGE: f64 = 300.0;
/// Smallest ullage as a share of the tank (numerical floor).
const MIN_ULLAGE: f64 = 1e-4;
/// A liquid under this (kg) is gone: a tank burned dry keeps no residual
/// for the sunlight to heat or a vent to cool (a 10⁻¹⁵ kg residual's heat
/// capacity took the booster's LOX ullage to 8.7·10⁸ K).
pub const RESIDUAL: f64 = 1e-6;
/// Pressurant (mol) under which an ullage holds no gas.
const NO_GAS: f64 = 1e-9;
const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

/// Surface tension (N/m) at the propellants' usual temperatures: LOX
/// 0.0193 at 66.5 K (NIST, falcon9.md §3.5); RP-1 0.026, NTO 0.027, MMH
/// 0.034, LCH₄ 0.014, LH₂ 0.0019, water 0.072 (class values, E).
pub fn surface_tension(fluid: &str) -> f64 {
    match fluid {
        "LOX" => 0.0193,
        "RP-1" => 0.026,
        "NTO" => 0.027,
        "MMH" => 0.034,
        "LCH4" => 0.014,
        "LH2" => 0.0019,
        "water" => 0.072,
        _ => 0.025,
    }
}

/// A tank's geometry and liquid now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// Unit axis from the outlet (the base) along the fill.
    pub axis: DVec3,
    /// The tank's length as a cylinder of its volume, and its radius (m).
    pub length: f64,
    pub radius: f64,
    /// Liquid density (kg/m³), volume (m³) and height over the outlet (m).
    pub density: f64,
    pub liquid_volume: f64,
    pub height: f64,
    /// Ullage volume (m³).
    pub ullage: f64,
    /// The tank's centre (craft axes).
    pub centre: DVec3,
}

/// The liquid of tank `t` (its table), if known.
pub fn liquid_of(t: &TankDef) -> Option<&'static Liquid> {
    fluids().liquid(&t.fluid).ok()
}

/// Draws `kg` from a tank's liquid; a residual under [`RESIDUAL`] goes too.
pub fn drain(s: &mut TankState, kg: f64) {
    s.liquid = (s.liquid - kg).max(0.0);
    if s.liquid < RESIDUAL {
        s.liquid = 0.0;
    }
}

/// Keeps a tank's state physical after its heat and its venting: a residual
/// under [`RESIDUAL`] goes, the liquid's temperature stays where it is a
/// liquid (its freezing to its critical point, the fluid table's range),
/// and an ullage without gas is at the liquid's temperature.
fn keep_physical(t: &TankDef, s: &mut TankState) {
    drain(s, 0.0);
    if let Some(l) = liquid_of(t) {
        s.liquid_k = s.liquid_k.clamp(l.freezing_k, l.critical_k);
    }
    if s.ullage_moles < NO_GAS {
        s.ullage_k = s.liquid_k;
    }
}

/// A tank's geometry with its liquid now.
pub fn geometry(t: &TankDef, s: &TankState) -> Geometry {
    let density = liquid_of(t).map_or(1000.0, |l| l.density(s.liquid_k));
    let r = t.region.radius;
    let area = math::PI * r * r;
    let length = t.volume / area;
    let liquid_volume = s.liquid / density;
    let height = (liquid_volume / area).min(length);
    let ullage = (t.volume - liquid_volume).max(MIN_ULLAGE * t.volume);
    let axis = t.region.axis.normalize();
    Geometry {
        axis,
        length,
        radius: r,
        density,
        liquid_volume,
        height,
        ullage,
        centre: t.region.base + axis * (0.5 * length),
    }
}

/// The ullage's pressure (Pa): the pressurant's in the ullage plus the
/// liquid's vapour's ([`vapour`]).
pub fn pressure(t: &TankDef, s: &TankState) -> f64 {
    let g = geometry(t, s);
    s.ullage_moles * R_GAS * s.ullage_k / g.ullage + vapour(t, s, g.ullage)
}

/// The vapour's partial pressure in a tank's ullage of `ullage` m³ (Pa):
/// the liquid's vapour pressure, at most what all of the liquid gives as
/// gas there (a residual cannot fill a dry tank with saturated vapour: a
/// few grams of LOX at its 4 bar boiling point would vent all the helium).
pub(crate) fn vapour(t: &TankDef, s: &TankState, ullage: f64) -> f64 {
    let all = |l: &Liquid| s.liquid / l.molar_mass * R_GAS * s.liquid_k / ullage;
    liquid_of(t).map_or(0.0, |l| l.vapour_pressure(s.liquid_k).min(all(l)))
}

/// The proper acceleration along a tank's axis towards its outlet (m/s²;
/// positive settles it): the craft's at its centre of mass `com` plus the
/// rotation's at the tank's centre.
pub fn settling_accel(g: &Geometry, com: DVec3, accel: DVec3, omega: DVec3, alpha: DVec3) -> f64 {
    let r = g.centre - com;
    (accel + alpha.cross(r) + omega.cross(omega.cross(r))).dot(g.axis)
}

/// The Bond number of a tank's liquid under acceleration `a`.
pub fn bond(t: &TankDef, g: &Geometry, a: f64) -> f64 {
    g.density * a.abs() * g.radius * g.radius / surface_tension(&t.fluid)
}

/// How far from the outlet the liquid may move and still cover it (m).
pub fn cover(g: &Geometry) -> f64 {
    0.05 + 0.1 * g.height
}

/// Moves a tank's liquid for `dt` under the settling acceleration `a` (see
/// the module): the slug between the walls above the Bond threshold, the
/// capillary drift towards the middle below it.
pub fn settle(s: &mut TankState, t: &TankDef, g: &Geometry, a: f64, dt: f64) {
    let free = (g.length - g.height).max(0.0);
    if bond(t, g, a) >= BOND_INERTIAL {
        s.drift_speed -= a * dt;
        s.drift += s.drift_speed * dt;
        if s.drift <= 0.0 {
            s.drift = 0.0;
            s.drift_speed = s.drift_speed.max(0.0);
        } else if s.drift >= free {
            s.drift = free;
            s.drift_speed = s.drift_speed.min(0.0);
        }
    } else {
        let sigma = surface_tension(&t.fluid);
        let tau = math::sqrt_pos(g.density * g.radius * g.radius * g.radius / sigma);
        s.drift += (0.5 * free - s.drift) * (1.0 - math::exp(-dt / tau));
        s.drift_speed = 0.0;
    }
}

/// Whether a tank's outlet is covered by its liquid (see the module).
pub fn covered(s: &TankState, g: &Geometry) -> bool {
    s.liquid > 0.0 && s.drift <= cover(g)
}

/// Where a tank's liquid sits (F2; the mass model, the thermal nodes): the
/// slug of its height, `s.drift` from the outlet along the tank (within
/// the walls).
pub fn slug(t: &TankDef, s: &TankState) -> Slug {
    let g = geometry(t, s);
    let drift = s.drift.clamp(0.0, (g.length - g.height).max(0.0));
    Slug { mass: s.liquid, base: t.region.base + g.axis * drift, axis: g.axis, height: g.height, radius: g.radius }
}

/// Where a tank's ullage gas sits: the centroid of the tank's length the
/// slug leaves free, below and above it (the far dome when there is none).
pub fn ullage_centre(t: &TankDef, slug: &Slug) -> DVec3 {
    let length = t.volume / (math::PI * t.region.radius * t.region.radius);
    let below = (slug.base - t.region.base).dot(slug.axis).max(0.0);
    let above = (length - below - slug.height).max(0.0);
    if below + above <= 0.0 {
        return t.region.base + slug.axis * length;
    }
    let along = (below * below * 0.5 + above * (length - 0.5 * above)) / (below + above);
    t.region.base + slug.axis * along
}

/// What every tank and bottle holds for the thermal nodes (F2), in the
/// volume grid's order (`ShipDef::regions`: tanks, then bottles): each
/// liquid where it sits ([`slug`]) at its specific heat, each bottle's gas
/// filling it at its cᵥ (cₚ/γ). The ullages' gas (kilograms) is left out.
pub fn held(def: &ShipDef, state: &ShipState) -> Vec<crate::craft::design::Held> {
    use crate::craft::design::Held;
    let f = fluids();
    let liquids = def.tanks.iter().zip(&state.tanks).map(|(t, s)| {
        let slug = slug(t, s);
        let from = (slug.base - t.region.base).dot(slug.axis);
        let specific_heat = liquid_of(t).map_or(0.0, |l| l.specific_heat);
        let at = slug.base + slug.axis * (0.5 * slug.height);
        Held { mass: s.liquid, specific_heat, from, to: from + slug.height, at }
    });
    let regions = def.regions();
    let gases = (def.bottles.iter().zip(&state.bottles).zip(&regions[def.tanks.len()..])).map(|((b, s), r)| {
        let gas = f.gas(&b.gas).ok();
        Held {
            mass: s.moles * gas.map_or(0.0, |g| g.molar_mass),
            specific_heat: gas.map_or(0.0, |g| g.cp / g.gamma),
            from: 0.0,
            to: r.0.axis.length(),
            at: b.pos,
        }
    });
    liquids.chain(gases).collect()
}

/// Every tank's liquid where it sits ([`slug`]) and the gas as point
/// masses (each bottle's at its place, each ullage's at [`ullage_centre`]):
/// what `MassModel::with_slugs` takes.
pub fn contents(def: &ShipDef, state: &ShipState) -> (Vec<Slug>, Vec<(DVec3, f64)>) {
    let f = fluids();
    let slugs: Vec<Slug> = def.tanks.iter().zip(&state.tanks).map(|(t, s)| slug(t, s)).collect();
    let bottles = (state.bottles.iter().zip(&def.bottles))
        .map(|(s, b)| (b.pos, s.moles * f.gas(&b.gas).map_or(0.0, |g| g.molar_mass)));
    let ullages = (def.tanks.iter().zip(&state.tanks).zip(&slugs)).map(|((t, s), slug)| {
        (ullage_centre(t, slug), s.ullage_moles * f.gas(&t.load.pressurant).map_or(0.0, |g| g.molar_mass))
    });
    let points = bottles.chain(ullages).collect();
    (slugs, points)
}

/// A tank's conditions for the engines it feeds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Feed {
    /// The outlet covered (settled liquid).
    pub covered: bool,
    /// Ullage pressure, the liquid's acceleration head over the outlet and
    /// the vapour pressure (Pa).
    pub pressure: f64,
    pub head: f64,
    pub vapour: f64,
    /// The liquid's density (kg/m³), the settling acceleration (m/s²), the
    /// outlet and the tank's axis (craft axes): the line's own head down to
    /// an inlet.
    pub density: f64,
    pub accel: f64,
    pub outlet: DVec3,
    pub axis: DVec3,
}

impl Feed {
    /// Net positive suction pressure (Pa) at an inlet at `inlet` (craft
    /// axes) drawing at `level` of its full flow: the ullage, the liquid's
    /// head and the line's (full of liquid down from the outlet), less the
    /// line's loss and the vapour pressure.
    pub fn npsp(&self, level: f64, inlet: DVec3) -> f64 {
        let drop = (self.outlet - inlet).dot(self.axis).max(0.0);
        let line = self.density * self.accel.max(0.0) * drop;
        self.pressure + self.head + line - LINE_LOSS * level * level - self.vapour
    }
}

/// Every tank's feed conditions now, with the settling acceleration `a`
/// at each (`settling_accel`).
pub fn feeds(def: &ShipDef, state: &ShipState, a: &[f64]) -> Vec<Feed> {
    (def.tanks.iter().zip(&state.tanks).enumerate())
        .map(|(i, (t, s))| {
            let g = geometry(t, s);
            Feed {
                covered: covered(s, &g),
                pressure: pressure(t, s),
                head: g.density * a[i].max(0.0) * g.height,
                vapour: liquid_of(t).map_or(0.0, |l| l.vapour_pressure(s.liquid_k)),
                density: g.density,
                accel: a[i],
                outlet: t.region.base,
                axis: g.axis,
            }
        })
        .collect()
}

/// The bottles plumbed to tank `i`'s pressurant port (indices into
/// `ShipDef::bottles`).
pub fn bottles_of(def: &ShipDef, i: usize) -> Vec<usize> {
    let port = format!("{}.press", def.tanks[i].id);
    let owner = |end: &str| end.split(':').next().unwrap_or("").to_string();
    let mut out: Vec<usize> = (def.lines.iter())
        .filter_map(|l| {
            let other = if l.to == port {
                owner(&l.from)
            } else if l.from == port {
                owner(&l.to)
            } else {
                return None;
            };
            def.bottles.iter().position(|b| b.id == other)
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether tank `i`'s pressurant is heated now: a lit pump-fed engine draws
/// on it (its gas generator's heat exchanger, falcon9.md §3.3).
fn heated(def: &ShipDef, state: &ShipState, i: usize) -> bool {
    (def.engines.iter().zip(&state.engines)).any(|(e, s)| {
        matches!(s.status, EngineStatus::Starting | EngineStatus::Running)
            && e.data.cycle != Cycle::PressureFed
            && e.feeds.iter().any(|f| f.contains(&(i as u32)))
    })
}

/// Choked flow (kg/s) of a gas at `p`, `t` through a line of diameter `d`.
fn choked(gamma: f64, r: f64, p: f64, t: f64, d: f64) -> f64 {
    let area = 0.25 * math::PI * d * d;
    let k = math::sqrt_pos(gamma) * math::pow(2.0 / (gamma + 1.0), (gamma + 1.0) / (2.0 * (gamma - 1.0)));
    CD * area * p * k / math::sqrt_pos(r * t)
}

/// Holds tank `i`'s ullage at its loading pressure from its bottles for
/// `dt` (see the module); returns the moles let in.
fn pressurise(def: &ShipDef, state: &mut ShipState, i: usize, dt: f64) -> f64 {
    let t = &def.tanks[i];
    let Ok(gas) = fluids().gas(&t.load.pressurant) else { return 0.0 };
    let (liquid_k, moles0, k0) = (state.tanks[i].liquid_k, state.tanks[i].ullage_moles, state.tanks[i].ullage_k);
    let g = geometry(t, &state.tanks[i]);
    let target = t.load.pressure - vapour(t, &state.tanks[i], g.ullage);
    if moles0 * R_GAS * k0 / g.ullage >= target {
        return 0.0;
    }
    let hot = heated(def, state, i);
    let port = format!("{}.press", t.id);
    let line = def.lines.iter().find(|l| l.to == port || l.from == port).map_or(0.02, |l| l.diameter);
    let mut let_in = 0.0;
    for b in bottles_of(def, i) {
        let (bd, bs) = (&def.bottles[b], &state.bottles[b]);
        if bd.gas != t.load.pressurant || bs.moles <= 0.0 {
            continue;
        }
        let p_bottle = bs.moles * R_GAS * bs.k / bd.volume;
        if p_bottle <= t.load.pressure {
            continue;
        }
        let t_in = match (hot, liquid_k < 150.0) {
            (true, true) => HEATED_CRYO_K,
            (true, false) => HEATED_K,
            _ => bs.k,
        };
        // The moles that bring the ullage to its target once mixed: the
        // incoming gas's enthalpy cₚ·T is γ·cᵥ·T.
        let s = &state.tanks[i];
        let wanted = ((target * g.ullage / R_GAS - s.ullage_moles * s.ullage_k) / (gas.gamma * t_in)).max(0.0);
        let cap = choked(gas.gamma, gas.r(), p_bottle, bs.k, line) * dt / gas.molar_mass;
        let dn = wanted.min(cap).min(bs.moles);
        if dn <= 0.0 {
            break;
        }
        let bs = &mut state.bottles[b];
        let n1 = bs.moles - dn;
        bs.k *= math::pow(n1 / bs.moles, gas.gamma - 1.0);
        bs.moles = n1;
        let s = &mut state.tanks[i];
        s.ullage_k = (s.ullage_moles * s.ullage_k + dn * gas.gamma * t_in) / (s.ullage_moles + dn);
        s.ullage_moles += dn;
        let_in += dn;
        if dn >= wanted {
            break;
        }
    }
    let_in
}

/// Vents tank `t` down to its relief pressure (its MEOP); a liquid at
/// saturation there boils what heats it. Returns the mass vented (kg).
fn relieve(t: &TankDef, s: &mut TankState) -> f64 {
    let Some(liquid) = liquid_of(t) else { return 0.0 };
    let molar = fluids().gas(&t.load.pressurant).map_or(0.004, |g| g.molar_mass);
    let mut vented = 0.0;
    // Saturated at the relief pressure: the excess heat boils off.
    let saturation = liquid.saturation_temperature(t.pressure);
    if s.liquid_k > saturation && s.liquid > 0.0 {
        let boiled = (s.liquid * liquid.specific_heat * (s.liquid_k - saturation) / liquid.latent_heat).min(s.liquid);
        s.liquid -= boiled;
        s.liquid_k = saturation;
        vented += boiled;
    }
    let g = geometry(t, s);
    let vapour = vapour(t, s, g.ullage);
    let gas = s.ullage_moles * R_GAS * s.ullage_k / g.ullage;
    if gas + vapour > t.pressure && s.ullage_moles > 0.0 {
        let keep = ((t.pressure - vapour).max(0.0) * g.ullage / (R_GAS * s.ullage_k)).min(s.ullage_moles);
        let out = s.ullage_moles - keep;
        // The vapour leaves with the pressurant, in proportion, from the liquid.
        let with = if gas > 0.0 { out * vapour / gas } else { 0.0 };
        let evaporated = (with * liquid.molar_mass).min(s.liquid);
        s.liquid -= evaporated;
        s.liquid_k -= evaporated * liquid.latent_heat / (s.liquid.max(1e-9) * liquid.specific_heat);
        s.ullage_moles = keep;
        vented += out * molar + evaporated;
    }
    vented
}

/// What a tank's slow step reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surroundings {
    /// Sunlight at the vessel (W/m², along the light, body axes).
    pub sun: DVec3,
    /// The air's temperature, if in an atmosphere (K).
    pub air_k: Option<f64>,
}

/// Whether tank `t` sits inside a component, whose heaters hold it (no
/// heat from outside reaches its liquid).
fn heated_inside(t: &TankDef) -> bool {
    matches!(t.owner, crate::craft::handle::Owner::Component(_))
}

/// Solar absorptivity and emissivity of tank `t`'s wall: its part's finish,
/// else its material (0.3 and 0.1 when neither is known).
fn optics(def: &ShipDef, t: &TankDef) -> (f64, f64) {
    let part = def.part_index(t.part).map(|i| &def.parts[i]);
    let mats = crate::materials::materials();
    let finish = part.and_then(|p| p.finish.as_deref()).and_then(|f| mats.finish(f).ok());
    let bare = part.and_then(|p| mats.get(&p.material).ok());
    let alpha = finish.map(|f| f.absorptivity).or(bare.map(|m| m.absorptivity)).unwrap_or(0.3);
    let epsilon = finish.map(|f| f.emissivity).or(bare.map(|m| m.emissivity)).unwrap_or(0.1);
    (alpha, epsilon)
}

/// Heat into tank `i`'s liquid (W): the air's film on its wetted wall and
/// the sunlight its wetted wall absorbs, less its radiation in vacuum; none
/// for a tank inside a component (held by its heaters).
pub fn heat_in(def: &ShipDef, state: &ShipState, i: usize, env: &Surroundings) -> f64 {
    let t = &def.tanks[i];
    let s = &state.tanks[i];
    if s.liquid <= 0.0 || heated_inside(t) {
        return 0.0;
    }
    let g = geometry(t, s);
    let wetted = 2.0 * math::PI * g.radius * g.height + math::PI * g.radius * g.radius;
    let (alpha, epsilon) = optics(def, t);
    let flux = env.sun.length();
    let sunlit = if flux > 0.0 {
        let c = (env.sun / flux).dot(g.axis).abs().min(1.0);
        let side = math::sqrt_pos(1.0 - c * c);
        alpha * flux * (2.0 * g.radius * g.height * side + math::PI * g.radius * g.radius * c)
    } else {
        0.0
    };
    match env.air_k {
        Some(air) => H_AIR * wetted * (air - s.liquid_k) + sunlit,
        None => sunlit - epsilon * STEFAN_BOLTZMANN * wetted * math::powi(s.liquid_k, 4),
    }
}

/// The slow part of the plumbing over `dt`: each tank's heat, its relief and
/// boil-off, its pressurisation, the bottles' heat exchange and the
/// ullage's relaxation. Returns the mass vented overboard (kg).
pub fn slow(def: &ShipDef, state: &mut ShipState, env: &Surroundings, dt: f64) -> f64 {
    let mut vented = 0.0;
    for i in 0..def.tanks.len() {
        let q = heat_in(def, state, i, env);
        let s = &mut state.tanks[i];
        // A residual goes before it heats (its vapour would read the
        // table's top, 42 bar for LOX).
        drain(s, 0.0);
        if s.liquid > 0.0 {
            if let Some(l) = liquid_of(&def.tanks[i]) {
                s.liquid_k += q * dt / (s.liquid * l.specific_heat);
            }
            keep_physical(&def.tanks[i], s);
            s.ullage_k += (s.liquid_k - s.ullage_k) * (1.0 - math::exp(-dt / TAU_ULLAGE));
        }
        let out = relieve(&def.tanks[i], &mut state.tanks[i]);
        keep_physical(&def.tanks[i], &mut state.tanks[i]);
        state.tanks[i].vented += out;
        vented += out;
    }
    // Bottles exchange heat with the liquid they sit in.
    for b in 0..def.bottles.len() {
        let pos = def.bottles[b].pos;
        let around = (def.tanks.iter().zip(&state.tanks)).find(|(t, s)| {
            let g = geometry(t, s);
            t.part == def.bottles[b].part && (pos - t.region.base).dot(g.axis) <= g.height
        });
        if let Some((_, s)) = around {
            let k = s.liquid_k;
            let bs = &mut state.bottles[b];
            bs.k += (k - bs.k) * (1.0 - math::exp(-dt / TAU_BOTTLE));
        }
    }
    vented
}

/// The fast part over a tick of `dt` after the liquids drained: the
/// ullage's adiabatic expansion, the pressurisation, each liquid's settling
/// under the settling accelerations `a`.
pub fn fast(def: &ShipDef, state: &mut ShipState, before: &[f64], a: &[f64], dt: f64) {
    // The regulator runs: the structure reads the tanks' real pressures.
    state.pressure_regulated = true;
    for i in 0..def.tanks.len() {
        let t = &def.tanks[i];
        let gamma = fluids().gas(&t.load.pressurant).map_or(1.667, |g| g.gamma);
        let s = &state.tanks[i];
        let g = geometry(t, s);
        let v0 = before[i].max(MIN_ULLAGE * t.volume);
        if g.ullage != v0 {
            state.tanks[i].ullage_k *= math::pow(v0 / g.ullage, gamma - 1.0);
        }
        let n = pressurise(def, state, i, dt);
        state.tanks[i].pressurant_in += n;
        let g = geometry(t, &state.tanks[i]);
        settle(&mut state.tanks[i], t, &g, a[i], dt);
    }
}

/// Each tank's ullage volume now (before a tick's draw).
pub fn ullages(def: &ShipDef, state: &ShipState) -> Vec<f64> {
    def.tanks.iter().zip(&state.tanks).map(|(t, s)| geometry(t, s).ullage).collect()
}

/// Each tank's settling acceleration (`settling_accel`).
pub fn accels(def: &ShipDef, state: &ShipState, com: DVec3, accel: DVec3, omega: DVec3, alpha: DVec3) -> Vec<f64> {
    (def.tanks.iter().zip(&state.tanks))
        .map(|(t, s)| settling_accel(&geometry(t, s), com, accel, omega, alpha))
        .collect()
}

#[path = "plumbing_coast.rs"]
mod coast;
pub use coast::{equilibrate, float, heat_law, HeatLaw};

#[cfg(test)]
#[path = "plumbing_tests.rs"]
mod tests;
