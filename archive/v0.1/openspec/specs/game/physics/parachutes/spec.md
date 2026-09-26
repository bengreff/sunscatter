# Parachutes

Deployable drag devices for atmospheric braking. Two sizes available, single-use, with animated canopy rendering and altitude-based two-stage deployment.

## Part Definition

### Requirement: ParachuteData struct

`PartDefinition` SHALL include an optional `parachute: Option<ParachuteData>` field containing:
- `deployed_width: f64` - canopy diameter in grid squares

Parts with parachute data SHALL use category `Aerodynamic` and shape `Rectangle`.

### Requirement: Part sizes

Two parachute sizes SHALL be defined in `data/parts/parachutes.ron`. Both are dome-shaped (semicircle, bottom-aligned) within a 1x1 grid hitbox. Deployed width = 40x dome width.
- **Small** (`parachute_small`): 0.5x0.5 grid dome (0.25m), deployed_width 20.0 (10m canopy), mass 0.03t, Tiny size
- **Large** (`parachute_large`): 1.0x1.0 grid dome (0.5m), deployed_width 40.0 (20m canopy), mass 0.10t, Tiny size

## Flight State

### Requirement: FlightPart parachute fields

`FlightPart` SHALL include:
- `is_parachute: bool` - true if part has parachute data
- `parachute_deployed: bool` - currently deploying/deployed
- `parachute_spent: bool` - used once, permanently disabled
- `parachute_deploy_fraction: f64` - 0.0-1.0 animation progress
- `parachute_deployed_width_m: f64` - canopy diameter in meters (from definition)
- `parachute_fully_deployed: bool` - true when altitude <= 2000m (full drag mode)

### Requirement: Two-stage deployment

Parachutes SHALL have two deployment stages based on altitude:
- **Partial deployment** (above 2000m): canopy deploys at 50% visual width, drag multiplier = 1x
- **Full deployment** (at or below 2000m): canopy deploys at 100% visual width, drag multiplier = 50x

`parachute_fully_deployed` is set to `true` when `altitude <= 2000.0` for any deployed parachute. The transition is instantaneous when crossing the 2000m threshold.

### Requirement: Deploy animation

`FlightVessel::update_parachute_deploy(dt, altitude)` SHALL:
1. Animate `parachute_deploy_fraction` toward the target at 1.0 fraction/second (1 second full deploy). Only active on non-destroyed, non-decoupled parachute parts.
2. Set `parachute_fully_deployed = altitude <= 2000.0` for deployed parachutes.

### Requirement: Auto-retract on vacuum/landing

`FlightVessel::auto_retract_parachutes(in_atmosphere, is_landed)` SHALL set `parachute_deployed = false` and `parachute_spent = true` for any deployed parachute when:
- The vessel leaves atmosphere (vacuum)
- The vessel lands

When landing (`is_landed`), `parachute_deploy_fraction` SHALL be set to `0.0` instantly (no retract animation), so the canopy disappears immediately on touchdown.

Spent parachutes cannot be re-deployed.

### Requirement: Staging activation

Parachutes SHALL be stageable parts. When placed in the editor, parachutes are automatically added to stage 0, the same as engines, decouplers, and fairings. Parachutes appear by name in the staging panel in both editor and flight.

When `activate_next_stage()` fires, any non-spent, non-deployed parachute in that stage SHALL have `parachute_deployed` set to `true`, but only if the vessel is in atmosphere and not landed. If the vessel is in vacuum or landed, staged parachutes are skipped (not deployed, not spent).

### Requirement: Manual cut

A deployed parachute MAY be manually cut via a "Cut" button in the flight part info panel. Cutting SHALL immediately set `parachute_deployed = false`, `parachute_spent = true`, `parachute_deploy_fraction = 0.0`, and `parachute_fully_deployed = false`. The parachute becomes permanently spent and cannot be re-deployed.

### Requirement: Single-use lifecycle

Parachute state progression SHALL be: Ready -> Deployed -> Spent (via auto-retract or manual cut). Once spent, a parachute cannot be re-deployed. The deploy button SHALL show "Spent" with a disabled state.

## Drag Integration

### Requirement: Parachute drag width

`FlightVessel::parachute_drag_width()` SHALL return the sum of `parachute_deployed_width_m * parachute_deploy_fraction` for all active deployed parachutes.

### Requirement: Altitude-dependent drag multiplier

`FlightVessel::parachute_drag_multiplier()` SHALL return:
- `100.0` if any deployed, non-destroyed, non-decoupled parachute has `parachute_fully_deployed == true`
- `1.0` otherwise

Parachute drag SHALL be added to the body cross-section before computing drag force, independent of vessel orientation: `total_cross_section = body_cross_section + parachute_drag_width * parachute_drag_multiplier`.

### Requirement: VesselPhysicsData fields

`VesselPhysicsData` SHALL include:
- `parachute_drag_width: f64` - populated from `FlightVessel::parachute_drag_width()`
- `parachute_drag_multiplier: f64` - populated from `FlightVessel::parachute_drag_multiplier()`

## Deploy UI

### Requirement: Flight part info panel

When clicking a parachute part in flight, the info panel SHALL show:
- "Deployed Width: X.X m"
- **Ready state**: "Deploy" button, enabled only when `in_atmosphere && !is_landed`
  - Disabled hover text: "Cannot deploy in vacuum" or "Cannot deploy while landed"
- **Deployed state**: "Cut" button to manually cut the parachute (marks as spent)
- **Spent state**: Disabled "Spent" button with hover text "Parachute already used"

### Requirement: Editor info panel

The editor part info panel SHALL show "Deployed Width: X.X m" for parachute parts.

## Canopy Rendering

### Requirement: Canopy geometry

When `parachute_deploy_fraction > 0`, a canopy SHALL be rendered:
- Semicircle oriented with flat edge facing the ship, dome pointing retrograde
- Alternating white/orange wedge pattern (12 segments)
- 5 thin white cable lines from flat edge to the part's sprite top attachment point
- Cable width: 0.1% of canopy radius, minimum 0.015m
- Cable length = 2 x deployed_width x deploy_fraction x visual_width_scale
- Canopy radius = 0.5 x deployed_width x deploy_fraction x visual_width_scale
- Canopy vertices are generated in meter space and transformed to screen space by the caller

### Requirement: Visual width scaling

The canopy SHALL accept a `visual_width_scale` parameter:
- `0.5` when partially deployed (above 2000m, `parachute_fully_deployed == false`)
- `1.0` when fully deployed (at or below 2000m, `parachute_fully_deployed == true`)

This scales both the canopy radius and cable length.

### Requirement: Cable anchor point

Cables SHALL anchor to the dome top of the parachute part, 0.25 grid squares (0.125m) below the sprite top. For bottom-aligned sprites within a hitbox, the anchor Y coordinate SHALL be: `local_y - hitbox_half_h + sprite_half_h * 2.0 - 0.125`.

`ShipPartRenderData` SHALL include `sprite_half_h: f64` (visual sprite half-height in meters) and `parachute_fully_deployed: bool` for this purpose.

### Requirement: Retrograde orientation

The canopy SHALL point in the retrograde direction (opposite velocity). When velocity is near zero, it SHALL fall back to opposite the vessel heading.

### Requirement: Render pass order

Canopy vertices SHALL be rendered in a fourth pass after fairing shells, as part of the main ship rendering pipeline.
