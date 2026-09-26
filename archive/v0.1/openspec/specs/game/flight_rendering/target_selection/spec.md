# Target Selection

## Overview
Players can select a celestial body or background vessel as a navigation target. Single-clicking a body or vessel shows a popup to set it as the active target. The target is displayed in the bottom bar with an autopilot mode to point toward it.

## Interaction
- **Single-click** a body or vessel icon in flight view: shows a popup centered below the object with either "Set as Target" (if not the current target) or "Unselect Target" (if already the current target)
- **Double-click** is preserved: focuses camera on body or switches to vessel (unchanged)
- Clicking empty space clears the popup and falls through to orbit click detection (maneuver nodes)

## Target Types
- `SelectedTarget::Body(usize)` - a celestial body by index
- `SelectedTarget::Vessel(u64)` - a background vessel by ID

## UI Elements

### Target Popup
- Position computed dynamically each frame from the target's world position (not stored at click time)
- For bodies: centered horizontally below the body circle, offset by visual radius + 6pt
- For vessels: centered horizontally below the vessel icon, offset 14pt
- Uses `egui::Align2::CENTER_TOP` pivot so the popup is centered below the anchor point
- Tracks the body/vessel as the camera moves (e.g., during ship tracking)
- Dismissed when: target is set/unset, user clicks elsewhere, double-click occurs, or target goes off-screen
- All positions are in egui logical points (physical pixels / scale_factor) for HiDPI correctness
- Flight mode has no camera drag — camera only moves by focusing on bodies (double-click) or tracking the ship

### Bottom Bar
- **TGT button**: autopilot mode button (same style as PRO/RET/R-/R+/MAN)
  - Only shown when a target is selected
  - Toggles `AutopilotTarget::Target` mode
  - Ship rotates to point toward the target
- **Target name**: displayed as "-> {name}" in light blue (rgb 130, 190, 255)
- **Clear button**: small "x" to remove the target selection

### Autopilot
- `AutopilotTarget::Target` variant added to the autopilot enum
- Target angle computed each frame in `main.rs`: `atan2(target_y - ship_y, target_x - ship_x)` using absolute positions
- Angle stored in `render_state.selected_target_angle` and used by autopilot rotation logic

## Implementation Files
- `src/render/types.rs` - `SelectedTarget` enum, `TargetPopup` struct
- `src/render/state.rs` - Target state fields, popup UI, TGT button, target display
- `src/ship/mod.rs` - `AutopilotTarget::Target` variant
- `src/main.rs` - Click handling, angle computation, autopilot wiring

## Transfer Planner Integration
- **Nav → Planner**: When the navigation target changes to a body that is a valid transfer target, the planner dropdown auto-selects it and switches mode (Hohmann or Lambert) accordingly
- **Planner → Nav**: When the user selects a target from the planner dropdown, the navigation target (`selected_target`) is synced to match, keeping both systems in agreement
- If the navigation target body appears in the Hohmann target list, it is selected and mode switches to Hohmann
- If it appears in the Lambert target list instead, it is selected and mode switches to Lambert

## Data Flow
1. Single-click detected in `handle_flight_mouse_input()` -> `target_popup` set on `RenderState`
2. Popup rendered in egui; "Set as Target" click sets `selected_target` and `selected_target_name`
3. Each frame, `main.rs` computes angle from ship to target absolute position
4. Autopilot uses the precomputed angle when `AutopilotTarget::Target` is active
5. Target cleared via "x" button on bottom bar
6. When transfer planner opens, `selected_target` is checked to auto-populate `transfer_selected_target`
