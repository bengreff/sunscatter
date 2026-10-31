# The camera

As built in `game::camera`, its rules pure in `camera::rules` (rule 8) and wired to the rig each frame; the terrain it keeps clear of is `sim`'s (`sim::forces::altitude_above`), the surface physics and drawing use. From the old spec in [map-view-lighting-controls.md](../archive/plans/map-view-lighting-controls.md) §1 and §7, and [playtest-1.md](../archive/plans/playtest-1.md) D (the camera bugs). The system: [the game](../systems/game.md). MCP `set_camera` moves it for shots ([mcp.md](mcp.md)).

**Status: playable.** Every rule has a table test; used in every flight and in the mission tests.

## Zoom

- **Closest:** 1 m from the focus (`MIN_DISTANCE`), 1.3 bounding radii from a focused vessel (`VESSEL_MIN_RADII`), 5 m for a body focus (its surface is the clearance rule's). The default distance (\`) is 60 m, or four radii of a larger craft.
- **Farthest** (`max_distance`): far enough to frame the body farthest from the focus with a 1.25 margin, derived from the bodies, so a body added to the data (or a dwarf planet receding) stays within reach.
- **Speed:** each wheel line zooms by the setting's factor; a trackpad's pixels count a fifth as much (`TRACKPAD_SHARE`: the owner found trackpad zoom five times too fast, 2026-10-26). Settings ▸ Controls holds the zoom speed, trackpad zoom, mouse sensitivity and invert.

## Collision

- **Surfaces:** the camera keeps `max(2 m, 0.002 × the focus distance)` above any surface (`surface_margin`). A zoom or rotation that would break it is refused (`accept`: the camera stops), unless it moves out of a surface it was already too close to (the ground can move under it). A surface rising behind the camera pulls it in towards the focus (`pull_in`); when nothing on the ray is clear it is lifted along the up direction (`lift`), so it is never left under the ground.
- **The craft:** the closest zoom keeps it outside the focused craft (1.3 bounding radii).

## Orientation

- **The yaw reference** is carried from frame to frame (`carry_reference`: parallel transport onto the new up), so it never flips over a pole. Below 2 % of a body's radius it turns with the ground under it (`co_rotates`), so a view of a craft on the pad keeps its heading as time passes; higher, it is inertial.
- **Up** turns to a new reference body over 1.5 s (`blended_up`, a smoothstep along the great circle) instead of snapping.
- **Guards:** degenerate vectors fall back (`safe_dir`) instead of making NaN; a focused vessel that no longer exists returns the focus to the active ship (`resolve_focus`).

## Tests

`camera::rules` tests: the zoom table; a trackpad zooms a fifth as fast and a wheel as before; the farthest zoom frames the farthest body; the closest zoom follows the craft's size; zooming or rotating into terrain is refused; orbiting below a mountain stops at its slope; a rising surface pulls the camera in; the camera is never left under the ground; the margin grows with distance; the yaw reference does not flip over a pole and turns with the ground only near it; up blends between reference bodies; degenerate vectors do not make NaN; a deleted focused vessel returns the focus to the ship.
