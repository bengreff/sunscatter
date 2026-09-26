# Tech Tree Screen

Full-screen tech tree for unlocking nodes and upgrading efficiency lines. Displays a directed graph of 39 tech nodes and 11 efficiency line nodes organized into technology areas with explicit col/row positioning.

## GameMode & Navigation

- `GameMode::TechTree` — dedicated full-screen mode
- Entry points:
  - **Management screen**: "Open Tech Tree" button -> `ManagementAction::OpenTechTree` -> `game.enter_tech_tree(GameMode::Management)`
  - **Editor**: "Research" button -> `EditorAction::OpenTechTree` -> `game.enter_tech_tree(GameMode::Editor)`
- `Game::enter_tech_tree(from: GameMode)` stores `tech_tree_return_mode = Some(from)`, sets `GameMode::TechTree`, unpauses
- `Game::leave_tech_tree()` returns to `tech_tree_return_mode` (defaults to `GameMode::Management` if None), unpauses

## TechTreeScreenAction

```
None, Back, ChangeWarp(usize)
```

Returned from `render_tech_tree_screen()`. `Back` triggers `game.leave_tech_tree()`. `ChangeWarp` sets `game.warp_index`.

## Function Signature

`render_tech_tree_screen()` takes `&mut TechTree` and `&mut ScienceState` for direct mutation (unlock nodes, upgrade lines, deduct science). Also takes `warp_levels`, `current_warp_index`, `date_str`, `paused`, `active_toasts`, and `&PartDefinitions` (for part info display in the detail panel).

## Top Panel

Horizontal layout:
- **"Back" button** (left) -> `TechTreeScreenAction::Back`
- **"Tech Tree" heading** (white, size 18)
- **Time warp buttons**: selectable labels for each warp level, formatted as `Nx`/`NK`/`NM`/`NB`. Selected index highlighted. Click -> `ChangeWarp(i)`
- **Date string** (right of warp buttons)
- **"Science: {available}"** (right-aligned, blue `rgb(100,180,255)`, size 16)

## Pause Overlay

When `paused == true`: centered `egui::Area` (foreground order) with semi-transparent black background (`rgba(0,0,0,180)`), "Paused" heading (size 32), "Back" button (size 18). Renders toasts, then returns early (skips graph and detail panel).

## Detail Side Panel (Right) — Tech Nodes

Shown when a tech node is selected. `egui::SidePanel::right`, width 280, non-resizable, with vertical `ScrollArea`.

Selected node ID stored in egui temp data via `egui::Id::new("tech_tree_selected_node")` as `Option<String>`.

Panel contents:
- **Node name** (heading, white, size 18)
- **Era label + node ID** (gray, size 11): e.g. "Era 3: Nuclear & Electric | nuclear_thermal"
- **Status badge** (colored, strong):
  - "Unlocked" — green `rgb(80,180,80)` (node is in `unlocked` set)
  - "Available" — blue `rgb(80,140,220)` (prerequisites met AND affordable)
  - "Need Science" — yellow `rgb(180,140,60)` (prerequisites met, NOT affordable)
  - "Locked" — gray `rgb(120,120,120)` (prerequisites not met)
- **"Cost: {cost} Science"**
- **Prerequisites list** (if non-empty): each with check mark (green) if met, lock icon (red) if not, showing display name
- **"Unlocks Parts:" list** (if non-empty): part names as clickable links. Clicking a part name toggles its selection (stored in `egui::Id::new("tech_tree_selected_part")` as `Option<String>`). When selected, the part's full info panel is shown inline below the list using `render_part_info()` from `editor/ui.rs`, displaying stats (mass, cost, engine/tank/pod details) from `PartDefinitions::find_by_name()`
- **"Unlocks Buildings:" list** (if non-empty): `BuildingType::display_name()` values
- **"Unlock ({cost} sci)" button**: shown only when `!is_unlocked && can_unlock && affordable`. On click: `tech_tree.unlock(&id)` then `science.available -= cost`
- **"Close" link** (gray, size 12): clears selection in temp data

