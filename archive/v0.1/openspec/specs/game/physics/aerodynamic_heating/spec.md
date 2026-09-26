# Aerodynamic Heating

Convective aerodynamic heating, radiative cooling, per-part thermal destruction, vessel splitting, heat shields, and heat visualization.

## Per-Part Temperature Model

### Requirement: Per-part thermal fields on PartDefinition

`PartDefinition` SHALL have the following fields with serde defaults:
- `max_heat_tolerance: f64` — default 1000.0 K
- `specific_heat: f64` — default 900.0 J/(kg*K)
- `emissivity: f64` — default 0.8
- `is_heat_shield: bool` — default false (for rendering dispatch)

### Requirement: Per-part thermal fields on FlightPart

`FlightPart` SHALL have:
- `temperature: f64` — initialized to 300.0 K in `from_blueprint()`
- `max_heat_tolerance: f64` — initialized from `PartDefinition.max_heat_tolerance`

### Requirement: Ship temperature field (fallback)

The `Ship` struct SHALL have a `temperature: f64` field (Kelvin) and a `heat_flux: f64` field (W/m^2). Initial/ambient temperature SHALL be `AMBIENT_TEMPERATURE = 300.0` K. This is used when no `FlightVessel` is present.

### Requirement: Temperature constants

The system SHALL define the following constants:
- `STEFAN_BOLTZMANN = 5.670374419e-8` W/(m^2*K^4)
- `SUTTON_GRAVES_K = 1.7415e-4` (Sutton-Graves convective heating constant for N₂/O₂ atmosphere, used in both ship-level and per-part heating)
- `SKIN_THERMAL_MASS_PER_METER = 10.0` kg/m — reference thermal mass per meter of exposed width, modeling the thin skin layer that absorbs convective heat
- `default_heat_tolerance() = 1000.0` K (used as fallback for ship-level heat fraction display)
- `AMBIENT_TEMPERATURE = 300.0` K

## Heat Shields

### Requirement: Heat shield part definitions

Four heat shield parts SHALL exist in `data/parts/aerodynamic.ron` under the `Aerodynamic` category:
- Tiny (HS-1): 1x0.5 grid, 0.02t, hitbox_height 1
- Small (HS-3): 3x0.5 grid, 0.08t, hitbox_height 1
- Medium (HS-5): 5x0.5 grid, 0.15t, hitbox_height 1
- Large (HS-9): 9x0.5 grid, 0.3t, hitbox_height 1

All heat shields SHALL have:
- `max_heat_tolerance: 4000.0` K
- `specific_heat: 1600.0` J/(kg*K)
- `emissivity: 0.95`
- `is_heat_shield: true`

### Requirement: Heat shield rendering

Heat shields SHALL be rendered with `generate_heat_shield_details()`:
- Near-black ablative face (bottom 60%) with colors `[0.05, 0.05, 0.05, 1.0]` — convex dome shape with curved bottom edge bulging downward (8 segments, sag = 30% of visual height)
- Dark backing structure (top 40%) with colors `[0.12, 0.12, 0.12, 1.0]` — flat rectangle
- Drawn on the upper half of the hitbox (shield_top = y + hitbox_half_h)
- Dispatched before decouplers in `generate_part_vertices()`, `generate_single_ghost_vertices()`, and `generate_part_shape_vertices()`

## Per-Part Heating

### Requirement: Aero environment extraction

`Ship::compute_aero_environment()` SHALL return `Option<(density, airspeed, airspeed_dir_world)>` — the atmospheric density, surface-relative airspeed, and airspeed direction vector. Returns `None` if not in atmosphere or density < 1e-15 or airspeed < 1.0.

### Requirement: Ship-level heating conditional on vessel

`Ship::update_temperature()` SHALL skip all processing when a `FlightVessel` exists (per-part system handles it). The ship-level model is kept as a fallback for no-vessel mode.

### Requirement: Airspeed direction coordinate transform

