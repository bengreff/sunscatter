# Atmosphere Model

Atmospheric density and pressure profiles for celestial bodies, and body rotation for computing surface-relative airspeed.

## Pressure and Density

### Requirement: Pressure at altitude

Atmospheric pressure at a given altitude SHALL be computed as `pressure = surface_pressure * exp(-altitude / scale_height)`.

### Requirement: Surface density

Surface air density SHALL be computed using the ideal gas law with fixed temperature T=250K and specific gas constant R=287 J/(kg*K) for N2/O2 mix: `surface_density = surface_pressure / (287.0 * 250.0)`.

### Requirement: Density at altitude

Atmospheric density at a given altitude SHALL be computed as `density = surface_density * exp(-altitude / scale_height)`.

### Requirement: Atmosphere cutoff

Atmospheric effects (drag, heating) SHALL be skipped when altitude is negative or exceeds `visible_height()` (5 scale heights).

## Body Rotation

### Requirement: No planetary rotation

All celestial bodies SHALL have `sidereal_period = None`. Bodies do not rotate and maintain their orientation in space. Airspeed equals velocity relative to the body center (no surface velocity subtraction).

### Requirement: Sidereal period field

Each `CelestialBody` SHALL have an optional `sidereal_period: Option<f64>` field (seconds) to support future rotation if needed. Currently all bodies use `None`.

### Requirement: Surface velocity calculation

Surface rotational velocity at a given distance from body center SHALL be computed as `v = TAU * distance / period`. If `sidereal_period` is `None` or zero, surface velocity SHALL be 0.0.
