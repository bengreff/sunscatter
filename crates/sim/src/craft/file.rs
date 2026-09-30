//! Craft files: `data/craft/<craft>/craft.ron` and `geometry.ron`
//! (realism-1 §3a). The structs mirror the files; [`validate_craft`] and
//! [`validate_geometry`] reject values the simulation cannot use.

use glam::DVec3;
use serde::{Deserialize, Serialize};

/// `craft.ron`: the craft's numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftFile {
    pub name: String,
    pub crew: u32,
    /// Everything but the propellant (kg).
    pub dry_mass: f64,
    pub propellant: PropellantFile,
    pub engine: EngineFile,
    pub attitude_control: AttitudeControlFile,
    pub chute: ChuteFile,
    pub aero: AeroFile,
    pub thermal: ThermalLimits,
    pub impact: ImpactLimits,
    pub contact: ContactFile,
    pub antenna: AntennaFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropellantFile {
    /// Loaded at the start (kg).
    pub mass: f64,
    /// Tank capacity (kg).
    pub capacity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    /// Vacuum thrust at full throttle (N).
    pub thrust_vac: f64,
    /// Specific impulse in vacuum and at sea level (s).
    pub isp_vac: f64,
    pub isp_sl: f64,
    /// Lowest throttle while running, in (0, 1].
    pub min_throttle: f64,
    /// Gimbal range about the mount (deg).
    pub gimbal_deg: f64,
    pub mount: Mount,
    /// Share of the jet power (½·F·vₑ) absorbed by the nozzle's radiatively
    /// cooled wall (the geometry's `nozzle` primitive) while it runs (D077);
    /// the chamber's wall heat goes back into the flow (regenerative
    /// cooling) and is not a heat source.
    pub nozzle_heat_fraction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    /// Where the thrust acts (body axes, m).
    pub pos: DVec3,
    /// Thrust direction (body axes; normalised on load).
    pub dir: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttitudeControlFile {
    /// Reaction-control thrusters: the only attitude actuator besides the
    /// engine's gimbal (no reaction wheels: wheels big enough to turn a
    /// 20 t craft do not fly on such craft).
    pub rcs: RcsFile,
}

/// Reaction-control thrusters in two rings (fore and aft) of four quads
/// around the body axis, at ±X and ±Y (the Shuttle's forward and aft RCS
/// layout). Attitude uses pure couples only (no net force): pitch and yaw
/// fire two lateral thrusters in one ring against two in the other, roll
/// fires the tangential thruster of every quad.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RcsFile {
    /// Thrust of one thruster (N).
    pub thrust: f64,
    /// Specific impulse (s); the thrusters draw on the main propellant.
    pub isp: f64,
    /// Body-axis stations of the fore and aft rings (m).
    pub fore_z: f64,
    pub aft_z: f64,
    /// Distance of the quads from the body axis (m).
    pub radius: f64,
}

impl RcsFile {
    /// Couple per body axis at full command (N·m): pitch and yaw
    /// `2·F·(fore − aft)`, roll `8·F·r`.
    pub fn torque(&self) -> DVec3 {
        let pitch = 2.0 * self.thrust * (self.fore_z - self.aft_z);
        DVec3::new(pitch, pitch, 8.0 * self.thrust * self.radius)
    }

