## 1. SymmetryMode Enum and PlacedPart Data Model

- [x] 1.1 Replace `SymmetryMode` enum in `src/parts/blueprint.rs`: remove `Radial2/3/4/6/8`, keep `Off`, add `Mirror`. Update `count()`, `cycle_next()` (toggle), and `display()` methods.
- [x] 1.2 Add `mirror_partner: Option<PlacedPartId>` field to `PlacedPart` in `src/parts/blueprint.rs` with default `None`.
- [x] 1.3 Add `mirror_partner_index: Option<usize>` field to `BlueprintPart` in `src/parts/blueprint.rs` with `#[serde(default)]`.
- [x] 1.4 Update `parts_to_blueprint()` to convert `mirror_partner` PlacedPartId to `mirror_partner_index` using the id-to-index map.
- [x] 1.5 Update `blueprint_to_parts()` to convert `mirror_partner_index` back to `mirror_partner` PlacedPartId using the index-to-id map.

## 2. Center Line and Ghost Preview

- [x] 2.1 Add a `center_line_x()` method to `EditorState` that returns `Option<f64>` — the root part's X position, or `None` if no root part exists.
- [x] 2.2 Update `update_ghost()` in `src/editor/state.rs` to compute the mirrored ghost position when symmetry mode is Mirror and center line exists. Store as new field `mirror_ghost_position: Option<[f64; 2]>`.
- [x] 2.3 In `update_ghost()`, check overlap for both primary and mirrored ghost. Set `ghost_valid = true` only if neither overlaps. Skip mirroring if ghost snaps to the center line X coordinate.
- [x] 2.4 Update `generate_ghost_vertices()` in `src/editor/render.rs` to render the mirrored ghost when `mirror_ghost_position` is `Some`. Use the same validity color for both ghosts.

## 3. Mirror Placement

- [x] 3.1 Update `place_part()` in `src/editor/state.rs`: when symmetry mode is Mirror and `mirror_ghost_position` is `Some`, place two parts. Set `mirror_partner` on each to point to the other. Add both to stage 0 if applicable (engines/decouplers).
- [x] 3.2 When ghost is on center line (no `mirror_ghost_position`), place a single part with `mirror_partner = None` even in Mirror mode.

## 4. Linked Selection and Rendering

- [x] 4.1 Update `generate_part_vertices()` in `src/editor/render.rs`: when determining part color, if a part's `mirror_partner` equals `selected_placed_part`, render it with `PART_SELECTED_COLOR`.
- [x] 4.2 Update hover highlight: if a part's `mirror_partner` is the hovered part, also apply hover color.

## 5. Linked Deletion

- [x] 5.1 Update `delete_part()` in `src/editor/state.rs`: before removing the part, check `mirror_partner`. If set, also remove the partner from `parts` HashMap, clear the partner from all stages, and clear `selected_placed_part` / `staging_selected_engine` if it was the partner.
- [x] 5.2 Ensure no stale `mirror_partner` references remain after deletion (the partner is removed, so no orphan cleanup needed beyond removing both).
- [x] 5.3 Update the "Delete Part" button label in `src/editor/ui.rs`: show "Delete Parts (x2)" when the selected part has a `mirror_partner`.

## 6. Linked Dragging

- [x] 6.1 Update `start_drag()` in `src/editor/state.rs`: if the dragged part has a `mirror_partner`, also save the partner's original position (new field `drag_partner_start_pos: Option<[f64; 2]>`).
- [x] 6.2 Update `update_drag()`: when dragging a mirrored pair, compute the partner's mirrored position from center line. Move the partner in real-time. Check overlap for both parts (excluding both from the check). Set `drag_valid` only if both are overlap-free.
- [x] 6.3 Update `finish_drag()`: if dragging a mirrored pair and the drag is invalid, revert both parts to their pre-drag positions.
- [x] 6.4 Update `cancel_drag()`: revert both the primary and partner to their pre-drag positions.

## 7. Linked Part Info Panel

- [x] 7.1 Update tank controls in `src/editor/ui.rs`: when changing fuel type, fill/empty state, or crossfeed on a part with `mirror_partner`, apply the same change to the partner part.

## 8. Staging Display

- [x] 8.1 Update the staging panel in `src/editor/ui.rs`: when rendering parts in a stage, if a part has a `mirror_partner` that is also in the same stage, display a single entry with " x2" appended to the name. Track which partner IDs have been rendered to avoid showing duplicates.
- [x] 8.2 When drag-reordering a mirrored entry in staging (moving to a different stage), move both the part and its `mirror_partner`.

## 9. Verification

- [x] 9.1 Build and run: verify symmetry toggles between Off and Mirror via R key and toolbar button.
- [ ] 9.2 Verify mirror ghost appears reflected across center line, both turn red if either overlaps.
- [ ] 9.3 Verify mirror placement creates two linked parts; center-line placement creates one.
- [ ] 9.4 Verify selecting one mirrored part highlights both.
- [ ] 9.5 Verify deleting one mirrored part removes both.
- [ ] 9.6 Verify dragging one mirrored part moves both in mirror.
- [ ] 9.7 Verify staging shows "x2" for mirrored pairs in the same stage.
- [ ] 9.8 Verify save/load preserves mirror links.
- [ ] 9.9 Verify old blueprints load correctly (mirror_partner defaults to None).
