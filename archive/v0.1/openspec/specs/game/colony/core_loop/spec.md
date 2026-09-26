# Colony Core Loop

Layer 1 of the colony system. Implements colony establishment, production/consumption simulation, building construction, maintenance, science labs, notifications, and the colony management UI.

## Cargo Container Parts

- `PartCategory::Cargo` variant in `definition.rs` with display name "Cargo"
- `CargoData` struct: `capacity_kg: f64`
- Four sizes in `data/parts/cargo.ron`:
  - Tiny (1×2, 0.05t dry, 1,000 kg capacity)
  - Small (3×3, 0.3t dry, 10,000 kg capacity)
  - Medium (5×4, 1.0t dry, 50,000 kg capacity) — fits starter colony (Habitat + SmallSolarFarm = ~24,000 kg)
  - Large (9×5, 3.0t dry, 200,000 kg capacity)

### Cargo Manifest

Cargo containers store two types of payload:
- **Resources**: `cargo_resources: Vec<(String, f64)>` — resource display name + mass in kg
- **Buildings**: `cargo_buildings: Vec<String>` — BuildingType display names for pre-assembled buildings

Both fields exist on `BlueprintPart`, `PlacedPart`, and `FlightPart`. Cargo mass (resources + building `total_build_mass()`) counts toward vessel mass and must fit within `CargoData.capacity_kg`.

### Editor Cargo UI

Part info panel shows cargo section for parts with `CargoData`:
- Capacity progress bar (used/total kg, red when >95%)
- Resource list with amounts and remove buttons
- Building list with mass and remove buttons
- "Add Resource" / "Add Building" ComboBox dropdowns (resources filtered: ship fuels excluded via `ResourceType::is_ship_fuel()`)
- DragValue for editing resource amounts
- Mirror partner gets same manifest

## Gas Giant Restriction

- `CelestialBody.is_gas_giant: bool` — true for Jupiter, Saturn, Uranus, Neptune
- Surface colonies cannot be established on gas giants

## Colony Establishment (`game.rs`)

`Game::establish_colony(body_index)` validates:
1. Ship is `ShipState::Landed` on the target body
2. No existing colony on the body
3. Body is not Earth (`body_index != earth_index`)
4. Body is not a gas giant
5. Vessel cargo contains minimum colony buildings: Habitat + Small Solar Farm (`FlightVessel::has_colony_buildings()`)

On success:
- Extracts all cargo from containers via `FlightVessel::extract_all_cargo()` — returns (buildings, resources, food_kg)
- Creates `Colony` with buildings from cargo as operational `BuildingInstance`s
- Adds a Stockpile if none was in cargo
- Transfers resources and food to colony inventory
- Pre-stocks 1,000 kg food per Habitat building
- Sets `crew` from vessel's total pod crew capacity
- Cargo containers are emptied but NOT destroyed (parts persist)
- Pushes `ColonyEstablished` notification

## Cargo Transfer to Colony

"Transfer Cargo" button shown in flight HUD when:
- Ship is landed, body has a colony, vessel has non-empty cargo containers

On click: `extract_all_cargo()` transfers buildings → colony buildings, resources → colony inventory, food → colony food_stored.

Click handler pushes `RenderRequest::TransferCargo { body_index }` onto `RenderState::render_requests`. Related state fields: `vessel_has_cargo`, `landed_body_has_colony`.

### Flight HUD Integration

"Establish Colony" button shown in bottom panel when:
- Ship is landed, body has no colony, body is not Earth, body is not gas giant, vessel has colony buildings in cargo

Related state fields: `can_establish_colony`, `landed_body_index`. Click handler pushes `RenderRequest::EstablishColony { body_index }` onto `RenderState::render_requests`; `main.rs` drains the queue after render and calls `game.establish_colony(body_index)`.

## Colony Simulation (`simulation.rs`)

### Orchestrator: `Game::update_colonies(dt_sim)`

Called from all frame renderers (flight, tracking station, colony, colony overview, management, tech tree) inside `if !game.paused`. R&D science (`game.update_rd_science(dt_sim)`) and contract checking (`game.check_contracts()`) are also called in every frame's simulation block, immediately before `update_colonies`.

