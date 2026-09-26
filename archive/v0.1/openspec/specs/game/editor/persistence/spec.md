# Persistence

Saving, loading, and creating vessel blueprints in the vehicle editor.

## Blueprint Format

### Requirement: VesselBlueprint structure

A `VesselBlueprint` SHALL contain: `name` (String), `parts` (Vec of `BlueprintPart`), `root_part_index` (usize into parts Vec), and `stages` (Vec<Vec<usize>> using indices into parts Vec).

### Requirement: BlueprintPart structure

A `BlueprintPart` SHALL contain: `definition_id`, `position` ([f64; 2]), `rotation` (f64), `parent_index` (Option<usize>), `attachment_type` (Root/Stack/Radial), `stage` (u32), `fuel_type` (default Empty), `tank_filled` (default false), `crossfeed_enabled` (default false), and `mirror_partner_index` (default None).

### Requirement: Mirror partner serialization

`BlueprintPart` SHALL have a `mirror_partner_index: Option<usize>` field with `#[serde(default)]`. During blueprint save, `mirror_partner` PlacedPartId SHALL be converted to the partner's index in the parts vec. During blueprint load, the index SHALL be converted back to a PlacedPartId. Old blueprints without this field SHALL deserialize with `None`.

#### Scenario: Save and load preserves mirror links

- **WHEN** a vessel with mirror-linked parts is saved and loaded
- **THEN** the mirror links SHALL be preserved and both parts SHALL reference each other

#### Scenario: Old blueprint compatibility

- **WHEN** a blueprint saved before mirror symmetry is loaded
- **THEN** all parts SHALL have `mirror_partner = None` and load successfully

### Requirement: Backward-compatible deserialization

The `fuel_type`, `tank_filled`, `crossfeed_enabled`, and `mirror_partner_index` fields SHALL use `#[serde(default)]` so older blueprints without these fields can still be loaded.

### Requirement: RON serialization

Blueprints SHALL be serialized to RON format using `ron::ser::to_string_pretty` with default pretty config, and deserialized using `ron::from_str`.

### Requirement: Deterministic part ordering

When converting editor state to a blueprint, parts SHALL be sorted by `PlacedPartId` before assigning blueprint indices. This ensures that saving the same editor state twice produces byte-identical RON output, so diffs remain meaningful and stage indices do not drift across runs.

#### Scenario: Save produces deterministic output

- **WHEN** the same vessel state is saved multiple times (including across process restarts)
- **THEN** the resulting blueprint RON file SHALL be byte-identical each time

## Save Flow

### Requirement: Save dialog

Clicking "Save" in the toolbar SHALL open a dialog with a text field bound to the vessel name, and Save/Cancel buttons.

### Requirement: Save validation

Saving SHALL require a root part and non-empty parts list. The blueprint SHALL be validated: parts must not be empty, `root_part_index` must be in bounds, and all `parent_index` values must be valid.

### Requirement: Disconnected part filtering

When converting editor state to a blueprint via `to_blueprint()`, the system SHALL perform a BFS flood-fill from the root part through welding hitbox overlap to find all connected parts. Parts not reachable from the root SHALL be excluded from the blueprint. Stages SHALL be filtered to remove excluded part IDs, and empty stages SHALL be removed.

#### Scenario: Disconnected part excluded at save

- **WHEN** a part has been dragged away from the vessel leaving a gap (no welding hitbox overlap with any connected part)
- **THEN** that part SHALL be silently excluded from the saved blueprint and from the launched vessel

#### Scenario: All connected parts included

- **WHEN** all parts have welding hitbox overlap forming a connected graph from the root
- **THEN** all parts SHALL be included in the blueprint

### Requirement: Filename sanitization

The save filename SHALL be derived from the blueprint name: alphanumeric characters, `-`, and `_` are preserved; spaces become `_`; all other characters become `_`. The file extension SHALL be `.ron`.

### Requirement: Save location

Blueprints SHALL be saved to `{directory}/{sanitized_name}.ron` and stored in the in-memory `BlueprintRegistry` HashMap keyed by blueprint name.

## Load Flow

### Requirement: Load dialog

Clicking "Load" in the toolbar SHALL open a dialog with a scrollable list (max height 200px) of blueprint names. If no blueprints exist, it SHALL display "No saved blueprints".

### Requirement: Tech tree validation on load

`Game::load_blueprint()` SHALL validate that all parts in the blueprint are unlocked via the tech tree before loading. If any parts are locked (not available via `tech_tree.is_part_available()`), the load SHALL fail with an error listing the locked part names. The editor SHALL display an alert message with the locked part names so the user understands why the load failed.

In the load dialog, blueprints containing locked parts SHALL be visually indicated with a lock icon and grayed out (disabled button). Clicking a grayed-out blueprint does nothing. The locked set is precomputed before rendering the dialog by checking each blueprint's parts against the tech tree.

### Requirement: Load clears state first

Loading a blueprint SHALL call `clear()` first, then populate parts, root, stages, and vessel name from the blueprint.

### Requirement: Auto-stage engines on load

If the loaded blueprint has empty stages, all engine parts SHALL be collected into a single stage 0.

#### Scenario: Legacy blueprint without stages
- **WHEN** a blueprint is loaded with no stage data
- **THEN** all engines SHALL be placed in stage 0
- **AND** the camera SHALL auto-focus on the loaded parts

### Requirement: Update part ID counter on load

After loading, `next_part_id` SHALL be set to one greater than the maximum existing part ID, ensuring new parts get unique IDs.

### Requirement: Auto-focus camera on load

After loading a blueprint, the camera SHALL auto-focus by centering on the bounding box of all parts and setting zoom to fit.

### Requirement: Registry load-all on startup

On startup, the registry SHALL create the blueprints directory if it does not exist, read all `.ron` files, parse each as a `VesselBlueprint`, and store them in the in-memory HashMap. Failed loads SHALL log warnings but not prevent other blueprints from loading.

## New Vessel

### Requirement: New vessel clears state

Clicking "New" SHALL clear: parts, root part, part ID counter (reset to 1), palette selection, placed part selection, ghost state, stages, staging selection, and vessel name (reset to "Untitled Vessel").

### Requirement: New vessel preserves settings

Clearing state SHALL NOT reset: camera position/zoom, arrow key state, symmetry mode, selected category, dialog visibility, hover state, drag state, or TWR settings.
