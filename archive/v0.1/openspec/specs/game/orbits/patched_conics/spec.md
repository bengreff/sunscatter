# Patched Conics

Trajectory prediction across SOI boundaries, SOI transition detection, and frame conversion.

## Configuration

### Requirement: Maximum patched conic segments

The system SHALL predict a maximum of `MAX_PATCHED_CONICS = 3` SOI changes beyond the current orbit, producing up to 4 trajectory segments (e.g., Earth orbit → Sun transfer → Mars intercept → Mars orbit, or Earth orbit → Moon flyby → Earth return → Moon re-encounter).

### Requirement: Trajectory caching

Patched conic trajectories SHALL be cached and reused for `TRAJECTORY_CACHE_FRAMES = 30` frames (~0.5 seconds at 60fps). Cache SHALL be invalidated when:
- Ship is thrusting (throttle > 0)
- Ship is on-rails (position advances rapidly per frame; stale trajectory causes closest approach and other markers to rubber-band)
- SOI body has changed since last calculation
- Frame limit exceeded

### Requirement: Patched conic segment structure

Each `PatchedConicSegment` SHALL define: `orbit`, `parent_idx`, `retrograde`, `start_true_anomaly`, `end_true_anomaly` (None if orbit completes full loop), `start_time`, and `end_time` (None if no SOI exit).

## Hyperbolic SOI Exit

### Requirement: Hyperbolic orbit SOI exit prediction

For hyperbolic orbits, the exit true anomaly SHALL be calculated from the orbit equation:
1. Semi-latus rectum: `p = |a| * (e^2 - 1)`
2. `cos(nu_exit) = (p / soi_radius - 1) / e`
3. If `|cos(nu_exit)| <= 1`: `exit_ta = acos(cos(nu_exit))`; negate for retrograde
4. If `|cos(nu_exit)| > 1` (no intersection): use `exit_ta = acos(-1/e) - HYPERBOLIC_ANGLE_MARGIN`

### Requirement: Hyperbolic SOI exit continuation

After predicting a hyperbolic SOI exit, the system SHALL:
1. Calculate exit position and velocity from the orbit at the exit mean anomaly
2. Calculate transit time to exit for correct body position computation
3. Convert to parent frame via `convert_to_parent_frame()`
4. Calculate a new orbit in the parent body's frame
5. Continue the prediction loop with the new orbit (allowing further SOI transitions up to `MAX_PATCHED_CONICS`)

## Unified Trajectory Loop

### Requirement: Unified trajectory computation

Both live trajectory and maneuver node predictions SHALL use a shared `compute_patched_segments()` loop that:
1. Handles hyperbolic and elliptical orbits at each iteration
2. For hyperbolic orbits: calculates SOI exit analytically, pushes segment, continues to parent if crossings remain
3. For elliptical orbits at max crossings: pushes full orbit as final segment
4. For elliptical orbits below max: searches for SOI intersection, pushes truncated segment if found, converts frame, and continues
5. Terminates when: no SOI transition found, max crossings exhausted, orbit calculation fails, or no parent body exists

### Requirement: Galactic mass profile trajectory subdivision

For parent bodies with `galactic_mass_profile = true`, trajectory arcs SHALL be subdivided into 32 piecewise Keplerian sub-arcs to account for the varying enclosed mass M(r). At each sub-arc boundary, state vectors (position, velocity) are extracted from the current sub-orbit and orbital elements are recomputed using `effective_mass_at(r)` at the local distance. Each sub-arc is pushed as a separate `PatchedConicSegment` with the same `parent_idx`.

#### Hyperbolic subdivision
For hyperbolic orbits, each sub-arc advances by `1/remaining` of the true anomaly range from current position to the SOI exit TA (recomputed on each sub-orbit). If the orbit becomes elliptical during subdivision (mass decrease makes the orbit bound), a full-orbit segment is pushed and subdivision stops. After subdivision, the SOI exit continuation uses exit state from the final sub-orbit.

#### Elliptical subdivision (SOI intersection)
For elliptical orbits with an SOI intersection, the arc is subdivided using equal time steps (`intersect_time / 32`). At each step, mean anomaly is advanced by `n * dt_step` on the current sub-orbit, where n is the mean motion for the local enclosed mass.

#### Elliptical subdivision (full orbit)
For elliptical orbits without SOI intersection, one full orbital period is subdivided using equal time steps (`period_estimate / 32`). The resulting sub-arcs form a rosette pattern reflecting the non-Keplerian nature of galactic orbits.

#### SOI intersection mass for galactic profiles
`find_soi_intersection()` SHALL use `effective_mass_at(r)` at the ship's current orbital distance (not the semi-major axis) for period and mean motion calculations when the parent has `galactic_mass_profile`.

## Elliptical SOI Intersection

### Requirement: Elliptical orbit SOI intersection detection

