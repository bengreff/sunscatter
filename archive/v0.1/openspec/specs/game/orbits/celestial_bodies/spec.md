# Celestial Bodies

Solar system body definitions, gravitational constants, Keplerian orbital elements, and Kepler equation solvers.

## Constants

### Requirement: Gravitational constant

The system SHALL define the gravitational constant as `G = 6.67430e-11` m^3/(kg*s^2).

### Requirement: Physics scale factor

The system SHALL define `PHYSICS_SCALE = 1.0`, meaning all values match real life exactly. Body masses are scaled by `PHYSICS_SCALE^2`, radii and distances by `PHYSICS_SCALE`.

### Requirement: Light-year constant

The system SHALL define `LIGHT_YEAR = 9.461e15` meters.

## Body Properties

### Requirement: Celestial body properties

Each `CelestialBody` SHALL have the following properties: `name` (String), `description` (String, one-sentence description of the body), `mass` (kg, f64 — the physical mass of the object itself), `radius` (meters, f64), `color` (RGBA [f32; 4]), `parent` (Optional index of parent body, None for root), `orbit` (Optional Keplerian orbital elements, None for root), `soi_radius` (meters, f64), `atmosphere` (Optional atmospheric data), `sidereal_period` (Optional rotation period in seconds, None for tidally locked or negligible rotation), `accretion_disc` (Optional accretion disc data for black holes), and `galactic_mass_profile` (bool, true only for the galactic center body).

### Requirement: Surface gravity calculation

Surface gravity SHALL be calculated as `g = G * mass / radius^2`.

### Requirement: Galactic mass profile

Bodies with `galactic_mass_profile = true` (Sgr A*) SHALL use a 4-component enclosed mass model `M(r)` for gravitational calculations instead of the body's stored mass:

**M(r) = M_bh + M_bulge(r) + M_disk(r) + M_halo(r)**

| Component | Formula | Parameters |
|-----------|---------|-----------|
| SMBH | M_bh = 8.26e36 kg (constant) | Point mass (~4.15M solar) |
| Bulge | M_b × r²/(r+a)² (Hernquist) | M_b = 2.0e40 kg, a = 7.0e18 m (~740 ly) |
| Disk | M_d × [1-(1+r/R_d)e^(-r/R_d)] (exponential) | M_d = 9.0e40 kg, R_d = 8.5e19 m (~9000 ly) |
| DM Halo | M_0 × [ln(1+x)-x/(1+x)], x=r/r_s (NFW) | M_0 = 1.555e42 kg, r_s = 5.7e20 m (~60,000 ly) |

The `effective_mass_at(distance)` method SHALL return `galactic_enclosed_mass(distance)` when `galactic_mass_profile` is true, and `self.mass` otherwise. All orbital mechanics calculations (gravity, orbit determination, mean motion, vis-viva) SHALL use `effective_mass_at()` for the parent body's gravitational mass.

The stored `mass` field of Sgr A* SHALL be the actual SMBH mass (8.26e36 kg), used for surface gravity and display. At the Sun's orbital distance (2.46e20 m), M(r) ≈ 1.784e41 kg, preserving existing orbital dynamics.

### Requirement: Sphere of influence calculation

SOI radius SHALL be calculated using the formula `soi = semi_major_axis * (mass / parent_mass)^0.4`. The galactic center root body's SOI radius SHALL be `f64::INFINITY`. Stars orbiting the galactic center use the standard SOI formula with the parent's `effective_mass_at(semi_major_axis)` as `parent_mass`.

### Requirement: Atmosphere data

Bodies with atmospheres SHALL define `Atmosphere` with: `surface_pressure` (Pascals), `scale_height` (meters), and `color` (RGB [f32; 3]). The visible atmosphere height SHALL be `scale_height * (100_000 / 8_500)` (~100 km for Earth-like scale heights).

### Requirement: Accretion disc data

