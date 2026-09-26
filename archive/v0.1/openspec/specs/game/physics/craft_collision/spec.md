# Craft Collision

Collision detection between the active vessel and inactive vessels (debris, jettisoned stages).

## Requirements

### Requirement: OBB collision during physics warp

During physics warp (time_warp <= RAILS_WARP_THRESHOLD, i.e. 1x through 10x), the system SHALL check for collisions between the active vessel and each inactive vessel in the same SOI.

Detection uses a two-phase approach:
1. **Broad phase**: Circle check using circumscribed radius (`sqrt(hw² + hh²)`) of each vessel. Skip if circles don't overlap.
2. **Narrow phase**: Oriented Bounding Box (OBB) overlap test using the Separating Axis Theorem (SAT). Each vessel's OBB is defined by its position, rotation, `bounding_half_width()`, and `bounding_half_height()`. The SAT tests four axes (two per OBB: local right and local up).

### Requirement: Collision response with separation and bounce

When a collision is detected:
1. **Separation**: The active vessel is pushed apart along the collision normal (center-to-center vector) by half the minimum overlap distance to prevent continued overlap.
2. **Bounce**: The relative velocity is reflected along the collision normal with a coefficient of restitution of 0.3 (mostly inelastic). The bounce only applies when the active vessel is moving toward the inactive vessel (relative velocity dot collision normal < 0).
3. Only the first collision per frame is processed (break after first hit).

### Requirement: On-rails exclusion

Collision detection is NOT performed during on-rails warp (time_warp > RAILS_WARP_THRESHOLD, i.e. 100x and above). On-rails vessels use Keplerian propagation and collision detection would be unreliable at high time steps.

## Implementation
- Collision check runs in `render_flight_frame()` after inactive vessel propagation
- Uses `vessel.bounding_half_width()` and `vessel.bounding_half_height()` for OBB half-extents
- Uses `ship.rotation` for OBB orientation
- Only checks vessels in the same SOI (`soi_body` match)
- `obb_overlap()` helper function at bottom of `main.rs` implements the SAT test