    /// Propellant flow per body axis at full command (kg/s): four
    /// thrusters for pitch or yaw, eight for roll.
    pub fn flow(&self) -> DVec3 {
        let one = self.thrust / (self.isp * crate::vessel::G0);
        DVec3::new(4.0 * one, 4.0 * one, 8.0 * one)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChuteFile {
    /// Drag coefficient × area when deployed (m²).
    pub cd_area: f64,
    /// Highest dynamic pressure at which it can deploy (Pa).
    pub deploy_max_q: f64,
    /// Attachment point (body axes, m).
    pub mount: DVec3,
}

/// Aerodynamic data the cells cannot give (D061, `sim::aero`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AeroFile {
    /// Subsonic drag coefficient on the projected area (blunt bodies ≈ 0.8).
    pub cd0: f64,
}

/// Temperature limits (D065): a skin cell or an interior node above its
/// limit destroys the craft; and the interior volume nodes of the thermal
/// network (`sim::craft::volume`, `sim::thermal`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalLimits {
    pub skin_max_k: f64,
    /// The limit of every interior node for now.
    pub internal_max_k: f64,
    /// Edge of the interior grid's cubes (m).
    pub node_size: f64,
    /// Specific heat of the dry interior (structure and equipment: the dry
    /// mass less the skin, spread by volume) and of the propellant (J/(kg·K)).
    pub interior_specific_heat: f64,
    pub propellant_specific_heat: f64,
    /// Effective conductivity of the interior between nodes (W/(m·K)).
    pub interior_conductivity: f64,
    /// Conductance from each skin cell to the node beneath it per unit of
    /// the cell's area (W/(m²·K)).
    pub internal_coupling: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImpactLimits {
    /// Highest normal speed at a contact point beyond the gear's stroke (m/s).
    pub max_speed: f64,
}

/// Ground contact (D066, realism-1 §3e): a spring-damper along the terrain
/// normal and Coulomb friction at each contact point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactFile {
    /// Landing-gear feet: soft, with a stroke.
    pub foot: Spring,
    /// Hull points (and a foot beyond its stroke): stiff, no stroke.
    pub hull: Spring,
    /// Sliding speed below which friction is viscous rather than Coulomb
    /// (regularised friction, m/s): a craft on a slope it holds creeps at
    /// about `stick_speed · tan(slope) / friction`.
    pub stick_speed: f64,
}

/// One kind of contact point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spring {
    /// N/m.
    pub stiffness: f64,
    /// N·s/m.
    pub damping: f64,
    /// Travel before the point bottoms out (m); zero for hull points.
    #[serde(default)]
    pub stroke: f64,
    /// Coulomb friction coefficient.
    pub friction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AntennaFile {
    pub gain_dbi: f64,
    pub power_w: f64,
}

/// `geometry.ron`: the craft's shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryFile {
    /// Skin of primitives that do not name their own.
    pub skin: Skin,
    /// Where the propellant sits.
    pub tank: Tank,
    pub primitives: Vec<Primitive>,
}

/// Skin material of a surface (D065: per-cell thermal data).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skin {
    /// kg/m².
    pub areal_mass: f64,
    /// J/(kg·K).
    pub specific_heat: f64,
    pub emissivity: f64,
    /// Thermal conductivity (W/(m·K)).
    pub conductivity: f64,
    /// m.
    pub thickness: f64,
    /// The material's temperature limit (K); the craft's `skin_max_k` if
    /// not given.
    #[serde(default)]
    pub max_k: Option<f64>,
    /// Conductance to the interior node beneath per unit area
    /// (W/(m²·K)); the craft's `internal_coupling` if not given (a nozzle
    /// joined only at its mount has its own).
    #[serde(default)]
    pub coupling: Option<f64>,
}

/// The propellant region: a cylinder from `base` along `axis` (its length is
/// the tank height).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tank {
    pub base: DVec3,
    pub axis: DVec3,
    pub radius: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Primitive {
    pub name: String,
    pub shape: Shape,
    /// A landing-gear foot: its bottom is a contact point.
    #[serde(default)]
    pub foot: bool,
    /// Overrides the default skin.
    #[serde(default)]
    pub skin: Option<Skin>,
    /// The engine's nozzle: its cells take the engine's wall heat (D077).
    #[serde(default)]
    pub nozzle: bool,
}

/// A closed primitive solid in body axes (m).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    /// A cone frustum from `base` along `axis` (length = height).
    Frustum {
        base: DVec3,
        axis: DVec3,
        r_base: f64,
        r_top: f64,
    },
    Cylinder {
        base: DVec3,
        axis: DVec3,
        radius: f64,
    },
    /// The part of the sphere (`center`, `radius`) at least `radius − height`
    /// along `axis` from the centre, closed by its base disc.
    SphereCap {
        center: DVec3,
        axis: DVec3,
        radius: f64,
        height: f64,
    },
    /// An axis-aligned box.
    Box {
        center: DVec3,
        half: DVec3,
    },
    /// A round strut between two points (a cylinder).
    Strut {
        from: DVec3,
        to: DVec3,
        radius: f64,
    },
}

fn positive(what: &str, x: f64) -> Result<(), String> {
    if x.is_finite() && x > 0.0 {
        Ok(())
    } else {
        Err(format!("{what} must be positive and finite, got {x}"))
    }
}

