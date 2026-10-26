//! The boundary layer on each wetted cell (D070, D085): its edge state,
//! laminar–turbulent transition, skin friction and convective heating,
//! with the wall's own temperature. One owner for both: friction
//! ([`super::friction`]) and acreage heating (`thermal::heating`) read the
//! same layer, heating through Reynolds' analogy.
//!
//! * **Edge state** (Newtonian tangent wedge, the model behind Tauber's
//!   closed-form heating relations, NASA TP-2914, 1989, eqs. 57–58): a cell
//!   at incidence θ (sin θ = s = −n·d) keeps the tangential velocity,
//!   u_e = V·cos θ; the normal kinetic energy becomes enthalpy, T_e/T∞ =
//!   1 + (γ−1)/2·M²·s²; the pressure is the free stream's plus the
//!   modified-Newtonian impact pressure on exposed windward cells, p_e/p∞ =
//!   1 + (γ/2)·M²·Cp,max·f·s² (f the cell's exposure); ρ_e/ρ∞ =
//!   (p_e/p∞)/(T_e/T∞). At s = 0 the edge is the free stream (a plate
//!   along the flow). Viscosity by Sutherland's law.
//! * **Laminar** (Blasius with Eckert's reference temperature, wall at
//!   T_w): T*/T_e = 1 + 0.032·M_e² + 0.58·(T_w/T_e − 1) (White, *Viscous
//!   Fluid Flow*, 3rd ed., eq. 7-44), C* = ρ*μ*/(ρ_e·μ_e), local c_f =
//!   0.664·√(C*/Re_x) on the edge's ½ρ_e·u_e², averaged exactly over a
//!   cell's run lengths; momentum thickness Re_θ = 0.664·√(C*·Re_x).
//! * **Turbulent** (White's c_f = 0.455/ln²(0.06·Re_x), *Viscous Fluid
//!   Flow* eq. 6-78, transformed by Van Driest II with the wall at T_w:
//!   White eq. 7-102; Hopkins & Inouye, AIAA J. 9(6), 1971, found it the
//!   best of the transformations against flat-plate and cone data, within
//!   ~10 %), from the leading edge.
//! * **Transition**: the layer starts to turn turbulent where Re_θ first
//!   reaches max([`RE_THETA_LOW`], [`RE_THETA_PER_MACH`]·M_e): the smooth
//!   flat plate's onset at Re_x 5·10⁵ at low edge Mach numbers, the
//!   Re_θ/M_e = const correlation of the Shuttle orbiter's flight data at
//!   hypersonic speeds (Tauber eq. 51 and fig. 11); or where the edge
//!   Reynolds number of the run reaches sharp slender cones' flight values
//!   against the edge Mach number ([`SLENDER_ONSET`], Softley via Tauber
//!   fig. 10), whichever comes first. Onset x_t from the
//!   cell's own edge state (local similarity); the transitional region is
//!   as long as the laminar run before it (Tauber §4.2), the turbulent
//!   share across it Dhawan & Narasimha's intermittency γ = 1 −
//!   exp(−0.412·ξ²), ξ = (x − x_t)/λ (J. Fluid Mech. 3, 1958), with
//!   3.34·λ = x_t (γ = 0.99 at 2·x_t). Friction and heating are (1−γ)·laminar
//!   + γ·turbulent.
//! * **Heating** (Colburn's Reynolds analogy, St = (c_f/2)·Pr^−⅔; Tauber
//!   eq. 52: the form that fitted Zoby & Graves' flight and tunnel data
//!   best): q̇ = St·ρ_e·u_e·c_p·(T_aw − T_w), T_aw = T_e·(1 + r·(γ−1)/2·M_e²),
//!   r = √Pr laminar, ∛Pr turbulent. Negative when the wall is hotter than
//!   the recovery temperature (the air cools it). In air at entry speeds
//!   the turbulent heating is at least Tauber's real-gas closed form
//!   ([`Edge::tauber_turbulent`]): with a perfect gas the reference state
//!   is far too hot once air dissociates.
//! * **The gas** ([`super::gas`]): viscosity, Prandtl number and recovery
//!   factors of the atmosphere's gas (air, CO₂, N₂).
//!
//! Per unit free stream: shear per unit q∞, heat flux per unit ρ∞·V·c_p
//! (kelvin), so friction needs no airspeed or gas constant.

