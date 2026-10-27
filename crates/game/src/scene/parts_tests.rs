use super::*;
use crate::test_rocket::rocket;

#[test]
fn colours_come_from_the_finish_else_the_bare_material() {
    let m = sim::materials::materials();
    let grey = |c: [f32; 3]| (c[0] - c[1]).abs() < 1e-6 && (c[1] - c[2]).abs() < 1e-6;
    let white = colour("al-2219", Some("white-paint"), m);
    let black = colour("cfrp", Some("black-paint"), m);
    let bare = colour("al-2219", None, m);
    let carbon = colour("cfrp", None, m);
    // (colour, expected albedo from the absorptivity)
    let al = m.get("al-2219").unwrap().absorptivity;
    let cases = [
        (white, 1.0 - m.finish("white-paint").unwrap().absorptivity),
        (black, 1.0 - m.finish("black-paint").unwrap().absorptivity),
        (bare, 1.0 - al),
        (carbon, 1.0 - m.get("cfrp").unwrap().absorptivity),
    ];
    for (c, albedo) in cases {
        assert!(grey(c), "{c:?}");
        assert!((f64::from(c[0]) - albedo.clamp(0.02, 0.9)).abs() < 1e-6, "{c:?} vs {albedo}");
    }
    assert!(white[0] > 0.6 && black[0] < 0.15, "white body, black legs: {white:?} {black:?}");
    // Copper is tinted; an unknown material is the neutral grey.
    let copper = colour("copper-narloy-z", None, m);
    assert!(copper[0] > copper[1] && copper[1] > copper[2], "{copper:?}");
    assert_eq!(colour("unobtainium", None, m), UNKNOWN);
    assert_eq!(colour("al-2219", Some("no-such-paint"), m), bare, "an unknown finish leaves the bare material");
}

#[test]
fn static_parts_merge_by_colour_and_moving_parts_stand_alone() {
    let (w, b, g) = ([0.8; 3], [0.05; 3], [0.5; 3]);
    let colours = [w, w, g, b, b, w];
    let mechanism = [None, None, None, Some(0), Some(1), None];
    assert_eq!(
        groups(&colours, &mechanism),
        [
            Group::Static { colour: w, parts: vec![0, 1, 5] },
            Group::Static { colour: g, parts: vec![2] },
            Group::Moving { colour: b, part: 3, mechanism: 0 },
            Group::Moving { colour: b, part: 4, mechanism: 1 },
        ]
    );
    assert!(groups(&[], &[]).is_empty());
}

#[test]
fn merged_meshes_keep_every_triangle_with_its_own_vertices() {
    let a = RenderMesh {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
    };
    let b =
        RenderMesh { positions: vec![[5.0; 3]; 4], normals: vec![[1.0, 0.0, 0.0]; 4], indices: vec![0, 1, 2, 2, 3, 0] };
    let m = merge([&a, &b]);
    assert_eq!(m.positions.len(), 7);
    assert_eq!(m.indices, [0, 1, 2, 3, 4, 5, 5, 6, 3]);
    assert_eq!(m.normals[3], [1.0, 0.0, 0.0]);
}

#[test]
fn a_hinge_turns_its_part_about_the_axis_through_its_point() {
    let pos = DVec3::new(1.0, 0.0, 0.0);
    // Stowed at 0°, deployed 90° about +Y through (1, 0, 0): a point 1 m
    // up the leg from the hinge swings out to +X.
    let t = hinge(pos, DVec3::Y, 0.0, 90.0);
    let p = t.transform_point(Vec3::new(1.0, 0.0, 1.0));
    assert!((p - Vec3::new(2.0, 0.0, 0.0)).length() < 1e-5, "{p}");
    // The hinge point stays; at the stowed angle nothing moves.
    assert!((t.transform_point(pos.as_vec3()) - pos.as_vec3()).length() < 1e-6);
    let still = hinge(pos, DVec3::Y, 30.0, 30.0);
    assert!((still.transform_point(Vec3::new(3.0, 2.0, 1.0)) - Vec3::new(3.0, 2.0, 1.0)).length() < 1e-6);
}

