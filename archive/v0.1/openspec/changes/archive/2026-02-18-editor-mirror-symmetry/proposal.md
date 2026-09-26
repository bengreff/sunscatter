## Why

The editor currently has radial symmetry options (x2, x3, x4, x6, x8) that only make sense in 3D. Since this is a 2D side-view editor, radial symmetry is meaningless. The editor needs mirror symmetry that reflects parts across the vessel's center line, which is the standard symmetry tool for 2D rocket building.

## What Changes

- **Define center line**: The vertical line running through the root part (first part placed) becomes the vessel's center line for symmetry calculations.
- **Replace radial symmetry with mirror symmetry**: Remove all `Radial2/3/4/6/8` modes. Replace with two modes: `Off` and `Mirror`. **BREAKING** — `SymmetryMode` enum variants change.
- **Mirror ghost preview**: In Mirror mode, show two ghost previews — the primary at cursor and a mirrored copy reflected across the center line. Both ghosts must be valid for placement to proceed.
- **Mirror part placement**: Placing a part in Mirror mode places two parts simultaneously — the primary and its mirror. Both are added to the parts HashMap with linked IDs.
- **Linked mirror parts**: Mirrored parts are linked together:
  - Selecting one selects both
  - Deleting one deletes both
  - Dragging one moves both (mirrored)
  - Parts palette / staging shows a single icon with "x2" badge
  - Part info panel applies changes to both (e.g., fuel type)

## Capabilities

### New Capabilities

- `editor-mirror-symmetry`: Mirror symmetry placement, ghost preview, linked part pairs, center line definition

### Modified Capabilities

- `game/editor`: Symmetry mode cycling changes from radial to Off/Mirror
- `game/editor/parts`: Part selection, deletion, and dragging must handle linked mirror pairs

## Impact

- `src/parts/blueprint.rs`: `SymmetryMode` enum simplified to `Off`/`Mirror`
- `src/editor/state.rs`: Ghost/placement/selection/deletion/drag logic updated for mirror pairs; center line calculation added; mirror link tracking
- `src/editor/ui.rs`: Symmetry button display, part info panel for linked parts
- `src/editor/render.rs`: Render mirrored ghost preview
- `src/parts/blueprint.rs`: Blueprint serialization must preserve mirror links
- `openspec/specs/game/editor/spec.md`: Symmetry requirements updated
- `openspec/specs/game/editor/parts/spec.md`: Selection/deletion/drag requirements updated for linked parts
