# Trade Route Mechanics (Layer 4)

## Overview

Trade routes connect colonies via computed (not simulated) cargo transfers. Ships are abstract entities that consume fuel, load cargo at a source colony, transit for a computed flight time, then unload at a destination. The system reuses the existing Lambert solver for orbital mechanics and the blueprint/vessel system for delta-v calculation.

## Data Model

### Source: `src/colony/trade.rs`

**TradeShipId** / **TradeRouteId** — newtype wrappers around `u64`.

**TradeShipState** — enum:
- `Stationed` — ship is docked at a colony or Earth
- `InTransit` — ship is en route between locations

**TradeShip** — represents an abstract trade vessel:
- `id: TradeShipId`, `name: String`, `blueprint_name: String`
- `state: TradeShipState`, `location: Option<usize>` (None = Earth)
- `origin: Option<usize>`, `cargo: CargoManifest`, `crew: u32`
- `current_leg: usize`, `transit_remaining: f64` (seconds)
- `assigned_route: Option<TradeRouteId>`, `cached_delta_v: f64`
- `returning: bool` — true when ship is on the return leg

**RouteLeg** — one hop in a multi-leg route:
- `from_body: Option<usize>`, `to_body: Option<usize>` (None = Earth)
- `delta_v: f64` (m/s), `flight_time: f64` (seconds)

**CargoManifest** — `items: Vec<(ResourceType, f64)>` with `total_mass()` and `is_empty()`.

**AutomationMode** — enum: `Manual`, `WindowBased`, `FrequencyBased`, `DvThreshold`. (Legacy — new routes use route_category + interval_days/ships_per_window.)

**TradeRoute** — defines a recurring trade path:
- `id: TradeRouteId`, `name: String`, `legs: Vec<RouteLeg>`
- `blueprint_name: String`
- `outbound_cargo: CargoManifest`, `return_cargo: CargoManifest`
- `crew: u32`, `automation: AutomationMode`
- `frequency_days: f64`, `dv_threshold: f64`, `priority: i32`
- `min_stockpile: f64`, `paused: bool`, `last_launch_time: f64`
- `assigned_ship_id: Option<TradeShipId>`
- `total_delta_v: f64`, `total_flight_time: f64`, `max_cargo_capacity: f64`
- `route_category: RouteCategory` — SameSOI, Interplanetary, or Interstellar (default SameSOI, serde default)
- `interval_days: f64` — launch frequency for SameSOI/Interstellar routes (default 30.0, serde default)
- `ships_per_window: u32` — ships per transfer window for Interplanetary routes (default 1, serde default)
- `alert_reason: Option<String>` — set when automation launch fails (insufficient resources/food), cleared on success (serde default)

**FleetManager** — owns all ships and routes:
- `ships: Vec<TradeShip>`, `routes: Vec<TradeRoute>`
- `next_ship_id: u64`, `next_route_id: u64`
- Methods: `create_route()`, `build_ship()`, `launch_ship()`, `update_fleet()`, `check_automation()`
- Query methods: `get_ship()`, `get_route()`, `ships_at()`, `routes_involving()`, `route_source()`, `route_destination()`

### Integration
- `Game` struct has `pub fleet: FleetManager` field
- `SaveGame` has `pub fleet: FleetManager` with `#[serde(default)]` for backward compatibility
- Fleet resets in `reset_for_new_game()`

## Transfer Delta-V Computation

### Source: `src/colony/transfer.rs`

**AtmosphereClass** — Airless (<1000 Pa), Thin (1000-10000 Pa), Thick (>10000 Pa).

**Gravity loss factor**: 1.1 (airless), 1.15 (thin), 1.3 (thick).

**Landing dv fraction**: 1.0 (airless, full propulsive), 0.0 (atmospheric, free aerobraking).

**Surface orbital velocity**: `sqrt(G * mass / radius)`.

**TransferType** — Same parent, child-to-parent, parent-to-child, or interplanetary.

**compute_hohmann_simple()** — Returns `(departure_dv, arrival_dv, transfer_time)` for a Hohmann transfer between two circular orbits. Both burn delta-vs are computed via vis-viva at departure and arrival radii.

**ParentToChild / ChildToParent transfers** — Uses a synthetic parking orbit (200 km above parent surface) and computes a full Hohmann transfer to/from the child's orbit. Earth→Moon: ~3,925 m/s total, ~5.1 day transit.

