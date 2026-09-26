# Colony Overview Screen

Full-screen colony dashboard accessed from the main menu. Lists all established colonies as summary cards with key status indicators, providing a top-level view before drilling into individual colony management.

## GameMode & Navigation

- `GameMode::ColonyOverview` — dedicated game mode
- **Entry**: Main Menu "Colonies" button (`MainMenuAction::Colonies`) calls `Game::enter_colony_overview()`, which sets mode and unpauses
- **Exit**: Pause overlay "Main Menu" button, or "Open" on a colony card

## ColonyOverviewAction

```
None, OpenColony(usize), GoToMainMenu, ChangeWarp(usize)
```

- `OpenColony(body_index)` — enters `GameMode::Colony` for that body via `game.enter_colony(bi, GameMode::ColonyOverview)`, so the colony screen returns here
- `GoToMainMenu` — calls `game.enter_main_menu()`
- `ChangeWarp(idx)` — sets `game.warp_index`

## Top Panel

Horizontal bar containing:
- "Colony Overview" heading (18pt white)
- Time warp buttons: selectable labels for each warp level, with abbreviated labels (e.g. "1x", "10K", "1M", "1B"). Current level highlighted
- "Current: Nx" display
- Date string
- Right-aligned: company money (green, formatted via `format_money`) and science (blue, `"Science: {:.0}"`)

## Pause Overlay

When `game.paused`:
- Semi-transparent black overlay (alpha 180) centered on screen
- "Paused" heading (32pt white)
- "Main Menu" button (18pt) returning `GoToMainMenu`
- Toast notifications still render on top
- Early return — central panel is not drawn

## Central Panel — Colony List

Vertical `ScrollArea` with "Colonies" heading (24pt white).

### Empty State

When `colony_manager.colonies` is empty:
- "No colonies established yet." (16pt, gray)
- "Land a vessel with a Habitat and Solar Farm to start a colony." (13pt, darker gray)

### Colony Cards

One card per colony in `colony_manager.colonies`. Each card is a styled `egui::Frame`:
- Background: rgba(30, 35, 50, 220), 6px rounding, 12px inner margin, 40px horizontal outer margin

Card layout (horizontal row):
1. **Name & body** (vertical): colony name (16pt bold white) + body name (12pt gray, looked up from `body_names[colony.body_index]`)
2. **Crew**: `"Crew: X / Y"` where Y = `colony.crew_capacity()`
3. **Power net**: `power_generated - power_consumed`, formatted as kW or MW (if abs >= 1000). Green if net >= 0, red if negative
4. **"CREW AT RISK" tag** (red, 11pt strong): shown when `habitat_power_fraction < 1.0` and crew > 0
5. **Food days**: `colony.food_days_remaining()`. Red if < 10 days, yellow if < 30 days, gray otherwise. Shows "Food: stable" if infinite, else `"Food: {days}d"`
6. **"NO FOOD" tag** (red, 11pt strong): shown when `food_stored <= 0.0` and crew > 0
7. **Building count**: `"{N} buildings"` from `colony.buildings.len()`
8. **"Open" button** (right-aligned): returns `OpenColony(colony.body_index)`

### Trade Routes Section

Below the colony list (after separator): trade route summary showing active routes and ships. See [Trade Routes spec](../trade_routes/spec.md) for the full trade route system.

## Toast Notifications

`render_toasts(ctx, active_toasts)` called at end of frame (both paused and unpaused paths).

## Rendering Pipeline (`menus.rs`)

`RenderState::render_colony_overview()` follows the standard wgpu + egui two-pass pattern:
1. Camera buffer update
2. Geometry render pass (planets in background — vertex/index buffer drawn with MSAA)
3. Egui render pass (colony overview UI on top)
4. Returns `(new_warp_index, ColonyOverviewAction)`

## Main Loop Integration (`render_colony_overview_frame`)

Per frame:
1. **Simulation** (if not paused): advance solar system time, update inactive vessels on-rails, `game.update_colonies(dt_sim)`
2. **Camera**: tracks Sun, builds scaled positions, body data, orbit data, accretion discs
3. **Render**: calls `render_state.render_colony_overview()` with colony manager, body names, warp levels, pause state, date, company money, science
4. **Action dispatch**: processes returned `ColonyOverviewAction`

## UI Implementation

`render_colony_overview_screen()` in `colony_overview_ui.rs`. Pure egui function accepting colony manager, body names, warp config, pause state, economy data, and active toasts. Returns `ColonyOverviewAction`.
