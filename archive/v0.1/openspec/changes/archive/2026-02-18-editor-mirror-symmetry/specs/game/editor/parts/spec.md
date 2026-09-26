## MODIFIED Requirements

### Requirement: Click to select placed part

Left-clicking on a placed part SHALL select it and clear any palette selection. If the clicked part has a `mirror_partner`, both parts SHALL render with the selected color.

#### Scenario: Select mirrored part highlights both

- **WHEN** the user clicks on a placed part that has a mirror partner
- **THEN** both the clicked part and its mirror partner SHALL render with the selected color `[0.5, 0.7, 1.0, 1.0]`

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

### Requirement: Delete button for placed parts

The info panel for placed parts SHALL include a "Delete Part" button. If the part has a `mirror_partner`, the button label SHALL read "Delete Parts (x2)" and clicking it SHALL delete both parts.

#### Scenario: Delete button for mirrored pair

- **WHEN** a mirrored part is selected and the user clicks the delete button
- **THEN** both the selected part and its mirror partner SHALL be deleted