fn finite_vec(what: &str, v: DVec3) -> Result<(), String> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(format!("{what} must be finite, got {v}"))
    }
}

fn nonzero_vec(what: &str, v: DVec3) -> Result<(), String> {
    finite_vec(what, v)?;
    if v.length() > 1e-9 {
        Ok(())
    } else {
        Err(format!("{what} must not be zero"))
    }
}

/// Checks `craft.ron`'s values.
pub fn validate_craft(c: &CraftFile) -> Result<(), String> {
    positive("dry_mass", c.dry_mass)?;
    positive("propellant.capacity", c.propellant.capacity)?;
    let p = c.propellant.mass;
    if !(p.is_finite() && (0.0..=c.propellant.capacity).contains(&p)) {
        return Err(format!("propellant.mass must be in [0, capacity], got {p}"));
    }
    let e = &c.engine;
    positive("engine.thrust_vac", e.thrust_vac)?;
    positive("engine.isp_vac", e.isp_vac)?;
    positive("engine.isp_sl", e.isp_sl)?;
    if e.isp_sl > e.isp_vac {
        return Err(format!("engine.isp_sl ({}) must not exceed isp_vac ({})", e.isp_sl, e.isp_vac));
    }
    if !(e.min_throttle > 0.0 && e.min_throttle <= 1.0) {
        return Err(format!("engine.min_throttle must be in (0, 1], got {}", e.min_throttle));
    }
    if !(e.gimbal_deg.is_finite() && (0.0..=30.0).contains(&e.gimbal_deg)) {
        return Err(format!("engine.gimbal_deg must be in [0, 30], got {}", e.gimbal_deg));
    }
    finite_vec("engine.mount.pos", e.mount.pos)?;
    nonzero_vec("engine.mount.dir", e.mount.dir)?;
    let r = &c.attitude_control.rcs;
    if !(r.thrust.is_finite() && r.thrust >= 0.0 && r.isp.is_finite() && r.isp > 0.0) {
        return Err(format!("attitude_control.rcs needs thrust >= 0 and isp > 0, got {} N, {} s", r.thrust, r.isp));
    }
    if !(r.fore_z.is_finite() && r.aft_z.is_finite() && r.fore_z > r.aft_z && r.radius.is_finite() && r.radius >= 0.0) {
        return Err(format!(
            "attitude_control.rcs needs fore_z > aft_z and radius >= 0, got {}, {}, {}",
            r.fore_z, r.aft_z, r.radius
        ));
    }
    if !(c.chute.cd_area.is_finite() && c.chute.cd_area >= 0.0) {
        return Err(format!("chute.cd_area must be non-negative, got {}", c.chute.cd_area));
    }
    positive("chute.deploy_max_q", c.chute.deploy_max_q)?;
    finite_vec("chute.mount", c.chute.mount)?;
    positive("thermal.skin_max_k", c.thermal.skin_max_k)?;
    positive("thermal.internal_max_k", c.thermal.internal_max_k)?;
    let t = &c.thermal;
    positive("thermal.node_size", t.node_size)?;
    positive("thermal.interior_specific_heat", t.interior_specific_heat)?;
    positive("thermal.propellant_specific_heat", t.propellant_specific_heat)?;
    for (what, x) in [("interior_conductivity", t.interior_conductivity), ("internal_coupling", t.internal_coupling)] {
        if !(x.is_finite() && x >= 0.0) {
            return Err(format!("thermal.{what} must be non-negative, got {x}"));
        }
    }
    if !(e.nozzle_heat_fraction.is_finite() && (0.0..1.0).contains(&e.nozzle_heat_fraction)) {
        return Err(format!("engine.nozzle_heat_fraction must be in [0, 1), got {}", e.nozzle_heat_fraction));
    }
    positive("aero.cd0", c.aero.cd0)?;
    positive("impact.max_speed", c.impact.max_speed)?;
    for (what, sp) in [("contact.foot", &c.contact.foot), ("contact.hull", &c.contact.hull)] {
        positive(&format!("{what}.stiffness"), sp.stiffness)?;
        if !(sp.damping.is_finite() && sp.damping >= 0.0) {
            return Err(format!("{what}.damping must be non-negative, got {}", sp.damping));
        }
        if !(sp.friction.is_finite() && sp.friction >= 0.0) {
            return Err(format!("{what}.friction must be non-negative, got {}", sp.friction));
        }
    }
    positive("contact.foot.stroke", c.contact.foot.stroke)?;
    if c.contact.hull.stroke != 0.0 {
        return Err(format!("contact.hull.stroke must be zero, got {}", c.contact.hull.stroke));
    }
    positive("contact.stick_speed", c.contact.stick_speed)?;
    if !c.antenna.gain_dbi.is_finite() {
        return Err("antenna.gain_dbi must be finite".into());
    }
    if !(c.antenna.power_w.is_finite() && c.antenna.power_w >= 0.0) {
        return Err(format!("antenna.power_w must be non-negative, got {}", c.antenna.power_w));
    }
    Ok(())
}