## Detail Side Panel (Right) — Efficiency Lines

Shown when an efficiency line node is selected (and no tech node is selected). Same panel ID `"tech_detail_panel"`, width 420.

Selected line ID stored via `egui::Id::new("tech_tree_selected_line")` as `Option<String>`. Clicking a tech node clears line selection and vice versa.

Panel contents:
- **Line name** (heading, white, size 18)
- **"Efficiency Line | {line_id}"** (gray, size 11)
- **Status badge**: "Maxed" (green) / "Tier N/15" (blue) / "Available" (blue) / "Locked" (gray)
- **Current efficiency**: "{1.11^tier * 100:.0}% efficiency" (green)
- **Next upgrade section** (if tier < 15): next tier efficiency, cost in Science, "Upgrade" button if available and affordable
- **Affects section**: "Affects:" heading, lists what buildings/recipes the line improves (hardcoded per line_id via `line_affects()` helper)
- **Prerequisites**: prerequisite node with met/unmet indicator; if `prerequisite_line_tier`, show "Requires {line_name} Tier {N}" with met/unmet
- **Recipes section**: "Unlocked Recipes:" heading, each recipe gate with check/circle marker and tier label; or "No recipe gates" (gray, italic) if empty
- **"Close" link** (gray, size 12): clears line selection

## Central Panel — Graph

`egui::CentralPanel` with `ScrollArea::both()` (no auto-shrink). Contains the unified node graph (tech nodes + efficiency line nodes).

### Layout Constants

```
NODE_W = 160, NODE_H = 35
COL_SPACING = 285, ROW_SPACING = 50
PADDING = 20
```

### Node Positioning

Both tech nodes and efficiency line nodes have explicit `col` and `row` fields (set in `data/tech/tree.ron`). Position computed as:
- `x = col * COL_SPACING + PADDING`
- `y = row * ROW_SPACING + PADDING`

### Technology Areas (Row Bands)

| Area | Rows | Description |
|------|------|-------------|
| Chemical Launch Vehicles | 0-1 | Kerolox/Methalox/Hydrolox progression, heavy lift |
| Main Highway: Nuclear → Fusion → AM | 2 | Nuclear thermal through photon drive (flat) |
| Branches & Power | 3 | NTR variants, shielding, fusion/AM power |
| Crewed & Colony | 4 | Habitation, colony infrastructure, deep space |
| Electric Propulsion & Science | 5 | Ion → advanced electric → MPD, science lab |
| Colony Production Chain | 6 | agriculture → mining → metallurgy → construction |
| Support & Specialized | 7 | life_support, chemical_processing, atmospheric_science, isotope_extraction |
| Advanced Manufacturing & Nuclear | 8 | electronics_mfg, precision_mfg, nuclear_engineering |

### Dynamic Canvas Size

Computed from max col/row across both tech nodes and efficiency lines:
```
max_col = max(nodes.col, lines.col)
max_row = max(nodes.row, lines.row)
canvas_w = (max_col + 1) * COL_SPACING + NODE_W + 2 * PADDING
canvas_h = (max_row + 1) * ROW_SPACING + NODE_H + 2 * PADDING
```

### Arrows (Prerequisites)

For tech nodes: arrows from each prerequisite's right edge midpoint to the node's left edge midpoint.
For efficiency line nodes: arrows from prerequisite_node → line, and from prerequisite line → line (if `prerequisite_line_tier` exists).

- **Green** `rgb(80,160,80)` if prerequisite is met (node unlocked, or line tier reached)
- **Gray** `rgb(80,80,80)` if not
- Line width 1.5, with filled triangular arrowhead (size 6)
- All edges go left-to-right, guaranteed by data layout

### Tech Node Rectangles

Rounded rect (radius 4) for each node:
- **Fill by status**:
  - Unlocked: dark green `rgb(40,100,40)`
  - Available (can_unlock): dark blue `rgb(30,60,120)`
  - Locked: dark gray `rgb(40,40,45)`