`num_ticks = ceil(total_days).clamp(1, 1000)`, `days_per_tick = total_days / num_ticks`. Per tick calls `simulate_colony_tick()` for each colony.

### Tick Order

0. **Reactor fuel consumption** — Before power calculation. Fission Reactor: 0.5 kg Enriched Uranium/day. Fusion Reactor: 3 kg He-3 + 2 kg Deuterium/day. When multiple reactors share the same fuel pool, each reactor's check-and-consume is atomic (via `ResourceStorage::remove` / `remove_all`) and they compete in iteration order: once the pool cannot fully cover a reactor's tick, that reactor (and any later reactor of the same type) produces no power this tick. Reactors without sufficient fuel produce no power.
1. **Power balance (habitat-priority)** — Sum generation (solar scaled by `(AU/sun_distance)^2` and degradation, reactors by degradation and fuel availability). Demand split into `habitat_demand` (Habitat buildings) and `other_demand` (all other buildings + factory recipe power). Habitats get power first: `habitat_power_fraction = min(1, total_gen / habitat_demand)`. Remaining power goes to other buildings: `other_power_fraction = min(1, (total_gen - habitat_demand) / other_demand)`. If `habitat_power_fraction < 1.0` and crew > 0, push `ColonyPowerLoss` notification (deduped by `habitat_unpowered_notified` flag).
2. **Maintenance** — Consume `maintenance_cost / 30 * days` per building. Robot capacity check. Degradation increases on shortfall; buildings hold at current degradation when fully maintained (no self-repair). Buildings with no maintenance costs (Stockpile) also do not self-repair. Each building's degradation is tracked independently — siblings of the same type do not share state.
3. **Construction** — Remaining robot capacity advances first queue item. On completion: add building to colony (no notification).
4. **Mines** — `2000 * days * (1 - degradation) * other_power_fraction * mining_mult` kg of assigned resource, capped by **per-resource storage allocation**. `mining_mult = 1.11^tier("mining")`. Per-resource cap = `storage_allocation.capacity_for(resource, storage_cap, active_resources)`.
4b. **Atmospheric Collectors** — `10,000 * days * (1 - degradation) * other_power_fraction * atmo_mult` kg of assigned resource, capped by per-resource storage allocation. `atmo_mult = 1.11^tier("atmospheric_science")`.
5. **Factories** — `(outputs_per_batch / batch_hours) * 24 * days * (1 - degradation) * other_power_fraction * factory_mult` throughput. `factory_mult = 1.11^tier(recipe.efficiency_line_id())`. Consume proportional inputs. Storage check uses **per-resource caps** for each output (accounting for input mass freed by the same resource type). Unit-counted resources (MirrorSegment, CollectorStation) are converted to kg via `ResourceType::storage_mass_per_unit(sail_tier)` before comparing against kg caps. Recipe-to-line mapping: MetalSmelting/AlloyForging → "metallurgy", Electronics/Superconductor → "electronics_mfg", PrecisionInstruments → "precision_mfg", Electrolysis/Deuterium/Sabatier/Methane/Kerosene → "chemical_processing", Uranium/Tritium/NPU → "nuclear_engineering", He3 recipes → "isotope_extraction".
6. **Greenhouses** — Basic: `0.5 * days * water_factor * (1-degradation) * other_power_fraction * agri_mult` kg food (water_factor = water_fill / 2,000). Advanced: `2.5 * days * water_factor * (1-degradation) * other_power_fraction * agri_mult` kg food (water_factor = water_fill / 5,000). `agri_mult = 1.11^tier("agriculture")`.
6b. **Food cap** — If `food_capacity() > 0` and `food_stored > food_capacity()`, clamp to capacity.
7. **Food consumption** — `0.5 * crew * days` kg. If food hits 0 and `!food_depleted_notified`, set flag and push notification. Reset flag when food rises above 0.
7b. **Crew death** — Crisis if food_stored <= 0 or habitat_power_fraction < 1.0 (and crew > 0). On crisis start, record `crew_at_crisis_start`. Deaths per day = `crew_at_crisis_start * 0.005` (1% per 2 days), linear not compounding. Fractional deaths accumulate in `crew_death_accumulator`; whole deaths are subtracted from crew only when the accumulator reaches >= 1.0. Crisis clears (and accumulator resets) when food > 0 and habitat power is full.
8. **Science labs** — Extraction formula: `10 * landing_value * (1 - e^(-N * t / 15))` where N = `lab_count * other_power_fraction`, t = elapsed years. Delta added to `science.available`.