For elliptical orbits, SOI intersections SHALL be found by:
1. Early exit: if orbit does not escape SOI and no child bodies have overlapping orbital ranges, return `None`
2. Child body filtering: only check children whose orbital range (periapsis - soi_radius to apoapsis + soi_radius) overlaps the ship's orbital range (periapsis to apoapsis)
3. Analytical fast path for SOI exit (no children): compute exit true anomaly from `cos(nu) = (p/soi_radius - 1) / e` where `p = a*(1 - e^2)`

### Requirement: SOI intersection sampling

When child bodies exist, the system SHALL use coarse sampling with `SOI_INTERSECTION_SAMPLES = 200` samples over one full orbit period:
1. For each sample, compute ship position from mean anomaly
2. Check if ship distance from parent exceeds `soi_radius * SOI_EXIT_THRESHOLD` (0.99) for SOI exit
3. Check if ship distance from any child body is within `child_soi_radius * SOI_ENTRY_THRESHOLD` (1.01) for SOI entry
4. Skip samples beyond the current best intersection time
5. Minimum time threshold: `MIN_INTERSECTION_TIME = 0.01` seconds

### Requirement: SOI intersection binary search refinement

Child body SOI entry intersections SHALL be refined with binary search:
- `SOI_REFINE_ITERATIONS = 10` iterations
- Search window: 2x the sample spacing around the approximate intersection
- Both ship and child body positions are evaluated at the midpoint time
- Converges on the boundary crossing point

## SOI Transitions During Physics

### Requirement: SOI transition during physics simulation (precise)

During physics substeps, SOI transitions SHALL be detected by comparing distances before and after the substep:
- SOI exit: `curr_dist > soi_radius AND prev_dist <= soi_radius` (distances relative to SOI body center)
- SOI entry (child): `curr_dist_to_child < child_soi_radius AND prev_dist_to_child >= child_soi_radius`

All distance calculations SHALL use consistent coordinate frames. For child SOI entry, ship position (`rel_position`, relative to SOI body) and child body position (`get_body_position_at_time`, relative to its parent = same SOI body) are both in the SOI-body-relative frame, so distances are computed directly as `ship_rel_pos - child_rel_pos`.

### Requirement: Precise SOI crossing interpolation

When a SOI transition is detected during physics, the exact crossing fraction SHALL be found via binary search with `BINARY_SEARCH_ITERATIONS = 50` iterations:
1. For SOI exit: interpolate between previous and current relative positions, find where distance equals SOI radius
2. For SOI entry: interpolate both ship position and child body position (both moving), find where relative distance equals child SOI radius
3. At the crossing point: convert position/velocity to new reference frame
4. Propagate remaining timestep `(1 - fraction) * dt` in the new frame

## Frame Conversion

### Requirement: SOI exit frame conversion

When exiting to a parent SOI, the conversion SHALL:
1. Compute current SOI body position and velocity at crossing time
2. Add SOI body position/velocity to ship relative position/velocity: `new_pos = rel_pos + soi_body_pos`, `new_vel = rel_vel + soi_body_vel`
3. Set `soi_body` to the parent index

### Requirement: SOI entry frame conversion

When entering a child SOI, the conversion SHALL:
1. Compute child body position and velocity at crossing time
2. Subtract child body position/velocity from ship state: `new_pos = pos - child_pos`, `new_vel = vel - child_vel`
3. Set `soi_body` to the child index

### Requirement: SOI transition on-rails handling (precise interpolation)

When on-rails and a SOI transition is detected, the system SHALL find the exact boundary crossing time within the current timestep using binary search (`BINARY_SEARCH_ITERATIONS = 50` iterations), then perform frame conversion at that precise moment:

1. **Compute previous mean anomaly**: `prev_M = current_M - direction * mean_motion * dt`
2. **Binary search**: interpolate mean anomaly between `prev_M` and `current_M`; at each sample, evaluate ship position from the orbit and (for entry) child body position at the interpolated time; converge on the fraction where distance equals SOI radius
3. **Frame conversion at crossing time**: compute ship state vectors from orbit at crossing mean anomaly; compute body position/velocity at crossing time; convert to new reference frame
4. **Propagate remaining time**: calculate new orbit in new frame; if elliptical, advance mean anomaly by `remaining_dt = (1 - fraction) * dt` and recompute position/velocity; if hyperbolic or invalid, linearly propagate position and fall off rails

## Maneuver Node Placement

### Requirement: Maneuver nodes on any visible trajectory segment

Maneuver nodes SHALL be placeable on any visible orbit line, not just the current orbit (segment 0). Clicking near any rendered trajectory segment — including later patched conic segments after SOI transitions and predicted trajectories from existing maneuver nodes — SHALL open the "Create Maneuver Node" popup.

The orbit click detection SHALL search:
1. All segments of the current trajectory (`current_trajectory`)
2. All segments of all predicted trajectories (`predicted_trajectories`, one per existing maneuver node)

The closest match across all trajectories wins. The clicked segment's orbital parameters are stored directly with the pending click, so node creation does not depend on segment indices.

