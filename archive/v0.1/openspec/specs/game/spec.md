# Game

Game-wide requirements shared across all modes (editor and flight).

## Game Modes

### Requirement: Nine game modes

The system SHALL support nine game modes represented by the `GameMode` enum: `TitleScreen`, `MainMenu`, `Editor`, `Flight`, `TrackingStation`, `Colony`, `ColonyOverview`, `Management`, `TechTree`.

#### Scenario: Game starts on title screen
- **WHEN** the application launches
- **THEN** the game mode SHALL be `TitleScreen`

#### Scenario: New game
- **WHEN** the player clicks "New Game" on the title screen and enters a save name
- **THEN** the game SHALL reset to fresh state and transition to `MainMenu`

#### Scenario: Load game
- **WHEN** the player clicks "Load Game" on the title screen and selects a save
- **THEN** the game SHALL restore the saved state and transition to `MainMenu`

#### Scenario: Mode switching
- **WHEN** the player launches a vessel from the editor
- **THEN** the game mode SHALL switch to `Flight`

## Save System

### Requirement: Save game persistence

The system SHALL persist game state in folder-based saves at `data/saves/{sanitized_name}/save.ron` as RON-serialized `SaveGame` structs. Legacy flat files (`data/saves/{name}.ron`) are supported for loading via fallback.

### Requirement: SaveGame format

Each save file SHALL contain: format version (u32), save name, simulation_time, all vessels (active first, then inactive) with ship state and optional FlightVessel, next_vessel_id, debris_counter, blueprint copies, and editor vessel name.

### Requirement: Auto-save

The system SHALL auto-save every 5 minutes when a game is loaded (i.e., `save_name` is `Some`).

### Requirement: Save on quit

When quitting from any game mode to the title screen, the system SHALL save the current game state before transitioning.

### Requirement: Quicksave

The system SHALL support quicksaving from the flight pause overlay. Each quicksave creates an indexed file `data/saves/{name}/quicksave_{N}.ron` where N increments from the highest existing index. Quicksaves use the same `SaveGame` format as main saves.

### Requirement: Load quicksave

The flight pause overlay SHALL show a "Load Quicksave" button (when quicksaves exist) that opens a scrollable list of available quicksaves sorted newest-first. Selecting a quicksave restores that state and unpauses.

### Requirement: Save directory layout

```
data/saves/
  {sanitized_name}/
    save.ron              <- main save (auto-save, save-on-quit)
    quicksave_1.ron       <- first quicksave
    quicksave_2.ron       <- second quicksave
```

### Requirement: Legacy save compatibility

`load_from_file()` SHALL first check for `data/saves/{id}/save.ron`, then fall back to `data/saves/{id}.ron`. `list_saves()` SHALL find both folder-based and legacy flat saves, with folders taking priority for deduplication.

### Requirement: Save file listing

`SaveFileInfo` uses `save_id` (sanitized directory name) rather than a filename. The title screen load dialog passes `save_id` to `LoadGame`.

## Coordinate System

### Requirement: Double-precision positions

All physics positions SHALL use `f64` (double precision) to prevent precision loss at large distances. Rendering vertices SHALL use `f32`, with conversion happening at render time.

### Requirement: Grid square size

Each grid square SHALL measure 0.5m x 0.5m, defined by `GRID_SQUARE_SIZE = 0.5`. All part dimensions in grid squares are converted to meters by multiplying by this constant.

## Part Size System

### Requirement: Five part sizes

The system SHALL support five part sizes with the following grid widths:

| Size   | Grid Width | Meters |
|--------|-----------|--------|
| Tiny   | 1         | 0.5m   |
| Small  | 3         | 1.5m   |
| Medium | 5         | 2.5m   |
| Large  | 9         | 4.5m   |
| XL     | 13        | 6.5m   |

## Part Categories

### Requirement: Nine part categories

The system SHALL support nine part categories in this order: Pods ("Command"), Propulsion ("Engines"), FuelTanks ("Fuel Tanks"), Structural ("Structural"), Aerodynamic ("Aerodynamic"), Utility ("Utility"), Electricity ("Electricity"), Interstellar ("Interstellar"), Cargo ("Cargo").

### Requirement: Root part eligibility

Only parts with category `Pods` SHALL be eligible as root parts, as determined by `can_be_root()`.

## Propellant System

### Requirement: Eight propellant types

The system SHALL support eight propellant types. The first three pair an oxidizer (LOX) with a fuel; the remaining five are standalone:

| Propellant    | Display       | Fuel Type      | O2 (kg/sq) | Fuel (kg/sq) |
|--------------|--------------|---------------|------------|-------------|
| Kerolox      | LOX/RP-1     | Rp1           | 470.0      | 185.0       |
| Methalox     | LOX/CH4      | Methane       | 270.0      | 75.0        |
| Hydrolox     | LOX/LH2      | Hydrogen      | 155.0      | 25.0        |
| Hydrogen     | Hydrogen     | Pure H2 (NTR) | —          | —           |
| Xenon        | Xenon        | Electric prop  | —          | —           |
| FusionFuel   | D+He3        | Fusion         | —          | —           |
| Antimatter   | Antimatter   | AM engines     | —          | —           |
| NuclearPulse | Nuclear Pulse| Orion-style    | —          | —           |