Bodies with accretion discs (e.g., supermassive black holes) SHALL define `AccretionDisc` with: `inner_radius` (meters, f64), `outer_radius` (meters, f64), `color_inner` (RGB [f32; 3]), and `color_outer` (RGB [f32; 3]). For Sgr A*, the inner radius equals the event horizon (body radius), the outer radius is 30× the event horizon, `color_inner` is warm orange `[1.0, 0.6, 0.25]`, and `color_outer` is deep red `[0.8, 0.08, 0.02]`.

The disc is rendered as 16 concentric ring strips. Colors interpolate from inner to outer using a `t^1.5` curve to produce an orange→red transition. A brightness multiplier `exp(-6*t³)` is applied to both RGB and alpha, fading the outer edge to the same near-black level as atmosphere edges (`exp(-6) ≈ 0.0025`). The body itself is rendered as a pure black circle (the event horizon) on top of the disc.

### Requirement: Landing altitude

Each `CelestialBody` SHALL provide a `landing_altitude()` that returns the altitude below which warp is restricted and on-rails mode is blocked:
- For atmospheric bodies: the atmosphere `visible_height()`
- For airless bodies: `radius * 0.01` (1% of body radius)

#### Scenario: Earth landing altitude
- **WHEN** Earth has an atmosphere with scale_height 8,500 m
- **THEN** `landing_altitude()` SHALL return approximately 100,000 m

#### Scenario: Moon landing altitude
- **WHEN** the Moon has no atmosphere and radius 1,737,000 m
- **THEN** `landing_altitude()` SHALL return approximately 17,370 m (1% of radius)

## Solar System

### Requirement: Solar system body hierarchy

The solar system SHALL contain the following bodies as a hierarchy indexed from 0. Sagittarius A* is the galactic center root body with `galactic_mass_profile = true`. Its stored mass (8.26e36 kg) is the actual SMBH mass; gravitational calculations use the enclosed galactic mass M(r) from the 4-component model, producing the correct Sun orbital velocity of ~220 km/s at 26,000 ly distance. The Sun's `mean_anomaly_at_epoch` is `3π/2` so it spawns at the bottom (-y) of its galactic orbit.

| Index | Body             | Parent             | Mass (kg)    | Radius (m) | SMA (m)     | Eccentricity |
|-------|------------------|--------------------|-------------|------------|-------------|-------------|
| 0     | Sagittarius A*   | None (root)        | 8.26e36     | 1.2e10     | N/A         | N/A         |
| 1     | Sun              | Sgr A* (0)         | 1.989e30    | 6.96e8     | 2.46e20     | 0.07        |
| 2     | Mercury          | Sun (1)            | 3.301e23    | 2.44e6     | 5.79e10     | 0.2056      |
| 3     | Venus            | Sun (1)            | 4.867e24    | 6.052e6    | 1.082e11    | 0.0068      |
| 4     | Earth            | Sun (1)            | 5.972e24    | 6.371e6    | 1.496e11    | 0.0167      |
| 5     | Moon             | Earth (4)          | 7.342e22    | 1.737e6    | 3.844e8     | 0.0549      |
| 6     | Mars             | Sun (1)            | 6.417e23    | 3.39e6     | 2.279e11    | 0.0934      |
| 7     | Phobos           | Mars (6)           | 1.066e16    | 1.127e4    | 9.376e6     | 0.0151      |
| 8     | Deimos           | Mars (6)           | 1.476e15    | 6.2e3      | 2.346e7     | 0.0002      |
| 9     | Jupiter          | Sun (1)            | 1.898e27    | 6.991e7    | 7.785e11    | 0.0489      |
| 10    | Io               | Jupiter (9)        | 8.932e22    | 1.822e6    | 4.218e8     | 0.0041      |
| 11    | Europa           | Jupiter (9)        | 4.800e22    | 1.561e6    | 6.711e8     | 0.0094      |
| 12    | Ganymede         | Jupiter (9)        | 1.482e23    | 2.634e6    | 1.070e9     | 0.0013      |
| 13    | Callisto         | Jupiter (9)        | 1.076e23    | 2.410e6    | 1.883e9     | 0.0074      |
| 14    | Saturn           | Sun (1)            | 5.683e26    | 5.823e7    | 1.432e12    | 0.0565      |
| 15    | Titan            | Saturn (14)        | 1.345e23    | 2.575e6    | 1.222e9     | 0.0288      |
| 16    | Rhea             | Saturn (14)        | 2.307e21    | 7.64e5     | 5.27e8      | 0.001       |
| 17    | Iapetus          | Saturn (14)        | 1.806e21    | 7.36e5     | 3.56e9      | 0.0283      |
| 18    | Dione            | Saturn (14)        | 1.095e21    | 5.62e5     | 3.774e8     | 0.0022      |
| 19    | Uranus           | Sun (1)            | 8.681e25    | 2.536e7    | 2.867e12    | 0.0457      |
| 20    | Neptune          | Sun (1)            | 1.024e26    | 2.462e7    | 4.515e12    | 0.0113      |

