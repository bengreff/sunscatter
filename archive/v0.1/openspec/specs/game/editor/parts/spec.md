# Parts

Part selection, placement, dragging, deletion, and info display in the vehicle editor.

## Part Palette

### Requirement: Category tabs

The parts palette SHALL display selectable tabs for each `PartCategory`. The default selected category SHALL be `Pods`.

### Requirement: Tech tree gating

Parts in the palette SHALL be filtered by `tech_tree.is_part_available(&part.name)`. Only parts unlocked via the tech tree or in the default unlocked list are shown. This filter applies to both the Interstellar flat list and the size-grouped categories.

### Requirement: Size grouping

Within a selected category, parts SHALL be grouped by `PartSize` under collapsible headers (default open). Sizes with no unlocked parts in the current category SHALL be hidden entirely. Size categories: Tiny (1 grid square, 0.5m), Small (3, 1.5m), Medium (5, 2.5m), Large (9, 4.5m), XL (13, 6.5m). Categories: Command, Engines, Fuel Tanks, Structural, Aerodynamic, Utility, Electricity, Interstellar.

### Requirement: Flat Interstellar palette

The Interstellar category SHALL display parts in a flat list sorted by width then height, without size sub-grouping or collapsible headers. All other categories use the standard size-grouped display.

### Requirement: Palette selection toggle

Clicking a part in the palette SHALL select it. Clicking an already-selected part SHALL deselect it. Selecting a palette part SHALL clear any placed-part selection.

## Ghost Preview

### Requirement: Ghost follows cursor

When a palette part is selected, a ghost preview SHALL appear at the cursor position, updated on every mouse move via `update_ghost()`.

### Requirement: Ghost grid snapping

The ghost SHALL snap to the grid using editor hitbox dimensions: parts with odd hitbox width snap to grid square centers (`floor(x / grid) * grid + grid/2`), parts with even hitbox width snap to grid lines (`floor(x / grid + 0.5) * grid`). X and Y axes snap independently. All engine hitbox widths SHALL be odd to ensure grid-center alignment for vertical stacking.

### Requirement: Ghost validity coloring

The ghost SHALL render in green (`[0.3, 0.9, 0.3, 0.4]`) when placement is valid and red (`[0.9, 0.3, 0.3, 0.4]`) when invalid. For procedurally-rendered parts (engines, pods, decouplers), the part renders at alpha 0.5 with a validity overlay on top.

### Requirement: Ghost overlap check

The ghost SHALL be marked invalid when its hitbox AABB overlaps any existing part's hitbox AABB. Touching edges (exact boundary contact) SHALL NOT count as overlap.

### Requirement: Weld adjacency check

The ghost SHALL be marked invalid unless its welding hitbox (build hitbox * 1.05) overlaps at least one existing part's welding hitbox. This check is skipped when placing the first part (no existing parts). Decouplers SHALL have an extended upward reach of 10 grid squares (5.0m) for the weld adjacency check, allowing them to be placed up to 10 squares below the part they attach to.

### Requirement: Mirror ghost preview

When symmetry mode is Mirror and a palette part is selected, the editor SHALL display two ghost previews: the primary ghost at the cursor position (grid-snapped) and a mirrored ghost reflected across the center line. The mirrored ghost X position SHALL be `center_line_x * 2 - primary_x`. The mirrored ghost Y position SHALL equal the primary ghost Y position.

### Requirement: Mirror-aware ghost shape

When the selected part definition has a `mirror_def_id` field set, the mirrored ghost SHALL render using the mirror definition's shape instead of the primary definition's shape. If `mirror_def_id` references a nonexistent definition, the primary definition SHALL be used as fallback. Symmetric parts (those without `mirror_def_id`) SHALL use the same definition for both ghosts.

#### Scenario: Mirror ghost positions

- **WHEN** symmetry mode is Mirror, center line is at X = 2.0, and the cursor snaps to `[3.5, 1.0]`
- **THEN** the primary ghost SHALL be at `[3.5, 1.0]` and the mirrored ghost SHALL be at `[0.5, 1.0]`

#### Scenario: Mirror ghost on center line