use super::air;
use super::gas::{Gas, Properties};
use crate::math;

/// Prandtl number of air.
pub const PRANDTL: f64 = 0.72;
/// Recovery factors: √Pr laminar, ∛Pr turbulent.
pub const LAMINAR_RECOVERY: f64 = 0.85;
pub const TURBULENT_RECOVERY: f64 = 0.89;
/// Colburn's analogy factor 2·St/c_f = Pr^−⅔ (Pr 0.72).
pub const COLBURN: f64 = 1.244_8;
/// Transition onset at low edge Mach numbers: the momentum-thickness
/// Reynolds number of a smooth flat plate at Re_x 5·10⁵ (0.664·√Re_x).
pub const RE_THETA_LOW: f64 = 469.5;
/// Transition onset at hypersonic speeds: Re_θ/M_e. Tauber (NASA TP-2914,
/// eq. 51, after Amirkabirian's Shuttle correlation) gives 150–350
/// depending on the roughness height over the momentum thickness; the
/// smooth end, 350, puts STS-1's transition front on the windward
/// centreline where it was measured at 52.9, 47.5 and 44.2 km with this
/// edge model (`tests/aero_boundary.rs`; STS-1 to 4 values at onset
/// were 200–400, his fig. 11).
pub const RE_THETA_PER_MACH: f64 = 350.0;
/// Transition onset of sharp slender cones in flight: the edge Reynolds
/// number of the run against the edge Mach number (Softley et al., as
/// faired in Tauber, NASA TP-2914, fig. 10: Re/ft 2·10⁶, uniform wall
/// temperature; read from the figure), held at its ends. Below M_e ≈ 3
/// the momentum-thickness criterion always comes first; at hypersonic edge
/// Mach numbers this one does (Re_θ/M_e = 350 would put a sharp cone's onset
/// ~30 times later than flight at M_e 10).
pub const SLENDER_ONSET: [(f64, f64); 11] = [
    (4.9, 3.0e6),
    (6.0, 3.25e6),
    (7.0, 3.6e6),
    (8.0, 4.2e6),
    (9.0, 5.0e6),
    (10.0, 6.2e6),
    (11.0, 8.0e6),
    (12.0, 1.07e7),
    (13.0, 1.5e7),
    (14.0, 2.2e7),
    (14.7, 3.2e7),
];
/// Tauber's real-gas turbulent heating of air is weighted in between these
/// airspeeds (m/s; his relation holds above 3,962).
const TAUBER_FROM: f64 = 3_500.0;
const TAUBER_FULL: f64 = 4_500.0;
/// Roughness-induced transition: Re_θ·(k/θ)·(T_e/T_w)^1.31 at onset
/// (Paredes et al.'s revision of Reshotko & Tumin, via Hein et al. 2019,
/// eq. 15).
pub const ROUGHNESS_TRIP: f64 = 455.0;
/// Dhawan & Narasimha's intermittency constant.
const DN: f64 = 0.412;
/// ξ at the end of transition (γ = 0.99): √(ln 100 / 0.412).
const XI_END: f64 = 3.343;

/// The free stream of one evaluation, as the boundary layer needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stream {
    /// Free-stream Mach number, γ, static temperature (K).
    pub mach: f64,
    pub gamma: f64,
    pub temperature: f64,
    /// ρ∞·V/μ∞ (1/m).
    pub re_per_m: f64,
    /// μ∞ (Pa·s).
    pub mu: f64,
    /// (γ−1)/2·M²: T_e/T∞ = 1 + this·s².
    kt: f64,
    /// (γ/2)·M²·Cp,max: p_e/p∞ = 1 + this·f·s².
    kp: f64,
    /// The gas (viscosity, Prandtl number).
    props: &'static Properties,
    gas: Gas,
    /// Airspeed (m/s) and density (kg/m³), for the real-gas turbulent
    /// heating of air ([`Edge::tauber_turbulent`]); zero: none.
    speed: f64,
    rho: f64,
    /// Surface roughness height k (m; peak to valley), for roughness-induced
    /// transition ([`ROUGHNESS_TRIP`]); zero: smooth.
    roughness: f64,
}