fn validate_skin(what: &str, s: &Skin) -> Result<(), String> {
    positive(&format!("{what}.areal_mass"), s.areal_mass)?;
    positive(&format!("{what}.specific_heat"), s.specific_heat)?;
    if !(s.emissivity > 0.0 && s.emissivity <= 1.0) {
        return Err(format!("{what}.emissivity must be in (0, 1], got {}", s.emissivity));
    }
    if !(s.conductivity.is_finite() && s.conductivity >= 0.0) {
        return Err(format!("{what}.conductivity must be non-negative, got {}", s.conductivity));
    }
    positive(&format!("{what}.thickness"), s.thickness)?;
    if let Some(t) = s.max_k {
        positive(&format!("{what}.max_k"), t)?;
    }
    match s.coupling {
        Some(g) if !(g.is_finite() && g >= 0.0) => Err(format!("{what}.coupling must be non-negative, got {g}")),
        _ => Ok(()),
    }
}

fn validate_shape(what: &str, s: &Shape) -> Result<(), String> {
    match *s {
        Shape::Frustum { base, axis, r_base, r_top } => {
            finite_vec(&format!("{what}.base"), base)?;
            nonzero_vec(&format!("{what}.axis"), axis)?;
            if !(r_base.is_finite() && r_top.is_finite() && r_base >= 0.0 && r_top >= 0.0 && r_base + r_top > 0.0) {
                return Err(format!("{what}: radii must be non-negative and not both zero"));
            }
        }
        Shape::Cylinder { base, axis, radius } => {
            finite_vec(&format!("{what}.base"), base)?;
            nonzero_vec(&format!("{what}.axis"), axis)?;
            positive(&format!("{what}.radius"), radius)?;
        }
        Shape::SphereCap { center, axis, radius, height } => {
            finite_vec(&format!("{what}.center"), center)?;
            nonzero_vec(&format!("{what}.axis"), axis)?;
            positive(&format!("{what}.radius"), radius)?;
            if !(height > 0.0 && height <= 2.0 * radius) {
                return Err(format!("{what}.height must be in (0, 2·radius], got {height}"));
            }
        }
        Shape::Box { center, half } => {
            finite_vec(&format!("{what}.center"), center)?;
            if !(half.is_finite() && half.min_element() > 0.0) {
                return Err(format!("{what}.half must be positive, got {half}"));
            }
        }
        Shape::Strut { from, to, radius } => {
            finite_vec(&format!("{what}.from"), from)?;
            nonzero_vec(&format!("{what}: to − from"), to - from)?;
            positive(&format!("{what}.radius"), radius)?;
        }
    }
    Ok(())
}

/// Checks `geometry.ron`'s values.
pub fn validate_geometry(g: &GeometryFile) -> Result<(), String> {
    validate_skin("skin", &g.skin)?;
    finite_vec("tank.base", g.tank.base)?;
    nonzero_vec("tank.axis", g.tank.axis)?;
    positive("tank.radius", g.tank.radius)?;
    if g.primitives.is_empty() {
        return Err("primitives must not be empty".into());
    }
    for p in &g.primitives {
        let what = format!("primitive {:?}", p.name);
        validate_shape(&what, &p.shape)?;
        if let Some(s) = &p.skin {
            validate_skin(&format!("{what}.skin"), s)?;
        }
    }
    if g.primitives.iter().filter(|p| p.nozzle).count() > 1 {
        return Err("at most one primitive may be the nozzle".into());
    }
    Ok(())
}
