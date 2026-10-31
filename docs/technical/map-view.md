# The map view and orbit lines

As built in `game::map_view` (the rule), `game::map` (it gathers sizes and draws icons), `game::trajectory::line` and `settings` (where a line ends), `game::relations::Dominance` (the dominant body) and `game::tracking` (the tracking station). Decisions: D054 (map view per object), D056 (orbit lines end after one revolution about the dominant body, tidal dominance), D050 (the tracking station), rule 1 (display only). From the old spec in [map-view-lighting-controls.md](../archive/plans/map-view-lighting-controls.md) §2 and §5. The system: [the game](../systems/game.md). How lines are sampled and drawn: owners.md's rows for `game::trajectory::lines` and `LineFrame` (D089); apsis markers: D076.

**Status: playable.** Pure rules with table tests; used in every flight and in the tracking station.

## The rule (D054)

There is no global map mode. Each object, a body or a vessel, is measured with **one scale**: pixels per metre at the camera's distance to its focus, as the camera is placed (`map_view::scale`: `k = viewport_height / (2·tan(fov/2)·focus_distance)`). Objects in the same scene switch together however far they are from the camera, and the rule does not depend on perspective depth.

- `sprite_px = k·radius` (a vessel's bounding radius is 10 m, `map::VESSEL_RADIUS`); `orbit_px = k·orbit_radius`, the osculating semi-major axis about its primary, or the distance if unbound (a star has none, which passes).
- **In map view** ⇔ the sprite is under 1 px and the orbit at least 1 px (`in_map`), with hysteresis: it leaves when the sprite grows above 2 px or the orbit shrinks below 0.5 px.
- **Its orbit line is drawn** ⇔ in map view and its orbit is seen from outside: the orbit radius at most one focus distance, leaving above 1.5 (`line_shown`). From inside, an orbit wraps around the camera as a great circle across the sky (from the pad every dwarf planet's line did). The active vessel's line is exempt (it is the flight's own prediction, starting under the camera), and so are pinned objects.
- **Icons:** drawn in map view; one within 6 px of a heavier object's icon is hidden (the KSP convention), and so is one behind the disc of a nearer object drawn at least 1 px wide (`occluded`). Ap/Pe markers and hover follow the icon.
- **The 3D model** is always drawn, at any distance (it just becomes sub-pixel).

## Hover and picking

- **Hover** (`hover`): among drawn icons within 8 px of the cursor that no nearer full-size disc covers, the heaviest wins (a planet over its moons, a star over its planets); ties go to the nearest centre. An object drawn at full size is never hovered. Moons are reached by zooming in until their icons separate.
- **A double-click** picks a body (`pick_body`, its menu): among bodies drawn at any size whose disc, at least 16 px across the centre, is under the cursor and that no nearer body's disc hides there, the heaviest, as on hover.

## The tracking station (D050)

Its own full screen (M, or the pause menu): the object list on the left (vessels grouped by status with track toggles, bodies as a star → planet → moon tree), details and actions (Focus, Switch to, Delete with confirmation), and the map with its own camera. The same rule applies, with two differences: a tracked object is **pinned** (in map view whatever its orbit size, its icon never merged away, its line always drawn), and clicking an icon selects and focuses it. Only tracked vessels get orbit lines; the active vessel always does. Its screens and ways in: [interface-map.md](interface-map.md).

## Where a line ends (D056)

`trajectory::line::LineRule` is fed path points in time order and reports where the line ends (`EndReason`):
- **Revolutions:** the angle swept about the dominant body in the orbit plane reaches the setting (one revolution by default) with that body dominant the whole way. A change of dominant body restarts the count, so a transfer is drawn until one revolution about the destination.
- **Time cap:** the setting (vessels 365.25 days by default).
- **Escape:** leaving the region of interest (a distance from the root body).
- And, by the caller, the computed part of the stored trajectory: the renderer never integrates (rule 4), and the look-ahead the coast computes follows this rule.

**The dominant body** (`relations::Dominance`) is tidal: the innermost Laplace sphere `d·(m/M)^(2/5)` containing the object, so Earth for the Moon and for low orbit, the Moon in low lunar orbit, the Sun for Earth and for deep space. The radii come from `sim::vessel::laplace_radius` and the body hierarchy from `hold::primary`, taken once at start; the attitude hold's automatic reference is the same rule with the radii at each instant ([attitude-holds.md](attitude-holds.md)). Display only: physics never uses it (rule 1).

**Settings** (Settings ▸ Orbits, `trajectory::settings::OrbitSettings`, saved in `settings.ron`): a vessel's line is 1 revolution (the default), 2 revolutions, until impact or escape, or a fixed time (1 day by default), never longer than the time cap; a body's line is 1 revolution (the default; the ephemeris span caps it) or off; bodies can be hidden one by one, or only the current system's shown (bodies whose dominant body is the active vessel's).

## Tests

`map_view::tests`: the scale; the map-view table (a sub-pixel ship in low orbit is in map view, a 10 m sprite is not, the Moon from a Moon-scale zoom is not, at Solar-System scale Earth is and the Moon is not); hysteresis holds inside the bands with no flip-flop; pinned objects ignore the orbit bound and are never merged; icons next to a heavier icon, or behind a nearer disc, are hidden; hover prefers the heaviest then the nearest; full-size discs block icons behind them and are not hoverable; orbit lines appear only when seen as orbits (from the pad and low orbit no heliocentric line and no Moon line; the Earth–Moon system the Moon's; the inner and outer Solar System the planets' that fit) and hold inside their band; a double-click picks the heaviest body under it. `trajectory/line_tests.rs`: the line ends after the requested revolutions; a change of dominant body restarts the count; the caps end it; circular low orbits end at one revolution; a Moon flyby changes dominant body and restarts; the Moon's line is one lunar orbit about Earth; the prediction's and bodies' lines follow the rule. `relations`' tests (tidal dominance).
