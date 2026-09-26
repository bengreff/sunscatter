# Management Screen

Company management screen for finances, R&D budget, science overview, tech tree access, and contract management. Accessed as a dedicated game mode from the main menu.

## GameMode::Management

Full-screen management UI entered via "Management" button on the main menu. Uses `ManagementAction` enum for all user actions.

### ManagementAction

```
None, OpenTechTree, GoToMainMenu, ChangeWarp(usize),
AcceptContract(ContractType), SetRdBudget(f64)
```

### render_management_screen()

Full-screen egui layout in `render/management_ui.rs`. Returns `(ManagementAction, f64)` where the f64 is the updated R&D budget value.

Parameters: `company: &Company`, `science: &ScienceState`, `contracts: &ContractManager`, `rd_budget: f64`, `warp_levels: &[f64]`, `current_warp_index: usize`, `date_str: &str`, `paused: bool`, `active_toasts`.

- **Top panel**: "Management" heading, time warp selector buttons (formatted as x/K/M/B), current warp display, date string
- **Central panel**: ScrollArea with sections: Finances, R&D, Science, Contracts
- **Pause overlay**: Semi-transparent overlay with "Paused" heading and "Main Menu" button. Returns early when paused (skips central panel).

#### Finances

- Company money displayed in large (24pt) green text via `format_money()`

#### Research & Development

- R&D Budget: DragValue in millions/year (0-1000 M/yr range, 0.5 step)
- Internal value stored in raw currency units; UI converts to/from millions for display
- Changes emit `ManagementAction::SetRdBudget(budget)` with the raw value

#### Science

- Available science in blue text (18pt), formatted to integer
- Cumulative breakdown in small gray text: Discovery | R&D | Lab
- "Open Tech Tree" button emits `ManagementAction::OpenTechTree`

#### Contracts

**Available contracts**: Iterates `ContractType::all()`. Each contract shown in a styled frame (dark background, rounded) with:
- Contract display name (bold) on the left
- Payout in green (right-aligned) via `format_money()`
- "Accept" small button if not already active, or "Active" badge in blue if already accepted
- Description in small gray text below
- Accepting emits `ManagementAction::AcceptContract(ContractType)`

**Active contracts**: Listed below available contracts (only if non-empty). Each shows contract name and payout.

### Toast Notifications

Rendered via `render_toasts()` on both paused and unpaused states.

## Main Loop Integration

### Main Menu Frame

- "Management" button on main menu enters `GameMode::Management`
- `render_management_frame()` processes `ManagementAction` variants:
  - `GoToMainMenu` returns to `GameMode::MainMenu`
  - `OpenTechTree` enters `GameMode::TechTree`
  - `ChangeWarp(idx)` updates warp index
  - `AcceptContract(ct)` delegates to contract manager
  - `SetRdBudget(val)` updates company R&D budget
- Colony simulation (`game.update_colonies(dt_sim)`) runs while unpaused
- Notifications processed after render (toasts, warp stops)