- **WHEN** symmetry mode is Mirror and the primary ghost snaps to the center line X coordinate
- **THEN** only one ghost SHALL be displayed (the part is on the center line, no mirror needed)

### Requirement: Mirror ghost validity

Both mirror ghosts SHALL share the same validity state. The ghost pair SHALL be valid only when NEITHER ghost overlaps any existing part AND both ghosts are weld-connected (the mirror ghost must touch at least one existing part or the primary ghost via welding hitbox overlap). Both ghosts SHALL render in green when valid and red when invalid.

#### Scenario: One ghost overlaps

- **WHEN** symmetry mode is Mirror, the primary ghost has no overlap, but the mirrored ghost overlaps an existing part
- **THEN** both ghosts SHALL render in red (invalid) and placement SHALL be blocked

#### Scenario: Both ghosts clear

- **WHEN** symmetry mode is Mirror and neither ghost overlaps any existing part
- **THEN** both ghosts SHALL render in green (valid) and placement SHALL be allowed

## Placement

### Requirement: Left-click placement

Left-clicking on empty space with a valid ghost SHALL place the selected part at the ghost position. Placement requires: a palette part is selected, the ghost position exists, and the ghost is valid.

### Requirement: First part becomes root

The first part placed SHALL become the root part, regardless of its category.

### Requirement: Auto-stage engines, decouplers, and fairings

When an engine, decoupler, or fairing base is placed, it SHALL be automatically added to stage 0. If no stages exist, a new stage SHALL be created.

### Requirement: Mirror part placement

When symmetry mode is Mirror and placement is valid, left-clicking SHALL place two parts simultaneously: the primary part at the ghost position and the mirrored part at the mirrored position. Both parts SHALL be assigned unique `PlacedPartId` values and linked via `mirror_partner` fields pointing to each other.

### Requirement: Mirror-aware part definition

When placing a mirrored pair, the mirrored part SHALL use the `mirror_def_id` definition if the primary part's definition specifies one and it exists in the registry. Otherwise, the mirrored part SHALL use the same definition as the primary part. This enables asymmetric parts (e.g., right side nose cone) to automatically use their counterpart (left side nose cone) for the mirror copy.

#### Scenario: Asymmetric mirror placement

- **WHEN** the user places a right side nose cone (`nosecone_side_right_small`, `mirror_def_id: "nosecone_side_left_small"`) in Mirror mode
- **THEN** the primary part SHALL use `nosecone_side_right_small` and the mirrored part SHALL use `nosecone_side_left_small`

#### Scenario: Symmetric mirror placement

- **WHEN** the user places a fuel tank (no `mirror_def_id`) in Mirror mode
- **THEN** both parts SHALL use the same definition ID

#### Scenario: Mirror placement creates linked pair

- **WHEN** the user clicks to place a part in Mirror mode at `[3.5, 1.0]` with center line at X = 2.0
- **THEN** two parts SHALL be placed: one at `[3.5, 1.0]` and one at `[0.5, 1.0]`, each with `mirror_partner` pointing to the other's ID

#### Scenario: Center line placement creates single part

- **WHEN** the user clicks to place a part in Mirror mode and the ghost is on the center line
- **THEN** only one part SHALL be placed with `mirror_partner = None`

#### Scenario: Mirror placement auto-staging

- **WHEN** two engines are placed via mirror placement
- **THEN** both engines SHALL be added to stage 0

### Requirement: Monotonic part IDs

Part IDs SHALL be monotonically increasing, starting from 1 and incrementing on each placement.

## Selection

### Requirement: Click to select placed part

Left-clicking on a placed part SHALL select it and clear any palette selection. If the clicked part has a `mirror_partner`, both parts SHALL render with the selected color.

#### Scenario: Select mirrored part highlights both

- **WHEN** the user clicks on a placed part that has a mirror partner
- **THEN** both the clicked part and its mirror partner SHALL render with the selected color `[0.5, 0.7, 1.0, 1.0]`

### Requirement: Hover highlight

Moving the cursor over a placed part SHALL highlight it. Hit testing SHALL use hitbox dimensions (not visual dimensions) for AABB point-in-rect testing.