### Helper Functions

- `habitability_multiplier(score)` — `(200 - score) / 100` (100=1x, 0=2x)
- `sun_distance(body_index, solar_system)` — Walk parent chain to find sun-orbiting ancestor
- `Colony::storage_capacity()` — 500,000 kg per operational Stockpile (total pool; per-resource caps via `StorageAllocation`)
- `ResourceType::storage_mass_per_unit(sail_tier)` — kg per inventory unit. Most resources return 1.0 (stored in kg). MirrorSegment returns `mirror_mass_at_tier(sail_tier)`, CollectorStation returns `COLLECTOR_MASS_KG` (5,000).
- `ResourceInventory::total_storage_mass(sail_tier)` — sum of `amount * storage_mass_per_unit()` for all resources. Used for storage display instead of `total_mass()` to correctly account for unit-counted resources.
- `compute_production_map(colony, tech_tree)` — lightweight HashMap of resource → production rate for active resource detection
- `Colony::food_capacity()` — Habitat: 3,000 kg, FoodStorage: 10,000 kg per operational building
- `Colony::crew_capacity()` — 20 per operational Habitat
- `Colony::operational_building_count(bt)` — Count of non-degraded buildings of type
- `sun_distance(body_index, solar_system)` — pub helper, walk parent chain to find sun-orbiting ancestor SMA

### Construction

- `Colony::queue_building(bt, hab_score, body_radius_m, count)` — Validates resources, deducts for `min(count, max_affordable)` units. If an existing queue item of the same building type exists, merges into it (increases count, adds reserved resources). Otherwise pushes a new `ConstructionQueueItem`. `count=0` means "all affordable".
- `ConstructionQueueItem` fields: `count` (batch total, default 1), `completed` (units finished so far, default 0), `total_mass` (per-unit mass). On completion of each unit in a batch, the building is added, `completed` increments, and `mass_assembled` resets. Queue item removed when `completed >= count`. UI shows remaining count (`count - completed`) as multiplier, decreasing as buildings complete.
- Habitability multiplier applied to build cost for affected buildings
- Robot throughput: ConstructionRobot 20,000 kg/day, LightConstructionRobot 5,000 kg/day (both scaled by `1.11^tier("construction")`)
- Maintenance priority: robots service maintenance first (also scaled by construction multiplier), remaining capacity goes to construction
- Building construction completions do NOT push notifications (monitored via colony UI progress bars)

### Maintenance

- Per building per tick: `cost/30 * days_per_tick`
- **Life support multiplier**: Habitability-affected buildings (Habitat, Greenhouses) have maintenance costs divided by `1.11^tier("life_support")`. At tier 0 = 1.0× cost, at tier 15 ≈ 0.21× cost.
- Robot throughput cap: sum maintenance demand mass vs robot capacity. Robot capacity scaled by `1.11^tier("construction")`.
- `degradation_delta = max(resource_shortfall, robot_shortfall) * days / 30`
- No self-repair: buildings hold at current degradation when fully maintained
- **Independent tracking**: Each building's degradation is independent. Starved siblings do not drag down fully-maintained buildings of the same type.

### Science Labs

- `CelestialBody::landing_science_value()` — Distance-scaled value per body
- Extraction uses diminishing returns: `10 * lv * (1 - e^(-N*t/15))`
- `lab_elapsed_years` tracked per colony

## Notification System (`notification.rs`)

### NotificationKind Enum
- `ColonyEstablished { colony_name }` — does not stop warp
- `ColonyFoodDepleted { colony_name }` — stops warp
- `ColonyResourceDepleted { colony_name, resource }` — does not stop warp
- `ColonyPowerLoss { colony_name }` — stops warp
- `ConstructionComplete { colony_name, building }` — defined but not pushed (construction monitored via colony UI)
- `SoiTransition { body_name }` — stops warp; pushed from `main.rs` when player ship changes SOI body

