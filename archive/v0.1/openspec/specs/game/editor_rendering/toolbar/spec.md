# Toolbar

The top toolbar with vessel actions, symmetry mode, launch button, and part count.

### Requirement: Toolbar layout

The toolbar SHALL be a top panel containing: "Vehicle Editor" heading, separator, New/Save/Load buttons, separator, symmetry mode control, separator, Launch button, separator, Research button, Contracts button, R&D budget drag value, and right-aligned company balance and part count.

### Requirement: Launch button enabled state

The Launch button SHALL be enabled only when `can_launch()` returns true (root part exists and parts are non-empty). The launch button text SHALL be "Launch" (with rocket emoji prefix).

### Requirement: Launch control validation

Launching SHALL fail with an error if the vessel has no controllable part (no pod or probe core with `can_control: true`). The error message "Vessel has no controllable part (add a pod or probe core)" SHALL be displayed as a red alert banner at the top of the editor for 3 seconds.

### Requirement: Exit to Flight button

The toolbar SHALL include an "Exit to Flight" button that returns to flight mode without launching a new vessel.

### Requirement: Part count and balance display

The toolbar SHALL display company balance (green, formatted via `format_money()`) and "Parts: {count}" right-aligned.

### Requirement: Research button

A "Research" button SHALL navigate to the full-screen Tech Tree (`EditorAction::OpenTechTree`), which transitions `GameMode` to `TechTree` and records the Editor as the return mode. Clicking "Back" in the Tech Tree SHALL return the user to the Editor.

### Requirement: Contracts button

A "Contracts" button SHALL open the contract board window (`EditorAction::OpenContracts`).

### Requirement: R&D budget control

An R&D budget DragValue SHALL be displayed labeled "R&D:", showing budget in millions per year (suffix "M/yr"), range 0-1000, step 0.5. Changes emit `EditorAction::SetRdBudget(f64)`.

### Requirement: Launch cost check

Launching SHALL fail with an error if the vessel cost exceeds company money. The error message includes both the vessel cost and available funds, formatted via `format_money()`.