### Requirement: Bidirectional staging-grid selection linking

Selecting a placed part on the build grid SHALL highlight its corresponding entry in the staging panel with a blue color (`Color32::from_rgb(128, 179, 255)`). If the selected part has a `mirror_partner` in the same stage, the combined "x2" entry SHALL also highlight. Clicking or drag-starting a part entry in the staging panel SHALL select that part on the build grid, clearing any palette selection and ghost preview.

#### Scenario: Grid selection highlights staging entry

- **WHEN** the user selects a placed engine on the build grid that is in stage 1
- **THEN** the engine's label in the staging panel SHALL render with highlight color

#### Scenario: Staging click selects grid part

- **WHEN** the user clicks a part label in the staging panel
- **THEN** that part SHALL become the selected placed part on the grid and the part info panel SHALL appear

#### Scenario: Mirror pair highlight via staging

- **WHEN** the user selects one engine of a mirror pair on the grid
- **THEN** the combined "x2" entry in the staging panel SHALL highlight

## Part Info Panel

### Requirement: Info panel visibility

The part info panel SHALL appear when either a palette part or a placed part is selected.

### Requirement: Common part info

The info panel SHALL display: name, description, size, mass (tonnes and kg), cost, and grid dimensions for all parts.

### Requirement: Engine info

For engines, the info panel SHALL display: propellant type (including secondary propellant if present, e.g. "D+He3 + Antimatter"), thrust (vacuum and sea level), specific impulse (vacuum and sea level), per-component mass flow rates at vacuum (e.g. "LOX: 47.3 kg/s", "RP-1: 18.6 kg/s"), gimbal range (or "Fixed" if 0), throttleable status, single-engine vacuum TWR on Earth, and waste heat at full throttle (only when `engine.waste_heat_watts > 0`). Mass flow rates are formatted as g/s for very small flows, kg/s for normal, t/s for huge.

### Requirement: Reactor info

For reactors, the info panel SHALL display: continuous power output and (when non-zero) waste heat in Watts via SI-prefix formatting (MW / GW / TW).

### Requirement: Radiator info

For radiators, the info panel SHALL display: tier label (Heat Pipe / Liquid Droplet / Phononic Metamaterial) with operating temperature, full-deployment rejection capacity, deploy time, and a note that the wing must be deployed in vacuum.

### Requirement: Thermal stats row

The editor stats bar SHALL display a "Thermal: X / Y" row when any placed part has `waste_heat_watts > 0` or `radiator.rejection_watts > 0`. X is the sum of engine + reactor waste heat (engines at full throttle); Y is the sum of radiator rejection capacity (assuming all deployed). The row is colored green when `Y ≥ X`, yellow within a 10% margin, red on deficit. Tooltip explains the thermal cascade thresholds.

### Requirement: Tank info for palette selection

For tanks selected in the palette, the info panel SHALL display: dry mass (from `def.mass`, the RON part mass). Tanks without a `fixed_fuel_type` SHALL show propellant capacity for the three common chemical fuels (RP-1, CH4, LH2). Tanks with a `fixed_fuel_type` SHALL display a single locked-fuel label plus the capacity for that fuel.

### Requirement: Tank info for placed parts

For placed tanks, the info panel SHALL additionally display: fuel type selector buttons (only for standard tanks), draggable fuel bars showing current and max amounts, Fill/Empty convenience buttons, and dry/propellant/total mass breakdown. Selecting a non-Empty fuel type SHALL automatically set `fill_fraction` to 1.0. Selecting Empty SHALL set `fill_fraction` to 0.0.

The fuel-type selector SHALL filter `FuelType::all()` by `FuelType::is_standard_tank_compatible()`, exposing only the seven fuels valid for standard tanks: Empty, LOX/RP-1, LOX/CH4, LOX/LH2, Monopropellant, LH2 (pure), D+He3 (fusion fuel). Xenon, Antimatter, and Pulse Units SHALL be excluded — they require their dedicated specialized tanks.