impl Stream {
    /// From the free stream: Reynolds number per metre, Mach, γ, static
    /// temperature (K) and the Newtonian stagnation pressure coefficient
    /// `cp_max` (zero: no impact pressure).
    pub fn new(re_per_m: f64, mach: f64, gamma: f64, temperature: f64, cp_max: f64) -> Self {
        let m2 = mach * mach;
        let props = Gas::Air.properties();
        Stream {
            mach,
            gamma,
            temperature,
            re_per_m,
            mu: props.viscosity(temperature),
            kt: 0.5 * (gamma - 1.0) * m2,
            kp: 0.5 * gamma * m2 * cp_max,
            props,
            gas: Gas::Air,
            speed: 0.0,
            rho: 0.0,
            roughness: 0.0,
        }
    }

    /// With the surface's roughness height `k` (m).
    pub fn with_roughness(mut self, k: f64) -> Self {
        self.roughness = k.max(0.0);
        self
    }

    /// The same stream in `gas` (Earth air in [`Self::new`]).
    pub fn with_gas(mut self, gas: Gas) -> Self {
        self.props = gas.properties();
        self.gas = gas;
        self.mu = self.props.viscosity(self.temperature);
        self
    }

    /// With the airspeed (m/s) and density (kg/m³) of the free stream.
    pub fn with_speed(mut self, speed: f64, rho: f64) -> Self {
        (self.speed, self.rho) = (speed, rho);
        self
    }

    /// From an aerodynamic flow (Cp,max as the pressure model's).
    pub fn of(flow: &super::Flow) -> Self {
        let cp = air::cp_max(flow.gas.real_gas_gamma(flow.gamma, flow.speed), flow.mach);
        let rho = if flow.speed > 0.0 { 2.0 * flow.q / (flow.speed * flow.speed) } else { 0.0 };
        Stream::new(flow.reynolds_per_m, flow.mach, flow.gamma, flow.temperature, cp)
            .with_gas(flow.gas)
            .with_speed(flow.speed, rho)
            .with_roughness(flow.roughness)
    }

    /// The edge state of a cell at incidence `s` = sin θ (positive
    /// windward) with exposure `f` (0..1).
    pub fn edge(&self, s: f64, f: f64) -> Edge {
        let s2 = (s * s).min(1.0);
        let tr = 1.0 + self.kt * s2;
        let impact = if s > 0.0 { self.kp * f * s2 } else { 0.0 };
        let rho = (1.0 + impact) / tr;
        let u = (1.0 - s2).sqrt();
        let temperature = self.temperature * tr;
        let mu = self.props.viscosity(temperature);
        Edge {
            temperature,
            rho,
            u,
            mach: self.mach * u / tr.sqrt(),
            re_per_m: self.re_per_m * rho * u * self.mu / mu,
            mu,
            heating: self.kt * u * u,
            props: self.props,
            roughness: self.roughness,
            real: (self.gas == Gas::Air && self.speed > 0.0 && self.mach > 0.0 && s > 0.0).then(|| {
                let r_gas = self.speed * self.speed / (self.gamma * self.mach * self.mach * self.temperature);
                let cp = self.gamma * r_gas / (self.gamma - 1.0);
                RealGas { rho: self.rho, speed: self.speed, cp, h_inf: cp * self.temperature, s: s.min(1.0), f }
            }),
        }
    }
}

/// A cell's boundary-layer edge state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    /// Static temperature (K).
    pub temperature: f64,
    /// ρ_e/ρ∞ and u_e/V.
    pub rho: f64,
    pub u: f64,
    pub mach: f64,
    /// ρ_e·u_e/μ_e (1/m).
    pub re_per_m: f64,
    /// μ_e (Pa·s).
    pub mu: f64,
    /// u_e²/(2·c_p·T∞) = (γ−1)/2·M²·(u_e/V)²: T_aw = T_e + r·T∞·this.
    heating: f64,
    /// The gas.
    props: &'static Properties,
    /// Air's free stream on a windward cell, for the real-gas turbulent
    /// heating ([`Self::tauber_turbulent`]).
    real: Option<RealGas>,
    /// Surface roughness height (m).
    roughness: f64,
}

/// What Tauber's real-gas turbulent relation needs of a windward cell.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RealGas {
    /// Free-stream density (kg/m³), airspeed (m/s), c_p and enthalpy.
    rho: f64,
    speed: f64,
    cp: f64,
    h_inf: f64,
    /// sin θ of the cell (the wedge angle's sine).
    s: f64,
    /// The cell's exposure (0..1): the share of the impact pressure it
    /// gets, as in the edge state.
    f: f64,
}