The world-space airspeed direction SHALL be transformed to **part-local coordinates** (Y=forward convention, matching the editor layout where nose is at +Y). Ship physics uses X=forward (rotation=0 → nose along +X), so the transform applies the inverse rotation with a −π/2 offset: compute physics-local via inverse rotation, then rotate +90° (`part_x = -phys_y, part_y = phys_x`).

### Requirement: Fairing airflow shielding

Before computing per-part exposure, `FlightVessel::update_part_temperatures()` SHALL identify all parts inside a non-decoupled fairing envelope. For each non-destroyed, non-decoupled fairing base with a non-empty `fairing_shape`:
1. Build a segment list from the shape vertices, converting grid-square offsets to world coordinates relative to the fairing base top edge
2. For each other part, check if it falls entirely within the fairing envelope: part bottom must be at or above the base top, part top must be at or below the tip Y, and the part's half-width plus its lateral offset from the fairing center must fit within the interpolated envelope half-width at the part's center Y
3. Parts passing all checks are added to a `fairing_shielded` set

Fairing-shielded parts SHALL have their exposed area set to 0.0, receiving zero aerodynamic heating. They still undergo radiative cooling.

#### Scenario: Part inside fairing gets no heating
- **WHEN** a part's hitbox fits entirely within an active fairing envelope
- **THEN** its exposed area SHALL be 0.0 and it receives no convective heating

#### Scenario: Fairing decoupled removes shielding
- **WHEN** a fairing base is decoupled (jettisoned via staging)
- **THEN** parts previously inside the envelope are no longer shielded and receive normal heating

### Requirement: Per-part exposure calculation (1D interval occlusion)

`FlightVessel::update_part_temperatures()` SHALL:
1. Project each part onto the velocity axis (in part-local coordinates, Y=forward)
2. Sort parts by projection (most forward = most positive first)
3. For each part, compute the perpendicular cross-section interval [min, max]
4. Track a set of occluded perpendicular intervals (sorted, non-overlapping)
5. Calculate the exposed width as the part's interval minus any overlapping occluded intervals
6. After processing each part, add its interval to the occluded set

### Requirement: Per-part heat input (Sutton-Graves)

Heat flux SHALL use the Sutton-Graves convective heating correlation: `q_flux = K * sqrt(ρ) * V³`, where `K = 1.7415e-4` (the Sutton-Graves constant for N₂/O₂ atmosphere). Total heat input per part: `q_in = SUTTON_GRAVES_K * sqrt(density) * airspeed^3 * exposed_width`. The exposed width is the part's perpendicular cross-section minus any occluded intervals (no depth factor).

### Requirement: Per-part radiative cooling (net radiation)

Heat output per part SHALL use the net radiation formula: `q_out = emissivity * STEFAN_BOLTZMANN * (T^4 - T_ambient^4) * surface_area`, where `surface_area = 2 * (width + height)` (perimeter approximation) and `T_ambient = 300 K`. The `T_ambient^4` term ensures parts naturally settle to ambient temperature. No gameplay multipliers are applied.

### Requirement: Per-part temperature update

Temperature update per part: `dT = (q_in - q_out) / (thermal_mass_kg * specific_heat) * dt`, where `thermal_mass_kg = part_width * SKIN_THERMAL_MASS_PER_METER`. This uses width-proportional thermal mass (modeling the thin skin layer) instead of total part mass, so all exposed parts heat at approximately the same rate regardless of size. Temperature SHALL be clamped to a minimum of 300.0 K.

### Requirement: No-atmosphere cooling

When density or airspeed is insufficient for heating, parts SHALL still undergo radiative cooling toward ambient temperature.

### Requirement: No heat conduction

Parts SHALL heat and cool independently — no heat conduction between parts.

## Thermal Destruction

### Requirement: Per-part destruction

`FlightVessel::destroy_overheated_parts()` SHALL destroy (set `destroyed = true`) any part whose `temperature >= max_heat_tolerance`. Destroyed part indices are returned for staging cleanup.

### Requirement: Staging cleanup on destruction

