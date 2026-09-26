# Ship Physics

Ship state machine, thrust, rotation, Velocity Verlet integration, collision detection, and autopilot.

## State Machine

### Requirement: Ship state machine

The ship SHALL have two states in the `ShipState` enum:
- `Flying`: Active physics simulation
- `Landed { body_index, surface_angle }`: Resting on a body's surface at a specific angle

### Requirement: Ship position and velocity frame

Ship `rel_position` and `rel_velocity` SHALL be stored RELATIVE to the current SOI body (identified by `soi_body` index). All position/velocity values use f64.

### Requirement: Absolute position calculation

Absolute position SHALL be computed as `soi_body_position + rel_position`.

### Requirement: Absolute velocity calculation

Absolute velocity SHALL be computed as `soi_body_velocity + rel_velocity`.

### Requirement: Default spawn in LEO

When spawning on Earth (body index 3), the ship SHALL be placed in Low Earth Orbit at 400 km altitude (ISS orbit). Circular orbital velocity SHALL be `sqrt(G * earth_mass / orbital_radius)`. The spawn angle SHALL be `PI/2` (top of orbit), with velocity perpendicular to position (prograde direction).

## Throttle

### Requirement: Throttle control

The throttle SHALL be a value in the range [0.0, 1.0], clamped after every update. Throttle behavior:
- Throttle up: `throttle += THROTTLE_RATE * dt` where `THROTTLE_RATE = 0.25` per second
- Throttle down: `throttle -= THROTTLE_RATE * dt`
- Full throttle: instantly set to 1.0
- Zero throttle: instantly set to 0.0
- On-rails mode forces throttle to 0.0

## Rotation

### Requirement: Rotation with acceleration model

Ship rotation SHALL use an acceleration-based model:
- RCS acceleration: `rcs_accel = rcs_torque / moment_of_inertia` (from vessel data), falling back to `ROTATION_ACCEL = 30 deg/s^2` (0.5236 rad/s^2). RCS thrusters consume monopropellant when providing torque.
- Gimbal acceleration: `gimbal_accel = gimbal_torque / moment_of_inertia` (always applied at 0.5x in physics, signed). Gimbal deflection accepts a fractional command in [-1, 1] for proportional control.
- Rotate left: `rotational_velocity += rcs_accel * dt`
- Rotate right: `rotational_velocity -= rcs_accel * dt`
- No input: apply rotation drag of `ROTATION_DRAG = 9 deg/s^2` (0.157 rad/s^2), reducing `rotational_velocity` toward zero
- Rotation angle updated: `rotation += rotational_velocity * dt`

#### Scenario: Rotation drag stops rotation
- **WHEN** no rotation input is active and `rotational_velocity` is positive but less than `ROTATION_DRAG * dt`
- **THEN** `rotational_velocity` SHALL be set to 0.0 (not go negative)

## RCS Translation

### Requirement: RCS translation controls

When RCS is enabled, WASD keys SHALL provide vessel-local translation:
- W: translate forward (vessel +Y)
- S: translate backward (vessel -Y)
- A: translate left (vessel -X)
- D: translate right (vessel +X)
- When RCS is disabled, WASD SHALL have no effect

### Requirement: RCS translation physics

The `Ship` SHALL store `rcs_translate: [f64; 2]` representing [forward, right] in vessel-local frame, each -1..1. RCS translation acceleration SHALL be computed in `physics_substep`:
- `accel_mag = rcs_translation_force / total_mass`
- Forward direction: `[cos(rotation), sin(rotation)]`
- Right direction: `[sin(rotation), -cos(rotation)]`
- Total RCS acceleration: `(forward * translate[0] + right * translate[1]) * accel_mag`

Translation SHALL only apply when `rcs_translation_force > 0` and `total_mass > 0`. The `rcs_translation_force` is the sum of `rcs_thrust` from all non-decoupled RCS thrusters that have monopropellant in their fuel zone.

### Requirement: RCS translation fuel consumption

Translation SHALL consume monopropellant proportionally to translation magnitude (capped at 1.0). Each RCS thruster drains `mass_flow_rate * magnitude * dt` from its fuel zone, using the same zone-based proportional drain as rotation fuel consumption.

## Vessel Physics Data

### Requirement: Vessel physics data bridge

The `VesselPhysicsData` struct SHALL bridge vessel data into physics: `total_mass` (tonnes), `max_thrust_vac` (kN), `max_thrust_asl` (kN), `vessel_height` (meters), `bottom_extent` (meters), `moment_of_inertia`, `rcs_torque` (kN*m from RCS thrusters), `gimbal_torque` (kN*m, signed), `vessel_half_width` (meters, for aerodynamic cross-section), and `rcs_translation_force` (kN total from all RCS thrusters for translation).

## Thrust

### Requirement: Thrust acceleration calculation