For tanks whose `TankData::fixed_fuel_type` is `Some(ft)` (Xenon, Antimatter, and Pulse magazines), the selector SHALL be replaced by a single "Fuel Type: {ft} (locked)" label, and the placed part SHALL be initialized with `fuel_type = ft` and `fill_fraction = 1.0` upon placement.

### Requirement: Draggable tank fill bars

Each fuel bar SHALL be a custom-drawn interactive rectangle that responds to click and drag. Clicking or dragging on a bar SHALL set `fill_fraction` (0.0–1.0) based on the mouse X position relative to the bar. Both the oxidizer and fuel bars control the same `fill_fraction` (they scale together). The oxidizer bar SHALL only be displayed when the fuel type produces oxidizer (i.e., `ox_cap > 0.0`). Monopropellant tanks SHALL show only the fuel bar.

### Requirement: Fill fraction data model

`PlacedPart` SHALL use `fill_fraction: f64` (0.0–1.0) instead of `tank_filled: bool`. `BlueprintPart` SHALL include both `fill_fraction: f64` and `tank_filled: bool` (serde-defaulting) for backward compatibility with saved blueprints. When loading, `fill_fraction` takes priority; if zero, `tank_filled: true` maps to `fill_fraction: 1.0`.

### Requirement: Battery info

For batteries, the info panel SHALL display: capacity in Wh (e.g. "Capacity: 5,000 Wh"). In flight, it SHALL additionally show a progress bar with "current / max Wh".

### Requirement: Solar panel info

For solar panels, the info panel SHALL display: output at Earth distance (1 AU) in Watts (e.g. "Output @1AU: 450 W") and an "Extend"/"Retract" button toggling `PlacedPart.deployed`. When the panel has a `mirror_partner`, the toggle SHALL apply to both parts. In flight, the info popup SHALL display current output adjusted for distance from the Sun using inverse-square law, multiplied by `deploy_fraction`, and an "Extend"/"Retract" button.

### Requirement: RTG info

For RTGs, the info panel SHALL display: constant output in Watts (e.g. "Output: 300 W").

### Requirement: Reactor info

For reactors, the info panel SHALL display: constant output in Watts (e.g. "Output: 500 MW"). Reactors generate power using the same constant-output pattern as RTGs but at much higher levels.

### Requirement: Shield info

For shields, the info panel SHALL display: shield type (Whipple, FRES, or Geodesic), maximum rated velocity as a fraction of c, and base power consumption in Watts (0 for passive Whipple shields).

### Requirement: Radiator info

For radiators, the info panel SHALL display: heat rejection capacity in Watts (e.g. "Heat Rejection: 2.0 GW").

### Requirement: Pod info

For pods, the info panel SHALL display: crew capacity.

### Requirement: Built-in pod RCS

Command pods MAY include built-in RCS thrusters and monopropellant. When a pod definition has `rcs: Some(...)` and `resources: {"monopropellant": N}`, the pod SHALL provide attitude control via bilateral RCS nozzles without requiring separate RCS blocks. Built-in pod RCS uses the same fuel zone system as standalone RCS — monopropellant in a pod above a non-crossfeed decoupler is isolated from tanks below it.

### Requirement: RCS info

For RCS thrusters, the info panel SHALL display: thrust (kN), specific impulse (seconds), and fuel type (Monopropellant).

### Requirement: RCS part variants

RCS thrusters SHALL be thin side-mount parts (0.5 grid wide, 1x1 hitbox). Small RCS (RV-1) has `grid_height: 0.75` for a compact visual; medium RCS (RV-3) has `grid_height: 1.0`. Each RCS part SHALL have a mirror variant: right-mount (default) and left-mount (`is_mirrored: true`). Mirror variants SHALL be linked via `mirror_def_id` for automatic mirror-mode placement. Parts: `rcs_small`/`rcs_small_left`, `rcs_medium`/`rcs_medium_left`.

### Requirement: Decoupler info for placed parts

For placed decouplers, the info panel SHALL display a "Fuel Crossfeed" checkbox toggling `crossfeed_enabled`.

### Requirement: Radial decoupler

