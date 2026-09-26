# Main Menu

## Overview
The main menu is the in-game hub, accessible after starting or loading a game. It displays an animated solar system background with a time warp bar and a centered menu overlay offering navigation to the Editor and Tracking Station.

## Behavior

### Display
- Solar system bodies and orbits render as the background (same wgpu pipeline as flight)
- Camera always focuses on the Sun at a fixed zoom (0.002) to show the full solar system
- Camera zoom and position are locked (scroll zoom has no effect)
- Time warp panel at top of screen (functional — solar system advances in real-time, with date display)
- Centered overlay with semi-transparent dark background:
  - "Sunscatter" title heading (36pt, white)
  - "Editor" button (20pt) — opens the ship editor
  - "Tracking Station" button (20pt) — opens the tracking station

### State
- Solar system simulation runs (bodies orbit, time advances)
- Time warp controls are active (can speed up background animation)
- Active vessel propagates on rails (if on_rails)
- Inactive vessels propagate on rails; deleted if entering atmosphere or below landing altitude

### Navigation
- Clicking "Editor" transitions to `GameMode::Editor` via `game.enter_editor()`
- Clicking "Tracking Station" transitions to `GameMode::TrackingStation` via `game.enter_tracking_station()`, focuses camera on Earth at half-screen zoom
- Escape key pauses (shows pause overlay with "Title Screen" button)

### Pause Overlay (Main Menu)
- When paused, the overlay shows "Paused" heading and "Title Screen" button
- "Title Screen" saves the game and returns to the title screen via `save_and_quit_to_title()`

## Implementation
- `GameMode::MainMenu` variant in `src/game.rs`
- `render_main_menu()` method on `RenderState` in `src/render/state.rs`
- `render_main_menu_frame()` orchestrator in `src/main.rs`
- Returns `MainMenuAction` enum (`None`, `Editor`, `TrackingStation`, `Quit`)

# Title Screen

## Overview
The title screen is the pre-game entry point. It displays a static solar system background (no time advancement, no time warp bar) with "New Game" and "Load Game" buttons.

## Behavior

### Display
- Solar system bodies and orbits render as static background (no simulation update)
- Camera fixed on Sun at zoom 0.002
- No time warp panel
- Centered overlay:
  - "Sunscatter" title heading (48pt, white)
  - "New Game" button (20pt) — opens name input dialog
  - "Load Game" button (20pt) — opens save file list

### New Game Dialog
- Name input field (default "default")
- "Start" button — creates fresh game, transitions to MainMenu
- "Back" button — returns to main title screen
- Enter key in the text field also starts the game

### Load Game Dialog
- Scrollable list of save files from `data/saves/`
- Each entry shows: save name, vessel count, simulation date
- Clicking a save loads it and transitions to MainMenu
- "Back" button — returns to main title screen

### Escape / Quit
- Escape shows quit confirmation overlay: "Quit Game?" + "Quit" button
- "Quit" exits the application

## Implementation
- `GameMode::TitleScreen` variant in `src/game.rs`
- `TitleScreenUiState` struct in `src/game.rs` (show_new_game, show_load_game, new_game_name)
- `render_title_screen()` method on `RenderState` in `src/render/state.rs`
- `render_title_screen_frame()` orchestrator in `src/main.rs`
- Returns `TitleScreenAction` enum (`None`, `NewGame(String)`, `LoadGame(String)`, `QuitGame`)
