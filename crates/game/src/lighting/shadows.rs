//! Craft shadows that do not depend on the camera (D110). Bevy fits a
//! directional light's shadow cascades to the camera's view, so a craft's
//! shadows changed as the camera turned around it. Each craft group near
//! the camera instead gets a cascade of its own: an orthographic box along
//! the brightest star's light, fitted to the group's bounding sphere, so
//! what shadows a craft depends only on the craft, the star and what lies
//! between them. One extra light carries these cascades
//! ([`CraftShadowLight`]): it gives no light, sees only craft (render layer
//! [`CRAFT_LAYER`]), and `ship.wgsl` samples it for the star's shadow.
//! Bodies' shadows on craft are the eclipse rule's (`rules::eclipse_factor`).

use bevy::light::cascade::{Cascade, Cascades};
use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;
use glam::DVec3;

/// The render layer of the craft (their meshes are on the default layer
/// too, so the camera draws them): the only casters [`CraftShadowLight`]
/// sees, so the ground and bodies never shadow craft through it.
pub const CRAFT_LAYER: usize = 2;

/// Cascades of the craft shadow light: the craft groups shadowed at once.
pub const MAX_GROUPS: usize = 4;

/// Craft closer than this many of their radii join one group's cascade.
const JOIN: f64 = 1.5;

/// A group's cascade grows no larger than this (m): beyond it, craft keep
/// cascades of their own (a cascade's texel grows with its size).
const MAX_GROUP_RADIUS: f64 = 400.0;

/// The light carrying the craft groups' shadow cascades (no light of its
/// own), along the brightest star's light.
#[derive(Component)]
pub struct CraftShadowLight;

/// This frame's craft groups (camera-relative centre, radius), each the
/// cascade of the same index; filled by `scene::ships::update_ships`.
#[derive(Resource, Default, Debug)]
pub struct CraftGroups(pub Vec<(DVec3, f64)>);

/// Groups the craft (camera-relative bounding spheres) into at most
/// [`MAX_GROUPS`] cascades: the largest on screen first; a craft within
/// [`JOIN`] radii of a group joins it while the group stays under
/// [`MAX_GROUP_RADIUS`]. Returns the groups and each craft's group (none:
/// too small on screen to get one this frame). The groups depend on the
/// camera only through which craft get one; a group's box never does.
pub fn group(craft: &[(DVec3, f64)]) -> (Vec<(DVec3, f64)>, Vec<Option<usize>>) {
    let size = |&(c, r): &(DVec3, f64)| r / c.length().max(r).max(1e-9);
    let mut order: Vec<usize> = (0..craft.len()).collect();
    order.sort_by(|&a, &b| size(&craft[b]).total_cmp(&size(&craft[a])).then(a.cmp(&b)));
    let mut groups: Vec<(DVec3, f64)> = Vec::new();
    let mut of = vec![None; craft.len()];
    for k in order {
        let (c, r) = craft[k];
        let joined = groups.iter().position(|&(gc, gr)| {
            let (_, merged) = enclose((gc, gr), (c, r));
            (gc - c).length() <= JOIN * (gr + r) && merged <= MAX_GROUP_RADIUS
        });
        match joined {
            Some(g) => {
                groups[g] = enclose(groups[g], (c, r));
                of[k] = Some(g);
            }
            None if groups.len() < MAX_GROUPS => {
                groups.push((c, r));
                of[k] = Some(groups.len() - 1);
            }
            None => {}
        }
    }
    (groups, of)
}

/// The smallest sphere holding two spheres.
fn enclose(a: (DVec3, f64), b: (DVec3, f64)) -> (DVec3, f64) {
    let d = (b.0 - a.0).length();
    if d + b.1 <= a.1 {
        return a;
    }
    if d + a.1 <= b.1 {
        return b;
    }
    let r = 0.5 * (d + a.1 + b.1);
    let centre = if d > 0.0 { a.0 + (b.0 - a.0) * ((r - a.1) / d) } else { a.0 };
    (centre, r)
}

/// The cascade of a sphere (`centre`, `radius`; world space) for a light
/// turned by `rotation` (its -Z along the light), on a shadow map of
/// `size` texels: a square box around the sphere, its near plane on the
/// sphere's light side. Not snapped to texels: the box moves with the
/// craft, so the map is still relative to it. Casters between the star
/// and the near plane are kept (Bevy clamps their depth to it).
pub fn cascade(rotation: Quat, centre: Vec3, radius: f32, size: f32) -> Cascade {
    let world_from_light = Mat4::from_quat(rotation);
    let light_from_world = world_from_light.transpose();
    let c = light_from_world.transform_point3(centre);
    let diameter = 2.0 * radius.max(0.01);
    let near = Vec3::new(c.x, c.y, c.z + radius);
    let cascade_from_world =
        Mat4::from_cols(light_from_world.x_axis, light_from_world.y_axis, light_from_world.z_axis, (-near).extend(1.0));
    let world_from_cascade = Mat4::from_cols(
        world_from_light.x_axis,
        world_from_light.y_axis,
        world_from_light.z_axis,
        world_from_light * near.extend(1.0),
    );
    // Reverse Z as Bevy's: 1 at the near plane, 0 a diameter behind it.
    let clip_from_cascade = Mat4::from_cols(
        Vec4::new(2.0 / diameter, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / diameter, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0 / diameter, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 1.0),
    );
    Cascade {
        world_from_cascade,
        clip_from_cascade,
        clip_from_world: clip_from_cascade * cascade_from_world,
        texel_size: diameter / size,
    }
}