`DecouplerData` SHALL include an `is_radial: bool` field (serde-default `false`). Radial decouplers separate sideways (disconnecting side-mounted parts) instead of using Y-position-based stack separation. They are rendered as a simple dark rectangle without the ring band or adapter trapezoid. The TT-38K Radial Decoupler is a Tiny-size (1x2) radial decoupler with 10 kN ejection force.

### Requirement: Fairing base part definitions

`PartDefinition` SHALL support an optional `fairing: Option<FairingData>` field. `FairingData` contains `ejection_force: f64` (kN, used when jettisoning). Five fairing base parts SHALL exist in `data/parts/structural.ron` under the `Aerodynamic` category:

| ID | Name | Size | Grid | Mass | Ejection Force | Heat Tolerance |
|----|------|------|------|------|----------------|----------------|
| `fairing_tiny` | AE-FF0 Fairing | Tiny | 1x1 | 0.01t | 5 kN | 2000 K |
| `fairing_small` | AE-FF1 Fairing | Small | 3x1 | 0.05t | 15 kN | 2000 K |
| `fairing_medium` | AE-FF2 Fairing | Medium | 5x1 | 0.1t | 30 kN | 2000 K |
| `fairing_large` | AE-FF3 Fairing | Large | 9x1 | 0.2t | 60 kN | 2000 K |
| `fairing_xl` | AE-FF4 Fairing | XL | 13x1 | 0.4t | 100 kN | 2000 K |

All fairing bases have `shape: Rectangle` and `grid_height: 1.0` (1 grid square tall). The hitbox and visual height are identical — the disc fills the entire 1-square hitbox.

### Requirement: Linked part info panel

When a part with a `mirror_partner` is selected, changes made in the part info panel SHALL apply to both parts. This includes fuel type selection, fill fraction (via drag, Fill, or Empty), and crossfeed toggle.

#### Scenario: Fuel type change applies to both

- **WHEN** the user changes the fuel type of a selected tank that has a mirror partner
- **THEN** both the selected tank and its mirror partner SHALL update to the new fuel type

### Requirement: Delete button for placed parts

The info panel for placed parts SHALL include a "Delete Part" button. If the part has a `mirror_partner`, the button label SHALL read "Delete Parts (x2)" and clicking it SHALL delete both parts.

#### Scenario: Delete button for mirrored pair

- **WHEN** a mirrored part is selected and the user clicks the delete button
- **THEN** both the selected part and its mirror partner SHALL be deleted

## Dragging

### Requirement: Drag offset

When a drag begins, the offset between the part center and the mouse cursor SHALL be recorded. During the drag, this offset SHALL be applied so the part does not jump to the cursor position.

### Requirement: Drag grid snapping

During a drag, the part position SHALL snap to the grid using the same rules as ghost snapping. If the part has a `mirror_partner`, the partner SHALL move to the mirrored position relative to the center line. Both positions update in real-time.

#### Scenario: Drag mirrored pair

- **WHEN** a part with a mirror partner is dragged to a new position
- **THEN** the partner SHALL move to the position reflected across the center line

### Requirement: Drag overlap detection

During a drag, overlap SHALL be checked against all parts except the one being dragged. When dragging a mirrored pair, overlap SHALL be checked for both parts, excluding both from the overlap check. Invalid drag positions SHALL show a red overlay on both parts.

#### Scenario: Mirrored drag overlap

- **WHEN** a mirrored pair is being dragged and the partner's mirrored position overlaps another part
- **THEN** both parts SHALL show a red overlay indicating invalid position

### Requirement: Revert on invalid drop

When a drag finishes at an overlapping position, the part SHALL revert to its pre-drag position. When dragging a mirrored pair, if either position is invalid, both parts SHALL revert to their pre-drag positions.

#### Scenario: Mirrored drag revert

- **WHEN** a mirrored pair drag finishes with the partner overlapping another part
- **THEN** both parts SHALL revert to their pre-drag positions

### Requirement: Right-click cancels drag

Right-clicking during a drag SHALL cancel the drag and revert the part to its pre-drag position.

## Deletion

### Requirement: Delete key

Pressing Delete or Backspace with a placed part selected SHALL delete that part. If the part has a `mirror_partner`, both parts SHALL be deleted.

