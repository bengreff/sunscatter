# Multi-Vessel System

## Overview
The game supports multiple simultaneous vessels. Decoupling creates debris vessels, inactive vessels propagate on-rails, and the player can switch between vessels via keyboard shortcuts, tracking station, or double-click in flight map view.

## Data Model

### Requirement: Active vessel direct access
The active vessel's `ship` and `vessel` fields remain directly on `FlightState` (not wrapped in a Vec). This preserves all existing code that reads `game.flight.ship.*` or `game.flight.vessel.*` with zero changes.

### Requirement: Vessel identity
Each vessel has a unique `VesselId` (u64). `FlightState` tracks `active_vessel_id`, `active_vessel_name`, and `next_vessel_id` (monotonically increasing counter).

### Requirement: Inactive vessel storage
Inactive vessels are stored in `FlightState.inactive_vessels: Vec<TrackedVessel>`. Each `TrackedVessel` contains: `id`, `name`, `ship: Ship`, `vessel: Option<FlightVessel>`, `maneuver_nodes: Vec<ManeuverNode>`, `is_debris: bool`.

### Requirement: Debris classification
Vessels are classified as debris (`is_debris = true`) when they have no non-destroyed, non-decoupled part with `can_control = true` (checked via `FlightVessel::has_control()`). Fairing halves are always classified as debris. The active vessel (player-controlled) is never classified as debris when shelved. Debris classification persists across save/load via `SavedVessel.is_debris` (with `#[serde(default)]` for backward compatibility with old saves).

### Requirement: Active vessel control gating
The active vessel's `has_control()` is checked every frame. When the active vessel has no functioning command pod (all pods destroyed or decoupled), all player inputs are disabled: throttle is forced to 0, rotation/translation inputs are zeroed, autopilot is forced off, staging (spacebar) is blocked, and autopilot/RCS HUD buttons are non-functional. The vessel becomes uncontrollable debris that continues on its current trajectory. A bare ship (no FlightVessel) is always considered controllable.

### Requirement: Debris auto-cleanup
Debris vessels are automatically deleted when they are more than 2000m from ALL controllable vessels (active vessel + non-debris inactive vessels), or when they are in a different SOI from all controllable vessels. This cleanup runs every frame in flight mode via `FlightState::cleanup_distant_debris()`, called from `render_flight_frame()` in main.rs after inactive vessel propagation.

## Debris Creation

### Requirement: Decoupling creates debris
When `activate_next_stage()` or manual decouple marks parts as `decoupled = true`, `extract_decoupled_parts()` is called to split them into a new `FlightVessel`. The debris vessel is wrapped in a `TrackedVessel` and added to `inactive_vessels`. Stack decoupling has three phases: (1) Y-position-based: all parts whose top edge is at or below the decoupler's bottom edge are decoupled. (2) Fairing-side: parts beside the adapter/fairing zone are decoupled — this catches RCS blocks or other parts side-mounted on the fairing. The adapter zone spans from the decoupler's visual ring top (`center - hitbox_half_h + visual_height`) to the tank/pod bottom above. A part qualifies if its center Y is within this zone, its center X is off the center axis (`> 0.1m`), and within reach of the fairing (`< max(dec_half_w, tank_half_w) + GRID_SQUARE_SIZE`). (3) Connectivity-based: `decouple_disconnected()` runs a BFS from the vessel root to catch any remaining disconnected parts.

### Requirement: Radial decoupler staging
Radial decouplers (`is_radial: true`) skip Y-position-based and fairing-side decoupling. Only the decoupler itself is marked `decoupled = true`. The BFS connectivity check (`decouple_disconnected()`) then handles disconnecting all parts that were only reachable through the radial decoupler.

### Requirement: Debris naming
Debris vessels are auto-named "Debris {N}" where N is a monotonically increasing counter (`debris_counter`).

### Requirement: Extracted parts marked destroyed
After `extract_decoupled_parts()` clones decoupled parts into a debris vessel, the original parts in the parent vessel SHALL be marked `destroyed = true`. This prevents previously-decoupled parts from being re-extracted into duplicate debris on subsequent staging events.

### Requirement: Debris properties
- Full color (`[1.0, 1.0, 1.0, 1.0]`), same as active vessels
- No stages (empty `stages` vec)
- All engines disabled
- Put on rails immediately after creation
- Position offset by the decoupled parts' center of mass relative to the active vessel
- `is_debris` set based on `has_control()` (decoupled stages without pods are debris; stages with command pods are controllable)