### Notification Struct
- `kind: NotificationKind`, `time: f64`, `read: bool`
- Stored in `Game.notifications: Vec<Notification>`

### Processing (main.rs)
After each render call: iterate unread notifications, push toast message to `render_state.active_toasts`, set warp_index=0 for warp-stopping notifications.

### Toast Rendering (`flight.rs`)
`render_toasts(ctx, toasts)` — fading egui::Area overlays at top-center. Fade out over last second (4-5s). Expired after 5s, cleaned up by main.rs.

## Colony Management UI (`colony_ui.rs`)

### GameMode::Colony

Full-screen colony management as a dedicated `GameMode::Colony`. Uses `ColonyScreenAction` enum for all user actions.

### ColonyScreenAction
```
None, QueueBuilding(body_index, BuildingType, count),
AddMineAssignment(body_index, ResourceType, count), RemoveMineAssignment(body_index, ResourceType, count),
AddCollectorAssignment(body_index, ResourceType, count), RemoveCollectorAssignment(body_index, ResourceType, count),
AddFactoryAssignment(body_index, FactoryRecipe, count), RemoveFactoryAssignment(body_index, FactoryRecipe, count),
ReturnToFlight, GoToTrackingStation, GoToMainMenu, ChangeWarp(idx), SwitchColony(body_index),
SetStorageAllocation { body_index, resource, percent }, UnpinStorageAllocation { body_index, resource },
DebugAddResource(body_index, ResourceType, kg), DebugAddBuilding(body_index, BuildingType),
DebugAddCrew(body_index, count)
```

Add/Remove assignment actions carry a `u32` count parameter specifying how many buildings to assign/unassign in one action (see Batch Assignment below). DebugAddResource routes Food to `colony.food_stored` instead of resource inventory. DebugAddCrew caps at `crew_capacity()`.

### render_colony_screen()
Full-screen egui layout called from `RenderState::render_colony()`. Accepts `solar_power_factor: f64` parameter for power display calculations.

#### Layout Architecture

Card-based UI with centralized color constants and extracted helper functions:

- **Color constants**: `COLOR_GREEN`, `COLOR_RED`, `COLOR_YELLOW`, `COLOR_ORANGE`, `COLOR_DEG_YELLOW`, `COLOR_GRAY`, `CARD_BG`
- **UI helpers**: `card_frame()` (dark card with rounded corners), `section_heading()` (14pt white strong), `status_indicator()` (label + value vertical pair)
- **Pre-computed data**: `ResourceRates` struct with production/consumption HashMaps, computed once by `compute_resource_rates()` before UI layout
- **Typography**: 14pt section headings, 12pt body labels, 11pt grid/table content

Panels:
- **Top panel**: Colony name heading, colony selector ComboBox (when multiple colonies), time warp buttons, date
- **Central panel**: ScrollArea with card-framed sections
- **Pause overlay**: "Return to Flight" (if came from flight), "Tracking Station", "Main Menu"

#### Card Sections

Each section rendered by a dedicated helper function, wrapped in `card_frame()` (fill `CARD_BG`, 12px inner margin, 6px rounding, 4px vertical outer margin).

**1. Overview card** — `render_overview_card()`
- Colony name (14pt strong heading), location (12pt gray)
- 2-column grid: Crew, Robots (count + crew needed, shown if >0), Food (with days and capacity), Storage, Power (net power color-coded green/red with "(CREW AT RISK)" suffix when `habitat_power_fraction < 1.0` and crew > 0)
- Crisis alert at bottom (red strong, only if `crew_at_crisis_start` is Some)

**2. Power card** — `render_power_card()`
- Net power summary at top (green/red, 12pt strong)
- Allocation warnings inline: Habitat % | Buildings %
- Combined production + demand striped grid (3 cols: Source, Count, Output/Draw) with green production and orange demand, separator row between sections
- Receiver Array row has a tooltip showing saturation explanation: "Laser-limited" (when laser power < receiver capacity) or "Receiver-limited" (when receiver capacity < laser power), with both values shown. Tooltip only appears when `receiver_laser_power_kw > 0`.