/// The layer on one cell (per unit free stream).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    /// Wall shear per unit q∞ (along the flow's tangential direction).
    pub shear: f64,
    /// Turbulent share (Dhawan–Narasimha intermittency).
    pub intermittency: f64,
    /// Laminar and turbulent heat flux per unit ρ∞·V·c_p (K): q̇ =
    /// ρ∞·V·c_p·this. Zero unless asked for.
    pub heat_laminar: f64,
    pub heat_turbulent: f64,
    /// Momentum-thickness Reynolds number at the cell's centre (laminar).
    pub re_theta: f64,
    /// Laminar heat flux of a plate along the flow at the same run lengths
    /// and wall (per unit ρ∞·V·c_p, K). Zero unless asked for.
    pub heat_aligned: f64,
}

impl Edge {
    /// Adiabatic wall temperature (K) for recovery factor `r`.
    pub fn recovery(&self, r: f64, t_inf: f64) -> f64 {
        self.temperature + r * t_inf * self.heating
    }

    /// Eckert's reference factor C* = ρ*μ*/(ρ_e·μ_e) for a wall at `tw` (K).
    pub fn reference(&self, tw: f64) -> f64 {
        let te = self.temperature;
        let t_star = te * (1.0 + 0.032 * self.mach * self.mach + 0.58 * (tw / te - 1.0)).max(0.05);
        te / t_star * self.props.viscosity(t_star) / self.mu
    }

    /// Van Driest II for a wall at `tw` (K): (1/F_c, F_Rx), c_f =
    /// c_f,inc(F_Rx·Re_x)/F_c.
    pub fn van_driest(&self, tw: f64, t_inf: f64) -> (f64, f64) {
        let te = self.temperature;
        let m = self.props.turbulent_recovery * t_inf * self.heating / te;
        let w = tw / te;
        let fc = if m < 1e-12 {
            let h = 0.5 * (w.sqrt() + 1.0);
            h * h
        } else {
            let a2 = m / w;
            let b = (1.0 + m) / w - 1.0;
            let den = (b * b + 4.0 * a2).sqrt();
            let s = asin_sum(((2.0 * a2 - b) / den).clamp(-1.0, 1.0), (b / den).clamp(-1.0, 1.0));
            m / (s * s)
        };
        (1.0 / fc, self.mu / self.props.viscosity(tw) / fc)
    }

    /// The turbulent share the surface's roughness forces on this edge with
    /// the wall at `tw` (K): Reshotko & Tumin's transient-growth
    /// correlation as revised by Paredes et al., Re_θ·(k/θ)·(T_e/T_w)^1.31 =
    /// 455 at the onset (Hein et al., J. Spacecraft and Rockets 56(2), 2019,
    /// eq. 15; Apollo- and Orion-like capsules). Re_θ·k/θ = ρ_e·u_e·k/μ_e,
    /// so the criterion is the edge's roughness Reynolds number, independent
    /// of the run: a cell whose edge passes it is tripped (fully turbulent;
    /// a roughness trip has no long transitional region), blended over
    /// ±5 % of the criterion for continuity. Zero on a smooth surface.
    pub fn tripped(&self, tw: f64) -> f64 {
        if self.roughness <= 0.0 || self.re_per_m <= 0.0 || tw <= 0.0 {
            return 0.0;
        }
        let ratio = self.temperature / tw;
        let value = self.re_per_m * self.roughness * math::exp(1.31 * math::ln(ratio));
        ((value / ROUGHNESS_TRIP - 0.95) / 0.1).clamp(0.0, 1.0)
    }

    /// Transition onset run length (m) for reference factor `c_star`, by the
    /// momentum-thickness criterion (in the layer's own, relieved, run).
    pub fn onset(&self, c_star: f64) -> f64 {
        let re_theta = RE_THETA_LOW.max(RE_THETA_PER_MACH * self.mach) / 0.664;
        re_theta * re_theta / (c_star * self.re_per_m).max(1e-300)
    }

