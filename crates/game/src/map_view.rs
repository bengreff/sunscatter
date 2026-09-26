//! The map-view rule (D054): whether each object (body or vessel) is shown
//! as a map object, whether its icon is drawn, and which icon is hovered.
//! This is the one owner of those decisions; icons, orbit lines, Ap/Pe
//! markers and hover in `map`, `trajectory` and the tracking station follow
//! it. Pure functions, tested by tables. Spec:
//! docs/features/map-view-lighting-controls.md §2.

use glam::Vec2;
use sim::frame::NodeId;

/// An object enters map view when its sprite is under this size (px) and
/// leaves when it grows above `SPRITE_EXIT_PX`.
pub const SPRITE_ENTER_PX: f64 = 1.0;
pub const SPRITE_EXIT_PX: f64 = 2.0;
/// Its orbit must be at least this size (px) to enter, and it leaves when
/// the orbit shrinks below `ORBIT_EXIT_PX`.
pub const ORBIT_ENTER_PX: f64 = 1.0;
pub const ORBIT_EXIT_PX: f64 = 0.5;
/// An icon this close (px) to a heavier object's icon is hidden (KSP).
pub const ICON_MERGE_PX: f32 = 6.0;
/// The cursor hovers an icon within this distance (px) of its centre.
pub const ICON_HIT_PX: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectId {
    Body(NodeId),
    /// A vessel by fleet index.
    Vessel(usize),
}