#### Scenario: Maneuver node on post-SOI segment
- **GIVEN** a trajectory showing Earth orbit → Moon flyby → Earth return
- **WHEN** the user clicks on the Earth return segment
- **THEN** a maneuver node SHALL be created on that segment with the correct orbital parameters and epoch

#### Scenario: Maneuver node on predicted trajectory
- **GIVEN** an existing maneuver node with delta-v that produces a green predicted trajectory
- **WHEN** the user clicks on the predicted trajectory line
- **THEN** a second maneuver node SHALL be created on that predicted orbit segment

## Maneuver Node Prediction

### Requirement: Predicted trajectory for maneuver nodes

The system SHALL support calculating a predicted trajectory from arbitrary state vectors (`pos`, `vel`, `parent_idx`, `epoch`) for maneuver node predictions. This uses the shared `compute_patched_segments()` helper with the same patched conics logic as the live trajectory, returning a `PatchedTrajectory` with up to `MAX_PATCHED_CONICS + 1` segments.

#### Scenario: Epoch-aware body positions
- The `epoch` parameter specifies when the maneuver occurs (absolute simulation time)
- SOI frame conversions SHALL use `epoch + transit_time` for body positions, NOT current `solar_system.time`
- For hyperbolic escape: transit time = time from burn point to SOI exit on the escape hyperbola
- For elliptical SOI intersections: transit time = `intersect_time` from SOI intersection finder
- This ensures interplanetary trajectory predictions show the correct heliocentric orbit based on where bodies will be when the maneuver actually happens

#### Scenario: Maneuver node predicts SOI escape
- **WHEN** a maneuver node creates a hyperbolic orbit relative to the current body
- **THEN** the predicted trajectory SHALL show the escape segment ending at the SOI boundary and continuation segments in parent frames (up to MAX_PATCHED_CONICS transitions)

#### Scenario: Maneuver node predicts Moon encounter
- **WHEN** a maneuver node creates an orbit that intersects the Moon's SOI
- **THEN** the predicted trajectory SHALL show the current orbit ending at the Moon's SOI boundary, the Moon flyby segment, and the return orbit around Earth

#### Scenario: Interplanetary transfer prediction
- **WHEN** a maneuver from LEO creates an escape trajectory from Earth
- **THEN** the predicted trajectory SHALL show: (1) the Earth escape hyperbola, (2) the heliocentric transfer orbit with intercept at the target planet, and (3) the hyperbolic approach at the target planet

## Closest Approach Indicator

### Requirement: Closest approach computation

When a navigation target is selected (body or vessel), the system SHALL compute and display the closest approach point on the trajectory segment that shares the same SOI as the target.

#### Visibility conditions
Closest approach markers SHALL NOT be displayed when:
- The ship is already in the target body's SOI (soi_body == target_idx)
- The trajectory intercepts the target body's SOI (any segment has parent_idx == target_idx)
- The matching trajectory segment is hyperbolic (eccentricity >= 1.0)

#### Scenario: SOI matching for body targets
- **GIVEN** a body target with index `target_idx`
- **THEN** the system SHALL find the first trajectory segment where `segment.parent_idx == bodies[target_idx].parent`
- **AND** if the target body's parent is `None` (Sun), no closest approach is computed

#### Scenario: SOI matching for vessel targets
- **GIVEN** a vessel target with id `target_id`
- **THEN** the system SHALL find the first trajectory segment where `segment.parent_idx == target_vessel.ship.soi_body`

#### Scenario: No matching segment
- **WHEN** no trajectory segment shares the target's SOI
- **THEN** no closest approach marker is displayed

### Requirement: Closest approach sampling algorithm

The closest approach point SHALL be found by:
1. **Coarse sampling**: 64 uniformly-spaced samples across the segment's true anomaly arc
2. For each sample, compute the ship's position on the orbit and the target's position at the corresponding absolute time (`simulation_time + segment.start_time + travel_time`)
3. Travel time is derived from mean anomaly difference divided by mean motion
4. Track the minimum distance sample
5. **Golden-section refinement**: 12 iterations around the best coarse sample for sub-sample accuracy

### Requirement: Closest approach rendering

Two closest approach markers SHALL be displayed:
1. **Ship marker**: Yellow (`[1.0, 1.0, 0.0, 0.9]`) filled circle at the ship's position at closest approach time
2. **Target marker**: Yellow (`[1.0, 1.0, 0.0, 0.9]`) filled circle at the target's position at closest approach time

Both markers use a 16-segment triangle fan, same size as Ap/Pe markers. Both are labeled "CA" in yellow text above the dot (11pt). On hover (20px radius): display the distance using standard altitude formatting (matching AP/PE style). Both markers show the same distance value.

#### Rendering precision
Closest approach world positions SHALL be stored as (parent_render_pos, orbit_offset, distance) with the parent position computed from the same frame's scaled_positions as the camera reference. The CA computation SHALL execute before the rendering call to ensure frame-consistent precision. The rendering SHALL use two-step subtraction: (parent - body_center) + (orbit_offset - ship_offset).
