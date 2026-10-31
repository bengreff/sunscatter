# Lighting

As built in `game::lighting` (pure rules in `rules`; `shadows`, `color`, `haze`; the systems gather a `LightScene` once a frame and fill the shaders' uniforms, `light.wgsl`), `game::ship_light` (a craft's light, `ship.wgsl`) and `game::sky` (the sky light and the flare). Starlight at a point and eclipses for the sim are `sim::light`'s; `lighting` re-exports its eclipse rule. Decisions: D055 (physical, per body, fixed exposure), D087 (exposure follows the starlight at the camera), D110 (a craft's light is the craft's), D073 (one look), D123 (visuals frozen). From the old spec in [map-view-lighting-controls.md](../archive/plans/map-view-lighting-controls.md) §3, as those decisions revised it. The system: [the game](../systems/game.md). Many stars, the exposure's formula and the Solar System's views: [solar-visuals.md](solar-visuals.md).

**Status: playable.** Pure rules with table tests; the one look of every flight. Visual work is frozen until the first two flights fly end to end (D123).

## Light at each object (D055)

- **Stars are data:** a luminosity in `visual.ron` and an efficacy (the Sun: 3.828e26 W at 93 lm/W, `luminous_power`), so sunlight at 1 AU is about 126,600 lux. There is no separate light object.
- **Every object is lit at its own position:** each star's flux `L/(4πd²)` there (`flux`), its eclipses, and planetshine. A single light evaluated at the camera is not allowed: it lit the Moon's night side with the Earthshine of a camera in low orbit (KSP's single-sun problem, and ours).
- **A body's surface** (`body_light`): each drawn star's flux, the bodies that shadow it from that star (`occluders`, `shadow_depth`: up to 4, the deepest in a fixed order, so a moon shadows its planet and far planets do not), and planetshine from the 2 brightest reflectors (`shine_from`: a Lambert sphere of its albedo, `lambert_phase`, peaking at full phase; below 1e-4 lux it would not show).
- **Drawn stars:** the renderer draws up to 4 stars as lights (`star_slots`, the brightest at the camera, one Bevy directional light each); a fifth star's light is ambient, so nothing is dropped silently. Each star's colour comes from its temperature (`color::star_tint`).
- **No ambient floor:** night sides get only planetshine and the starlight background (about 0.001 lux).
- **On the GPU,** terrain, rings and cloud decks share one uniform set (`Lights`) and one eclipse rule, mirroring `sim::light::eclipse_factor`.

## A craft's light (D110)

`object_light` lights the whole craft as one, at the craft: each drawn star with its eclipses by bodies and the air on the line to it (`haze::air_along`), so an orbital sunset happens on the whole craft at once; the sky light of the air around it (`sky::sky_radiance_at`); and planetshine as patches of what the craft sees of each reflector, 4 rings by 8 sectors (`shine_sources`, `shine_patches`: the lit area, its light and its angle to the craft, no special cases). `game::ship_light` draws it from a per-ship storage buffer, without Bevy's per-fragment atmosphere.

**Shadows** come from bodies and craft (a craft's own parts, other craft nearby, the ground under it) and do not change when the camera moves: the craft shadow light (`CraftShadowLight`, render layer `CRAFT_LAYER`) has cascades fitted to groups of craft (`shadows::group`, `fit_cascades`), not to the view.

## Exposure (D055, D087)

Fixed: no eye adaptation (built, then removed on the owner's review: it lit night sides up and made day sides blinding). The base exposure is set for the Sun's light at 1 AU (`reference_lux`) and shifted by the stops between every star's flux at the camera (inverse square, no eclipses: the light where the photographer stands, never the scene's luminance) and that reference, within −14 to +4 stops (out to about 128 AU, in to 0.25 AU; `exposure_ev100`). At Saturn a day side reads like Earth's, and night sides stay as dark against day sides as D055 asks.

## Tests

`lighting/tests.rs`: sunlight at 1 AU and the inverse square; the eclipse table; each body gets its own flux, occluder and shine; a moon shadows its planet and far planets do not; the deepest shadows kept in a fixed order; every visible eclipse on a surface selected; the phase function peaks at full phase; Earthshine on the Moon is full Earth at new Moon; stars drawn brightest first, the rest ambient; a moon eclipsed from one star but not the other; two stars of two colours light one moon; planetshine from two bodies in two colours; a ring's shadow under two stars; the shaders share the lighting limits; exposure follows the starlight at the camera; planetshine far away is the point sources, in low orbit it fills half the sky, over the terminator only the lit crescent shines; a ship in low orbit gets Earthshine in sunlight too; planetshine from the data is of the measured size; no NaN at zero distance or zero light. `lighting/ship_tests.rs`: sunlit white paint stays white and Earthshine is a bluish fraction. `color`'s: star tints relative to the Sun.