**3. Buildings card** — `render_buildings_card()`
- Batch size selector (11pt selectable labels: 1/10/100/1000/All, default 1)
- Building counts grid (excluding mines/factories/collectors)
- Mines sub-section with 13pt strong sub-heading, +/- buttons per resource
- Atmospheric Collectors sub-section (same pattern)
- Factories sub-section with recipe tooltips

**Batch Assignment**: Options: `1 | 10 | 100 | 1000 | All`. Default is 1. `All` (stored as 0) assigns/unassigns all available. Persists via `egui::Id("colony_batch_size")` in `ctx.data_temp`. Effective count is `min(batch_size, available)` (or all when batch_size == 0).

**Mines** use +/- buttons grouped by resource:
```
Mines (5x):
  Metal Ore: 3 (6.0 t/d)  [−] [+]
  Water: 1 (2.0 t/d)      [−] [+]
  Unassigned: 1
```
Only shows mineable resources from `body_mineable`. Labels show total production rate inline when count > 0, computed as `2000 * (1 - degradation) * other_power_fraction * mining_mult` per operational mine. [+] tooltip "Produces 2,000 kg/day".

**Atmospheric Collectors** same pattern. Labels show total production rate inline when count > 0, computed as `10,000 * (1 - degradation) * other_power_fraction * atmo_mult` per operational collector. [+] tooltip "Produces 10,000 kg/day".

**Factories** same pattern. Labels show total production rate inline when count > 0, based on primary output scaled by `(batches_per_day) * (1 - degradation) * other_power_fraction * factory_mult` per operational factory, where `factory_mult = 1.11^tier(recipe.efficiency_line_id())`. [+] tooltip shows recipe details (inputs → outputs, batch time, power). Recipes are filtered by `tech_tree.is_recipe_available(recipe.recipe_id())` — locked recipes are hidden unless already assigned (legacy/migration edge case). The [+] button is only shown for unlocked recipes. Assignment validation in `frames.rs` also guards against locked recipes via `is_recipe_available()`.

`FactoryRecipe::recipe_id()` returns the string ID matching `recipe_gates` in `data/tech/tree.ron`. `FactoryRecipe::efficiency_line_id()` returns the tech line governing throughput multiplier.

**4. Construction card** — `render_construction_card()`
- Progress bars (green fill) per queue item with estimated time remaining appended. Batch items display: `"Mine ×5 (3/5) — 1.2t / 3.0t — ~2.3d"`. Single items show: `"Mine (1.2t / 3.0t) — ~2.3d"`. Time format: `~Xh` when < 1 day, `~X.Xd` when >= 1 day, `stalled` when available capacity <= 0. Time is cumulative for queued items (each item's estimate includes all preceding items' remaining time). Available capacity = total robot capacity (scaled by construction tech multiplier) minus total maintenance demand.
- Batch size selector (1/10/100/All, default 1) persisted via `egui::Id("colony_build_batch_size")`
- "Queue:" ComboBox for adding buildings with tech gating (`tech_tree.is_building_available(bt)`), passes batch count

**5. Maintenance card** — `render_maintenance_card()`
- Striped grid (4 cols: Resource, Per 30d, In stock, Days left) with colored days
- Total maintenance, robot capacity summary
- Degraded buildings in yellow (`COLOR_DEG_YELLOW`)

**6. Resources card** — `render_resources_card()`
- Full-width card (`ui.set_min_width(ui.available_width())`)
- Storage header showing total used/capacity (uses `total_storage_mass()` to correctly account for unit-counted resources)
- Striped grid (7 cols: Resource, Amount, Cap, Production, Consumption, Days left, Alloc %)
- **Cap column**: per-resource allocated capacity (yellow when >95% full). Unit-counted resources (MirrorSegment, CollectorStation) display cap as unit count (`cap_kg / mass_per_unit`) rather than kg.
- **Alloc % column**: editable text input; editing pins the resource at that percentage. Right-click context menu to unpin (return to auto allocation). `ColonyScreenAction::SetStorageAllocation` / `UnpinStorageAllocation` variants.
- Mirror Segments displayed as count with mass in parentheses: `"5 (5.0 kg)"`. Production/consumption rates shown as `"+0.5/day"` / `"−0.5/day"` (no kg unit)
- Food row from `colony.food_stored`, production green, consumption orange (no Cap/Alloc % columns for food)
- Days left = `amount / (consumption - production)` when net negative; green infinity when net positive; blank when no rates
- Days left colored: red < 10d, yellow < 30d, white otherwise
- Uses pre-computed `ResourceRates`