## Orbital Elements

### Requirement: Keplerian orbital elements (2D)

Each body's `Orbit` SHALL define: `semi_major_axis` (meters), `eccentricity` (0 = circle, <1 = ellipse, >=1 = hyperbola), `argument_of_periapsis` (radians), and `mean_anomaly_at_epoch` (radians at t=0).

### Requirement: Mean anomaly propagation

Mean anomaly at time `t` SHALL be calculated as `M = M0 + n * t`, where `n = sqrt(mu / a^3)` is the mean motion and `mu = G * parent_mass`.

## Kepler Equation Solvers

### Requirement: Kepler equation solver for elliptical orbits

For elliptical orbits (e < 1.0), the Kepler equation `M = E - e*sin(E)` SHALL be solved using Newton-Raphson iteration with:
- Mean anomaly normalized to [0, 2pi) via `rem_euclid(TAU)`
- Initial guess `E = M`
- Update: `E -= (E - e*sin(E) - M) / (1 - e*cos(E))`
- Maximum 20 iterations
- Convergence threshold `|delta| < 1e-12`

#### Scenario: Circular orbit Kepler solution
- **WHEN** eccentricity is 0.0 and mean anomaly is 1.5
- **THEN** the eccentric anomaly SHALL equal the mean anomaly (1.5)

#### Scenario: Eccentric orbit Kepler convergence
- **WHEN** eccentricity is 0.5 and mean anomaly is pi
- **THEN** the solver SHALL converge within 20 iterations to precision < 1e-12

### Requirement: Kepler equation solver for hyperbolic orbits

For hyperbolic orbits (e >= 1.0), the equation `M = e*sinh(H) - H` SHALL be solved using Newton-Raphson iteration with:
- Initial guess: `H = M` if `|M| < 1.0`, else `H = sign(M) * asinh(|M| / e)`
- Update: `H -= (e*sinh(H) - H - M) / (e*cosh(H) - 1)`
- Maximum 30 iterations
- Convergence threshold `|delta| < 1e-12`
- Early exit if `|f_prime| < 1e-12`

## Anomaly Conversions

### Requirement: True anomaly from eccentric anomaly (elliptical)

For elliptical orbits, true anomaly SHALL be computed as `nu = 2 * atan2(sqrt(1+e) * tan(E/2), sqrt(1-e))`.

### Requirement: True anomaly from hyperbolic anomaly

For hyperbolic orbits, true anomaly SHALL be computed as `nu = 2 * atan(sqrt((e+1)/(e-1)) * tanh(H/2))`.

## Position and Velocity

### Requirement: Position from orbital elements

Position relative to parent SHALL be calculated as:
1. Compute distance `r`:
   - Elliptical: `r = a * (1 - e*cos(E))`
   - Hyperbolic: `r = |a| * (e*cosh(H) - 1)`
2. Compute angle: `angle = true_anomaly + argument_of_periapsis`
3. Position: `[r*cos(angle), r*sin(angle)]`

