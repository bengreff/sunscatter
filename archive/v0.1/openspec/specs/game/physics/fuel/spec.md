# Fuel System

Fuel consumption, fuel zones, and drain priority for asparagus/onion staging.

## Fuel Zones

### Requirement: Zone-based fuel sharing
Fuel is shared across all tanks within a fuel zone. Zones are computed by flood-filling the weld adjacency graph. Non-crossfeed decouplers act as barriers: they are assigned to a zone but don't propagate fuel flow to their neighbors. Crossfeed-enabled decouplers allow fuel to flow through them, merging tanks on both sides into a single zone.

### Requirement: Decoupler adapter connectivity
Non-radial decouplers connect **downward** via normal welding hitbox overlap but connect **upward** only through their adapter target — the closest aligned (same center X, ±0.01 tolerance) tank or pod whose bottom edge is at or above the decoupler's ring top. Ring top = `center_y - hitbox_height/2 + visual_height`. A part is considered "above" if its **center Y** is above ring_top — even if its welding hitbox extends below ring_top, it does not connect to the decoupler. Parts above the ring that are not the adapter target (e.g., fairing bases) have no structural connection through the decoupler. This prevents non-tank/pod parts from bridging across a decoupler in the connectivity graph. Radial decouplers are exempt (they use normal welding hitbox in all directions).

### Requirement: Engine fuel consumption
Engines consume fuel proportionally from all available tanks in their zone. Each engine computes its demand per-frame based on mass flow rate, throttle, and dt. Oxygen and fuel are drained separately. Engines without available fuel are deactivated. Oxygen is only required for propellant types that use it (Kerolox, Methalox, Hydrolox); propellants with zero oxidizer ratio (FusionFuel, Antimatter, Hydrogen, Xenon, NuclearPulse) skip the oxygen check.

### Requirement: Dual propellant engines
Engines MAY have a `secondary_propellant` and `secondary_fuel_fraction` (0.0-1.0). When present, the total mass flow is split: `(1 - fraction)` goes to the primary propellant and `fraction` goes to the secondary. Both fuels must be available in the engine's fuel zone for the engine to be active. The secondary fuel is drained from tanks containing that fuel type using the same zone and priority system as primary fuel. Example: AM-Cat Fusion uses D+He3 (primary, 99.76%) + Antimatter (secondary, 0.24%).

## Drain Priority (Asparagus/Onion Staging)

### Requirement: Priority computation
Each part is assigned a drain priority based on its reachability from the vessel root when crossfeed-enabled decouplers are removed. For each crossfeed-enabled decoupler D in stage S: temporarily remove D from the weld graph, BFS from the vessel root, and parts NOT reachable get `priority = min(existing_priority, S)`. Parts always reachable from root (no crossfeed decoupler between them and root) keep `priority = usize::MAX`.

### Requirement: Priority-based draining
Within each fuel zone, find the minimum priority among tanks that have fuel. Only drain from tanks at that minimum priority level (proportionally among those tanks). When those tanks are empty, the next priority level starts draining. This applies to both oxygen and fuel draining.

### Requirement: Delta-v calculation respects drain priority and propellant types
The per-stage delta-v calculation tracks fuel **per resource type** (oxygen, rp1, methane, hydrogen, fusion_fuel, antimatter, etc.) rather than as a single pool. Each engine's propellant type determines which resources it demands, using the same O/F ratios and secondary propellant fractions as runtime fuel consumption. An engine only contributes to delta-v if all its required resources are available in its zone. Phase time is `min(available / demand)` across all demanded resources — the first resource to run out limits the burn. Within each fuel zone, drain priority applies per-resource: only minimum-priority tanks for each resource contribute to availability, and after computing a stage's delta-v, resources are drained proportionally at the minimum priority level.

### Requirement: Asparagus staging behavior
In a typical asparagus/onion configuration (center tank + radial crossfeed decouplers + side tanks), side tanks drain first while the center tank stays full. When the radial decouplers fire, the empty side tanks are jettisoned. The center tank then provides fuel for the remaining stages.

## Save Compatibility

### Requirement: FlightPart forward-compatible deserialization
`FlightPart` uses struct-level `#[serde(default)]` so that save files created before new fields were added can still be loaded. A manual `Default` impl provides sensible zero/false/None values for all fields (except `rcs_torque_multiplier` which defaults to `1.0`). When adding new fields to `FlightPart`, add them to the `Default` impl — no per-field serde attributes are needed.

## Implementation
- `compute_fuel_zones()`, `compute_drain_priorities()` in `src/parts/vessel.rs`
- `compute_fuel_zones_simulated()`, `compute_drain_priorities_simulated()` for delta-v calculation
- `apply_decoupler_adapter_connections()` post-processes adjacency for all connectivity functions
- `consume_fuel()` Phase 3 uses priorities for proportional draining
- `calculate_stage_delta_v()` steps 6-11 use per-resource demand/availability with drain priorities (flight) or zone-wide availability (editor)
- `compute_editor_fuel_zones()` in `src/editor/state.rs` pre-computes adapter targets for editor BFS