    /// Transition onset run length (m) by the edge Reynolds number of
    /// sharp slender cones in flight against the edge Mach number
    /// ([`SLENDER_ONSET`]; in the body's own run).
    pub fn slender_onset(&self) -> f64 {
        let t = &SLENDER_ONSET;
        let m = self.mach;
        let re = if m <= t[0].0 {
            t[0].1
        } else if m >= t[t.len() - 1].0 {
            t[t.len() - 1].1
        } else {
            let k = t.partition_point(|e| e.0 <= m);
            let (a, b) = (t[k - 1], t[k]);
            // Log-linear in Re.
            a.1 * math::exp(math::ln(b.1 / a.1) * (m - a.0) / (b.0 - a.0))
        };
        re / self.re_per_m.max(1e-300)
    }

    /// The laminar mean skin-friction coefficient over run lengths `a..b`
    /// (centre `x`) with reference factor `c_star`: ∫0.664·√(k/x) dx/(b−a),
    /// k = C*/Re per metre.
    fn laminar_mean(&self, c_star: f64, a: f64, b: f64) -> f64 {
        let k = c_star / self.re_per_m;
        if b > a {
            1.328 * k.sqrt() * (b.sqrt() - a.sqrt()) / (b - a)
        } else {
            0.664 * (k / a.max(1e-300)).sqrt()
        }
    }

    /// Laminar heat flux per unit ρ∞·V·c_p (K) over run lengths `a..b`,
    /// wall at `tw` (K).
    pub fn laminar_heat(&self, a: f64, b: f64, tw: f64, t_inf: f64) -> f64 {
        if self.re_per_m <= 0.0 || b <= 0.0 {
            return 0.0;
        }
        let cf = self.laminar_mean(self.reference(tw), a, b);
        0.5 * self.props.colburn * self.rho * self.u * cf * (self.recovery(self.props.laminar_recovery, t_inf) - tw)
    }

    /// Turbulent skin-friction coefficient (on the edge's ½ρ_e·u_e²) at run
    /// length `x` (m, from the leading edge), wall at `tw` (K).
    pub fn turbulent_cf(&self, x: f64, tw: f64, t_inf: f64) -> f64 {
        let (inv_fc, frx) = self.van_driest(tw, t_inf);
        cf_turbulent(frx * self.re_per_m * x) * inv_fc
    }

    /// Turbulent heat flux per unit ρ∞·V·c_p (K) at run length `x` (m),
    /// wall at `tw` (K), whether or not the layer has turned turbulent there.
    pub fn turbulent_heat(&self, x: f64, tw: f64, t_inf: f64) -> f64 {
        if self.re_per_m <= 0.0 || x <= 0.0 {
            return 0.0;
        }
        let cf = self.turbulent_cf(x, tw, t_inf);
        let ours = 0.5
            * self.props.colburn
            * self.rho
            * self.u
            * cf
            * (self.recovery(self.props.turbulent_recovery, t_inf) - tw);
        ours.max(self.tauber_turbulent(x, tw))
    }

    /// Turbulent heating of air at entry speeds with real-gas properties
    /// (per unit ρ∞·V·c_p, K): Tauber's closed form of the reference-
    /// enthalpy method for an inclined plate above 3,962 m/s, fitted to
    /// equilibrium-air properties (Moeckel & Weston, Hansen) from 1 to
    /// 11 km/s and 3 to 60 km, ±15 % (NASA TP-2914, eq. 56b): q̇ =
    /// 2.45e-5·(ρ sin²δ cos^2.62 δ)^0.8·x^−0.2·V^3.7·(0.9 − h_w/H) W/m², x
    /// the turbulent run (m), with ρ sin²δ (the Newtonian impact) times the
    /// cell's exposure f as in the edge state. Weighted in from nothing at 3.5 km/s to full
    /// at 4.5 km/s; zero for other gases, leeward cells and without the free
    /// stream's speed. The layer takes the larger of it and the perfect-gas
    /// value, which runs 25–35 % under it at 7 km/s (the perfect gas's
    /// reference temperature is far too high once air dissociates).
    pub fn tauber_turbulent(&self, x: f64, tw: f64) -> f64 {
        let Some(r) = self.real else { return 0.0 };
        let w = ((r.speed - TAUBER_FROM) / (TAUBER_FULL - TAUBER_FROM)).clamp(0.0, 1.0);
        if w == 0.0 || x <= 0.0 {
            return 0.0;
        }
        let pow = |v: f64, e: f64| if v > 0.0 { math::exp(e * math::ln(v)) } else { 0.0 };
        let c2 = (1.0 - r.s * r.s).max(0.0);
        // cos^2.62 δ = (cos² δ)^1.31; ρ∞ sin² δ is the Newtonian impact
        // (the edge density's rise), so a shaded cell gets its exposure's
        // share, as in the edge state.
        let base = r.rho * r.f * r.s * r.s * pow(c2, 1.31);
        let big_h = r.h_inf + 0.5 * r.speed * r.speed;
        let q = 2.45e-5 * pow(base, 0.8) * pow(x, -0.2) * pow(r.speed, 3.7) * (0.9 - r.cp * tw / big_h);
        w * q / (r.rho * r.speed * r.cp)
    }