### Requirement: Ejection force
When a decoupler fires, its `ejection_force` (kN, from `DecouplerData`) is applied as a separation impulse to the debris vessel. The impulse direction is determined by the debris COM offset from the parent vessel: the offset is rotated from local to world coordinates and normalized. This naturally pushes stack-decoupled debris downward (COM offset is below the vessel center) and radial-decoupled debris sideways (COM offset is to the side). If the COM offset magnitude is less than 0.01, the fallback direction is opposite the vessel heading. The velocity change is `dv = (force_N * 0.1s) / mass_kg`, applied to the debris ship's `rel_velocity` before it goes on rails. Thermal breakup debris receives no ejection force (0 kN).

### Requirement: Active vessel recentering
After debris extraction, the active vessel recenters its parts on its new COM and adjusts the ship's world position accordingly.

## On-Rails Propagation

### Requirement: No active vessel outside flight
When leaving flight mode (to main menu or tracking station), the active vessel is shelved: put on rails via `shelve_active_vessel()` and moved into `inactive_vessels`. There is no active vessel concept in main menu or tracking station — all vessels are equal entries in the inactive list. When entering flight (via tracking station "Fly"), a vessel is pulled from `inactive_vessels` and activated via `activate_vessel()`.

### Requirement: Exit flight restrictions
The "Main Menu" button in the flight pause overlay is disabled (greyed out) when the ship is in atmosphere or in the landing zone while suborbital (below landing altitude AND not in a stable orbit). Landed ships can always exit. This prevents the ship from being put on rails in a state where it would be immediately deleted.

### Requirement: Vessels propagate on rails
Every frame (in flight for inactive vessels, and in tracking station/main menu for all vessels), vessels are propagated on rails using `ship.update_on_rails(dt_sim, solar_system)`. If not already on rails, `enter_rails_mode()` is called first.

### Requirement: Vessel deletion
Vessels are deleted (removed from `inactive_vessels`) when BOTH of these conditions are true:
- Orbit periapsis is below the planet's surface (`periapsis_below_surface()`). Uses cached orbit if available; falls back to computing orbital elements from state vectors (vis-viva + angular momentum) when cached orbit is unavailable.
- Current position is in atmosphere (`in_atmosphere()`) or below landing altitude (`below_landing_altitude()`), including negative altitudes (inside the body surface)

This means a vessel on an orbit that clips the atmosphere is not immediately deleted — it persists until it actually reaches the atmosphere/landing zone.

### Requirement: Atmospheric proximity preservation
In flight mode, inactive vessels within 3km of the active vessel are NOT deleted even if they are in atmosphere with a suborbital trajectory. This preserves recently-jettisoned stages so the player can see them separate. Outside of 3km (or in main menu/tracking station), atmospheric deletion applies normally.

## Vessel Switching

### Requirement: Keyboard switching
`]` cycles forward and `[` cycles backward through all vessels (sorted by ID). The active vessel ID list includes both active and inactive vessels.

### Requirement: Switch mechanics
`FlightState.switch_to_vessel(target_id, current_maneuver_nodes, solar_system)`:
1. Saves current active vessel as a `TrackedVessel` in `inactive_vessels` (put on rails)
2. Removes target from `inactive_vessels`
3. Loads target as active (takes off rails, resets input, sets tracking)
4. Returns target's maneuver nodes for loading into render state

### Requirement: Maneuver node persistence
Each vessel's maneuver nodes are saved with the vessel during switching and restored when switching back. `RenderState.swap_maneuver_nodes()` handles the exchange.

### Requirement: Warp reset on switch
Time warp resets to 1x when switching vessels.

### Requirement: Double-click switching in flight
Double-clicking a background vessel icon in flight map view switches to that vessel. Hit detection uses stored screen positions with a 20px radius threshold.

### Requirement: Tracking station switching
The tracking station sidebar "Fly" button switches to the selected vessel and enters flight mode.

## Rendering

### Requirement: Full vessel part rendering
Inactive vessels with part data (`FlightVessel`) are rendered with their full parts using the same rendering pipeline as the active vessel (generate_part_shape_vertices, decoupler adapters). Inactive vessel parts use the same colors as the active vessel (no dimming). Vessels without part data fall back to triangle icon rendering (8px screen-fixed).

### Requirement: Background vessel orbits
Inactive vessels with valid orbits have their orbit lines drawn in dimmed grey (`[0.5, 0.5, 0.5, 0.3]`). Only elliptical orbits (e < 1) are rendered, using adaptive segment count via `orbit_segments()` (max `ORBIT_SEGMENTS` = 5120). Background vessel orbits SHALL only be visible in map view (when the active ship is smaller than 5 pixels on screen), matching the active vessel's orbit line visibility.

### Requirement: Tracking station vessel rendering
All vessels are rendered with full parts and orbit lines in grey (`[0.6, 0.6, 0.6, 0.4]`). Vessels without part data fall back to colored triangle icons.