- **Stroke** (selection-aware):
  - Selected: blue `rgb(80,140,220)`, width 2
  - Connected (prerequisite or dependent of selected): white, width 2
  - Hovered: light gray `rgb(200,200,200)`, width 1
  - Default: dark gray `rgb(70,70,80)`, width 1
- **Label**: node name centered, white, proportional 12pt. Truncated to 20 chars + ".." if > 22 chars.

### Efficiency Line Node Rectangles

Same shape and stroke logic as tech nodes, but distinct fill colors:
- **Maxed** (tier 15): dark green `rgb(40,100,40)`
- **In progress** (tier 1-14): dark teal `rgb(30,80,80)`
- **Available** (tier 0, prereqs met): dark blue `rgb(30,60,120)`
- **Locked** (tier 0, prereqs not met): dark gray `rgb(40,40,45)`

Label: `"{name} ({tier}/15)"` if tier > 0, else just `"{name}"`.

### Click Detection

On painter click, check `interact_pointer_pos()` against each node's and line node's screen rect. Clicking a tech node stores in `"tech_tree_selected_node"` and clears `"tech_tree_selected_line"`, and vice versa.

### Connected Nodes Highlighting

When a tech node is selected, connected set includes: its prerequisites, its dependents, and any efficiency lines whose prerequisite_node matches.

When a line node is selected, connected set includes: its prerequisite_node, its prerequisite line (if any), and any lines that depend on it via `prerequisite_line_tier`.

## TechNodeData

```rust
pub struct TechNodeData {
    pub id: String,          // Descriptive ID (e.g. "basic_rocketry")
    pub name: String,
    pub era: u32,            // For detail panel display
    pub cost: f64,
    pub prerequisites: Vec<String>,
    pub col: u32,            // Horizontal graph position (0 = leftmost)
    pub row: u32,            // Vertical graph position (0 = topmost)
    pub unlocks_parts: Vec<String>,
    pub unlocks_buildings: Vec<BuildingType>,
}
```

## TechLineData

```rust
pub struct TechLineData {
    pub id: String,
    pub name: String,
    pub base_cost: f64,
    pub prerequisite_node: String,
    pub prerequisite_line_tier: Option<(String, u32)>,
    pub recipe_gates: Vec<(u32, String)>,
    pub col: u32,            // Horizontal graph position
    pub row: u32,            // Vertical graph position
}
```

### Efficiency Line Data Model

- Tier cost formula: `base_cost * (next_tier)^1.7`. Max tier = 15.
- Tier multiplier: `1.11^tier`
- `TechTree::upgrade_line(id)` increments tier if prerequisites met and not at max
- `TechTree::line_tier(id)` returns current tier (0 if not started)

## Save Compatibility

`SaveGame::restore_to_game()` migrates old tech IDs ("1.1", "2.3", etc.) to new descriptive IDs via `migrate_tech_ids()`. The mapping covers all 35 original nodes. New IDs (already in new format) pass through unchanged. Efficiency line tier keys are unchanged.

## RenderState Integration

`RenderState::render_tech_tree_screen()` in `menus.rs`:
- Updates camera buffer, gets surface texture
- Runs egui pass with `render_tech_tree_screen()` from `tech_tree_ui.rs`
- Standard wgpu geometry pass (planets in background) + egui render pass
- Returns `(new_warp_index, TechTreeScreenAction)`

## Main Loop Integration (`render_tech_tree_frame`)

1. Simulation: `game.check_contracts()`, `game.update_rd_science(dt_sim)`, `game.update_colonies(dt_sim)` when not paused
2. Render: `render_state.render_tech_tree_screen(tech_tree, science, ...)`
3. After render: process `TechTreeScreenAction` (Back -> `leave_tech_tree()`, ChangeWarp -> set index)
4. Notification processing and toast cleanup

## Toast Notifications

`render_toasts(ctx, active_toasts)` called at the end of the render function (and within pause overlay). Shares the same toast system as other screens.
