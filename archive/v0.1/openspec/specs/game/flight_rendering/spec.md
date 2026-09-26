# Flight Rendering

Rendering planets, the ship in flight, the HUD, and maneuver nodes. Uses wgpu for geometry and egui for UI overlays.

## Capabilities

- [Camera](camera/spec.md) - Flight camera behavior, zoom levels, body tracking, ship view vs map view
- [Bodies](bodies/spec.md) - Planet/moon rendering, atmosphere, surface, orbit lines, launchpad
- [Ship](ship/spec.md) - Ship triangle indicator, part-based rendering, exhaust plumes
- [HUD](hud/spec.md) - Flight HUD panels (velocity, altitude, orbit info, fuel, time warp, staging)
- [Maneuver Nodes](maneuver_nodes/spec.md) - Maneuver node creation, dragging, burning, delta-v display

## Rendering Pipeline

### Requirement: Two-pass rendering for flight mode

The flight scene uses two sequential render passes:
1. **MSAA geometry pass**: renders to MSAA texture (sample count 4), resolves to swapchain texture. Clear color = black `(0, 0, 0, 1)`. Draws all wgpu geometry (bodies, orbits, ship).
2. **egui overlay pass**: renders to swapchain texture directly (no MSAA), using `LoadOp::Load` to preserve the geometry pass output. Draws all egui UI elements.

### Requirement: Dynamic geometry buffer pre-allocation

Vertex buffer holds up to 500,000 vertices. Index buffer holds up to 1,500,000 u32 indices. These are pre-allocated at initialization and updated each frame via `queue.write_buffer`.

### Requirement: Camera uniform buffer

The camera uniform struct is `{ position: [f32; 2], zoom: f32, aspect_ratio: f32 }` (16 bytes, repr(C), Pod/Zeroable). It is bound to the vertex shader at binding 0 in bind group 0, updated each frame before rendering.

### Requirement: Body texture bind group

A `texture_2d_array` and sampler are bound at group 1 (binding 0 = texture, binding 1 = sampler). The texture array holds body textures loaded from `data/textures/bodies/`. A dummy 1-layer texture is created when no textures exist, ensuring the bind group is always valid. The bind group is set at all render pass sites.

### Requirement: World scale constant

All positions are scaled by `SCALE = 1e-9` and `BODY_SCALE = 1.0`, so the effective render scale is `1e-9` world units per meter. Ship world position = `absolute_position * SCALE * BODY_SCALE`.

### Requirement: Rendering layer order

Geometry is drawn in the following back-to-front order within a single MSAA render pass:
1. Atmosphere rings
2. Celestial body orbit lines (adaptive segments, max 5120)
3. Celestial bodies (filled circles)
4. Ship orbit/trajectory lines (patched conics, on top of bodies)
5. Predicted trajectory lines (green, from maneuver nodes)
6. Launchpad (ship view only)
7. Ship (part-based rendering or triangle, at actual world scale)
8. Ship indicator triangle (map view only, fixed screen size)

egui overlays are rendered in a separate non-MSAA pass on top.

### Requirement: Background clear color

The flight scene clears to pure black: `(r: 0.0, g: 0.0, b: 0.0, a: 1.0)`.

### Requirement: Window resize handling

On resize, the surface is reconfigured, the MSAA texture is recreated at the new dimensions, and the camera aspect ratio is updated. Resize is only processed when both dimensions are > 0.
