# Flight HUD

Flight HUD panels: time warp, orbital info, autopilot, vessel stats, velocity/altitude, throttle, fuel, staging.

## Time Warp Panel

### Requirement: Time warp panel at top of screen

An egui top panel named "time_warp_panel" SHALL display time warp controls and orbital information.

#### Scenario: Time warp button labels
- Warp levels: `[1, 2, 3, 5, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000, 10000000000, 100000000000, 1000000000000]` (16 levels, up to 1 trillion)
- Labels: `>= 1,000,000,000` -> "{X}B", `>= 1,000,000` -> "{X}M", `>= 1,000` -> "{X}K", else "{X}x"
- Current warp level shown as selected via `egui::SelectableLabel`

#### Scenario: Warp blocking while thrusting
- **WHEN** throttle > 0.0 AND ship has positive thrust acceleration
- **THEN** warp levels where warp > `RAILS_WARP_THRESHOLD` (10.0) SHALL be disabled

#### Scenario: Warp blocking near SOI boundary
- **WHEN** warp > `RAILS_WARP_THRESHOLD` (on-rails only) AND `time_to_intercept / warp_rate < 0.5` seconds of real time
- **THEN** that warp level SHALL be disabled
- Physics warp levels (≤ `RAILS_WARP_THRESHOLD`) are never blocked by SOI proximity, as physics substeps handle SOI transitions precisely

#### Scenario: Warp blocking below landing altitude
- **WHEN** ship is below the current SOI body's landing altitude
- **THEN** warp levels where warp > `RAILS_WARP_THRESHOLD` (10.0) SHALL be disabled (greyed out)

#### Scenario: Current warp display
- After separator: "Current: {warp}x" where warp is cast to i64

### Requirement: FPS counter and economy display

Right-aligned in the time warp panel row: company money (green, size 11, formatted via `format_money()`), science available ("Sci: {value:.0}", blue `rgb(100, 180, 255)`, size 11), separator, FPS counter ("{fps:.0} fps" in gray text at size 11). FPS smoothed via exponential moving average (`fps = fps * 0.95 + instant * 0.05`).

## Orbital Information

### Requirement: Orbital information display

Below time warp buttons, orbital information SHALL be shown horizontally when the ship has orbit data.

#### Scenario: Orbital info fields
- "Ap: {altitude}" "({time_to_ap})" | "Pe: {altitude}" "({time_to_pe})" | "T: {period}" | "e: {eccentricity:.3}" | "{soi_body_name}"

#### Scenario: Distance formatting
- `>= 1e9` -> "X.XX Gm", `>= 1e6` -> "X.X Mm", `>= 1e3` -> "X.X km", else "X m"

#### Scenario: Time formatting
- `>= 86400 * 365` -> "X.Xy", `>= 86400` -> "X.Xd", `>= 3600` -> "X.Xh", `>= 60` -> "X.Xm", else "Xs"

## Bottom Panel

### Requirement: Bottom panel with autopilot and vessel stats

An egui bottom panel named "flight_info_panel" with background `rgba(20, 20, 30, 200)` SHALL show autopilot buttons, vessel stats, and velocity/altitude in a horizontal row.

### Requirement: RCS toggle button

An "RCS" toggle button SHALL appear at the left of the bottom panel, before the SAS buttons. RCS SHALL be off by default. Pressing R on the keyboard SHALL toggle RCS on/off.

#### Scenario: RCS button colors
- Active fill = `rgb(80, 150, 80)`, inactive = `rgb(60, 60, 70)`
- Active text = white, inactive text = light gray
- Button style matches SAS buttons (text size 11, min size 35x20)
- **No control**: fill = `rgb(40, 40, 45)`, text = `rgb(80, 80, 80)`, clicks ignored

#### Scenario: RCS disabled effects
- **WHEN** RCS is disabled
- **THEN** RCS torque SHALL be 0 (no rotational authority from RCS thrusters), RCS translation force SHALL be 0, RCS fuel SHALL NOT be consumed, WASD translation SHALL have no effect, and no RCS plumes SHALL be rendered

### Requirement: Autopilot (SAS) buttons

The SAS system SHALL display buttons for: "PRO" (Prograde), "RET" (Retrograde), "R-" (RadialIn), "R+" (RadialOut), and "MAN" (ManeuverNode, only when a maneuver node is selected).

#### Scenario: Button colors
- Active fill = `rgb(80, 150, 80)`, inactive = `rgb(60, 60, 70)`
- Active text = white, inactive text = light gray
- Button text size = 11, minimum size = 35x20
- **No control**: fill = `rgb(40, 40, 45)`, text = `rgb(80, 80, 80)`, clicks ignored

#### Scenario: Toggle behavior
- Clicking active button toggles to Off; clicking inactive button activates that mode