Thrust acceleration SHALL account for atmospheric pressure:
- Atmospheric pressure fraction SHALL be computed from the SOI body's atmosphere: `pressure_frac = (pressure_at_altitude(alt) / 101325.0).clamp(0.0, 1.0)`. Below the surface, pressure_frac = 1.0. No atmosphere = 0.0.
- Effective thrust SHALL interpolate: `thrust = max_thrust_vac * (1 - pressure_frac) + max_thrust_asl * pressure_frac`
- With vessel data: `thrust_accel = throttle * thrust / total_mass`
- Without vessel data (fallback): `thrust_accel = throttle * MAX_THRUST_ACCELERATION` where `MAX_THRUST_ACCELERATION = 20.0 m/s^2`
- Thrust direction follows ship rotation: `[cos(rotation) * mag, sin(rotation) * mag]`

#### Scenario: Sea-level thrust on Earth
- **WHEN** a vessel is on the surface of an atmospheric body
- **THEN** thrust SHALL equal `max_thrust_asl` (pressure_frac = 1.0)

#### Scenario: Vacuum thrust
- **WHEN** a vessel is above the atmosphere or around an airless body
- **THEN** thrust SHALL equal `max_thrust_vac` (pressure_frac = 0.0)

### Requirement: Landed thrust calculation

When the ship is landed, atmospheric pressure SHALL be assumed to be full surface pressure (pressure_frac = 1.0 if the body has an atmosphere, 0.0 otherwise). The same thrust interpolation formula SHALL apply.

## Integration

### Requirement: Velocity Verlet integration

Each physics substep SHALL use velocity Verlet integration:
1. Calculate gravity acceleration at current position: `a_grav = -G*M/r^2 * r_hat` (only if distance > body radius)
2. Total acceleration = gravity + thrust + RCS translation + drag
3. Update position: `pos += vel*dt + 0.5*accel*dt^2`
4. Recalculate gravity at new position
5. Update velocity: `vel += 0.5*(accel_old + accel_new)*dt`

#### Scenario: No gravity inside body
- **WHEN** the ship's distance from the SOI body center is less than the body's radius
- **THEN** gravity acceleration SHALL be `[0.0, 0.0]`

### Requirement: Physics sub-stepping

The flying physics update SHALL divide the timestep into substeps:
- Maximum physics timestep: `MAX_PHYSICS_DT = 0.01` seconds
- Number of substeps: `ceil(dt / MAX_PHYSICS_DT)`, clamped to range [1, 1000]
- Each substep duration: `dt / num_steps`

### Requirement: Effective time for thrusting at high warp

When the ship is thrusting and time warp exceeds `RAILS_WARP_THRESHOLD`, the effective dt SHALL be reduced: `effective_dt = dt / time_warp * RAILS_WARP_THRESHOLD`. This caps physics simulation speed while allowing time warp UI to show faster rates.

## On-Rails Mode

### Requirement: Below landing altitude check

`Ship::below_landing_altitude()` SHALL return `true` when the ship's altitude above the current SOI body is non-negative and less than the body's `landing_altitude()`. This works for both atmospheric and airless bodies.

### Requirement: Suborbital check

`Ship::is_suborbital()` SHALL return `true` when any of the following are true:
- The ship is in `Landed` state
- The ship's orbital periapsis (from cached orbit or computed from state vectors) is below the body's radius

For elliptical orbits, periapsis is `a * (1 - e)`. For hyperbolic orbits, periapsis is `|a| * (e - 1)`. If the orbit is parabolic (`1/a` near zero), the ship is considered suborbital.

### Requirement: On-rails mode entry criteria

The ship SHALL enter on-rails mode when ALL of the following are true:
- Time warp > `RAILS_WARP_THRESHOLD` (10.0)
- Ship state is `Flying`
- Ship is not actually thrusting (throttle > 0 AND engine thrust > 0)
- Ship is NOT below landing altitude of the current SOI body

#### Scenario: On-rails with throttle
- **WHEN** time warp is 1000x but throttle is 0.5 and engines have thrust
- **THEN** the ship SHALL NOT enter on-rails mode

#### Scenario: On-rails blocked below landing altitude
- **WHEN** time warp is 1000x and throttle is 0 but ship is below landing altitude
- **THEN** the ship SHALL NOT enter on-rails mode

### Requirement: Predictive warp drop before atmospheric entry

When the ship is **flying** and the current warp level exceeds `RAILS_WARP_THRESHOLD`, warp SHALL be reduced using predictive logic to prevent the ship from phasing through the atmosphere during on-rails propagation:

1. **Already in danger zone**: If altitude < `landing_altitude()`, drop warp to 1x immediately.
2. **Approaching landing altitude**: Compute `landing_r = body.radius + body.landing_altitude()` and use `Ship::time_to_distance(landing_r)` to find the time until the orbit descends to that distance. Find the highest warp level where `dt * warp * 10 <= time_to_distance` (10-frame safety buffer). If that level is lower than the current warp, clamp warp down to it. This smoothly steps warp down as the ship approaches the atmosphere, reaching 1x by the time it arrives.
3. **Safe orbit**: If `time_to_distance()` returns `None` (periapsis above landing altitude), no warp drop is needed.