#### Scenario: Delete mirrored pair via keyboard

- **WHEN** the user presses Delete with a mirrored part selected
- **THEN** both the selected part and its mirror partner SHALL be removed

### Requirement: Right-click delete

Right-clicking with a placed part selected (no palette selection, no drag active) SHALL delete that part. If the part has a `mirror_partner`, both parts SHALL be deleted.

#### Scenario: Right-click delete mirrored pair

- **WHEN** the user right-clicks with a mirrored part selected
- **THEN** both the selected part and its mirror partner SHALL be removed

### Requirement: Stage cleanup on delete

Deleting a part SHALL remove it from all stages. When deleting a mirrored pair, both parts SHALL be removed from all stages. Empty stages SHALL be removed.

#### Scenario: Stage cleanup for mirrored pair

- **WHEN** a mirrored pair is deleted and both were in stage 0
- **THEN** both parts SHALL be removed from stage 0 and stage 0 SHALL be removed if empty

### Requirement: Root reassignment on delete

If the deleted part was the root, the root SHALL be reassigned to any remaining part. If no parts remain, root becomes `None`.

## Mirror Partner Data Model

### Requirement: Mirror partner field

`PlacedPart` SHALL have a `mirror_partner: Option<PlacedPartId>` field. When two parts are mirror-placed, each SHALL reference the other. Parts placed without mirroring or on the center line SHALL have `mirror_partner = None`.

### Requirement: Mirror definition field

`PartDefinition` SHALL have a `mirror_def_id: Option<String>` field (serde-defaulting to `None`). When set, mirror placement and ghost rendering SHALL use the referenced definition for the mirrored copy. The field SHALL form bidirectional pairs: if part A references part B, part B SHALL reference part A.

#### Scenario: Bidirectional link

- **WHEN** part A (ID=5) and part B (ID=6) are mirror-placed
- **THEN** part A's `mirror_partner` SHALL be `Some(6)` and part B's `mirror_partner` SHALL be `Some(5)`

## Hitbox System

### Requirement: Odd editor hitbox widths

All part editor hitbox widths SHALL be odd numbers. Odd widths snap parts to grid square centers, ensuring consistent vertical alignment when stacking parts. Even widths snap to grid lines, causing misalignment.

### Requirement: Flight hitbox fields

`PartDefinition` SHALL support optional `flight_hitbox_width: Option<f64>` and `flight_hitbox_height: Option<f64>` fields (serde-defaulting to `None`). When set, these define the flight collision box dimensions. When not set, they default to the editor hitbox dimensions. Values are in grid squares and can be fractional to allow precise collision boundaries matching the visible sprite content.

**Every engine SHALL set both `flight_hitbox_width` and `flight_hitbox_height`**, measured from the visible content of the engine sprite. Engines without these fields fall back to the editor placement hitbox, which is typically larger than the engine sprite content and causes the engine to appear "floating" above an empty hitbox area beneath the nozzle. The convention applies to chemical, NTR, electric, and interstellar engines alike: visible content extent comes from `Image.getbbox()` on the source sprite PNG, converted to grid units by `pixel_count × grid_dim / canvas_pixel_dim`. The convention preserves "top of nozzle aligns with grid" — the engine sprite is top-aligned within the editor hitbox via `hitbox_y_offset` (see requirement below).

For engines, `flight_hitbox_height` represents the visible content height (excluding transparent sprite padding), NOT the sprite quad height. The sprite quad height is computed from the image's natural aspect ratio (`flight_hitbox_width * pixel_height / pixel_width`) to avoid distortion. This decouples collision from rendering.

Accessor methods:
- `flight_hitbox_grid_width()` / `flight_hitbox_grid_height()` → `f64` (grid squares, can be fractional)
- `flight_hitbox_width_m()` / `flight_hitbox_height_m()` → `f64` (meters)

### Requirement: Engine sprite alignment in editor

