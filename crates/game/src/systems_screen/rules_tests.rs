use super::*;
use crate::test_rocket::rocket;
use sim::craft::command::ShipCommand;

fn rocket_facts(state: &ShipState) -> Vec<PartFacts> {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let colours = vec![[0.5; 3]; craft.parts.len()];
    facts(&craft.parts, &colours, def, state, None, &sim::systems::Knowledge::perfect())
}

fn named<'a>(f: &'a [PartFacts], name: &str) -> &'a PartFacts {
    f.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("{name}"))
}

#[test]
fn facts_gather_each_parts_joint_tanks_and_systems() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let mut state = ShipState::initial(def);
    let f = rocket_facts(&state);
    assert_eq!(f.len(), craft.parts.len());
    // The tank hangs from the capsule by the separation joint, and holds
    // both propellants at their loading.
    let tank = named(&f, "tank");
    let joint = tank.joint.as_ref().unwrap();
    assert_eq!((joint.name.as_str(), joint.class, joint.intact), ("separation", JointClass::Separation, true));
    let fluids: Vec<&str> = tank.tanks.iter().map(|t| t.0.as_str()).collect();
    assert_eq!(fluids, ["RP-1", "LOX"]);
    assert!((tank.tanks[0].1.unwrap() - 0.9 * tank.tanks[0].2).abs() < 1e-9, "fuel loaded to 90 %");
    assert!(tank.plumbing, "the bottle inside it");
    // The capsule: the root, the computer and its 50 W, a seat; linked.
    let capsule = named(&f, "capsule");
    assert!(capsule.joint.is_none());
    assert_eq!((capsule.computers, capsule.load, capsule.bus, capsule.linked), (1, 50.0, true, true));
    assert!(named(&f, "engine#1").plumbing && named(&f, "engine#1").linked);
    assert_eq!(named(&f, "leg#2").joint.as_ref().unwrap().class, JointClass::Bolted);
    assert!(f.iter().all(|p| p.integrity == 1.0 && p.damage == 0.0 && p.skin.is_none()));
    // Separated, switched off, damaged.
    state.apply(def, &ShipCommand::Separate("separation".into())).unwrap();
    state.apply(def, &ShipCommand::Power("computer".into(), false)).unwrap();
    state.parts[1].thermal_damage = 0.4;
    let f = rocket_facts(&state);
    assert!(!named(&f, "tank").joint.as_ref().unwrap().intact);
    assert_eq!(named(&f, "capsule").load, 0.0);
    assert_eq!(named(&f, "tank").damage, 0.4);
}

#[test]
fn temperatures_go_to_their_parts_nearest_their_limits() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    // Three cells of part 0 (one near its limit), one node of part 1.
    let temps = Temps {
        skin: &[300.0, 900.0, 1000.0],
        nodes: &[350.0],
        cell_part: &[0, 0, 0],
        node_part: &[1],
        skin_max: &[1100.0, 950.0, 2000.0],
        node_max: &[400.0],
    };
    let colours = vec![[0.5; 3]; craft.parts.len()];
    let f = facts(&craft.parts, &colours, def, &state, Some(&temps), &sim::systems::Knowledge::perfect());
    assert_eq!(f[0].skin, Some((900.0, 950.0)), "the cell nearest its limit, not the hottest");
    // Without temperature or level sensors the craft knows neither (D106).
    let blind = facts(&craft.parts, &colours, def, &state, Some(&temps), &sim::systems::Knowledge::default());
    assert!(blind.iter().all(|p| p.skin.is_none() && p.interior.is_none()));
    assert!(blind.iter().flat_map(|p| &p.tanks).all(|t| t.1.is_none()));
    let tank = blind.iter().find(|p| !p.tanks.is_empty()).unwrap();
    let shade = colour(Layer::Plumbing, tank, false);
    assert_eq!(
        shade,
        fluid_colour(&tank.tanks.iter().max_by(|a, b| a.2.total_cmp(&b.2)).unwrap().0).map(|c| c * UNKNOWN_FILL)
    );
    assert_eq!((f[0].interior, f[1].interior, f[1].skin), (None, Some((350.0, 400.0)), None));
    // Its thermal colour is near the red end.
    let c = colour(Layer::Thermal, &f[0], false);
    assert!(c[0] > 0.8 && c[2] < 0.2, "{c:?}");
    assert_eq!(colour(Layer::Thermal, &f[2], false), GHOST, "no temperature known");
}