When parts are destroyed, their indices SHALL be removed from all staging lists.

### Requirement: Vessel splitting on part destruction

After parts are destroyed, `FlightVessel::check_and_split()` SHALL:
1. BFS from `root_part_index` through `find_weld_connections()` to find reachable parts
2. If root is destroyed, pick a new root (prefer pods, then any non-destroyed part)
3. Group unreachable (non-destroyed, non-decoupled) parts into connected components
4. Create debris `FlightVessel` for each component (reusing `extract_decoupled_parts` pattern)
5. Mark source parts as destroyed in the parent vessel
6. Return `Vec<(FlightVessel, [f64; 2])>` — debris vessels with COM offsets

### Requirement: Complete vessel destruction

If no non-destroyed, non-decoupled parts remain, the vessel SHALL be removed (`game.flight.vessel = None`) and ship temperature/heat_flux reset to ambient.

## Heat Visualization

### Requirement: Per-part heat fraction

`ShipPartRenderData` SHALL include `heat_fraction: f32` computed as `((temperature - 300) / (max_tolerance - 300)).clamp(0, 1)`.

### Requirement: Ship-level heat fraction from hottest part

`ShipRenderData.temperature` and `heat_fraction` SHALL reflect the hottest part's temperature when a vessel exists.

### Requirement: Per-part heat tinting on vertices

When rendering parts in flight, per-part `temperature` (Kelvin) SHALL drive a blackbody glow color ramp via `apply_heat_tint()`:
- Below 500K: no visible glow (original color unchanged)
- 500K: dark red (R=0.3, G=0, B=0), subtly blended over the part color
- ~1000K: cherry red, glow begins to dominate part color
- ~2000K: orange (green channel rising)
- 4000K: bright yellow (R=1.0, G≈0.85, B=0)
- Blend factor increases from 0 at 500K to fully overriding part color by ~1500K
- Alpha unchanged

### Requirement: Heat tinting on ship triangle icon

The same blackbody heat tinting SHALL be applied to the ship's triangle indicator using the ship-level `temperature` (hottest part) via `apply_heat_tint()`.

### Requirement: Heat bar in left HUD panel

When `temperature > 350K`, a vertical heat bar SHALL be shown (uses hottest part temperature):
- Colors: `< 0.33` -> yellow, `< 0.66` -> orange, `>= 0.66` -> red
- Background `rgb(40, 40, 50)`, border gray 1px

### Requirement: Temperature readout in bottom panel

When `heat_fraction > 0.01`, temperature readout ("{temp}K") SHALL appear in the bottom panel after altitude, colored by heat bar thresholds.

## Waste-Heat Pool Integration

Per-part aero heating (above) is independent of, but couples through spillover with, the
ship-wide waste-heat pool driven by interstellar engines, reactors, and radiators.

### Requirement: Thermal pool state on FlightVessel

`FlightVessel` SHALL have:
- `thermal_pool_temp: f64` — Kelvin, init 300, serde default 300.
- `thermal_pool_capacity: f64` — J/K, recomputed in `recalculate_mass` as
  `dry_mass_kg × 500 J/(kg·K)`.
- `reactors_tripped: bool` — global flag set when the pool crosses the trip threshold.

### Requirement: Thermal pool constants

`FlightVessel` SHALL define:
- `POOL_AMBIENT_TEMP = 300.0 K` — minimum / equilibrium temperature.
- `REACTOR_TRIP_TEMP = 1200.0 K` — at or above, all reactors trip.
- `REACTOR_RESTART_TEMP = 800.0 K` — manual restart allowed below this (hysteresis).
- `PART_DAMAGE_TEMP = 1500.0 K` — above this, excess heat spills into per-part temps.
- `SPILL_TIME_CONSTANT_SEC = 10.0` — pool-to-part bleed rate.

### Requirement: Thermal pool update each tick

`FlightVessel::update_thermal_pool(dt, part_defs)` SHALL be called once per game tick after
`update_power`. It computes:
1. `gen_W` = sum over non-destroyed/non-decoupled parts of:
   - `engine.waste_heat_watts × throttle` (when `part.engine_active == true`)
   - `reactor.waste_heat_watts` (when not tripped)