Engine sprites SHALL be centered horizontally within the editor hitbox and snapped to the top of the editor hitbox height. The sprite quad width uses `flight_hitbox_width` and the height is computed from the sprite image's pixel aspect ratio (`flight_hitbox_width * pixel_height / pixel_width`) to preserve the natural proportions. The y_offset for sprite rendering is `(editor_half_h - sprite_half_h)`, which is zero when heights match and positive (shifting upward) when the sprite is shorter than the editor hitbox. `SpriteRect` stores `pixel_width` and `pixel_height` fields for this purpose.

### Requirement: Flight hitbox in collision

`FlightPart.hitbox_half_extents` SHALL be set from `flight_hitbox_width_m()` and `flight_hitbox_height_m()`, not the editor hitbox. This ensures collision detection uses the tighter flight hitbox matching the visible sprite, not the wider editor placement hitbox.

### Requirement: Hitbox Y offset for top-aligned parts

`FlightPart.hitbox_y_offset` (f64, serde-default 0.0) SHALL store the vertical offset from `local_position` to the flight hitbox center. For engines (top-aligned sprites): `hitbox_y_offset = (def.hitbox_height() - def.flight_hitbox_height_m()) / 2.0`. For all other parts: 0.0.

This offset SHALL be applied in:
- `bottom_extent()`: bottom = `local_position[1] + hitbox_y_offset - hitbox_half_extents[1]`
- `bounding_half_height()`: center_y = `local_position[1] + hitbox_y_offset`
- `check_terrain_collision()`: hitbox corner Y positions offset by `hitbox_y_offset`
- Flight click detection (`click_local_y`): engines use `local_position[1] + hitbox_y_offset`

This offset SHALL NOT be applied in `is_engine_covered()` / `is_engine_covered_simulated()`, which check editor-placement adjacency.

### Requirement: Engine editor highlight matches flight hitbox

In the editor, selection/hover/drag-invalid highlight overlays for engines SHALL use flight hitbox dimensions (`flight_hitbox_width_m` x `flight_hitbox_height_m`) positioned with the hitbox_y_offset, not the editor hitbox (`grid_width` x `grid_height`). This ensures the highlight rectangle matches the visible engine content.

## Fairing Build Mode

### Requirement: FairingShape data model

`FairingShape` in `blueprint.rs` SHALL contain:
- `vertices: Vec<(f64, f64)>` -- each entry is `(half_width_grid, y_offset_grid)` relative to the base top center, in grid squares
- `closed: bool` -- true if the shell terminates with a center-line point (triangle tip)

`PlacedPart`, `BlueprintPart`, and `FlightPart` SHALL each have a `fairing_shape: Option<FairingShape>` field. The shape is serialized as part of blueprints.

`FairingHalf` enum in `blueprint.rs` SHALL have variants `Left` and `Right`, used to indicate which half of a deployed fairing shell to render. `FlightPart` and `ShipPartRenderData` SHALL have a `fairing_half: Option<FairingHalf>` field (defaulting to `None` for full shells).

### Requirement: Enter fairing build mode on placement

After placing a fairing base part (any part with `fairing: Some(...)`), the editor SHALL immediately enter fairing build mode. `FairingBuildState` tracks: `part_id`, `base_top_y` (world Y of base top edge), `base_center_x`, `base_half_width` (in grid squares), `vertices` (completed points), `ghost_point` (current cursor), and `ghost_valid`.

### Requirement: Fairing ghost snapping

During fairing build mode, the cursor SHALL snap to half-grid positions (every 0.25m). The ghost point represents a symmetric shell vertex at the cursor's distance from center. The half-width is computed as the absolute distance from cursor X to `base_center_x`.

### Requirement: Fairing ghost validation

The ghost point SHALL be valid when all of:
1. Y offset is above the last vertex (or base top if first) by at least half a grid square
2. Half-width in grid squares is at most `base_half_width * 2.0` — fairings may extend past the base edge by up to one full base half-width (i.e., `max_half_width = base_half_width * 2.0`)
3. Y offset does not exceed `base_half_width * 2.0 * 10.0 * GRID_SQUARE_SIZE` (max height proportional to base width)

### Requirement: Fairing boundary validation

Non-fairing parts SHALL NOT be placeable (ghost turns invalid) if they cross a completed (closed) fairing boundary. "Crossing" means the part is partially inside and partially outside the fairing envelope at any sampled height. Parts fully inside or fully outside a fairing are permitted.

