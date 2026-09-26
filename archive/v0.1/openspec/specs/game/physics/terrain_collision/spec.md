# Terrain Collision

Collision detection between the ship/vessel and celestial body surfaces, including launchpad structures.

## Point-Mass Collision

### Requirement: Ship collision detection with bodies

After each flying physics update, the ship SHALL check for collisions with ALL solar system bodies:
1. Compute distance from ship absolute position to body world position
2. If distance < body_radius + ship_radius (half vessel height or `SHIP_SIZE/2 = 5.0m` fallback):
   - Switch SOI body if needed
   - Place ship on surface at collision angle
   - Zero velocity and throttle
   - Set state to `Landed` with the collision surface angle
   - Disable on-rails mode and clear cached orbit
3. Launchpad collision takes priority (checked first for the launchpad body)

## Part-Level Collision

### Requirement: Per-part terrain collision check

When a `FlightVessel` is loaded and the ship is `Flying`, the system SHALL perform per-part terrain collision checks each frame:
1. For each non-destroyed, non-decoupled part, compute the hitbox center Y as `local_position[1] + hitbox_y_offset` (where `hitbox_y_offset` accounts for top-aligned engines whose visible content is shorter than the editor hitbox)
2. Check all 4 corners of the part's hitbox centered on `(local_position[0], center_y)`
3. Transform corners from part-local space to world space using vessel position and rotation
4. Check each corner against the body surface radius and launchpad geometry
5. If any corner penetrates: land the vessel at the collision surface angle

### Requirement: Launchpad collision geometry

The launchpad on Earth (body index 3) SHALL be a trapezoidal raised platform:
- `LAUNCHPAD_HEIGHT = 10.0` meters above surface
- `LAUNCHPAD_TOP_WIDTH = 100.0` meters
- `LAUNCHPAD_BOTTOM_WIDTH = 120.0` meters
- `LAUNCHPAD_SURFACE_ANGLE = PI/2` (top of body)
- Width interpolates linearly from bottom to top

#### Scenario: Landing on launchpad
- **WHEN** a vessel descends onto the launchpad area
- **THEN** it SHALL land at `body_radius + LAUNCHPAD_HEIGHT + bottom_extent` instead of `body_radius + bottom_extent`

#### Scenario: Landing on ground beside launchpad
- **WHEN** a vessel descends outside the launchpad angular extent
- **THEN** it SHALL land at `body_radius + bottom_extent` (no launchpad offset)

## Landed State

### Requirement: Landed state behavior

While landed, the ship SHALL:
- Set rotation to the surface angle and rotational velocity to 0
- Position on the surface at `(body_radius + launchpad_offset + bottom_extent) * [cos(angle), sin(angle)]`
- If thrust acceleration exceeds surface gravity: transition to `Flying` with initial velocity `up_dir * net_accel * dt`
- If thrust is insufficient: remain landed with zero velocity

#### Scenario: Liftoff from surface
- **WHEN** the ship is landed and thrust acceleration > surface gravity
- **THEN** the ship state SHALL change to `Flying`, the ship SHALL have an initial upward velocity of `up_dir * net_accel * dt`, and the ship position SHALL be displaced upward by `0.5 * net_accel * dt^2` above the surface to prevent immediate re-collision with terrain

#### Scenario: Insufficient thrust on surface
- **WHEN** the ship is landed and thrust acceleration <= surface gravity
- **THEN** the ship SHALL remain `Landed` with zero velocity
