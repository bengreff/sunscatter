# Orbital Mechanics

State vector to orbital element conversion, anomaly handling, and orbital info display calculations.

## State Vector Conversion

### Requirement: State vectors to orbital elements

Conversion from state vectors `[position, velocity]` to orbital elements SHALL compute:
1. Specific orbital energy: `energy = v^2/2 - mu/r`
2. Semi-major axis: `a = -mu / (2 * energy)` (negative for hyperbolic)
3. Specific angular momentum: `h = rx*vy - ry*vx` (negative = retrograde)
4. Eccentricity vector: `ex = (vy*h)/mu - rx/r`, `ey = -(vx*h)/mu - ry/r`
5. Eccentricity magnitude: `e = sqrt(ex^2 + ey^2)`
6. Argument of periapsis: `omega = atan2(ey, ex)`
7. True anomaly: `nu = (atan2(ry, rx) - omega)` normalized by orbit type

#### Scenario: Minimum radius guard
- **WHEN** position radius `r < 1.0` meter
- **THEN** orbit calculation SHALL return `None`

#### Scenario: Invalid orbital elements
- **WHEN** semi-major axis or eccentricity is not finite, or eccentricity < 0
- **THEN** orbit calculation SHALL return `None`

## True Anomaly Handling

### Requirement: True anomaly normalization for elliptical orbits

For elliptical orbits (e < 1.0), true anomaly SHALL be normalized to [0, 2pi) using `rem_euclid(TAU)`.

### Requirement: True anomaly handling for hyperbolic orbits

For hyperbolic orbits (e >= 1.0), true anomaly SHALL be:
1. Maximum valid true anomaly: `max_ta = acos(-1/e)`
2. Normalized to [-PI, PI] range
3. Sign determined by radial velocity: for prograde, outgoing (receding, positive radial velocity) has positive ta; for retrograde, outgoing has negative ta
4. Clamped to `(-max_ta + HYPERBOLIC_ANGLE_MARGIN, max_ta - HYPERBOLIC_ANGLE_MARGIN)` where `HYPERBOLIC_ANGLE_MARGIN = 0.01`

### Requirement: On-rails orbit caching (elliptical only)

Only elliptical orbits (e < 1.0) SHALL be cached in `ShipOrbit` for on-rails mode. Hyperbolic orbits SHALL return `None` from `calculate_orbit_with_anomaly()`.

## Mean Anomaly Conversion

### Requirement: True anomaly to mean anomaly conversion (elliptical)

For elliptical orbits, conversion SHALL be:
1. `E = atan2(sin(nu) * sqrt(1 - e^2), e + cos(nu))`
2. `M = (E - e*sin(E)) mod 2pi`

### Requirement: True anomaly to mean anomaly conversion (hyperbolic)

For hyperbolic orbits, conversion SHALL be:
1. `tanh(H/2) = tan(nu/2) * sqrt((e-1)/(e+1))`
2. Clamp `tanh(H/2)` to `[-0.99999, 0.99999]` for numerical safety
3. `H = 2 * atanh(tanh(H/2))`
4. `M = e*sinh(H) - H`

## Orbital Info

### Requirement: Orbital info for UI display

The `OrbitalInfo` struct SHALL provide:
- Apoapsis: `a * (1 + e)`
- Periapsis: `a * (1 - e)`
- Orbital period: `T = 2*pi * sqrt(a^3 / mu)`
- Time to apoapsis: time to reach mean anomaly PI
- Time to periapsis: time to reach mean anomaly 0
- Time-to-anomaly calculation accounts for retrograde orbits (negating delta_M)