### Requirement: No control indicator

When the active vessel has no functioning command pod (`ship_has_control = false`), a red "NO CONTROL" label SHALL be displayed in the autopilot buttons row, between the SAS label and the autopilot buttons.

### Requirement: Vessel stats display

The bottom panel SHALL show: mass ("M: {mass:.2}t"), thrust ("T: {thrust:.0}kN"), drag ("D: {drag:.1}kN", only when drag > 0.01 kN), TWR ("TWR: {value:.2}" green if >= 1.0, red if < 1.0), delta-v ("Dv: {formatted}"), and G-force ("G: {value:.1}" color-coded: white < 3g, yellow < 6g, red >= 6g). G-force SHALL use atmospheric-adjusted thrust (interpolated between vacuum and sea-level values based on atmospheric pressure fraction), not vacuum thrust.

### Requirement: Atmospheric thrust and TWR adjustment

Thrust and TWR displayed in the bottom panel SHALL be adjusted for atmospheric pressure. The thrust SHALL be linearly interpolated between vacuum and sea-level values based on the atmospheric pressure fraction at the ship's current altitude: `thrust = thrust_vac * (1 - pressure) + thrust_asl * pressure`, where pressure is `atmosphere.pressure_at_altitude(alt) / 101325` clamped to [0, 1]. TWR SHALL use this atmospheric thrust value. Delta-v estimates SHALL always use vacuum ISP.

### Requirement: Velocity and altitude display

Right side of bottom panel SHALL show: "VEL" label with value at font size 13 strong, and "ALT" label with value at font size 13 strong.

#### Scenario: Velocity formatting
- `>= 1000` -> "{X.XX} km/s", else "{X.X} m/s"

#### Scenario: Altitude formatting
- `>= 1e9` -> "{X.XX} Gm", `>= 1e6` -> "{X.XX} Mm", `>= 1e3` -> "{X.XX} km", else "{X.X} m"

## Throttle Bar

### Requirement: Throttle bar on right side panel

An egui right panel named "throttle_panel" with width 50 and background `rgba(20, 20, 30, 200)`. Created after the staging panel so it appears to the left of staging in the UI. Contains only the THR label, percentage, and throttle bar.

### Requirement: Status panel on left side

An egui left panel named "status_panel" with width 50 and background `rgba(20, 20, 30, 200)`. Contains fuel, electricity, heat bars, stage indicator, XFER button, and debug button.

#### Scenario: Throttle bar layout
- "THR" label at font size 10, percentage at font size 12 strong
- Vertical bar: height 150px, width 20px, fill from bottom up

#### Scenario: Throttle bar colors
- Background = `rgb(40, 40, 50)`, border = gray 1.0px
- Fill: `< 0.5` -> green `rgb(100, 200, 100)`, `< 0.8` -> yellow `rgb(200, 200, 100)`, `>= 0.8` -> red `rgb(200, 100, 100)`

### Requirement: Fuel bar below throttle

When vessel has fuel data, show fuel bar with 10px gap below throttle. The fuel bar SHALL display fuel accessible to currently active engines (i.e., fuel in the same fuel zones as active engines), not total vessel fuel. This uses `compute_fuel_zones()` to determine which tanks feed active engines. If no engines are active, the bar shows 0%.

#### Scenario: Fuel bar colors
- `> 0.3` -> blue `rgb(80, 160, 220)`, `> 0.1` -> amber `rgb(220, 180, 80)`, `<= 0.1` -> red `rgb(220, 80, 80)`
- Bar height 80px, width 20px

### Requirement: Electricity bar below fuel bar

When the vessel has batteries (max electricity > 0), an electricity bar SHALL be shown below the fuel bar with 10px gap.

#### Scenario: Electricity bar layout
- "ELEC" label at font size 10, stored/max Wh readout at font size 11
- Bar height 80px, width 20px, fill from bottom proportional to `electricity_fraction`
- Stored and max Wh formatted as "{stored} / {max} Wh" where each value uses "{X.X}k" for >= 1000 or "{X}" otherwise

#### Scenario: Electricity bar colors
- `> 0.3` -> gold/yellow `rgb(220, 200, 80)`, `> 0.1` -> orange `rgb(220, 160, 60)`, `<= 0.1` -> red `rgb(220, 80, 80)`
- Background `rgb(40, 40, 50)`, border gray 1px

#### Scenario: Power generation/consumption text
- Below the bar, "+{gen}W" and "-{cons}W" SHALL be displayed at font size 9

#### Scenario: Power duration display
- **WHEN** net power (generation - consumption) < 0 AND stored electricity > 0
- **THEN** an amber `rgb(220, 180, 40)` duration label SHALL be shown below the power text at font size 9
- Duration formula: `seconds = (stored_wh / |net_watts|) * 3600`
- Duration formatted as `format_duration()`: "Xd Xh Xm Xs" / "Xh Xm Xs" / "Xm Xs" / "Xs"