    /// The layer over run lengths `a..b` (m, 0 ≤ a ≤ b; centre x =
    /// (a+b)/2), wall at `wall` (K; `None`: adiabatic). With `heat`, the
    /// heat fluxes too.
    pub fn layer(&self, a: f64, b: f64, wall: Option<f64>, t_inf: f64, heat: bool) -> Layer {
        self.layer_with(a, b, (1.0, 1.0), wall, t_inf, heat)
    }

    /// [`Self::layer`] on a body whose layer is relieved in 3D: the laminar
    /// layer (and its transition) at run lengths × `relief.0`, the
    /// turbulent at × `relief.1` ([`super::relief`]).
    pub fn layer_with(&self, a: f64, b: f64, relief: (f64, f64), wall: Option<f64>, t_inf: f64, heat: bool) -> Layer {
        let (a, b, turbulent_x) = (a * relief.0, b * relief.0, 0.5 * (a + b) * relief.1);
        let x = 0.5 * (a + b);
        if self.re_per_m <= 0.0 || x <= 0.0 {
            return Layer::default();
        }
        let t_lam = wall.unwrap_or_else(|| self.recovery(self.props.laminar_recovery, t_inf));
        let c = self.reference(t_lam);
        let cf_lam = self.laminar_mean(c, a, b);
        // The first of the two criteria (the slender-cone one in the body's
        // run, so × the laminar relief in the layer's).
        let x_t = self.onset(c).min(self.slender_onset() * relief.0);
        let smooth = if x <= x_t {
            0.0
        } else {
            let xi = XI_END * (x - x_t) / x_t;
            // Beyond ξ 9.5, 1 − exp(−0.412 ξ²) rounds to 1.
            if xi > 9.5 {
                1.0
            } else {
                1.0 - math::exp(-DN * xi * xi)
            }
        };
        let gamma = smooth.max(self.tripped(t_lam));
        let (mut cf_turb, mut t_turb) = (0.0, 0.0);
        if gamma > 0.0 {
            t_turb = wall.unwrap_or_else(|| self.recovery(self.props.turbulent_recovery, t_inf));
            cf_turb = self.turbulent_cf(turbulent_x, t_turb, t_inf);
        }
        let mut layer = Layer {
            shear: ((1.0 - gamma) * cf_lam + gamma * cf_turb) * self.rho * self.u * self.u,
            intermittency: gamma,
            re_theta: 0.664 * (c * self.re_per_m * x).sqrt(),
            ..Layer::default()
        };
        if heat {
            let flux = 0.5 * self.props.colburn * self.rho * self.u;
            layer.heat_laminar = flux * cf_lam * (self.recovery(self.props.laminar_recovery, t_inf) - t_lam);
            if gamma > 0.0 {
                let ours = flux * cf_turb * (self.recovery(self.props.turbulent_recovery, t_inf) - t_turb);
                layer.heat_turbulent = ours.max(self.tauber_turbulent(turbulent_x, t_turb));
            }
        }
        layer
    }
}

/// asin a + asin b (a, b in −1..1) with one arcsine: asin(a√(1−b²) +
/// b√(1−a²)), its supplement where both are positive and a² + b² > 1 (the
/// sum beyond π/2).
fn asin_sum(a: f64, b: f64) -> f64 {
    let s = math::asin((a * (1.0 - b * b).max(0.0).sqrt() + b * (1.0 - a * a).max(0.0).sqrt()).clamp(-1.0, 1.0));
    if a > 0.0 && b > 0.0 && a * a + b * b > 1.0 {
        math::PI - s
    } else if a < 0.0 && b < 0.0 && a * a + b * b > 1.0 {
        -math::PI - s
    } else {
        s
    }
}

