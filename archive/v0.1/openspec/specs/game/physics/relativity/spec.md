# Relativity

Threshold-based special and gravitational relativity overlays. Below 0.01c or away from compact objects, physics is unchanged. Delta-v calculation remains Newtonian.

## Special Relativity

### Requirement: Speed threshold

Relativistic effects SHALL activate only above 0.01c (~2,998 km/s). Below this threshold, `lorentz_gamma()` returns 1.0 and thrust is unmodified. At 0.01c the discontinuity is negligible (gamma = 1.000050, thrust reduction = 0.015%).

### Requirement: Relativistic thrust limiting

When speed exceeds the threshold, coordinate acceleration from thrust SHALL be decomposed into components parallel and perpendicular to velocity. Parallel (longitudinal) acceleration is divided by gamma cubed; perpendicular (transverse) by gamma. This follows from `F = d(gamma*m*v)/dt`. The effect is a natural speed cap — thrust effectiveness approaches zero as v approaches c.

### Requirement: Lorentz factor

`lorentz_gamma(speed)` SHALL return `1 / sqrt(1 - v^2/c^2)`, clamped to avoid division by zero via `max(1e-12)` on the denominator term.

### Requirement: Constants

- `SPEED_OF_LIGHT = 2.998e8` m/s
- `C_SQUARED = SPEED_OF_LIGHT^2`
- `RELATIVISTIC_SPEED_THRESHOLD = 0.01 * SPEED_OF_LIGHT`

## Gravitational Time Dilation

### Requirement: Compact object detection

`CelestialBody::is_compact()` SHALL return true when Schwarzschild radius exceeds 1% of body radius (`r_s / radius > 0.01`). This identifies black holes and neutron stars. Normal stars and planets return false.

### Requirement: Gravitational time factor

`gravitational_time_factor(gm, r, is_compact)` SHALL return `sqrt(1 - 2*GM/rc^2)` for compact bodies when `GM/rc^2 > 0.001`. Returns 1.0 for non-compact bodies or below threshold.

## Proper Time

### Requirement: Ship clock fields

Ship SHALL have `proper_time` and `mission_time` fields (f64 seconds, `#[serde(default)]` for backward compatibility). `mission_time` tracks coordinate/Earth time. `proper_time` ticks slower by `grav_factor / gamma`.

### Requirement: Time accumulation

Proper time SHALL accumulate once per frame in all three update paths:
- `update_flying()`: after substep loop, before temperature update
- `update_on_rails()`: after Keplerian state is derived, before SOI check
- `update_landed()`: at end of function (no velocity dilation, only gravitational)

Formula: `proper_time += dt * grav_factor / gamma` (flying/on-rails), `proper_time += dt * grav_factor` (landed).

### Requirement: Clock reset on launch

Both `proper_time` and `mission_time` SHALL reset to 0.0 when launching from the editor.

## HUD Display

### Requirement: Velocity in %c

When speed exceeds 1% c, velocity SHALL display as `X.XX% c` instead of km/s or m/s.

### Requirement: Gamma and split clocks

When `is_relativistic` or `grav_time_factor < 0.999`, the bottom info panel SHALL show: gamma value (purple), "Ship T+" proper time (blue), "Earth T+" mission time (orange).

### Requirement: Cruise velocity in staging panel

When total Newtonian delta-v maps to a relativistic cruise velocity above 0.5% c, the staging panel SHALL display the cruise velocity. Uses `v = c * tanh(dv/c)` conversion. Displayed below total delta-v in the staging panel.

## SOI Transitions

Frame conversions in `soi.rs` remain Galilean. Body orbital velocities are deeply Newtonian (~30 km/s max). The error from Galilean vs Lorentz velocity addition is negligible (order `v_body * v_ship / c^2`).

## Files

- `src/ship/mod.rs` — Constants, `lorentz_gamma()`, `gravitational_time_factor()`, `relativistic_cruise_velocity()`, Ship fields, relativistic thrust in `physics_substep()`, proper time accumulation
- `src/bodies.rs` — `CelestialBody::is_compact()`
- `src/render/types.rs` — Relativistic fields on `ShipRenderData`
- `src/main.rs` — Populate relativistic render data, clock reset on launch
- `src/render/state.rs` — HUD: %c velocity, gamma readout, split clocks, cruise velocity