### Requirement: Vessel deletion from tracking station
The tracking station sidebar shows an "X" delete button next to each vessel's "Fly" button. Clicking it removes the vessel from `inactive_vessels`. If the deleted vessel was being tracked by the camera, tracking resets to focus on Earth.

## Implementation
- `VesselId`, `TrackedVessel`, `FlightState` extensions in `src/game.rs`
- `extract_decoupled_parts()` in `src/parts/vessel.rs`
- `create_debris_vessel()`, `switch_to_vessel()`, `all_vessel_ids()` on `FlightState`
- `handle_post_decouple()`, `switch_to_next_vessel()`, `switch_to_next_vessel_by_id()` in `src/main.rs`
- `swap_maneuver_nodes()` in `src/render/maneuver.rs`
- `background_vessel_at_screen_pos()`, `background_vessel_screen_positions` field on `RenderState`
- Background vessel geometry in `update_bodies_orbits_ship_and_vessels()`
- `TrackingVesselData`, `TrackingStationAction` in `src/render/types.rs`

## Radiator Deploy and Thermal Cascade

### Requirement: Radiator deploy state

`FlightPart` SHALL carry `is_radiator: bool`, `radiator_rejection_watts: f64`, and
`radiator_deploy_time_sec: f64`. Deploy animation reuses the existing `deploy_target` /
`deploy_fraction` pair (shared with solar panels). `update_solar_deploy(dt)` selects the
appropriate deploy rate per part type: solar panels at 0.5/sec; radiators at
`1 / radiator_deploy_time_sec`.

### Requirement: Radiator deploy request

`RenderRequest::RadiatorDeploy { part_index, deploy }` SHALL toggle a radiator part's
`deploy_target`. The request mirrors to the part's `mirror_partner` if both are radiators.

### Requirement: Reactor restart request

`RenderRequest::ReactorRestart` SHALL invoke `FlightVessel::restart_reactors(100_000.0)`,
which clears `reactors_tripped` if the pool is below `REACTOR_RESTART_TEMP` and ≥100,000 Wh
of stored battery electricity is drained proportionally across battery parts. The HUD shows
this button only while `reactors_tripped == true`.

### Requirement: Tripped reactors produce no power

`update_power` SHALL gate both fuel-consuming and constant-output reactor generation on
`!self.reactors_tripped`. Solar panels, RTGs, and engine alternators are unaffected.

## Electric Engine Power Gating

### Requirement: Electric engines require power

`update_engine_states` SHALL check `EngineData.power_required` for each engine. If
`power_required > 0.0` and `total_electricity() <= 0.0`, the engine is set `engine_active = false`
and `engine_no_power = true`. The flight HUD displays "No Power" (purple) for such engines,
distinct from "No Fuel" (yellow).

### Requirement: Electric engine power consumption

`update_power` SHALL add `engine.power_required` to consumption for each active engine with
`power_required > 0.0`. This drains batteries while electric engines fire.

## Shield Activation

### Requirement: Shield toggle

Active shields (FRES, Geodesic) with `power_base_watts > 0` have a `shield_active: bool` field
on `FlightPart` (default false). The flight HUD provides Activate/Deactivate buttons.
`RenderRequest::ShieldToggle { part_index, active }` sets the field.

### Requirement: Shield power consumption

`update_power` SHALL add `shield.power_base_watts` to consumption for each part where
`shield_active == true` and `power_base_watts > 0.0`.

### Requirement: Shield auto-deactivation

After battery distribution in `update_power`, if `total_electricity() <= 0.0` and net power is
negative, all shield parts have `shield_active` set to false.

### Requirement: Whipple shields are passive

Whipple shields have `power_base_watts = 0.0` and are always on. No toggle is shown in the HUD.

## Life Support

### Requirement: Greenhouse food production

`GreenhouseData { food_production_rate: f64 }` on `PartDefinition` specifies kg of food produced
per day. Greenhouses require power (checked via `part.electricity > 0` as proxy).

### Requirement: Crew food consumption

Each crew member consumes 1.5 kg of food per day. `FlightVessel.food_stored` tracks current food
in kg. Ships launch with 30 days of food per crew member.

### Requirement: Starvation

When `food_stored` reaches 0, `starvation_timer` accumulates elapsed time. After 7 days of
continuous starvation, one crew member dies per day (picked from the first pod with
`crew_count > 0`). `total_crew` is recomputed after each death.

### Requirement: Control loss from crew death

`has_control()` requires crewed pods (`crew_capacity > 0`) to have `crew_count > 0` to provide
control. Probe cores (`crew_capacity == 0, can_control == true`) always provide control regardless
of crew state.

### Requirement: Food HUD indicator

The flight HUD left panel shows a FOOD bar when `total_crew > 0`, displaying days of food
remaining. Color thresholds: green > 30 days, yellow > 7 days, red < 7 days.
