## Context

The editor has a `SymmetryMode` enum with radial options (x2/x3/x4/x6/x8) that are stored and displayed but have zero functional effect — `place_part()`, `update_ghost()`, and all rendering functions ignore it entirely. Radial symmetry only makes sense in 3D; this is a 2D side-view editor. The feature needs to be replaced with mirror symmetry that reflects parts across the vessel's vertical center line.

Currently:
- `SymmetryMode` is defined in `parts/blueprint.rs` with 6 variants (Off + 5 radial)
- `EditorState` stores `symmetry_mode` and the toolbar/R-key cycle through modes
- `PlacedPart` has no mirror link tracking
- `BlueprintPart` has no mirror link serialization
- The root part (`root_part: Option<PlacedPartId>`) defines the first part placed but its X position is not explicitly used as a center line

## Goals / Non-Goals

**Goals:**
- Replace radial symmetry with Off/Mirror toggle
- Define center line as the X coordinate of the root part
- Mirror ghost preview: show two ghosts, both must be valid
- Mirror placement: place two linked parts simultaneously
- Linked part behavior: select/delete/drag as a pair
- Staging: show mirrored parts as single entry with "x2" badge
- Serialize/deserialize mirror links in blueprints

**Non-Goals:**
- Radial symmetry (3D only, not applicable)
- Mirror symmetry for parts ON the center line (parts placed exactly on center line are placed once, not mirrored)
- Automatic center line recalculation when root part is moved (center line updates when root part moves via drag)
- Part rotation or flipping (parts are symmetric shapes, mirroring is position-only)

## Decisions

### Decision 1: Center line is root part's X position

The center line is `root_part.position[0]` — the X coordinate of the first part placed. This is the simplest approach: no separate state to track, naturally follows the vessel's axis of symmetry.

When the root part is dragged, the center line moves with it. Mirror-linked parts do NOT auto-adjust when the root moves — they were placed relative to the center line at placement time and keep their absolute positions. This matches KSP behavior.

**Alternative considered**: Fixed center line at X=0. Rejected because parts can be placed anywhere and the root pod defines the vessel center conceptually.

### Decision 2: Mirror link via `mirror_partner` field on `PlacedPart`

Add `pub mirror_partner: Option<PlacedPartId>` to `PlacedPart`. When two parts are mirror-placed, each points to the other. This is a bidirectional link: part A's `mirror_partner = Some(B)` and part B's `mirror_partner = Some(A)`.

Serialize as `pub mirror_partner_index: Option<usize>` on `BlueprintPart`, using the same index-mapping pattern as `parent_index`.

**Alternative considered**: Symmetry group IDs (a shared group identifier). More complex, supports N-way symmetry we don't need. The bidirectional partner link is simpler for 2-part mirror pairs.

### Decision 3: Parts on center line are placed once

If the ghost snaps to a position where its X coordinate equals the center line (within floating-point tolerance of the grid snap), only one part is placed even in Mirror mode. The mirror of a center-line part is itself. This prevents overlapping duplicate parts on the vessel's axis.

### Decision 4: Mirror ghost rendering

`generate_ghost_vertices()` gains awareness of symmetry mode. In Mirror mode, it generates vertices for both the primary ghost and the mirrored ghost. The mirrored ghost position is `center_line * 2 - primary_x` (reflection formula). Both ghosts use the same validity color — if either would overlap an existing part, both show red.

`update_ghost()` similarly computes both positions and checks overlap for both. `ghost_valid` is true only if BOTH positions are free of overlap.

### Decision 5: Mirror drag moves both parts

When dragging a part that has a `mirror_partner`, both parts move. The primary follows the cursor with grid snapping. The partner's position is computed as the mirror of the primary's new position relative to the center line. `drag_valid` requires both positions to be overlap-free (excluding both dragged parts from the overlap check).

### Decision 6: Staging shows linked parts as single entry

In the staging panel, when rendering parts in a stage, if a part has a `mirror_partner` that is also in the same stage, only render one entry with a "x2" suffix on the name. The partner is hidden. Both are still in the stage data — this is a display-only optimization.

When a mirrored pair is drag-reordered in staging, both parts move together.

### Decision 7: SymmetryMode simplification

Replace the enum:
```rust
pub enum SymmetryMode {
    Off,
    Mirror,
}
```

`cycle_next()` toggles between Off and Mirror. `display()` returns "Off" or "Mirror". The R key and toolbar button toggle between the two states.

## Risks / Trade-offs

- **[Breaking change to SymmetryMode]** → Old blueprints with radial symmetry mode stored won't load. Mitigation: SymmetryMode was never serialized into blueprints (it's on EditorState, not VesselBlueprint), so no migration needed.
- **[Center line depends on root part existing]** → Mirror mode requires a root part to define the center line. Mitigation: If no root part exists, mirror mode falls back to Off behavior (single placement). Show a hint in the UI.
- **[Mirror link integrity]** → If one partner is somehow deleted without the other, the orphaned link must be cleaned up. Mitigation: `delete_part()` always clears the partner's `mirror_partner` field when deleting a linked part.
- **[Blueprint compatibility]** → New `mirror_partner_index` field on `BlueprintPart`. Mitigation: Use `#[serde(default)]` so old blueprints deserialize with `None`.