Rate sources:
- **Production**: mines (2,000 kg/day × power_fraction × (1−degradation) × mining_mult), atmospheric collectors (10,000 kg/day × power_fraction × (1−degradation) × atmo_mult), factory outputs, greenhouse food
- **Consumption**: maintenance costs (per 30d / 30), factory inputs, reactor fuel, food (0.5 kg/crew/day)

**7. Debug section** — `CollapsingHeader` (no card frame, closed by default)
- **Add Resource**: ComboBox (all ResourceType including Food) + DragValue + "Add" button
- **Add Building**: ComboBox + "Add" button → instantly adds operational building
- **Add Crew**: DragValue + "Add" button (capped at crew_capacity)

Uses egui `data_temp` for persistent ComboBox/DragValue state.

### render_colony() on RenderState
`render_colony()` method in `menus.rs` follows tracking station wgpu pattern. Accepts `solar_power_factor: f64` parameter and passes it through to `render_colony_screen()`.
- Camera buffer update, surface texture
- Geometry pass (planets in background, camera tracks colony body)
- Egui pass with `render_colony_screen()`

### Entry Points
- **Flight HUD**: "Open Colony" button (replaces old popup toggle) when landed on a body with a colony. Click pushes `RenderRequest::OpenColony { body_index }` onto `RenderState::render_requests`; `main.rs` drains the queue and calls `game.enter_colony(bi, GameMode::Flight)`.
- **Tracking Station**: "Colonies" section in sidebar listing all colonies with "Open" buttons. `TrackingStationAction::OpenColony(body_index)`. `main.rs` calls `game.enter_colony(bi, GameMode::TrackingStation)`.
- **Colony Overview**: "Open" buttons per colony. `ColonyOverviewAction::OpenColony(body_index)`. `main.rs` calls `game.enter_colony(bi, GameMode::ColonyOverview)`.

### Navigation Flow
- `Game::enter_colony(body_index, from_mode)` — stores view state, sets `GameMode::Colony`
- `Game::leave_colony()` — returns to `colony_return_mode` (Flight, TrackingStation, or ColonyOverview)
- Pause menu in colony screen: can go to tracking station, main menu, or return to flight

### Request Processing (main.rs)
`render_colony_frame()` handles all `ColonyScreenAction` variants directly — no intermediate RenderState fields needed.

## Main Loop Integration

### Flight Frame (`render_flight_frame`)
1. Before render: set `can_establish_colony`, `has_colonies`, `landed_body_index`
2. In simulation block: `game.update_colonies(dt_sim)`
3. After render: drain `RenderState::render_requests` (handles `EstablishColony`, `TransferCargo`, `OpenColony`, etc.), then process notifications and toast cleanup

### Main Menu Frame (`render_main_menu_frame`)
1. In simulation block: `game.update_colonies(dt_sim)`
2. After render: process notifications, toast cleanup

### Tracking Station Frame (`render_tracking_station_frame`)
1. In simulation block: `game.update_colonies(dt_sim)`
2. Before render: set `has_colonies`
3. After render: handle colony UI requests, notifications, toast cleanup

## Save System

`SaveGame.notifications: Vec<Notification>` with `#[serde(default)]` for backward compatibility. Persisted in `from_game()`, restored in `restore_to_game()`.

All new `Colony` fields (`lab_science_extracted`, `lab_elapsed_years`, `habitat_power_fraction`, `other_power_fraction`, `habitat_unpowered_notified`, `crew_at_crisis_start`, `crew_death_accumulator`, `receiver_power_kw`, `receiver_laser_power_kw`) use `#[serde(default)]`. Power fractions default to `1.0`.