### Requirement: Propellant capacity scaling

Tank propellant capacity SHALL scale linearly with `grid_area` (f64, volume-equivalent units). A tank with grid_area N stores `N * oxygen_per_sq` kg of oxidizer and `N * fuel_per_sq` kg of fuel. The `grid_area` value uses geometric mean scaling (p=1.5 power law) — halfway between 2D area and 3D volume — to prevent wide rockets from getting disproportionately more fuel per thrust.

For cylindrical tanks: `grid_area = w^1.5 * h / sqrt(5)`, where w and h are grid dimensions. This produces the same values as 2D area for medium (w=5) tanks, less capacity for narrower tanks, and more for wider tanks — but less aggressively than full 3D cylinder volume.

For spherical tanks: `grid_area = sqrt(d^2 * V_sphere / 0.491)`, the geometric mean of 2D cross-section area and 3D sphere volume.

### Requirement: Tank dry mass

Tank dry mass SHALL be the `mass` field from the RON part definition (in tonnes), derived from `grid_area * 0.491 * structural_density / 1000`. Structural densities: standard 71.7 kg/m^3, H2 19.8 kg/m^3, xenon 28.1 kg/m^3, fusion 16.2 kg/m^3.

## Part Definitions

### Requirement: RON-based part definitions

Part definitions SHALL be loaded from RON files in the `data/parts/` directory. Each file contains a `PartDefinitionFile` with a `parts` Vec of `PartDefinition` structs. Parts are keyed by their `id` string in the `PartDefinitions` registry.

### Requirement: Part definition fields

Each `PartDefinition` SHALL include: `id`, `name`, `description`, `category`, `mass` (dry, in tonnes), `cost`, `size`, `shape`, `grid_width` (visual, decimal), `grid_height` (visual, decimal), optional `top_width` (for trapezoids), optional `hitbox_width`/`hitbox_height` overrides, and optional component data (`engine`, `tank`, `pod`, `decoupler`, `fairing`, `rcs`).

### Requirement: Part shapes

The system SHALL support three part shapes: `Rectangle`, `Triangle` (base at bottom), and `Trapezoid` (with `top_width` and bottom width).

### Requirement: Wet mass calculation

A part's wet mass SHALL equal `dry_mass + sum(resource_values_kg) / 1000.0`, where resources are stored in kg and mass in tonnes.

## Hitbox System

### Requirement: Three hitbox types

Each part SHALL have three distinct bounds:

| Type           | Purpose              | Dimensions                                      |
|---------------|---------------------|------------------------------------------------|
| Visual sprite  | What is drawn        | `grid_width * 0.5m` x `grid_height * 0.5m` (decimal) |
| Build hitbox   | Collision detection  | `hitbox_grid_width` x `hitbox_grid_height` (integer grid squares, defaults to `ceil(visual)`) |
| Weld hitbox    | Connection detection | Build hitbox * 1.05 (5% padding via `WELD_HITBOX_PADDING`) |

## Rendering Pipeline

### Requirement: wgpu colored-triangle rendering

All rendering SHALL use wgpu with a custom vertex shader (`shader.wgsl`) and 4x MSAA. The vertex format is `Vertex { position: [f32; 2], color: [f32; 4] }` -- everything is colored triangles with no textures.

## UI Framework

### Requirement: egui immediate-mode UI

All UI SHALL be rendered using egui 0.27 in immediate mode. The egui integration SHALL consume input events before game input handlers, preventing input passthrough when the pointer is over UI.

### Requirement: FPS overlay

A non-interactive FPS counter SHALL be displayed in the top-right corner of every screen (flight, editor, title, main menu, tracking station, colony, colony overview, management, tech tree). It uses an exponential moving average (`fps` field on `RenderState`) and renders as a small grey label via `fps_overlay()` in `state.rs`.

## Component Data

### Requirement: Engine data

Engine definitions SHALL include: `thrust_vac` (kN), `thrust_asl` (kN), `isp_vac` (s), `isp_asl` (s), `throttleable` (bool), `gimbal_range` (degrees, 0 = fixed), and `propellant` type.

### Requirement: Pod data

Pod definitions SHALL include: `crew_capacity` and `torque` (reaction wheel torque).

### Requirement: Decoupler data

Decoupler definitions SHALL include: `ejection_force` (kN).

### Requirement: Fairing data

Fairing definitions SHALL include: `ejection_force` (kN, used when jettisoning the fairing shell).

### Requirement: Tank data

Tank definitions SHALL include: `grid_area` (f64, volume-equivalent units) for capacity calculation. The value uses geometric mean scaling (w^1.5 * h / sqrt(5) for cylinders), normalized so medium (w=5) tanks match 2D area values.