#[test]
fn a_flame_starts_at_the_nozzle_exit_and_grows_with_thrust() {
    // A bell from the gimbal point at z 0 down to z −1.5, thrust along +Z.
    let mesh = RenderMesh {
        positions: vec![[0.0, 0.0, 0.2], [0.4, 0.0, -1.5], [-0.4, 0.0, -1.5]],
        normals: vec![[1.0, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let area = std::f64::consts::PI * 0.4 * 0.4;
    let n = nozzle(DVec3::ZERO, DVec3::Z, area, &mesh);
    assert!((n.exit - DVec3::new(0.0, 0.0, -1.5)).length() < 1e-9 && (n.radius - 0.4).abs() < 1e-12);
    assert_eq!(n.exhaust, -DVec3::Z);
    for fraction in [0.4, 1.0] {
        let t = flame(&n, fraction);
        let length = FLAME_DIAMETERS * 0.8 * fraction;
        // The unit cone's base (y = −0.5) at the exit, its apex down the exhaust.
        let base = t.transform_point(Vec3::new(0.0, -0.5, 0.0));
        let apex = t.transform_point(Vec3::new(0.0, 0.5, 0.0));
        assert!((base - Vec3::new(0.0, 0.0, -1.5)).length() < 1e-5, "{fraction}: {base}");
        assert!((apex - Vec3::new(0.0, 0.0, -1.5 - length as f32)).length() < 1e-4, "{fraction}: {apex}");
        assert!((t.scale.x - 0.4).abs() < 1e-6);
    }
}

/// The test rocket: every part has a closed surface of its own; tank and
/// capsule white, legs black and on their mechanisms; four flames.
#[test]
fn the_test_rocket_draws_part_by_part() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let m = sim::materials::materials();
    for k in 0..craft.parts.len() {
        let mesh = part_mesh(&craft, k);
        assert!(!mesh.indices.is_empty() && mesh.indices.len().is_multiple_of(3), "{}", craft.parts[k].name);
    }
    let ids: Vec<u32> = craft.parts.iter().map(|p| p.id).collect();
    let colour_of = |name: &str| {
        let info = def.parts.iter().find(|p| p.id == name).unwrap();
        colour(&info.material, info.finish.as_deref(), m)
    };
    assert!(colour_of("tank")[0] > 0.6 && colour_of("capsule")[0] > 0.6);
    assert!(colour_of("leg#1")[0] < 0.15);
    let mech = mechanisms(def, &ids);
    let legs: Vec<usize> = (0..craft.parts.len()).filter(|&k| craft.parts[k].name.starts_with("leg")).collect();
    assert_eq!(legs.len(), 4);
    assert!(legs.iter().all(|&k| mech[k].is_some()), "{mech:?}");
    assert_eq!(mech.iter().filter(|m| m.is_some()).count(), 4, "only the legs move");
    // Ignited at full throttle in vacuum every engine burns at full
    // thrust; a pair alone, two of them; at 10 % an engine runs at its
    // minimum (40 %); without propellant nothing, unless debug mode.
    let sim = crate::state::SimState::new();
    let earth = sim.world.find("Earth").unwrap().node;
    let (r, vel) = (DVec3::X * 7.0e6, DVec3::Y * 7.5e3);
    let mut v = sim::vessel::Vessel::coasting(&sim.world, sim::vessel::VesselId(9), sim.clock, earth, r, vel, &craft);
    let at = |v: &sim::vessel::Vessel, throttle: f64| engine_fractions(def, &v.engine_thrusts(throttle, 0.0).unwrap());
    assert_eq!(at(&v, 1.0), [0.0; 4]);
    v.command(&sim.world, &sim::craft::command::ShipCommand::Ignite("pair".into())).unwrap();
    let close = |a: Vec<f64>, b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(at(&v, 1.0), [1.0, 0.0, 1.0, 0.0]), "{:?}", at(&v, 1.0));
    assert!(close(at(&v, 0.1), [0.4, 0.0, 0.4, 0.0]), "{:?}", at(&v, 0.1));
    assert!(close(at(&v, 0.0), [0.0; 4]));
    v.set_propellant(&sim.world, 0.0);
    assert!(close(at(&v, 1.0), [0.0; 4]), "no propellant");
    v.set_debug(&sim.world, true);
    assert!(close(at(&v, 1.0), [1.0, 0.0, 1.0, 0.0]), "debug mode");
}