2. `reject_W` = sum over deployed radiators of `radiator.rejection_watts × deploy_fraction`.
3. `dT = (gen_W − reject_W) / thermal_pool_capacity × dt`; pool temp clamped to ambient.

### Requirement: Reactor trip cascade

When `thermal_pool_temp >= REACTOR_TRIP_TEMP`, `reactors_tripped` SHALL be set to `true`.
While tripped, `update_power` SHALL skip all reactor power generation (both fuel-consuming
and constant-output reactors) and `update_thermal_pool` SHALL skip reactor waste-heat
generation. The flag is sticky — it does not auto-clear when the pool cools.

### Requirement: Manual reactor restart

`FlightVessel::restart_reactors(cost_wh)` SHALL clear `reactors_tripped` when:
- `reactors_tripped == true`,
- `thermal_pool_temp < REACTOR_RESTART_TEMP`,
- vessel stored electricity `>= cost_wh`.

On success, `cost_wh` is drained proportionally across battery parts. The flight HUD wires
this via a `RenderRequest::ReactorRestart` button visible only while tripped.

### Requirement: Per-part spillover above damage threshold

When `thermal_pool_temp > PART_DAMAGE_TEMP`, excess heat SHALL bleed into per-part
temperatures using a time-constant model:
- `spill_W = (thermal_pool_temp − PART_DAMAGE_TEMP) × thermal_pool_capacity / SPILL_TIME_CONSTANT_SEC`
- For each non-destroyed/non-decoupled part with width `w`:
  - `part_thermal_mass = w × SKIN_THERMAL_MASS_PER_METER`
  - `share = part_thermal_mass / sum_thermal_mass`
  - `dT_part = spill_W × share × dt / (part_thermal_mass × specific_heat)`
  - `part.temperature = max(AMBIENT, part.temperature + dT_part)`
- The same wattage is subtracted from the pool so energy is conserved.

Existing `destroy_overheated_parts` then handles destruction when any part exceeds its
`max_heat_tolerance` — the destruction path is shared with aero heating.

### Requirement: Radiator deploy state

Radiator parts (`PartDefinition.radiator.is_some()`) SHALL share the existing `deploy_target`
/ `deploy_fraction` fields with solar panels. `update_solar_deploy` animates both kinds:
solar panels deploy at 0.5/sec (2s full); radiators deploy at `1 / deploy_time_sec` (5s by
default). Deployed radiators contribute rejection capacity scaled by `deploy_fraction`;
stowed radiators contribute zero. Deploying in atmosphere does not get special handling —
the existing per-part aero heating uses the part's hitbox width which doesn't grow with
deployment, so the destruction risk is realized via the part's relatively low
`max_heat_tolerance` (1500K / 2800K / 6500K per tier).

### Requirement: Thermal pool surfaced in HUD

`ShipRenderData` SHALL carry `thermal_pool_temp`, `thermal_pool_gen_w`,
`thermal_pool_reject_w`, and `reactors_tripped`. The flight HUD SHALL show a vertical
waste-heat bar near the existing per-part heat bar when `thermal_pool_temp > 400 K` or
`reactors_tripped`, with color thresholds at 800K (green→yellow), 1200K (yellow→orange,
"TRIP"), and 1500K (orange→red).

### Requirement: Editor thermal stats

`ShipStats` SHALL carry `waste_heat_gen` and `waste_heat_reject` (Watts, computed as the
sum across all placed parts assuming engines at full throttle and radiators fully deployed).
The editor stats bar SHALL display a "Thermal: X / Y" row colored green (`reject ≥ gen`),
yellow (within 10% margin), or red (deficit).

### Requirement: On-rails behavior

While on rails (`Ship::on_rails == true`), reactors do not run per existing model — so
`gen_W` typically falls to 0 and the pool decays to ambient via residual radiator capacity.
