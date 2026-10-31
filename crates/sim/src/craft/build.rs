//! The build (D101, D102; `docs/technical/ship-model.md` §4): catalog + craft
//! file → every part and component generated and placed by its mounts →
//! a [`CraftDef`] for the existing pipeline (union surface, cells by area,
//! mass properties, volume nodes, the aero bake and thermal network, every
//! cell and node tagged by part) with its [`ShipDef`] (tanks, engines,
//! layers, sequence) → validation errors, each naming what is wrong.
//!
//! A part's dry mass is its generator's (or its entry's published one) plus
//! its components'; its shell carries its own share (spread over its own
//! surface, as every part's), each component its envelope as a solid at
//! its place (`super::Craft::from_def`). The [`BuildReport`] lists every
//! published value against the derived one and the unmodelled masses with
//! their reasons (D101: reported, never silently patched).

use super::assembly::{copies, nth, AssemblyFile, Loading};
use super::file::{AttitudeControlFile, CraftFile, GeometryFile, PropellantFile, RcsFile, Tank};
use super::parts::{aggregate, CraftDef, Part, PartPower};
use super::place::{self, Frame, Pose};
use super::ship::{BottleDef, ComponentDef, EngineDef, Owned, PartInfo, ShipDef, TankDef};
use crate::catalog::{Built, Catalog, Ctx, Function, Kind};
use crate::math;
use glam::DVec3;
use std::sync::Arc;

/// Stack mounts whose diameters differ by more than this share do not fit.
pub const SIZE_TOLERANCE: f64 = 0.01;
/// Solar constant at 1 AU (W/m²) for the parts' power placeholders.
const SOLAR_CONSTANT: f64 = 1361.0;

/// What the build reports besides the craft.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildReport {
    /// (id, published − derived dry mass, reason) of every entry that
    /// publishes its dry mass.
    pub unmodelled: Vec<(String, f64, String)>,
    /// Published values beyond their entry's tolerance from the derived.
    pub contradictions: Vec<String>,
    pub dry_mass: f64,
    /// Liquid and gas loaded (kg).
    pub loaded: f64,
    /// Lowest and highest points along the craft's Z, widest radius (m).
    pub z_range: (f64, f64),
    pub radius: f64,
}

/// A built craft of parts.
#[derive(Clone, Debug)]
pub struct Assembly {
    pub def: CraftDef,
    pub report: BuildReport,
}

/// A placed part.
pub(super) struct Placed {
    pub id: String,
    pub entry: String,
    pub built: Built,
    pub pose: Pose,
    pub parent: Option<u32>,
    /// The mounts used to attach it: (parent's allowable, child's, size),
    /// their buckling line loads, the joint's normal from the parent
    /// towards it and its centre (craft axes).
    pub joint: (f64, f64, f64),
    pub buckling: (f64, f64),
    pub normal: DVec3,
    pub at: DVec3,
    pub primitives: Vec<u32>,
}

/// A placed component.
pub(super) struct PlacedComponent {
    pub id: String,
    pub entry: String,
    pub built: Built,
    pub pose: Pose,
    pub part: u32,
}