`Ship::time_to_distance(target_distance)` computes time until the orbit reaches a given distance from the SOI body center. It uses the cached orbit if available, otherwise computes orbital elements from state vectors on-the-fly (preventing silent failure on the first frame of on-rails warp). It finds the two true anomalies where `r = target_distance` using the orbit equation, converts them to mean anomalies, and returns the minimum time to either crossing. Returns `None` for hyperbolic orbits or if periapsis >= target_distance.

Landed ships are exempt — the `update_landed` function is analytical (pins ship to surface via `surface_angle * body_radius`) and works correctly at any timestep, so landed ships MAY use any warp level including on-rails speeds (100x+).

#### Scenario: Landed ship at high warp
- **WHEN** the ship is in `Landed` state and warp is set to 100x or higher
- **THEN** warp SHALL NOT be reset — the ship stays on the surface, time advances at the selected rate

#### Scenario: Flying ship below landing altitude at high warp
- **WHEN** the ship is in `Flying` state and below landing altitude and warp > RAILS_WARP_THRESHOLD
- **THEN** warp SHALL be reset to 1x (warp index 0)

#### Scenario: Approaching atmosphere at high warp (ascending or descending)
- **WHEN** the ship is in `Flying` state, warp > RAILS_WARP_THRESHOLD, orbit periapsis is below landing altitude
- **THEN** warp SHALL be smoothly stepped down to the highest safe level where 10 frames won't overshoot the landing altitude crossing, ultimately reaching 1x as the ship nears the atmosphere

#### Scenario: High orbit with safe periapsis at high warp
- **WHEN** the ship is in `Flying` state, warp > RAILS_WARP_THRESHOLD, and periapsis is above `body.radius + body.landing_altitude()`
- **THEN** warp SHALL NOT be reset

### Requirement: On-rails mode propagation

While on-rails, the ship SHALL follow its cached Keplerian orbit exactly:
1. Advance mean anomaly: `M += direction * mean_motion * dt` (direction is -1 for retrograde, +1 for prograde)
2. Normalize mean anomaly to [0, 2pi)
3. Recompute `rel_position` and `rel_velocity` from the updated mean anomaly
4. Check for SOI transitions

### Requirement: On-rails entry and exit

When entering on-rails mode, the ship SHALL calculate and cache its current orbital elements. Only elliptical orbits (e < 1.0) can go on-rails; if orbit calculation fails, on-rails mode SHALL NOT be entered. When exiting on-rails mode, position and velocity SHALL be restored from the cached orbit at the current mean anomaly.

## Collision and Landing

See [Physics > Terrain Collision](../../physics/terrain_collision/spec.md) for collision detection with bodies, launchpad geometry, and landed state behavior.

## Autopilot

### Requirement: Autopilot target modes

The autopilot SHALL support the following `AutopilotTarget` modes:
- `Off`: No autopilot (default)
- `Prograde`: Point in velocity direction
- `Retrograde`: Point opposite to velocity (prograde + PI)
- `RadialOut`: Point perpendicular to velocity (prograde + PI/2)
- `RadialIn`: Point perpendicular to velocity (prograde - PI/2)
- `ManeuverNode`: Point in maneuver node delta-v direction

#### Scenario: Prograde target angle
- **WHEN** autopilot is set to Prograde and velocity is `[100.0, 0.0]`
- **THEN** the target angle SHALL be `atan2(0.0, 100.0) = 0.0`

#### Scenario: Low velocity ignores autopilot
- **WHEN** autopilot is set to Prograde and velocity magnitude < 0.1 m/s
- **THEN** the target angle SHALL be `None`

### Requirement: Autopilot rotation controller

The autopilot rotation controller SHALL use acceleration-based control with stopping distance braking:
1. Normalize angle difference to [-PI, PI]
2. If `|angle_diff| < 0.002 rad` (~0.1 degrees) and `|rotational_velocity| < 0.01 rad/s`: snap to target, zero rotational velocity
3. Calculate total acceleration: `total_accel = rcs_accel + gimbal_accel * 0.5` (the 0.5 matches the gimbal application factor in physics)
4. Calculate stopping distance: `s = v^2 / (2 * total_accel)`
5. Brake when going the right direction AND stopping distance >= 50% of remaining angle
6. Otherwise accelerate toward target

#### Scenario: Gimbal proportional control near target
- **WHEN** autopilot is accelerating toward the target angle (not braking)
- **THEN** the gimbal command SHALL be scaled by `(|angle_diff| / 0.15).clamp(0, 1)`, producing proportional deflection within ~8.6 degrees of the target
- **AND** when braking, full gimbal deflection is applied for maximum stopping power

#### Scenario: Gimbal command flow
- **GIVEN** autopilot is active with a target angle
- **THEN** `autopilot_desired_direction()` returns a value in [-1, 1] using combined RCS + gimbal torque for stopping distance
- **AND** engine gimbals deflect proportionally: `gimbal_angle = command * gimbal_range_rad`
- **AND** manual rotation (A/D keys) still commands full gimbal deflection (+1 or -1)