### Requirement: Heat bar below electricity bar

When ship temperature exceeds 350K, a vertical heat bar SHALL be shown:
- "HEAT" label at font size 10, temperature readout in Kelvin at font size 11
- Bar height 80px, width 20px, fill from bottom proportional to `heat_fraction`
- Colors: `< 0.33` -> yellow `rgb(220, 200, 80)`, `< 0.66` -> orange `rgb(220, 140, 40)`, `>= 0.66` -> red `rgb(220, 60, 60)`
- Background `rgb(40, 40, 50)`, border gray 1px

### Requirement: Critical part indicator below heat bar

Below the heat bar, when the hottest part has `heat_fraction > 0.01`, its name SHALL be displayed (truncated to 8 characters) at font size 8, colored by the same yellow/orange/red severity thresholds as the heat bar. This shows which part is closest to its critical temperature.

### Requirement: Stage indicator below fuel/heat bars

When vessel has stages, show "STG" label and "{current}/{total}" at font size 12 strong.

## Staging Panel

### Requirement: Staging panel on right side

When vessel has stages, an egui right panel named "flight_staging_panel" with default width 150 SHALL show the staging panel.

#### Scenario: Header
- "Staging" heading, total delta-v if > 0, "Active: {current}/{total}" in gray

#### Scenario: Stage display order
- Reverse order (highest index at top), 1-based labels
- Activated stages use dark gray label, unactivated use white

#### Scenario: Per-stage delta-v
- Green `rgb(120, 200, 120)` at font size 10
- `>= 1000` -> "{X.X}km/s", else "{X.0}m/s"

#### Scenario: Per-stage burn time
- Green `rgb(120, 200, 120)` at font size 10, displayed as "Burn: {duration}"
- Uses `format_duration()` (e.g., "1m 30s")
- Only shown when burn time > 0

#### Scenario: Selected part highlighting
- When a flight part is selected (clicked in the viewport), its corresponding staging entry SHALL be highlighted in light blue `rgb(128, 179, 255)`
- Selection is matched by comparing `selected_flight_part` cache index → `part_index` against each staging entry's `part_index`

#### Scenario: Drag-and-drop
- Stage headers are drag sources for reordering
- Parts within stages are drag sources for moving between stages
- "+" buttons between stages insert empty stages

#### Scenario: Empty stage deletion
- Empty stages (containing no parts) SHALL show a "✕" button in the stage header row
- Clicking it removes the stage via the staging reorder mechanism
- Non-empty stages SHALL NOT show a delete button

## Flight Part Info

### Requirement: Flight part info popup

When a flight part is selected, an egui Window with the part's name SHALL show part details.

#### Scenario: Engine info
- Thrust (vac/ASL), ISP (vac/ASL), propellant type
- Status: "Active" (green), "No Fuel" (amber), "Disabled" (red)
- Activate/Deactivate toggle button

#### Scenario: Tank info
- Fuel type, oxidizer progress bar (`rgb(80, 140, 200)`), fuel progress bar (`rgb(200, 160, 60)`)

#### Scenario: Pod info
- Crew capacity
- Monopropellant progress bar (`rgb(180, 200, 80)`) showing current/max kg, when pod has monopropellant

#### Scenario: Decoupler info
- Enable/Disable Crossfeed toggle, "Decouple" button

## Pause Overlay

### Requirement: Pause overlay with recovery and navigation

When the game is paused in flight mode, a centered overlay SHALL appear with pause controls.

#### Scenario: Recover Vessel button
- **WHEN** the active vessel is `Landed` on a recoverable body (currently Earth, `LAUNCHPAD_BODY_INDEX`)
- **THEN** a green-tinted `rgb(60, 130, 60)` "Recover Vessel" button SHALL appear above the "Main Menu" button
- **WHEN** clicked, the vessel is removed from the simulation (not shelved), maneuver nodes are discarded, and the game returns to the main menu

#### Scenario: Main Menu button
- **WHEN** the ship can safely exit flight (landed, or not in atmosphere/landing zone while suborbital)
- **THEN** a "Main Menu" button is enabled; the active vessel is shelved to inactive list
- **WHEN** the ship cannot safely exit, the button is disabled with explanatory text

#### Scenario: Launchpad clearing on launch
- **WHEN** a new vessel is launched from the editor
- **THEN** all inactive vessels that are `Landed` on `LAUNCHPAD_BODY_INDEX` near `LAUNCHPAD_SURFACE_ANGLE` SHALL be auto-recovered (removed) before the new vessel spawns