**RouteCategory** — enum (SameSOI, Interplanetary, Interstellar). Determines scheduling mode and transfer computation strategy.
- Derived from body hierarchy via `classify_route()`.
- SameSOI: bodies share a direct parent, or one is the parent of the other.
- Interplanetary: requires a Hohmann/Lambert transfer between different planetary SOIs.
- Interstellar: different star systems (future-proofed).

**compute_leg_delta_v()** — Core function. Given source/destination body indices, departure time, and speed factor (0.0 = Hohmann, 1.0 = express):
1. Classify transfer type based on body hierarchy
2. Compute transfer delta-v using `compute_hohmann()` or `compute_interplanetary()` from `src/ship/transfer.rs`
3. Add surface launch delta-v: `surface_orbital_velocity * gravity_loss_factor`
4. Add landing delta-v: `surface_orbital_velocity * landing_dv_fraction` (0 if atmospheric)
5. Return `LegResult { total_dv, launch_dv, transfer_dv, landing_dv, flight_time }`

**FlightVessel.extra_dry_mass_tonnes** — `#[serde(default)]` field. Added to wet_mass in `calculate_stage_delta_v()` to account for cargo payload mass reducing delta-v. Set by `blueprint_dv_with_cargo()`.

**blueprint_total_delta_v()** — Creates a throwaway `FlightVessel::from_blueprint()`, sums stage delta-vs via `calculate_stage_delta_v()`.

**blueprint_dv_with_cargo()** — Public function. Computes delta-v for a blueprint with additional cargo mass added. Sets `FlightVessel.extra_dry_mass_tonnes` so that `calculate_stage_delta_v()` correctly includes cargo mass in the wet/dry mass ratio. Used by route creation to show ship capability under load.

**blueprint_cargo_container_capacity()** — Sums `CargoData.capacity_kg` across all parts in the blueprint. Returns the physical container capacity in kg.

**compute_cargo_capacity()** — Binary search: find max payload mass where ship total delta-v >= route delta-v. ~20 iterations for 1 kg precision.

**compute_synodic_period()** — For interplanetary routes, computes the synodic period between two bodies (TAU / |omega_from - omega_to|). For SameSOI sibling orbits, computes the synodic period of the two orbits. Returns period in seconds.

**compute_interstellar_transfer()** — Computes relativistic transfer parameters for interstellar routes. Returns `InterstellarResult { distance, v_cruise, coordinate_time, proper_time, gamma }`. Uses v_cruise = dv_budget/2 (accel/decel split). Applies Lorentz factor when v_cruise >= 0.01c.

**estimate_flight_time()** — Estimates flight time given a delta-v budget. Uses Hohmann baseline from `compute_leg_delta_v(speed_factor=0)`, scales inversely with budget (clamped to >= 0.25x baseline). For interstellar routes, delegates to `compute_interstellar_transfer()`.

**next_launch_window()** — Wraps `hohmann_optimal_times()` for interplanetary transfers. For same-parent, uses synodic period calculation.

## Fleet Transit Simulation

Called from `Game::update_colonies()` after colony ticks.

**update_fleet()** — For each InTransit ship, decrement `transit_remaining` by `dt_days * 86400`. On arrival, call `process_arrival()`.

**process_arrival()** — Routes are one-way. Unloads cargo to destination colony inventory (or converts to money if Earth). Delivers crew to colony. For multi-hop routes, advances to next leg. On final arrival: sets ship Stationed at destination, unassigns ship from route (`assigned_ship_id = None`, `assigned_route = None`), sends ShipArrived notification. Ship stays at destination as colony inventory.

**launch_ship()** — Validates ship is stationed at route source. Rocket cost is NOT charged here (already paid by `build_ship`). For Earth source: charges fuel + cargo + food upfront, fails with error if insufficient funds. For colony source: consumes fuel and food from colony inventory, loads cargo (checking availability > min_stockpile), deducts crew. Food = crew × 0.5 kg/day × flight_days (consumed in transit, not loaded as cargo). Sets ship InTransit with first leg's flight_time. Sends ShipDeparted notification.

**build_ship()** — Deducts money (Earth: sum of part costs) or resources (colony: 2× vessel mass in structural metal), creates TradeShip with state Stationed at specified location.

## Route Automation / Scheduling

Called from `update_fleet()` once per tick, after transit updates.

**check_automation()** — For each non-paused route:
1. Skip if a ship is currently in transit on this route
2. Check scheduling condition (see below)
3. If launch is due: auto-build a new ship at source via `build_ship()`, assign it to route, then `launch_ship()` it
4. On arrival, ship stays at destination and is unassigned — next interval builds a fresh ship