### Requirement: Velocity from orbital elements using vis-viva

Velocity magnitude SHALL be calculated from the vis-viva equation: `v = sqrt(mu * (2/r - 1/a))`. This works for both elliptical (a > 0) and hyperbolic (a < 0) orbits. If `v^2 <= 0`, velocity SHALL be `[0, 0]`.

### Requirement: Velocity direction from flight path angle

Velocity direction SHALL be computed using the flight path angle:
1. `gamma = atan2(e*sin(nu), 1 + e*cos(nu))`
2. `radius_angle = true_anomaly + argument_of_periapsis`
3. For prograde: `velocity_angle = radius_angle + PI/2 - gamma`
4. For retrograde: `velocity_angle = radius_angle - PI/2 - gamma`
5. Velocity: `[v*cos(velocity_angle), v*sin(velocity_angle)]`

## Body Position Calculation

### Requirement: Body position calculation (recursive)

A body's world position SHALL be computed recursively: for a body with a parent, add the body's relative orbital position to its parent's world position. The root body (Sagittarius A*) SHALL be at `[0.0, 0.0]`.

### Requirement: Body velocity calculation (recursive)

Body velocity in absolute coordinates SHALL be computed recursively:
1. For a body with a parent: compute velocity magnitude from vis-viva equation, direction perpendicular to radius vector
2. Add parent's absolute velocity (recursive call)
3. Root body velocity is `[0.0, 0.0]`

### Requirement: Body position/velocity at future time

The system SHALL support computing body position and velocity at any time `t` relative to the body's parent, using the orbital elements' `position_at(t)` and `velocity_from_mean_anomaly(M_at_t)` methods.

### Requirement: Solar system time advancement

The solar system SHALL maintain a `time` field (seconds, f64) that advances by `dt` on each `update()` call.

## Data Loading

### Requirement: RON-based body definitions

Body data SHALL be loaded from `data/bodies/solar_system.ron` at startup. The file SHALL contain all body definitions with raw real-world values (no PHYSICS_SCALE applied). Each body definition SHALL include: `name`, `description`, `mass` (kg), `radius` (m), `color` (RGBA), `parent` (Optional body name string), `orbit` (Optional orbital elements), `atmosphere` (Optional atmospheric data), `sidereal_period` (Optional), `accretion_disc` (Optional, with inner/outer radii and inner/outer colors), and `galactic_mass_profile` (Optional bool, defaults to false).

### Requirement: Parent name resolution

Parent references in the RON file SHALL use body name strings (e.g., `parent: Some("Earth")`). During loading, names SHALL be resolved to body indices via a name-to-index HashMap. Bodies MUST be listed in order so that parents always precede their children.

### Requirement: Physics scale application

During loading, `PHYSICS_SCALE` SHALL be applied: masses multiplied by `PHYSICS_SCALE^2`, radii and semi-major axes by `PHYSICS_SCALE`. SOI radius SHALL be computed from the scaled values using `calculate_soi()`.

### Requirement: Hardcoded fallback

If the RON file fails to load (missing file or parse error), the system SHALL fall back to the hardcoded body definitions with a log warning. The hardcoded fallback includes descriptions for all 22 bodies (Sgr A* through Neptune, plus Crucible).

## Galaxy Background Image

### Requirement: Galaxy texture

The galaxy background SHALL be rendered from the NASA/Spitzer Milky Way face-on artist concept image (`data/textures/milky_way.jpg`, 5600x5600 source). The image is loaded at startup as an additional layer in the body texture array (resized to 1024x1024 to match the array format). The texture layer index is stored in `BodyTextureMap.galaxy_layer`.

### Requirement: Galaxy rendering

In galaxy view, a single textured quad spanning 100,000 light-years (centered on Sgr A*) SHALL be rendered using the galaxy texture layer. The quad is rendered after the accretion disc and before orbit lines. UV coordinates map the full image to the quad. The galaxy image is static — it does not orbit or move.