#[test]
fn layer_colours_table() {
    let base = PartFacts { material: [0.8, 0.8, 0.8], integrity: 1.0, ..Default::default() };
    let joint =
        |class, intact| Some(JointFact { name: "j".into(), class, intact, allowable: 1.0, radius: 1.0, ratio: None });
    let loaded = |ratio| Some(JointFact { ratio: Some(ratio), ..joint(JointClass::Bolted, true).unwrap() });
    let lox_half = PartFacts { tanks: vec![("LOX".into(), Some(50.0), 100.0)], ..base.clone() };
    let lox_full = PartFacts { tanks: vec![("LOX".into(), Some(100.0), 100.0)], ..base.clone() };
    // (layer, facts, colour)
    let table = [
        (Layer::Physical, base.clone(), [0.8, 0.8, 0.8]),
        (Layer::Structural, base.clone(), [0.85, 0.85, 0.88]),
        (Layer::Structural, PartFacts { joint: joint(JointClass::Separation, true), ..base.clone() }, [1.0, 0.6, 0.1]),
        (Layer::Structural, PartFacts { joint: joint(JointClass::Separation, false), ..base.clone() }, GHOST),
        (Layer::Structural, PartFacts { ground: true, ..base.clone() }, crate::scene::parts::GROUND),
        (Layer::Structural, PartFacts { joint: loaded(0.0), ..base.clone() }, [0.12, 0.75, 0.25]),
        (Layer::Structural, PartFacts { joint: loaded(1.2), ..base.clone() }, [0.9, 0.08, 0.04]),
        (Layer::Plumbing, base.clone(), GHOST),
        (Layer::Plumbing, PartFacts { plumbing: true, ..base.clone() }, [0.55, 0.57, 0.6]),
        (Layer::Plumbing, lox_full, [0.45, 0.7, 1.0]),
        (Layer::Plumbing, lox_half, [0.45 * 0.6, 0.7 * 0.6, 0.6]),
        (Layer::Electrical, PartFacts { source: true, load: 10.0, ..base.clone() }, [0.15, 0.8, 0.3]),
        (Layer::Electrical, PartFacts { load: 10.0, bus: true, ..base.clone() }, [1.0, 0.5, 0.1]),
        (Layer::Electrical, PartFacts { bus: true, ..base.clone() }, [0.95, 0.85, 0.15]),
        (Layer::Data, PartFacts { computers: 1, sensors: 2, ..base.clone() }, [0.1, 0.85, 0.9]),
        (Layer::Data, PartFacts { sensors: 2, ..base.clone() }, [0.6, 0.3, 0.95]),
        (Layer::Data, base.clone(), GHOST),
        (Layer::Health, base.clone(), [0.12, 0.75, 0.25]),
        (Layer::Health, PartFacts { integrity: 0.0, ..base.clone() }, [0.9, 0.08, 0.04]),
        (Layer::Health, PartFacts { damage: 0.5, ..base.clone() }, [0.9, 0.8, 0.1]),
    ];
    for (layer, f, want) in table {
        let got = colour(layer, &f, false);
        let close = got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-5);
        assert!(close, "{layer:?} {f:?}: {got:?} vs {want:?}");
    }
    // The selection is tinted towards the accent in every layer.
    let s = colour(Layer::Physical, &base, true);
    assert!(s[0] > s[2] && s != [0.8, 0.8, 0.8]);
    assert!(Layer::ALL.iter().all(|l| !l.name().is_empty() && !l.note().is_empty()));
}

#[test]
fn colour_ramps_hit_their_ends() {
    let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
    assert!(near(heat_colour(100.0, 1000.0), [0.05, 0.15, 0.9]));
    assert!(near(heat_colour(1000.0, 1000.0), [0.9, 0.08, 0.04]));
    assert!(near(heat_colour(5000.0, 1000.0), [0.9, 0.08, 0.04]), "beyond the limit stays red");
    assert!(near(health_colour(1.0, 0.0), [0.12, 0.75, 0.25]));
    assert_eq!(fluid_colour("unobtainium"), [0.6, 0.6, 0.6]);
}

#[test]
fn the_tree_lists_children_after_their_parent() {
    let craft = rocket();
    let names: Vec<(usize, &str)> =
        tree(&craft.parts).iter().map(|&(d, k)| (d, craft.parts[k].name.as_str())).collect();
    assert_eq!(names.len(), craft.parts.len());
    assert_eq!(names[..3], [(0, "capsule"), (1, "tank"), (2, "thrust")]);
    assert!(names[3..].iter().all(|&(d, n)| d == 3 && (n.starts_with("engine#") || n.starts_with("leg#"))));
}