/// Builds `file` from `catalog`, or lists every error found.
pub fn build(file: &AssemblyFile, catalog: &Catalog, ctx: &Ctx) -> Result<Assembly, Vec<String>> {
    let mut errors = Vec::new();
    let mut primitives = Vec::new();
    let parts = place_parts(file, catalog, ctx, &mut primitives, &mut errors);
    let mut parts = parts;
    let components = place_components(file, catalog, ctx, &parts, &primitives, &mut errors);
    let checked = mass_checks(file, &mut parts, &components, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut ship = ShipDef { name: file.name.clone(), ..ShipDef::default() };
    describe(&parts, &components, &mut ship);
    tanks(file, ctx, &parts, &components, &mut ship, &mut errors);
    super::layers::connect(file, &parts, &components, &mut ship, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut report = report(&parts, &components, &ship, &primitives);
    report.contradictions.extend(checked);
    report.contradictions.extend(underfed(&ship));
    let def = assemble(file, &parts, &components, primitives, ship);
    Ok(Assembly { def, report })
}

/// Places every part (patterns expanded, parents before children).
fn place_parts(
    file: &AssemblyFile,
    catalog: &Catalog,
    ctx: &Ctx,
    primitives: &mut Vec<super::Primitive>,
    errors: &mut Vec<String>,
) -> Vec<Placed> {
    let mut out: Vec<Placed> = Vec::new();
    let mut used: Vec<(u32, String)> = Vec::new();
    for p in &file.parts {
        for (id, k) in copies(&p.id, p.pattern) {
            let err = |e: String| format!("part {id:?}: {e}");
            if out.iter().any(|q| q.id == id) {
                errors.push(err("listed twice".into()));
                continue;
            }
            let built = match catalog.get(&p.entry).and_then(|e| e.build(ctx, &p.set)) {
                Ok(b) if b.generated.kind == Kind::Part => b,
                Ok(_) => {
                    errors.push(err(format!("{:?} is a component, not a part", p.entry)));
                    continue;
                }
                Err(e) => {
                    errors.push(err(e));
                    continue;
                }
            };
            let first = out.is_empty();
            let index = |name: &str| out.iter().position(|q| q.id == name).map(|i| i as u32);
            let placement = if let Some(m) = &p.mount {
                let on = nth(&m.on, k);
                let Some(parent) = index(&on) else {
                    errors.push(err(format!("its parent {on:?} is not listed before it")));
                    continue;
                };
                let (at, via) = (nth(&m.at, k), nth(&m.via, k));
                let q = &out[parent as usize];
                let pm = q.built.generated.mounts.iter().find(|x| x.name == at);
                let cm = built.generated.mounts.iter().find(|x| x.name == via);
                let (Some(pm), Some(cm)) = (pm, cm) else {
                    errors.push(err(format!("no mount {at:?} on {on:?} or {via:?} on {:?}", p.entry)));
                    continue;
                };
                if pm.kind != cm.kind {
                    errors.push(err(format!("mount {via:?} ({:?}) cannot attach to {at:?} ({:?})", cm.kind, pm.kind)));
                    continue;
                }
                let stack = pm.kind == crate::catalog::MountKind::Stack;
                if stack && (pm.size - cm.size).abs() > SIZE_TOLERANCE * pm.size.max(cm.size) {
                    let (a, b) = (pm.size, cm.size);
                    errors.push(err(format!("stack mount {via:?} ({b} m) does not fit {at:?} ({a} m)")));
                    continue;
                }
                if used.contains(&(parent, at.clone())) {
                    errors.push(err(format!("mount {at:?} of {on:?} is already taken")));
                    continue;
                }
                used.push((parent, at.clone()));
                let parent_frame =
                    Frame { pos: q.pose.point(pm.pos), normal: q.pose.dir(pm.normal), roll: q.pose.dir(pm.roll) };
                let child = Frame { pos: cm.pos, normal: cm.normal, roll: cm.roll };
                let pose = place::attach(parent_frame, child, m.roll_deg);
                let joint = (pm.allowable, cm.allowable, pm.size.max(cm.size));
                (pose, Some(parent), joint, (pm.buckling, cm.buckling), parent_frame.normal, parent_frame.pos)
            } else if let Some(a) = &p.at {
                let turn =
                    Pose { rot: place::about_z(p.pattern.map_or(0.0, |pat| pat.angle_deg(k))), pos: DVec3::ZERO };
                let local = turn.then(&place::placed(a.pos, a.axis, a.roll_deg));
                let parent = match &a.on {
                    Some(on) => match index(&nth(on, k)) {
                        Some(i) => Some(i),
                        None => {
                            errors.push(err(format!("its parent {on:?} is not listed before it")));
                            continue;
                        }
                    },
                    None if first => None,
                    None => {
                        errors.push(err("placed by coordinates on nothing: name the part it is on (`on`)".into()));
                        continue;
                    }
                };
                let away = parent.map_or(DVec3::Z, |i| (local.pos - out[i as usize].pose.pos).normalize_or(DVec3::Z));
                (local, parent, (0.0, 0.0, 0.0), (0.0, 0.0), away, local.pos)
            } else if first {
                (Pose::IDENTITY, None, (0.0, 0.0, 0.0), (0.0, 0.0), DVec3::Z, DVec3::ZERO)
            } else {
                errors.push(err(
                    "is not attached: a part attaches through a mounting point (touching is not attaching)".into(),
                ));
                continue;
            };
            let (pose, parent, joint, buckling, normal, at) = placement;
            let mut own = Vec::new();
            for prim in &built.generated.primitives {
                match pose.shape(&prim.shape) {
                    Ok(shape) => {
                        own.push(primitives.len() as u32);
                        primitives.push(super::Primitive { shape, ..prim.clone() });
                    }
                    Err(e) => errors.push(err(e)),
                }
            }
            let entry = p.entry.clone();
            out.push(Placed { id, entry, built, pose, parent, joint, buckling, normal, at, primitives: own });
        }
    }
    out
}

/// Places every component in its host part, checking that its centre lies
/// inside the part and that the part's components fit its volume.
fn place_components(
    file: &AssemblyFile,
    catalog: &Catalog,
    ctx: &Ctx,
    parts: &[Placed],
    primitives: &[super::Primitive],
    errors: &mut Vec<String>,
) -> Vec<PlacedComponent> {
    let mut out: Vec<PlacedComponent> = Vec::new();
    for c in &file.components {
        for (id, k) in copies(&c.id, c.pattern) {
            let err = |e: String| format!("component {id:?}: {e}");
            let host = nth(&c.host, k);
            let Some(part) = parts.iter().position(|p| p.id == host) else {
                errors.push(err(format!("its part {host:?} is not listed")));
                continue;
            };
            if parts.iter().any(|p| p.id == id) || out.iter().any(|q| q.id == id) {
                errors.push(err("its id is taken".into()));
                continue;
            }
            let built = match catalog.get(&c.entry).and_then(|e| e.build(ctx, &c.set)) {
                Ok(b) if b.generated.kind == Kind::Component => b,
                Ok(_) => {
                    errors.push(err(format!("{:?} is a part, not a component", c.entry)));
                    continue;
                }
                Err(e) => {
                    errors.push(err(e));
                    continue;
                }
            };
            let turn = Pose { rot: place::about_z(c.pattern.map_or(0.0, |p| p.angle_deg(k))), pos: DVec3::ZERO };
            let local = turn.then(&place::placed(c.pos, c.axis, c.roll_deg));
            let pose = parts[part].pose.then(&local);
            let inside = (parts[part].primitives.iter())
                .any(|&q| super::mesh::contains(&primitives[q as usize].shape, pose.pos));
            if !inside {
                errors.push(err(format!("its centre {} is not inside {host:?}", pose.pos)));
                continue;
            }
            out.push(PlacedComponent { id, entry: c.entry.clone(), built, pose, part: part as u32 });
        }
    }
    for (i, p) in parts.iter().enumerate() {
        let used: f64 = out.iter().filter(|c| c.part == i as u32).map(|c| c.built.generated.volume()).sum();
        let room = p.built.generated.volume();
        if used > room {
            errors.push(format!("part {:?}: its components ({used:.3} m³) do not fit its volume ({room:.3} m³)", p.id));
        }
    }
    out
}

/// Each published group mass (a stage, a spacecraft) against the derived
/// total of the parts it names with their components: the gap becomes the
/// carrying part's unmodelled mass with the check's reason (reported with
/// the entry's own); a gap beyond the tolerance is also a contradiction.
fn mass_checks(
    file: &AssemblyFile,
    parts: &mut [Placed],
    components: &[PlacedComponent],
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut contradictions = Vec::new();
    for c in &file.checks {
        let named = |p: &Placed| c.parts.iter().any(|s| super::command::matches(s, &p.id));
        let hosted =
            |i: usize| components.iter().filter(|x| x.part == i as u32).map(|x| x.built.generated.mass).sum::<f64>();
        let total: f64 =
            (parts.iter().enumerate()).filter(|(_, p)| named(p)).map(|(i, p)| p.built.generated.mass + hosted(i)).sum();
        let Some(carry) = parts.iter().position(|p| p.id == c.carry && named(p)) else {
            errors.push(format!("check {:?}: it names no part {:?} to carry the gap", c.name, c.carry));
            continue;
        };
        let (published, source) = (c.dry_mass.0, &c.dry_mass.1);
        let gap = published - total;
        let built = &mut parts[carry].built;
        if built.generated.mass + gap < 0.0 || !published.is_finite() {
            errors.push(format!("check {:?}: {published} kg is less than its parts without {:?}", c.name, c.carry));
            continue;
        }
        built.generated.mass += gap;
        let reason = format!("{}: {} (published {published} kg: {source})", c.name, c.reason);
        built.unmodelled = Some(match built.unmodelled.take() {
            Some((g, r)) => (g + gap, format!("{r}; {reason}")),
            None => (gap, reason),
        });
        if gap.abs() > crate::catalog::TOLERANCE * published {
            contradictions.push(format!(
                "{}: derived {total:.0} kg, published {published} kg ({source}); {gap:+.0} kg carried by {}",
                c.name, c.carry
            ));
        }
    }
    contradictions
}

/// Parts, components and their functions into the ship definition.
fn describe(parts: &[Placed], components: &[PlacedComponent], ship: &mut ShipDef) {
    for (i, p) in parts.iter().enumerate() {
        let g = &p.built.generated;
        ship.parts.push(PartInfo {
            id: p.id.clone(),
            entry: p.entry.clone(),
            part: i as u32,
            pose: p.pose,
            mass: g.mass,
            derived: g.derived_mass(),
            unmodelled: p.built.unmodelled.clone(),
            material: g.material.clone(),
            finish: g.finish.clone(),
            ground: g.functions.iter().any(|f| matches!(f, Function::Pad)),
            ratings: p.built.ratings,
            failures: p.built.failures.clone(),
        });
        functions(&p.id, &p.id, i as u32, &p.pose, &g.functions, true, ship);
    }
    for c in components {
        let g = &c.built.generated;
        ship.components.push(ComponentDef {
            id: c.id.clone(),
            entry: c.entry.clone(),
            part: c.part,
            pose: c.pose,
            mass: g.mass,
            envelope: g.envelope.expect("a component has an envelope"),
            power: g.power,
            ratings: c.built.ratings,
            failures: c.built.failures.clone(),
        });
        functions(&c.id, &c.id, c.part, &c.pose, &g.functions, false, ship);
    }
}

/// One owner's functions, moved into the craft's axes.
fn functions(id: &str, owner: &str, part: u32, pose: &Pose, fs: &[Function], main: bool, ship: &mut ShipDef) {
    let engines = fs.iter().filter(|f| matches!(f, Function::Engine(_))).count();
    let mut n = 0;
    fn owned<T>(id: &str, part: u32, item: T) -> Owned<T> {
        Owned { id: id.to_string(), part, item }
    }
    for f in fs {
        match f {
            Function::Engine(d) => {
                n += 1;
                let data = crate::catalog::EngineData { pos: pose.point(d.pos), dir: pose.dir(d.dir), ..d.clone() };
                let eid = if engines > 1 { format!("{id}/{n}") } else { id.to_string() };
                ship.engines.push(EngineDef {
                    id: eid,
                    owner: owner.into(),
                    part,
                    data,
                    feeds: [vec![], vec![]],
                    main,
                });
            }
            Function::Thrusters(t) => {
                let nozzles =
                    t.nozzles.iter().map(|z| crate::catalog::Nozzle { pos: pose.point(z.pos), dir: pose.dir(z.dir) });
                ship.thrusters.push(super::ship::ThrusterDef {
                    id: id.into(),
                    part,
                    thrust: t.thrust,
                    isp: t.isp,
                    propellant: t.propellant.clone(),
                    nozzles: nozzles.collect(),
                    enabled: t.enabled,
                    feed: super::ship::Feed::Bottles(Vec::new()),
                });
            }
            Function::Mechanism(m) => {
                let m = crate::catalog::Mechanism {
                    pos: pose.point(m.pos),
                    axis: pose.dir(m.axis),
                    foot: m.foot.map(|f| pose.point(f)),
                    ..*m
                };
                ship.mechanisms.push(owned(id, part, m));
            }
            Function::AeroSurface(s) => ship.surfaces.push(owned(
                id,
                part,
                crate::catalog::AeroSurface { pos: pose.point(s.pos), axis: pose.dir(s.axis), ..*s },
            )),
            Function::Bottle(b) => ship.bottles.push(BottleDef {
                id: id.into(),
                part,
                gas: b.gas.clone(),
                volume: b.volume,
                pressure: b.pressure,
                burst: b.burst,
                temperature_k: b.temperature_k,
                pos: pose.pos,
            }),
            Function::Valve(v) => ship.valves.push(owned(id, part, *v)),
            Function::Battery(b) => ship.batteries.push(owned(id, part, *b)),
            Function::Sensor(s) => ship.sensors.push(owned(id, part, s.clone())),
            Function::Computer(c) => ship.computers.push(owned(id, part, *c)),
            Function::Seat(n) => ship.seats.push(owned(id, part, *n)),
            Function::Solar(s) => ship.solars.push(owned(id, part, *s)),
            Function::LifeSupport(l) => ship.life_support.push(owned(id, part, *l)),
            Function::Chute(_) | Function::Docking(..) | Function::Control | Function::Pad => {}
        }
    }
}

/// Tank compartments with what components inside them displace, and their
/// loading.
fn tanks(
    file: &AssemblyFile,
    ctx: &Ctx,
    parts: &[Placed],
    components: &[PlacedComponent],
    ship: &mut ShipDef,
    errors: &mut Vec<String>,
) {
    let owners = (parts.iter().enumerate().map(|(i, p)| (&p.id, i as u32, &p.pose, &p.built.generated, true)))
        .chain(components.iter().map(|c| (&c.id, c.part, &c.pose, &c.built.generated, false)));
    for (owner, part, pose, g, of_part) in owners {
        for t in &g.tanks {
            let id = format!("{owner}:{}", t.name);
            let base = pose.point(t.base);
            let axis = pose.dir(t.axis).normalize();
            let height = t.volume / (math::PI * t.radius * t.radius);
            // Components of the same part whose centre is in a part's
            // compartment (a component's own tank holds nothing else).
            let displaced: f64 = (components.iter())
                .filter(|c| of_part && c.part == part)
                .filter(|c| {
                    let along = (c.pose.pos - base).dot(axis);
                    let across = (c.pose.pos - base - axis * along).length();
                    along > 0.0 && along < height && across < t.radius
                })
                .map(|c| c.built.generated.volume())
                .sum();
            let volume = t.volume - displaced;
            let declared = file.loading.iter().find(|l| l.tank == id).cloned();
            let load = declared.unwrap_or(Loading {
                tank: id.clone(),
                fill: 0.0,
                temperature_k: 293.0,
                pressure: 101_325.0,
                pressurant: "nitrogen".into(),
            });
            let Ok(liquid) = ctx.fluids.liquid(&t.fluid) else { continue };
            let in_range = load.temperature_k > liquid.freezing_k && load.temperature_k < liquid.critical_k;
            if !(0.0..=1.0).contains(&load.fill)
                || !in_range
                || load.pressure <= 0.0
                || ctx.fluids.gas(&load.pressurant).is_err()
            {
                errors.push(format!(
                    "loading {id:?}: fill in [0, 1], {} between {} and {} K, positive pressure, a listed pressurant",
                    t.fluid, liquid.freezing_k, liquid.critical_k
                ));
                continue;
            }
            let liquid_volume = volume * (1.0 - t.ullage);
            let capacity = liquid_volume * liquid.density(load.temperature_k);
            let h_cap = liquid_volume / (math::PI * t.radius * t.radius);
            ship.tanks.push(TankDef {
                id,
                part,
                fluid: t.fluid.clone(),
                volume,
                region: Tank { base, axis: axis * h_cap, radius: t.radius },
                capacity,
                pressure: t.pressure,
                burst: t.burst,
                load,
            });
        }
    }
    for l in &file.loading {
        if !ship.tanks.iter().any(|t| t.id == l.tank) {
            errors.push(format!("loading {:?}: no such tank (tanks are `part:compartment`)", l.tank));
        }
    }
}

fn report(parts: &[Placed], components: &[PlacedComponent], ship: &ShipDef, prims: &[super::Primitive]) -> BuildReport {
    let mut r = BuildReport::default();
    let built = parts.iter().map(|p| (&p.id, &p.built)).chain(components.iter().map(|c| (&c.id, &c.built)));
    for (id, b) in built {
        if let Some((gap, reason)) = &b.unmodelled {
            r.unmodelled.push((id.clone(), *gap, reason.clone()));
        }
        for o in b.overrides.iter().filter(|o| o.contradicts) {
            let d = o.derived.unwrap_or(f64::NAN);
            r.contradictions.push(format!("{id}: {} published {} ({}) vs derived {d}", o.key, o.published, o.source));
        }
        r.dry_mass += b.generated.mass;
    }
    r.loaded = ship.loaded();
    let (mut lo, mut hi, mut radius) = (f64::MAX, f64::MIN, 0.0f64);
    for p in prims {
        let (a, b) = super::broad::bounds(&p.shape);
        lo = lo.min(a.z);
        hi = hi.max(b.z);
        radius = radius.max(a.x.abs()).max(a.y.abs()).max(b.x.abs()).max(b.y.abs());
    }
    r.z_range = (lo, hi);
    r.radius = radius;
    r
}

/// The craft's definition: parts in placement order, the craft file's
/// whole-craft data, the aggregates.
fn assemble(
    file: &AssemblyFile,
    parts: &[Placed],
    components: &[PlacedComponent],
    primitives: Vec<super::Primitive>,
    ship: ShipDef,
) -> CraftDef {
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let i = i as u32;
        let g = &p.built.generated;
        let hosted: Vec<&PlacedComponent> = components.iter().filter(|c| c.part == i).collect();
        let all = || g.functions.iter().chain(hosted.iter().flat_map(|c| c.built.generated.functions.iter()));
        let engine = ship.engines.iter().find(|e| e.part == i && e.main).map(|e| super::engine::file_of(&e.data));
        let produced: f64 =
            all().filter_map(|f| if let Function::Solar(s) = f { Some(s.area * s.efficiency) } else { None }).sum();
        let battery: f64 =
            all().filter_map(|f| if let Function::Battery(b) = f { Some(b.capacity) } else { None }).sum();
        out.push(Part {
            id: i,
            name: p.id.clone(),
            primitives: p.primitives.clone(),
            parent: p.parent,
            dry_mass: g.mass + hosted.iter().map(|c| c.built.generated.mass).sum::<f64>(),
            tank: None,
            engine,
            rcs: None,
            chute: all().any(|f| matches!(f, Function::Chute(_))),
            interior_specific_heat: Some(g.specific_heat),
            node_max_k: Some(g.max_k),
            power: PartPower {
                produced: produced * SOLAR_CONSTANT,
                consumed: g.power + hosted.iter().map(|c| c.built.generated.power).sum::<f64>(),
                battery,
            },
            control: all().any(|f| matches!(f, Function::Control)),
            crew: all().filter_map(|f| if let Function::Seat(n) = f { Some(*n) } else { None }).sum(),
            max_load: f64::INFINITY,
        });
    }
    let chute = components.iter().find_map(|c| {
        c.built.generated.functions.iter().find_map(|f| match f {
            Function::Chute(s) => Some(crate::chute::ChuteSpec { mount: c.pose.point(s.mount), ..s.clone() }),
            _ => None,
        })
    });
    let first_skin = primitives.first().and_then(|p| p.skin);
    let geometry = GeometryFile {
        skin: first_skin.unwrap_or(super::file::DEFAULT_SKIN),
        tank: ship.tanks.first().map_or(Tank { base: DVec3::ZERO, axis: DVec3::Z, radius: 1.0 }, |t| t.region),
        primitives,
    };
    let mut spec = CraftFile {
        name: file.name.clone(),
        crew: file.crew,
        dry_mass: 0.0,
        propellant: PropellantFile { mass: 0.0, capacity: 0.0 },
        engine: super::parts::combine_engines(&[]),
        attitude_control: AttitudeControlFile {
            rcs: RcsFile { thrust: 0.0, isp: 1.0, fore_z: 1.0, aft_z: 0.0, radius: 0.0 },
        },
        chute: chute.unwrap_or_else(crate::chute::ChuteSpec::none),
        aero: file.aero,
        thermal: file.thermal,
        impact: file.impact,
        contact: file.contact,
        antenna: file.antenna,
    };
    aggregate(&mut spec, &out);
    let ship = Arc::new(ship);
    ship_aggregate(&mut spec, &ship, file.crew);
    CraftDef { spec, geometry, parts: out, ship: Some(ship) }
}

/// The craft file's propellant from a ship's tanks and bottles; its crew
/// as aboard.
pub fn ship_aggregate(spec: &mut CraftFile, ship: &ShipDef, crew: u32) {
    let capacity: f64 = ship.regions().iter().map(|r| r.1).sum();
    spec.propellant = PropellantFile { mass: ship.loaded().min(capacity), capacity };
    spec.crew = crew;
}

/// A pressure-fed engine's injector pressure drop as a share of its
/// chamber pressure (E: 15–25 % keeps the combustion from coupling with the
/// feed system, chugging; Sutton & Biblarz, *Rocket Propulsion Elements*,
/// on injectors; 20 % picked).
pub const INJECTOR_DROP: f64 = 0.2;

/// Whether a pressure-fed engine whose chamber runs at `chamber` (Pa) is
/// fed by tanks at `feed` (Pa): only a feed above the chamber plus the
/// injector's drop pushes the propellant in. A chamber pressure not given
/// (0) is not checked.
pub fn fed(feed: f64, chamber: f64) -> bool {
    chamber <= 0.0 || feed > chamber * (1.0 + INJECTOR_DROP)
}

/// The craft check of every pressure-fed engine ([`fed`]) against the
/// lowest loading pressure of the tanks feeding it: one line for each
/// engine fed below its chamber.
pub fn underfed(ship: &ShipDef) -> Vec<String> {
    let mut out = Vec::new();
    for e in ship.engines.iter().filter(|e| e.data.cycle == crate::catalog::Cycle::PressureFed) {
        let tanks = e.feeds.iter().flatten().filter_map(|&t| ship.tanks.get(t as usize));
        let Some(t) = tanks.min_by(|a, b| a.load.pressure.total_cmp(&b.load.pressure)) else { continue };
        let (feed, chamber) = (t.load.pressure, e.data.chamber_pressure);
        if !fed(feed, chamber) {
            out.push(format!(
                "{}: fed at {:.2} MPa from {}, below its chamber's {:.2} MPa plus the injector's {:.0} %",
                e.id,
                feed / 1e6,
                t.id,
                chamber / 1e6,
                100.0 * INJECTOR_DROP
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pressure-fed engine is fed only above its chamber pressure plus
    /// the injector's drop; a chamber not given passes.
    #[test]
    fn a_pressure_fed_engine_needs_its_feed_above_its_chamber() {
        // (feed MPa, chamber MPa, fed)
        let cases = [
            (16.5, 6.9, true),  // Dragon at 2,400 psi, the SuperDraco at 1,000 psi
            (1.7, 6.9, false),  // the tanks as first loaded
            (6.9, 6.9, false),  // no drop left for the injector
            (8.28, 6.9, false), // exactly the drop
            (8.3, 6.9, true),
            (0.1, 0.0, true), // no chamber pressure given
        ];
        for (feed, chamber, want) in cases {
            assert_eq!(fed(feed * 1e6, chamber * 1e6), want, "{feed} MPa into {chamber} MPa");
        }
    }
}
