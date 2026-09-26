## ADDED Requirements

### Requirement: Center line definition

The editor SHALL define the center line as the vertical line at the X coordinate of the root part's position. The center line SHALL update when the root part is moved via dragging.

#### Scenario: Center line follows root part

- **WHEN** the root part is at position `[2.0, 3.0]`
- **THEN** the center line SHALL be at X = 2.0

#### Scenario: Center line updates on root drag

- **WHEN** the root part is dragged from `[2.0, 3.0]` to `[3.0, 3.0]`
- **THEN** the center line SHALL update to X = 3.0

#### Scenario: No center line without root part

- **WHEN** no root part has been placed
- **THEN** the center line SHALL be undefined and mirror mode SHALL behave as Off mode

### Requirement: Mirror ghost preview

When symmetry mode is Mirror and a palette part is selected, the editor SHALL display two ghost previews: the primary ghost at the cursor position (grid-snapped) and a mirrored ghost reflected across the center line. The mirrored ghost X position SHALL be `center_line_x * 2 - primary_x`. The mirrored ghost Y position SHALL equal the primary ghost Y position.

#### Scenario: Mirror ghost positions

- **WHEN** symmetry mode is Mirror, center line is at X = 2.0, and the cursor snaps to `[3.5, 1.0]`
- **THEN** the primary ghost SHALL be at `[3.5, 1.0]` and the mirrored ghost SHALL be at `[0.5, 1.0]`

#### Scenario: Mirror ghost on center line

- **WHEN** symmetry mode is Mirror and the primary ghost snaps to the center line X coordinate
- **THEN** only one ghost SHALL be displayed (the part is on the center line, no mirror needed)

### Requirement: Mirror ghost validity

Both mirror ghosts SHALL share the same validity state. The ghost pair SHALL be valid only when NEITHER ghost overlaps any existing part. Both ghosts SHALL render in green when valid and red when invalid.

#### Scenario: One ghost overlaps

- **WHEN** symmetry mode is Mirror, the primary ghost has no overlap, but the mirrored ghost overlaps an existing part
- **THEN** both ghosts SHALL render in red (invalid) and placement SHALL be blocked

#### Scenario: Both ghosts clear

- **WHEN** symmetry mode is Mirror and neither ghost overlaps any existing part
- **THEN** both ghosts SHALL render in green (valid) and placement SHALL be allowed

### Requirement: Mirror part placement

When symmetry mode is Mirror and placement is valid, left-clicking SHALL place two parts simultaneously: the primary part at the ghost position and the mirrored part at the mirrored position. Both parts SHALL be assigned unique `PlacedPartId` values and linked via `mirror_partner` fields pointing to each other.

#### Scenario: Mirror placement creates linked pair

- **WHEN** the user clicks to place a part in Mirror mode at `[3.5, 1.0]` with center line at X = 2.0
- **THEN** two parts SHALL be placed: one at `[3.5, 1.0]` and one at `[0.5, 1.0]`, each with `mirror_partner` pointing to the other's ID

#### Scenario: Center line placement creates single part

- **WHEN** the user clicks to place a part in Mirror mode and the ghost is on the center line
- **THEN** only one part SHALL be placed with `mirror_partner = None`

#### Scenario: Mirror placement auto-staging

- **WHEN** two engines are placed via mirror placement
- **THEN** both engines SHALL be added to stage 0

### Requirement: Mirror partner field

`PlacedPart` SHALL have a `mirror_partner: Option<PlacedPartId>` field. When two parts are mirror-placed, each SHALL reference the other. Parts placed without mirroring or on the center line SHALL have `mirror_partner = None`.

#### Scenario: Bidirectional link

- **WHEN** part A (ID=5) and part B (ID=6) are mirror-placed
- **THEN** part A's `mirror_partner` SHALL be `Some(6)` and part B's `mirror_partner` SHALL be `Some(5)`

### Requirement: Mirror partner serialization

`BlueprintPart` SHALL have a `mirror_partner_index: Option<usize>` field with `#[serde(default)]`. During blueprint save, `mirror_partner` PlacedPartId SHALL be converted to the partner's index in the parts vec. During blueprint load, the index SHALL be converted back to a PlacedPartId. Old blueprints without this field SHALL deserialize with `None`.

#### Scenario: Save and load preserves mirror links

- **WHEN** a vessel with mirror-linked parts is saved and loaded
- **THEN** the mirror links SHALL be preserved and both parts SHALL reference each other

#### Scenario: Old blueprint compatibility

- **WHEN** a blueprint saved before mirror symmetry is loaded
- **THEN** all parts SHALL have `mirror_partner = None` and load successfully

### Requirement: Linked selection

Selecting a part that has a `mirror_partner` SHALL visually highlight both parts with the selected color. The `selected_placed_part` field tracks the clicked part, but rendering SHALL also apply the selected color to its mirror partner.

#### Scenario: Click selects both visually

- **WHEN** the user clicks on part A which has mirror_partner B
- **THEN** both part A and part B SHALL render with the selected color

### Requirement: Linked deletion

Deleting a part that has a `mirror_partner` SHALL delete both the part and its mirror partner. Both parts SHALL be removed from the parts HashMap and from all stages.

#### Scenario: Delete removes both

- **WHEN** the user deletes part A which has mirror_partner B
- **THEN** both part A and part B SHALL be removed from the editor

#### Scenario: Delete partner clears orphan link

- **WHEN** part A with mirror_partner B is deleted
- **THEN** no part in the editor SHALL have a stale mirror_partner reference

### Requirement: Linked dragging

Dragging a part that has a `mirror_partner` SHALL move both parts. The primary part follows the cursor with grid snapping. The partner's X position SHALL be `center_line_x * 2 - primary_x` and its Y position SHALL equal the primary's Y position. Drag validity SHALL require both positions to be overlap-free, excluding both dragged parts from the overlap check.

#### Scenario: Drag moves both parts

- **WHEN** part A (at `[3.5, 1.0]`) with mirror_partner B (at `[0.5, 1.0]`) is dragged to `[4.0, 2.0]` with center line at X = 2.0
- **THEN** part A SHALL move to `[4.0, 2.0]` and part B SHALL move to `[0.0, 2.0]`

#### Scenario: Invalid drag reverts both

- **WHEN** a mirror drag finishes at a position where either part overlaps another part
- **THEN** both parts SHALL revert to their pre-drag positions

### Requirement: Linked part info panel

When a part with a `mirror_partner` is selected, changes made in the part info panel SHALL apply to both parts. This includes fuel type selection, fill/empty toggle, and crossfeed toggle.

#### Scenario: Fuel type change applies to both

- **WHEN** the user changes the fuel type of a selected tank that has a mirror partner
- **THEN** both the selected tank and its mirror partner SHALL update to the new fuel type

### Requirement: Staging display for linked parts

In the staging panel, when a part and its `mirror_partner` are both in the same stage, they SHALL be displayed as a single entry with " x2" appended to the part name. Drag-reordering a mirrored entry in staging SHALL move both parts together.

#### Scenario: Mirrored pair shows as single entry

- **WHEN** engine A and its mirror partner engine B are both in stage 0
- **THEN** the staging panel SHALL show one entry: "Engine Name x2"

#### Scenario: Only one partner in stage

- **WHEN** engine A is in stage 0 but its mirror partner engine B is in stage 1
- **THEN** each stage SHALL show its respective engine as a normal entry (no x2 badge)