/// A box no caster reaches, for the cascades without a group.
fn unused(rotation: Quat, size: f32) -> Cascade {
    cascade(rotation, rotation * Vec3::new(0.0, 0.0, -1.0e9), 0.01, size)
}

/// Replaces the craft shadow light's camera-fitted cascades (Bevy's, just
/// built) with the craft groups' for the main camera.
pub fn fit_cascades(
    groups: Res<CraftGroups>,
    map: Res<DirectionalLightShadowMap>,
    cams: Query<Entity, With<crate::camera::MainCamera>>,
    mut lights: Query<(&GlobalTransform, &mut Cascades), With<CraftShadowLight>>,
) {
    let size = map.size as f32;
    for (t, mut cascades) in &mut lights {
        let rotation = t.rotation();
        let fitted: Vec<Cascade> = (0..MAX_GROUPS)
            .map(|g| match groups.0.get(g) {
                Some(&(c, r)) => cascade(rotation, c.as_vec3(), r as f32, size),
                None => unused(rotation, size),
            })
            .collect();
        for cam in &cams {
            cascades.cascades.insert(cam, fitted.clone());
        }
    }
}

/// Spawns the craft shadow light (shadows on with the look's shadows,
/// `look::apply`; turned with the brightest star's light,
/// `lighting::update_lights`).
pub fn spawn_light(commands: &mut Commands) {
    let config = bevy::light::CascadeShadowConfigBuilder {
        num_cascades: MAX_GROUPS,
        minimum_distance: 0.1,
        first_cascade_far_bound: 1.0,
        maximum_distance: 4.0,
        ..default()
    }
    .build();
    commands.spawn((
        CraftShadowLight,
        DirectionalLight { illuminance: 0.0, color: Color::BLACK, shadow_maps_enabled: false, ..default() },
        config,
        bevy::camera::visibility::RenderLayers::layer(CRAFT_LAYER),
        Transform::IDENTITY,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craft_near_each_other_share_a_cascade_and_far_ones_get_their_own() {
        let at = |x: f64, y: f64| DVec3::new(x, y, -100.0);
        // (case, craft, groups, each craft's group)
        type Case = (&'static str, Vec<(DVec3, f64)>, usize, Vec<Option<usize>>);
        let cases: [Case; 5] = [
            ("one craft", vec![(at(0.0, 0.0), 30.0)], 1, vec![Some(0)]),
            (
                "stacked: booster and second stage at separation",
                vec![(at(0.0, 0.0), 25.0), (at(0.0, 40.0), 10.0)],
                1,
                vec![Some(0), Some(0)],
            ),
            ("a kilometre apart", vec![(at(0.0, 0.0), 25.0), (at(1000.0, 0.0), 10.0)], 2, vec![Some(0), Some(1)]),
            (
                "six craft far apart: the four largest on screen (the nearest)",
                (0..6).map(|k| (at(1000.0 * k as f64, 0.0), 10.0 + k as f64)).collect(),
                4,
                vec![Some(0), Some(1), Some(2), Some(3), None, None],
            ),
            ("none", vec![], 0, vec![]),
        ];
        for (case, craft, n, of) in cases {
            let (groups, got) = group(&craft);
            assert_eq!(groups.len(), n, "{case}");
            assert_eq!(got, of, "{case}");
            // Every craft with a group lies inside it.
            for (k, g) in got.iter().enumerate() {
                if let Some(g) = g {
                    let (c, r) = groups[*g];
                    assert!((craft[k].0 - c).length() + craft[k].1 <= r + 1e-6, "{case}: craft {k}");
                }
            }
        }
    }

    #[test]
    fn a_cascade_depends_on_the_craft_and_the_light_only() {
        let rotation = Transform::IDENTITY.looking_to(Vec3::new(0.3, -1.0, 0.2).normalize(), Vec3::Y).rotation;
        let (centre, radius) = (Vec3::new(5.0, -3.0, -40.0), 20.0);
        let c = cascade(rotation, centre, radius, 2048.0);
        let clip = |p: Vec3| {
            let v = c.clip_from_world * p.extend(1.0);
            v.truncate() / v.w
        };
        // The sphere's centre maps to the box's centre, halfway in depth.
        let mid = clip(centre);
        assert!(mid.x.abs() < 1e-4 && mid.y.abs() < 1e-4 && (mid.z - 0.5).abs() < 1e-4, "{mid}");
        // Its light side is the near plane (1, reverse Z), its far side 0.
        let to_light = rotation * Vec3::Z;
        assert!((clip(centre + to_light * radius).z - 1.0).abs() < 1e-4);
        assert!(clip(centre - to_light * radius).z.abs() < 1e-4);
        // Across the light, the sphere's edge is the box's edge.
        let side = rotation * Vec3::X;
        assert!((clip(centre + side * radius).x - 1.0).abs() < 1e-4);
        assert!((c.texel_size - 40.0 / 2048.0).abs() < 1e-6);
        // Moving the craft and the camera together moves the box with it:
        // the same craft point lands on the same texel.
        let shift = Vec3::new(120.0, 7.0, -33.0);
        let moved = cascade(rotation, centre + shift, radius, 2048.0);
        let p = centre + Vec3::new(3.0, 4.0, -5.0);
        let a = c.clip_from_world * p.extend(1.0);
        let b = moved.clip_from_world * (p + shift).extend(1.0);
        assert!((a - b).length() < 1e-4, "{a} {b}");
    }
}