Scheduling by `AutomationMode`:
- **Manual**: Never auto-launches. Launch only via manual UI action.
- **WindowBased**: Interplanetary routes launch when `next_launch_window()` departure is within 1 day. SameSOI/Interstellar routes use `interval_days` (no transfer windows).
- **FrequencyBased**: Launch when `sim_time - last_launch_time >= frequency_days * 86400`
- **DvThreshold**: Launch when current Lambert delta-v < `dv_threshold`

Pre-launch validation: funds/resources available for ship build + fuel + cargo + food. On failure: set `alert_reason` on route, push RoutePaused notification only on first failure (stops warp). Routes **continue retrying** every cycle — if resources become available, the launch succeeds and alert clears automatically. Only one notification per failure (via `had_alert` guard). Alert clears on: successful launch, route edit, or route resume.

Routes processed sorted by priority descending, then by id ascending.

## Notifications

### Source: `src/colony/notification.rs`

New variants added:
- `RoutePaused { route_name, reason }` — informational (does not stop warp)
- `ShipConstructionComplete { ship_name, location }` — informational

Note: `ShipArrived` and `ShipDeparted` notification kinds exist in the enum but are not emitted — only error/failure notifications are shown to avoid toast spam during time warp.

## UI

### Source: `src/render/trade_ui.rs`

**TradeAction** enum (in `types.rs`): None, CreateRoute, PauseRoute, ResumeRoute, DeleteRoute, DeleteShip, EditRoute, OpenEditor.

**RouteCreationState** — Single-panel route creation/editing state (replaces old 7-step wizard):
- User selections: route_name, blueprint_name, source_body, dest_body, cargo_items, crew, dv_budget, interval_days, ships_per_window, automation (AutomationMode), priority (i32), min_stockpile (f64), frequency_days_input (f64), dv_threshold (f64)
- Cache fields (recomputed via hash key): cached_category, cached_leg, cached_min_dv, cached_ship_dv_empty/with_cargo, cached_cargo_capacity, cached_container_capacity, cached_synodic_period, cached_flight_time, cached_fuel_reqs, cached_crew_capacity, cached_has_probe_core
- `editing_route_id: Option<TradeRouteId>` — Some when editing, None when creating
- `start()` — new route with defaults
- `start_from_route(route, fleet)` — edit existing route (pre-fills all fields)

**Fleet Overview Panel** — Rendered in the colony overview screen below the colony list. Shows all routes with status (Active/Paused), source/destination, delta-v, and action buttons (pause/resume/delete). Shows route alert inline (red warning text) when automation fails (alert clears on next successful launch). Ship status line appears when a ship is in transit, showing "In transit to [Dest] — X / Y days". Launches are fully automatic — no manual launch button. Lists unassigned ships separately. Each route has an Edit button (pencil icon) that opens the creation panel pre-filled.

**Colony Trade Section** — Rendered in the per-colony screen as a **read-only** card section. Shows routes involving this colony, ships stationed here. No creation or management controls — gray text "Manage routes in Colony Overview" directs users to the colony overview screen.

**Route Creation Panel** — Single scrollable egui::Window (replaces old 7-step wizard). All sections visible at once:
1. **Route Name** — text input
2. **Ship Blueprint** — ComboBox with empty delta-v shown
3. **Source** — ComboBox: Earth + colonies with operational Launchpads
4. **Destination** — ComboBox: Earth + all colonies, excluding source
5. **Transfer Analysis** — Route category label, minimum delta-v (Hohmann), adjustable delta-v budget with "Min" button (sets to Hohmann minimum), ship delta-v with/without cargo, cargo capacity, travel time estimate, transfer window frequency (interplanetary), per-leg breakdown (launch/transfer/landing dv)
6. **Cargo Manifest** — resource ComboBox (ship fuels excluded) + DragValue + Add, list with Remove. Effective capacity = min(dv-limited capacity, physical container capacity). Add button disabled when no cargo containers on blueprint. Shows "No cargo containers" warning if blueprint has none.
7. **Crew** — DragValue clamped to blueprint pod capacity. Min crew = 0 if probe core present, else 1. Shows "/ N seats" and probe core info.
8. **Departure Inventory** — Fuel (full blueprint load), Cargo (manifest items), Food (crew x 0.5 kg/day x flight_days), Crew count. For Earth-source routes: itemized cost breakdown (Rocket + Fuel + Cargo + Food) and total launch cost.
8b. **Destination Inventory** — Ship (blueprint name), Remaining fuel (estimated: fuel_loaded x max(0, 1 - route_dv/ship_dv)), Cargo (delivered intact), Food remaining (0 kg, consumed in transit), Crew arriving.
9. **Scheduling** — SameSOI/Interstellar: "Every N days" DragValue; Interplanetary: "N ships per window" DragValue (auto-selected by route category). Automation mode ComboBox (Manual / Window-Based / Frequency-Based / Delta-V Threshold). Conditional inputs: frequency_days for FrequencyBased, dv_threshold for DvThreshold. Priority DragValue (-100 to 100, higher = processed first). Min stockpile DragValue (0 to 1,000,000 kg). Auto-build ships checkbox (colony sources only). All values wired into TradeRoute on create/update.
10. **Create/Update Route** + Cancel buttons