/// White's incompressible turbulent skin-friction coefficient
/// 0.455/ln²(0.06·Re_x) (eq. 6-78) at Re_x `re`.
pub fn cf_turbulent(re: f64) -> f64 {
    if re <= 0.0 {
        return 0.0;
    }
    let l = math::ln(0.06 * re);
    0.455 / (l * l)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: f64 = 216.65;

    #[test]
    fn colburn_factor_is_pr_to_the_minus_two_thirds() {
        let want = math::exp(-2.0 / 3.0 * math::ln(PRANDTL));
        assert!((COLBURN / want - 1.0).abs() < 1e-4, "{want}");
        // The recovery factors are the conventional rounded √Pr and ∛Pr.
        assert!((LAMINAR_RECOVERY / PRANDTL.sqrt() - 1.0).abs() < 1e-2);
        assert!((TURBULENT_RECOVERY / math::cbrt(PRANDTL) - 1.0).abs() < 1e-2);
    }

    #[test]
    fn the_edge_runs_from_the_free_stream_to_stagnation() {
        let s = Stream::new(1e6, 8.0, 1.4, T, 1.84);
        // Along the flow: the free stream.
        let e = s.edge(0.0, 1.0);
        assert_eq!((e.temperature, e.rho, e.u, e.mach, e.re_per_m), (T, 1.0, 1.0, 8.0, 1e6));
        // Facing it: at rest, at the total temperature T∞(1 + 0.2·M²) and
        // the Newtonian stagnation pressure.
        let e = s.edge(1.0, 1.0);
        assert!((e.temperature / (T * 13.8) - 1.0).abs() < 1e-12 && e.u == 0.0 && e.re_per_m == 0.0);
        assert!((e.rho * 13.8 - (1.0 + 0.7 * 64.0 * 1.84)).abs() < 1e-9);
        // At 30°: u_e = V cos 30°, the shadowed side has no impact pressure.
        let (lit, dark) = (s.edge(0.5, 1.0), s.edge(0.5, 0.0));
        assert!((lit.u - 0.75f64.sqrt()).abs() < 1e-15 && lit.temperature == dark.temperature);
        assert!(lit.rho > 10.0 * dark.rho);
    }

    #[test]
    fn the_wall_temperature_changes_the_reference_state() {
        // Eckert/White at M_e 0 (T* = T_e(1 + 0.58(T_w/T_e − 1))): a wall
        // at 2·T_e gives T* = 1.58·T_e and C* = (1/1.58)·μ(1.58·T_e)/μ(T_e).
        let e = Stream::new(1e6, 0.0, 1.4, T, 0.0).edge(0.0, 0.0);
        let want = air::sutherland_viscosity(1.58 * T) / air::sutherland_viscosity(T) / 1.58;
        assert!((e.reference(2.0 * T) / want - 1.0).abs() < 1e-12);
        assert!((e.reference(T) - 1.0).abs() < 1e-15);
        // Hot walls lower the friction, cold walls raise it (laminar and
        // turbulent), at M 5.
        let e = Stream::new(1e7, 5.0, 1.4, T, 0.0).edge(0.0, 0.0);
        let aw = e.recovery(TURBULENT_RECOVERY, T);
        let turb = |tw: f64| {
            let (inv_fc, frx) = e.van_driest(tw, T);
            cf_turbulent(frx * 1e7) * inv_fc
        };
        assert!(turb(0.3 * aw) > turb(aw) && turb(aw) > turb(1.5 * aw));
        assert!(e.reference(0.3 * aw) > e.reference(aw));
        // Van Driest II at low Mach number: its limit F_c = ((√(T_w/T_e) + 1)/2)².
        for w in [0.5, 1.0, 3.0] {
            let low = Stream::new(1e7, 1e-7, 1.4, T, 0.0).edge(0.0, 0.0);
            let zero = Stream::new(1e7, 0.0, 1.4, T, 0.0).edge(0.0, 0.0);
            let (a, b) = (low.van_driest(w * T, T).0, zero.van_driest(w * T, T).0);
            let h = 0.5 * (w.sqrt() + 1.0);
            assert!((b * h * h - 1.0).abs() < 1e-12 && (a / b - 1.0).abs() < 1e-6, "{w}: {a} {b}");
        }
    }

    #[test]
    fn roughness_trips_the_layer_at_the_reshotko_tumin_criterion() {
        // Re_θ·(k/θ)·(T_e/T_w)^1.31 = Re_k,e·(T_e/T_w)^1.31 = 455 (Hein et al.
        // 2019, eq. 15): at Re per metre 1e6 and T_w = T_e a 0.455 mm
        // roughness is at the criterion; twice that trips the layer
        // everywhere along the run, half of it nowhere.
        let e = Stream::new(1e6, 0.5, 1.4, T, 0.0).edge(0.0, 0.0);
        let at = |k: f64, x: f64| {
            let r = Stream::new(1e6, 0.5, 1.4, T, 0.0).with_roughness(k).edge(0.0, 0.0);
            r.layer(x, x, Some(T), T, false).intermittency
        };
        assert_eq!(e.tripped(T), 0.0);
        let k = ROUGHNESS_TRIP / 1e6;
        assert!((at(k, 0.01) - 0.5).abs() < 1e-9, "{}", at(k, 0.01));
        assert_eq!(at(2.0 * k, 0.01), 1.0);
        assert_eq!(at(0.5 * k, 0.01), 0.0);
        // A cold wall (T_w = T_e/2) trips at 2^−1.31 of the height.
        let r = Stream::new(1e6, 0.5, 1.4, T, 0.0).with_roughness(k * math::exp(-1.31 * math::ln(2.0))).edge(0.0, 0.0);
        assert!((r.tripped(0.5 * T) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn one_arcsine_for_two() {
        for a in [-1.0, -0.9, -0.5, 0.0, 0.3, 0.7, 0.95, 1.0] {
            for b in [-1.0, -0.8, -0.2, 0.0, 0.4, 0.8, 1.0] {
                let want = math::asin(a) + math::asin(b);
                assert!((asin_sum(a, b) - want).abs() < 1e-7, "{a} {b}: {} vs {want}", asin_sum(a, b));
            }
        }
    }

    #[test]
    fn transition_starts_at_the_criterion_and_ends_at_twice_the_run() {
        // Incompressible plate, adiabatic: onset at Re_x 5·10⁵.
        let e = Stream::new(1e6, 0.0, 1.4, T, 0.0).edge(0.0, 0.0);
        let onset = e.onset(1.0);
        assert!((onset * 1e6 / 5.0e5 - 1.0).abs() < 1e-3, "{onset}");
        let at = |x: f64| e.layer(x, x, None, T, false);
        assert_eq!(at(0.99 * onset).intermittency, 0.0);
        let mid = at(1.5 * onset).intermittency;
        assert!(mid > 0.5 && mid < 0.7, "{mid}");
        assert!((at(2.0 * onset).intermittency - 0.99).abs() < 1e-3);
        // The laminar Re_θ there is the criterion.
        assert!((at(onset).re_theta / RE_THETA_LOW - 1.0).abs() < 1e-9);
        // At edge Mach 3: Re_θ/M_e = 350.
        let e = Stream::new(1e7, 3.0, 1.4, T, 0.0).edge(0.0, 0.0);
        let c = e.reference(e.recovery(LAMINAR_RECOVERY, T));
        let theta = 0.664 * (c * e.re_per_m * e.onset(c)).sqrt();
        assert!((theta / (3.0 * RE_THETA_PER_MACH) - 1.0).abs() < 1e-9);
        // At edge Mach 10 a sharp cone's onset comes first: at Re_x 6.2·10⁶
        // (Softley), where Re_θ/M_e is ~120 (a plate's; a cone's is √3 less).
        let e = Stream::new(1e6, 10.0, 1.4, T, 0.0).edge(0.0, 0.0);
        let c = e.reference(1000.0);
        let x = e.slender_onset();
        assert!((x * 1e6 / 6.2e6 - 1.0).abs() < 1e-9 && x < e.onset(c));
        let at = |x: f64| e.layer(x, x, Some(1000.0), T, false).intermittency;
        assert!(at(0.99 * x) == 0.0 && at(1.5 * x) > 0.5);
        let theta = 0.664 * (c * e.re_per_m * x).sqrt();
        assert!(theta / 10.0 < 0.4 * RE_THETA_PER_MACH, "{}", theta / 10.0);
        // Log-linear between the figure's points.
        let e = Stream::new(1.0, 8.5, 1.4, T, 0.0).edge(0.0, 0.0);
        assert!((e.slender_onset() / (4.2e6 * (5.0f64 / 4.2).sqrt()) - 1.0).abs() < 1e-9);
    }
}
