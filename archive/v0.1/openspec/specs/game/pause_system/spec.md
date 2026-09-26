# Pause System

## Overview
A universal pause system triggered by the Escape key in any game mode. Pausing freezes simulation and displays a modal overlay on top of the current mode's rendering.

## Behavior

### Pause Toggle
- Escape key toggles `game.paused` in ALL modes (TitleScreen, MainMenu, Editor, Flight, TrackingStation)
- **Editor exception**: In Editor mode, if a part is selected (palette or placed), Escape deselects it first. Only when nothing is selected does Escape toggle pause.
- Handled before mode-specific keyboard input (mode handlers don't see Escape)
- When paused, time warp resets to index 0 (1x)
- When paused, mode-specific keyboard input is blocked (only Escape to unpause works)

### Simulation Freeze
- `game.update()` returns immediately when `paused == true`
- Solar system stops advancing (bodies freeze in place)
- Ship physics stop (no integration, no fuel consumption)
- Note: render frames still process (camera remains responsive for egui interaction)

### Pause Overlay
- Semi-transparent dark background (rgba 0,0,0,180) with rounded corners
- Action button depending on mode:
  - **Title Screen**: "Quit Game?" heading + "Quit" button (exits application)
  - **Main Menu**: "Paused" heading + "Title Screen" button (saves game, returns to title screen)
  - **Editor**: "Paused" heading + "Main Menu" button (returns to main menu, no save)
  - **Flight**: Two views (default and quicksave list):
    - **Default view**: "Paused" heading + "Quicksave" button + "Load Quicksave" button (only if quicksaves exist) + "Recover Vessel" (if landed on recoverable body) + "Main Menu" button (shelves vessel, returns to main menu, no save)
    - **Quicksave list view** (`show_quicksave_list`): "Load Quicksave" heading + scrollable list of quicksaves (showing index + sim date, newest first) + "Back" button
  - **Tracking Station**: "Paused" heading + "Main Menu" button (returns to main menu, no save)
- Overlay drawn at `egui::Order::Foreground` to appear above all other UI
- `show_quicksave_list` resets to `false` when unpausing, loading a quicksave, or pressing Escape while in quicksave list view

### Saving
- **Auto-save**: Every 5 minutes of real time, the game auto-saves (checked each frame in the `RedrawRequested` handler)
- **Title Screen transition**: Main Menu → Title Screen saves via `save_and_quit_to_title()` in `src/main.rs`
- **Window close**: `CloseRequested` handler saves before exiting
- Editor, Flight, and Tracking Station return to Main Menu without saving
- All saves are gated on `game.save_name.is_some()` (no save on title screen)

### State
- `paused: bool` field on `Game` struct
- `toggle_pause()` method: flips `paused`, resets `warp_index` to 0 when pausing
- Mode transition methods (`enter_editor()`, `enter_flight()`, `enter_main_menu()`, `enter_tracking_station()`, `enter_title_screen()`) all set `paused = false`

### Integration with Render Methods
- `render()` (flight): accepts `paused: bool` parameter, draws overlay inside egui closure, returns `PauseAction`
- `render_tracking_station()`: accepts `paused: bool`, draws overlay, returns `PauseAction`
- `render_editor()`: pause overlay drawn in the egui callback from `main.rs`
- `render_main_menu()`: pause overlay drawn in the egui callback from `main.rs`
- `render_title_screen()`: quit overlay drawn in the egui callback from `main.rs`

## Types
- `PauseAction` enum: `None`, `Resume`, `MainMenu`, `RecoverVessel`, `Quicksave`, `LoadQuicksave(String)` (defined in `src/render/types.rs`)
- `TitleScreenAction` enum: `None`, `NewGame(String)`, `LoadGame(String)`, `QuitGame` (defined in `src/render/types.rs`)
- `QuicksaveInfo` struct: `filename`, `index` (u32), `simulation_time` (f64), `modified` (SystemTime) (defined in `src/save.rs`, re-exported from `src/render/mod.rs`)

## Implementation
- `Game::paused` and `Game::toggle_pause()` in `src/game.rs`
- Escape handling in keyboard input dispatch in `src/main.rs`
- `save_and_quit_to_title()` in `src/main.rs`
- Overlay rendering distributed across render methods in `src/render/state.rs` and `src/main.rs`