Create button disabled when: no blueprint, no source, no dest, source==dest, route name empty, ship dv < budget.

Cache invalidation: hash of (source, dest, blueprint_name, dv_budget, total_cargo_mass). Recomputed when hash changes.

**Colony Overview "Colony Overview" Button** — Colony screen pause menu has a "Colony Overview" button that navigates back to the colony overview screen.

**Action Handling** — ColonyOverviewAction and ColonyScreenAction both have `Trade(TradeAction)` variants. `handle_trade_action()` in main.rs dispatches all trade actions to FleetManager methods. `ManualLaunch` builds a new ship and launches it immediately. `OpenEditor` is intercepted at the colony overview call site (populates RouteCreationState before reaching handle_trade_action). `EditRoute` updates existing route fields while preserving id and last_launch_time.

## Ship Hangar System

### Source: `src/colony/trade.rs`, `src/colony/buildings.rs`

Ships are persistent colony assets stored in hangars. When a trade ship arrives at a colony, it becomes a `StoredShip` in that colony's hangar. Trade routes source ships from hangars for launches.

**StoredShipId** — type alias for `u64`.

**StoredShip** — a ship stored in a colony hangar:
- `id: StoredShipId`, `name: String`
- `blueprint_name: Option<String>` — matches a named blueprint if built from one
- `blueprint: VesselBlueprint` — the ship's dry-state blueprint
- `dry_mass_kg: f64`, `cached_delta_v: f64`

**BuildingType::Hangar** — colony building, 200,000 kg capacity per operational hangar. Tech-gated under `colony_engineering` (same tier as Launchpad). Build cost: 50,000 Structural Metal, 10,000 High-Temp Alloys, 5,000 Electronics. Power draw: 50 kW. Maintenance: 125 Structural Metal + 25 High-Temp Alloys per 30 days.

**Colony hangar methods** (in `buildings.rs`):
- `hangar_capacity()` — 200,000 kg × operational hangar count
- `hangar_used()` — sum of stored ship dry masses
- `has_hangar()` — at least one operational Hangar building
- `store_ship(ship)` — stores ship if capacity allows, assigns ID
- `remove_ship(id)` — removes and returns ship by ID
- `find_matching_ship(blueprint_name)` — finds first ship matching blueprint
- `scrap_ship(id, part_defs)` — removes ship, converts to resources via `blueprint_material_costs()`, adds resources to colony inventory
- `queue_ship_construction(name, blueprint_name, blueprint, part_defs)` — queues ship as `ConstructionTarget::Ship` in construction queue

**ConstructionTarget** — enum (in `buildings.rs`):
- `Building(BuildingType)` — standard building construction
- `Ship { name, blueprint_name, blueprint, dry_mass_kg, cached_delta_v }` — ship construction

**TradeRoute.auto_build_ships** — `bool` (default false, serde default). When true, colony automation queues ship construction when hangar stock is low. Only meaningful for colony-source routes (Earth builds instantly).

**migrate_stationed_ships()** — Called on game load. Converts old `Stationed` TradeShips from pre-hangar saves into `StoredShip` objects in the appropriate colony's hangar.

### Hangar UI

**Colony Screen Hangar Card** (card 8, between Trade and Debug):
- Only shown if colony has at least one operational Hangar building
- Capacity bar: `hangar_used() / hangar_capacity()`
- Grid listing stored ships: name, blueprint name, dry mass, delta-v
- Per-ship Scrap button — `ColonyScreenAction::ScrapShip(body_index, ship_id)`

**Construction Queue Ship Display** — `render_construction_card()` uses `effective_target()` to detect `ConstructionTarget::Ship` items and displays "Ship: {name}" instead of the placeholder building type.

**Route Creation auto_build_ships** — Checkbox in Scheduling section, only shown for colony-source routes (not Earth). Wired into `RouteCreationState.auto_build_ships` and carried through to `TradeRoute.auto_build_ships`.