/// What the rule needs to know about one object this frame.
#[derive(Clone, Copy, Debug)]
pub struct Object {
    pub id: ObjectId,
    /// Physical radius (m); a vessel's bounding radius.
    pub radius: f64,
    /// Orbit radius about its primary (m): the osculating semi-major axis,
    /// or the distance if unbound. `None` if it has no primary (a star),
    /// which satisfies the orbit bound.
    pub orbit_radius: Option<f64>,
    /// Priority for overlapping icons and hover (GM for bodies, 0 for vessels).
    pub mass: f64,
    /// Screen position of its centre (`None` if behind the camera).
    pub screen: Option<Vec2>,
    /// Distance from the camera (m).
    pub depth: f64,
    /// Radius of its disc as actually drawn (px, perspective).
    pub disc_px: f64,
    /// Always shown in map view whatever its orbit size (tracked objects in
    /// the tracking station); its icon is never merged away.
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Visibility {
    /// In map view: icon, orbit line, hover and Ap/Pe markers apply.
    pub in_map: bool,
    /// The icon is drawn (and can be hovered).
    pub icon: bool,
}

/// Pixels per metre at the camera's distance to its focus: the one scale
/// every object is measured with.
pub fn scale(viewport_height_px: f64, fov: f64, focus_distance: f64) -> f64 {
    viewport_height_px / (2.0 * (fov * 0.5).tan() * focus_distance.max(1e-9))
}

/// Whether one object is in map view, given its sprite and orbit sizes (px)
/// and whether it was in map view last frame (hysteresis).
pub fn in_map(sprite_px: f64, orbit_px: Option<f64>, pinned: bool, was: bool) -> bool {
    let sprite_ok = if was { sprite_px <= SPRITE_EXIT_PX } else { sprite_px < SPRITE_ENTER_PX };
    let orbit_ok = pinned || orbit_px.is_none_or(|o| if was { o >= ORBIT_EXIT_PX } else { o >= ORBIT_ENTER_PX });
    sprite_ok && orbit_ok
}

/// Classifies every object at scale `k` (px/m). `was(id)` says whether the
/// object was in map view last frame.
pub fn classify(objects: &[Object], k: f64, was: impl Fn(ObjectId) -> bool) -> Vec<Visibility> {
    let map: Vec<bool> =
        objects.iter().map(|o| in_map(k * o.radius, o.orbit_radius.map(|r| k * r), o.pinned, was(o.id))).collect();
    objects
        .iter()
        .zip(&map)
        .map(|(o, &in_map)| {
            let merged = || {
                let Some(s) = o.screen else { return true };
                objects.iter().zip(&map).any(|(h, &h_map)| {
                    h_map && h.mass > o.mass && h.screen.is_some_and(|hs| hs.distance(s) < ICON_MERGE_PX)
                })
            };
            let hidden = o.screen.is_none_or(|s| occluded(objects, s, o.depth));
            let icon = in_map && !hidden && (o.pinned || !merged());
            Visibility { in_map, icon }
        })
        .collect()
}

/// Whether a screen point at distance `depth` from the camera is behind the
/// disc of a nearer object drawn at least 1 px wide (icons, Ap/Pe markers
/// and hover are hidden there).
pub fn occluded(objects: &[Object], point: Vec2, depth: f64) -> bool {
    objects
        .iter()
        .any(|b| b.depth < depth && b.disc_px >= 1.0 && b.screen.is_some_and(|s| s.distance(point) <= b.disc_px as f32))
}

/// The hovered object: among drawn icons under the cursor that no nearer
/// full-size disc covers, the heaviest wins; ties go to the nearest centre.
/// Objects drawn at full size (not in map view) are never hovered.
pub fn hover(objects: &[Object], vis: &[Visibility], cursor: Vec2) -> Option<usize> {
    objects
        .iter()
        .enumerate()
        .filter(|(i, o)| vis[*i].icon && !occluded(objects, cursor, o.depth))
        .filter_map(|(i, o)| {
            let d = o.screen?.distance(cursor);
            (d <= ICON_HIT_PX).then_some((i, o.mass, d))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
        .map(|(i, _, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTH_R: f64 = 6.371e6;
    const MOON_R: f64 = 1.737e6;
    const EARTH_GM: f64 = 3.986e14;
    const MOON_GM: f64 = 4.905e12;
    const SUN_GM: f64 = 1.327e20;
    const AU: f64 = 1.496e11;
    const LEO: f64 = 6.778e6;
    const MOON_ORBIT: f64 = 3.844e8;
    /// Screen height and field of view of the tests' camera.
    const H: f64 = 900.0;
    const FOV: f64 = std::f64::consts::FRAC_PI_4;

    fn body(n: u16, radius: f64, orbit: Option<f64>, gm: f64) -> Object {
        Object {
            id: ObjectId::Body(NodeId(n)),
            radius,
            orbit_radius: orbit,
            mass: gm,
            screen: Some(Vec2::new(800.0, 450.0)),
            depth: 1e9,
            disc_px: 0.0,
            pinned: false,
        }
    }

    fn vessel(i: usize, orbit: f64) -> Object {
        Object { id: ObjectId::Vessel(i), radius: 10.0, mass: 0.0, ..body(0, 0.0, Some(orbit), 0.0) }
    }

    fn at(mut o: Object, x: f32, y: f32) -> Object {
        o.screen = Some(Vec2::new(x, y));
        o
    }

    fn fresh(objects: &[Object], focus_distance: f64) -> Vec<Visibility> {
        classify(objects, scale(H, FOV, focus_distance), |_| false)
    }

    #[test]
    fn scale_is_pixels_per_metre_at_the_focus() {
        // At 1,000 m with a 90° field of view the screen height spans 2,000 m.
        assert!((scale(900.0, std::f64::consts::FRAC_PI_2, 1000.0) - 0.45).abs() < 1e-12);
    }

    #[test]
    fn map_view_table() {
        let earth = body(1, EARTH_R, Some(AU), EARTH_GM);
        let moon = body(2, MOON_R, Some(MOON_ORBIT), MOON_GM);
        let sun = body(0, 6.957e8, None, SUN_GM);
        let ship = vessel(0, LEO);
        // (case, object, focus distance (m), expected in map view)
        let cases = [
            ("LEO ship seen from Earth-scale zoom", ship, 1e8, true),
            ("ship with a 10 m sprite, camera 60 m away", ship, 60.0, false),
            ("Moon seen from Moon scale", moon, 5e6, false),
            ("Earth at solar-system scale", earth, 1e12, true),
            ("Moon at solar-system scale: orbit under 1 px", moon, 1e12, false),
            ("Sun at solar-system scale: no primary", sun, 1e12, true),
            ("Earth seen from LEO", earth, 1e7, false),
        ];
        for (case, o, d, expected) in cases {
            assert_eq!(fresh(&[o], d)[0].in_map, expected, "{case}");
            assert_eq!(fresh(&[o], d)[0].icon, expected, "{case}: icon");
        }
    }

    #[test]
    fn hysteresis_holds_state_inside_the_bands() {
        // (sprite px, orbit px, was, expected)
        let cases = [
            (1.5, Some(10.0), false, false),
            (1.5, Some(10.0), true, true),
            (0.9, Some(10.0), false, true),
            (2.1, Some(10.0), true, false),
            (0.1, Some(0.8), false, false),
            (0.1, Some(0.8), true, true),
            (0.1, Some(0.4), true, false),
            (0.1, Some(1.0), false, true),
        ];
        for (sprite, orbit, was, expected) in cases {
            assert_eq!(in_map(sprite, orbit, false, was), expected, "sprite {sprite}, orbit {orbit:?}, was {was}");
        }
    }

    #[test]
    fn no_flip_flop_across_the_band() {
        let mut state = false;
        let mut flips = 0;
        for step in 0..200 {
            // The sprite wobbles between 1.1 and 1.9 px: never enters.
            let sprite = 1.5 + 0.4 * (step as f64 * 0.7).sin();
            let next = in_map(sprite, Some(50.0), false, state);
            flips += usize::from(next != state);
            state = next;
        }
        assert_eq!(flips, 0);
        // Once in (below 1 px), the same wobble never leaves.
        let mut state = in_map(0.5, Some(50.0), false, false);
        for step in 0..200 {
            state = in_map(1.5 + 0.4 * (step as f64 * 0.7).sin(), Some(50.0), false, state);
            assert!(state);
        }
    }

    #[test]
    fn pinned_objects_ignore_the_orbit_bound_and_are_never_merged() {
        let k = scale(H, FOV, 1e12);
        let earth = at(body(1, EARTH_R, Some(AU), EARTH_GM), 400.0, 300.0);
        let mut ship = at(vessel(0, LEO), 402.0, 300.0);
        assert_eq!(classify(&[earth, ship], k, |_| false)[1], Visibility { in_map: false, icon: false });
        ship.pinned = true;
        assert_eq!(classify(&[earth, ship], k, |_| false)[1], Visibility { in_map: true, icon: true });
    }

    #[test]
    fn icons_next_to_a_heavier_icon_are_hidden() {
        let k = scale(H, FOV, 1e12);
        let earth = at(body(1, EARTH_R, Some(AU), EARTH_GM), 400.0, 300.0);
        // A made-up moon far enough out to be in map view at this scale.
        let moon = |x: f32| at(body(2, MOON_R, Some(5e10), MOON_GM), x, 300.0);
        let near = classify(&[earth, moon(404.0)], k, |_| false);
        assert!(near[1].in_map && !near[1].icon && near[0].icon);
        let apart = classify(&[earth, moon(410.0)], k, |_| false);
        assert!(apart[1].icon && apart[0].icon);
    }

    #[test]
    fn icons_behind_a_nearer_disc_are_hidden() {
        let k = scale(H, FOV, 1e8);
        let mut moon = at(body(2, MOON_R, Some(MOON_ORBIT), MOON_GM), 800.0, 450.0);
        (moon.disc_px, moon.depth) = (300.0, 5e6);
        let mut ship = at(vessel(0, LEO), 850.0, 450.0);
        ship.depth = 4e8;
        assert_eq!(classify(&[moon, ship], k, |_| false)[1], Visibility { in_map: true, icon: false });
        ship.depth = 1e6;
        assert_eq!(classify(&[moon, ship], k, |_| false)[1], Visibility { in_map: true, icon: true });
        ship.depth = 4e8;
        ship.screen = Some(Vec2::new(1200.0, 450.0));
        assert!(classify(&[moon, ship], k, |_| false)[1].icon, "beside the disc");
        assert!(occluded(&[moon], Vec2::new(900.0, 450.0), 1e7) && !occluded(&[moon], Vec2::new(900.0, 450.0), 1e6));
    }

    #[test]
    fn hover_prefers_the_heaviest_then_the_nearest() {
        let visible = |n: usize| vec![Visibility { in_map: true, icon: true }; n];
        let planet = at(body(1, EARTH_R, Some(AU), EARTH_GM), 400.0, 300.0);
        let moon = at(body(2, MOON_R, Some(MOON_ORBIT), MOON_GM), 407.0, 300.0);
        // The cursor is nearer the moon, but the planet wins.
        assert_eq!(hover(&[planet, moon], &visible(2), Vec2::new(405.0, 300.0)), Some(0));
        // Out of the planet's reach, the moon.
        assert_eq!(hover(&[planet, moon], &visible(2), Vec2::new(413.0, 300.0)), Some(1));
        // Equal priority: the nearest centre.
        let a = at(vessel(0, LEO), 400.0, 300.0);
        let b = at(vessel(1, LEO), 406.0, 300.0);
        assert_eq!(hover(&[a, b], &visible(2), Vec2::new(404.0, 300.0)), Some(1));
        assert_eq!(hover(&[a, b], &visible(2), Vec2::new(402.0, 300.0)), Some(0));
        // Nothing near the cursor.
        assert_eq!(hover(&[a, b], &visible(2), Vec2::new(100.0, 100.0)), None);
    }

    #[test]
    fn full_size_discs_block_icons_behind_them_and_are_not_hoverable() {
        let mut earth = at(body(1, EARTH_R, Some(AU), EARTH_GM), 400.0, 300.0);
        earth.disc_px = 200.0;
        earth.depth = 1e7;
        let mut behind = at(vessel(0, LEO), 450.0, 300.0);
        behind.depth = 2e7;
        let mut front = behind;
        front.depth = 5e6;
        let vis = [Visibility::default(), Visibility { in_map: true, icon: true }];
        let cursor = Vec2::new(450.0, 300.0);
        assert_eq!(hover(&[earth, behind], &vis, cursor), None);
        assert_eq!(hover(&[earth, front], &vis, cursor), Some(1));
        // The disc itself (not in map view) is never hovered.
        assert_eq!(hover(&[earth], &vis[..1], Vec2::new(400.0, 300.0)), None);
    }
}