#[test]
fn the_orbit_camera_looks_at_its_target_from_its_angles() {
    let target = Vec3::new(1.0, 2.0, 3.0);
    // (yaw, pitch, where the camera is relative to the target)
    let table = [
        (0.0, 0.0, Vec3::new(0.0, 0.0, 10.0)),
        (std::f32::consts::FRAC_PI_2, 0.0, Vec3::new(10.0, 0.0, 0.0)),
        (0.0, 1.0, Vec3::new(0.0, 10.0 * 1f32.sin(), 10.0 * 1f32.cos())),
    ];
    for (yaw, pitch, at) in table {
        let t = orbit_pose(target, yaw, pitch, 10.0);
        assert!((t.translation - target - at).length() < 1e-4, "{yaw} {pitch}: {}", t.translation);
        assert!((t.forward().as_vec3() - (target - t.translation).normalize()).length() < 1e-5);
    }
    // The craft's nose (+Z) points up in the view.
    assert!((craft_to_view() * Vec3::Z - Vec3::Y).length() < 1e-6);
}

#[test]
fn a_click_picks_the_nearest_part_on_the_ray() {
    // Two unit squares facing +Z, at z 0 and z 2 (the second shifted in x).
    let square = |z: f32| RenderMesh {
        positions: vec![[-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    };
    let (near, far) = (square(2.0), square(0.0));
    let parts = [(&far, Transform::IDENTITY), (&near, Transform::from_xyz(1.5, 0.0, 0.0))];
    let down = -Vec3::Z;
    // (ray origin, picked)
    let table = [
        (Vec3::new(0.0, 0.0, 5.0), Some(0)),
        (Vec3::new(1.0, 0.0, 5.0), Some(1)),
        (Vec3::new(2.4, 0.0, 5.0), Some(1)),
        (Vec3::new(3.0, 0.0, 5.0), None),
        (Vec3::new(0.0, 0.0, -5.0), None),
    ];
    for (origin, want) in table {
        assert_eq!(pick(origin, down, &parts), want, "{origin}");
    }
}

#[test]
fn the_regions_tile_the_screen_and_keep_the_view_wide() {
    use bevy_egui::egui::{pos2, Rect};
    // (width, height, left width, view width)
    let table =
        [(1920.0, 1080.0, 280.0, 1260.0), (1280.0, 800.0, 280.0, 620.0), (900.0, 600.0, 280.0 * 600.0 / 660.0, 300.0)];
    for (w, h, left, view) in table {
        let r = regions(Rect::from_min_max(pos2(0.0, 0.0), pos2(w, h)));
        assert!((r.left.width() - left).abs() < 1e-3, "{w}: {}", r.left.width());
        assert!((r.view.width() - view).abs() < 1e-3, "{w}: {}", r.view.width());
        // Tiled: the view between the columns, the editors under it.
        assert_eq!((r.view.min.x, r.view.max.x), (r.left.max.x, r.right.min.x));
        assert_eq!((r.view.max.y, r.bottom.min.y, r.bottom.max.y), (r.bottom.min.y, r.view.max.y, h));
        assert_eq!((r.top.max.y, r.left.min.y), (r.view.min.y, r.view.min.y));
        let area = r.top.area() + r.left.area() + r.right.area() + r.bottom.area() + r.view.area();
        assert!((area - w * h).abs() < 1.0, "{area}");
    }
}

#[test]
fn layers_draw_their_networks_between_the_items_ends() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    // Each item at a point named by its id's length and its part (enough to
    // tell the ends apart).
    let at = |id: &str, part: u32| Vec3::new(id.len() as f32, part as f32, 0.0);
    let plumbing = networks(Layer::Plumbing, def, &at);
    assert_eq!(plumbing.len(), def.lines.len());
    // The bottle (in the tank) to each thruster block (in the capsule).
    let tank = def.part_named("tank").unwrap() as f32;
    assert!(plumbing.iter().any(|(a, b)| *a == Vec3::new(6.0, tank, 0.0) && b.x == 5.0), "{plumbing:?}");
    // The bus: the computer to the two blocks; the link: the computer to
    // four engines and two blocks.
    assert_eq!(networks(Layer::Electrical, def, &at).len(), 2);
    let data = networks(Layer::Data, def, &at);
    assert_eq!(data.len(), 6);
    assert!(data.iter().all(|(a, _)| a.x == "computer".len() as f32));
    for layer in [Layer::Physical, Layer::Structural, Layer::Thermal, Layer::Health] {
        assert!(networks(layer, def, &at).is_empty(), "{layer:?}");
    }
}

/// A part opens the screen at the layer it is mostly about.
#[test]
fn a_part_opens_at_its_main_layer() {
    let tank = PartFacts { tanks: vec![("LOX".into(), None, 1.0)], source: true, ..PartFacts::default() };
    let battery = PartFacts { source: true, computers: 1, ..PartFacts::default() };
    let computer = PartFacts { computers: 2, ..PartFacts::default() };
    // (facts, layer)
    let table = [
        (tank, Layer::Plumbing),
        (battery, Layer::Electrical),
        (computer, Layer::Data),
        (PartFacts::default(), Layer::Structural),
    ];
    for (f, want) in table {
        assert_eq!(Layer::main(&f), want, "{f:?}");
    }
}