During drag, the same boundary check SHALL apply — a dragged part SHALL NOT be droppable at a position that crosses a fairing boundary.

When closing a fairing (placing the tip vertex), the close SHALL be rejected if any existing part crosses the about-to-be-closed fairing envelope.

The boundary check interpolates the fairing shell's half-width at the part's bottom, top, and each fairing vertex y within the part's height range, checking whether the part is fully inside or fully outside at each sample. Any mix or straddling constitutes a crossing.

### Requirement: Add fairing vertex on left-click

Left-clicking during fairing build mode with a valid ghost SHALL add a vertex `(half_width_grid, y_offset_grid)` to the shape. If the half-width is less than 0.25 grid squares, the vertex is treated as a closing point: it is stored as `(0.0, y_offset_grid)`, the shape is marked `closed: true`, and build mode exits. On close, the system validates that no existing part crosses the fairing boundary — if any does, the close is rejected.

### Requirement: Undo fairing vertex on right-click

Right-clicking during fairing build mode SHALL remove the last vertex. If no vertices remain, right-click exits build mode entirely.

### Requirement: Exit fairing build mode

Pressing Escape exits fairing build mode. The current vertices (if any) are saved to the part's `fairing_shape` with `closed: false`. Deselecting also exits build mode. Deleting the fairing base part exits build mode and discards the shape.

## Part Rotation

### Requirement: Ghost rotation via R key

Pressing R with a palette part selected (ghost mode) SHALL rotate the ghost preview by 90° clockwise. The ghost rotation is stored in `EditorState.ghost_rotation` as radians (0, π/2, π, 3π/2 cycle). The rotation resets to 0 when deselecting or clearing the editor.

### Requirement: Placed part rotation via R key

Pressing R with a placed part selected (no palette selection) SHALL rotate the part by 90° clockwise in place, provided the rotated hitbox does not overlap any other part. If the part has a `mirror_partner`, the partner SHALL rotate in the opposite direction (negated rotation). If the rotated hitbox would overlap, the rotation is rejected (no change).

### Requirement: Rotated hitbox dimensions

At 0° and 180° rotation, parts use their normal hitbox width and height. At 90° and 270° rotation, hitbox width and height SHALL be swapped. This applies to: editor placement hitbox, welding hitbox, flight hitbox, and click detection hitbox. `PartDefinition` provides `rotated_hitbox_width(rotation)` / `rotated_hitbox_height(rotation)` and similar rotated accessor methods.

### Requirement: Rotated grid snapping

Ghost snapping SHALL use the rotated hitbox dimensions for odd/even alignment: parts with odd rotated hitbox width snap to grid square centers, parts with even rotated hitbox width snap to grid lines. The same applies to the height axis.

### Requirement: Rotation stored on PlacedPart

`PlacedPart.rotation` SHALL be set to `ghost_rotation` when placing a part. For mirror placements, the mirrored part's rotation SHALL be the negation of the primary part's rotation.

### Requirement: Rotation rendering in editor

Part vertices SHALL be rotated around the part center by `part.rotation` radians after generation. This applies to all rendering paths (sprites, procedural engines/pods/decouplers, shape fallbacks). Ghost vertices SHALL be rotated by `ghost_rotation` (primary ghost) or `-ghost_rotation` (mirror ghost).

### Requirement: Rotation rendering in flight

Flight rendering SHALL apply per-part rotation after scaling and before vessel rotation. The `ShipPartRenderData` struct includes a `rotation: f64` field set from `FlightPart.rotation`.

### Requirement: R key replaces symmetry toggle

The R key SHALL rotate parts instead of toggling symmetry mode. Symmetry mode remains accessible via the UI button in the editor toolbar.

## Deselection

### Requirement: Escape deselection

Pressing Escape SHALL deselect the palette part if one is selected, otherwise deselect the placed part. Escape does NOT delete the placed part.

### Requirement: Right-click deselection

Right-clicking with a palette part selected (no drag active) SHALL deselect the palette part and clear the ghost.
