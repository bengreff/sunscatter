# Mass Driver

Electromagnetic mass driver buildings for launching payloads from airless colony surfaces. Replaces the original `Railgun` building type with four tiers of coilgun/mass driver architecture.

## Building Tiers

Four building variants: `MassDriverMk1` through `MassDriverMk4`. Each defined by track length and max power draw.

| Parameter | Mk I | Mk II | Mk III | Mk IV |
|---|---|---|---|---|
| Track length | 2 km | 10 km | 50 km | 200 km |
| Max payload | 10 t | 50 t | 200 t | 200 t |
| Ship max v (1,000g) | 6.3 km/s | 14.0 km/s | 31.3 km/s | 62.6 km/s |
| Mirror max v (10,000g) | 19.8 km/s | 44.3 km/s | 99.0 km/s | 198 km/s |
| Power draw | 10 MW | 100 MW | 1 GW | 100 GW |
| System mass | 500 t | 5,000 t | 50,000 t | 200,000 t |
| Tech era | 4 | 6 | 8 | 9 |

## Physics

### Acceleration Limits
- **Blueprint ships**: 1,000g (9,810 m/s^2) — safe for AM containment, pressurized tanks
- **Mirror segments**: 10,000g (98,100 m/s^2) — solid folded panels, no moving parts

### Key Equations
```
Launch velocity:      v = sqrt(2 * a * s)        (a = accel, s = track length)
Kinetic energy:       KE = 0.5 * m * v^2
Energy per launch:    E = KE / 0.90              (90% superconducting efficiency)
Recharge time:        t = E / P_avg              (P_avg = colony power to driver)
```

### Body Restriction
Airless bodies only (AtmosphereClass::Airless in `transfer.rs`: surface pressure < 1,000 Pa).

## Payload Types

### Blueprint Ship
Any uncrewed vessel blueprint. Mass driver imparts velocity; ship uses own engines for remainder. Mass limited by tier max payload.

### Mirror Segment
Standardized 1 km^2 reflective panel for Dyson swarm. Base mass 3,500 kg (varies with Sail Technology tier). See `dyson_swarm/spec.md`.

## Simulation

### Energy Accumulation
Each tick, the mass driver accumulates energy from colony power:
```
energy_this_tick = power_draw_kw * 1000 * other_power_fraction * days * 86400
```

### Auto-Fire (Mirror Launch)
When accumulated energy >= launch energy AND a MirrorSegment is in stockpile:
1. Consume 1 MirrorSegment from colony resources
2. Deduct launch energy from accumulator
3. Add mirror to DysonSwarm deployment queue (arrival after ~1.1 days)
4. Fire MirrorLaunched notification

Energy cap: stored energy capped at one launch worth (prevents infinite accumulation when no mirrors available).

## Trade Route Integration

### Delta-V Credit

`compute_leg_delta_v_with_mass_driver()` in `transfer.rs` applies mass driver velocity credit:
- `launch_dv` eliminated (mass driver handles surface-to-orbit)
- If mass driver velocity > escape velocity: `v_infinity = sqrt(v_launch^2 - v_esc^2)` reduces the **departure burn** only (not the combined transfer_dv)
- `LegResult` has separate `departure_burn` and `arrival_burn` fields
- Ship only needs delta-v for `reduced_departure_burn + arrival_burn + landing_dv`

### Route Configuration

`TradeRoute` fields:
- `use_mass_driver: bool` — enable mass driver departure for this route
- `mass_driver_tier: Option<BuildingType>` — specific tier, or None for best available

### Ship Queue (`WaitingForMassDriver` state)

When a trade ship launches on a mass driver route:
1. `launch_ship()` validates: airless body, operational mass driver, ship mass <= max payload
2. Ship state set to `WaitingForMassDriver` (blocks route from launching another ship)
3. Ship pushed to `Colony.mass_driver_ship_queue` as `MassDriverShipEntry`
4. `process_mass_driver()` processes ship queue **before** mirror launches:
   - Computes launch energy at 1,000g accel for ship mass
   - If energy available: deducts energy, transitions ship to `InTransit`
   - If not: breaks (mirrors also wait until ship launches)
5. Edge case: if required driver tier is no longer operational, ship refunded to `Stationed`

### Eligibility (UI)

Mass driver section shown in route creation panel when ALL are true:
- Source is a colony (not Earth)
- Source body is airless (< 1,000 Pa atmosphere)
- Colony has operational mass driver
- Blueprint has probe core (uncrewed only)

UI shows: checkbox, tier dropdown with launch velocities, delta-v savings vs chemical, payload mass warning.

## Tech Tree

| Building | Tech Node | Era | Science |
|---|---|---|---|
| Mk I | deep_space_hab | 5 | 2,500 |
| Mk II | mass_driver_tech | 6 | 5,000 |
| Mk III | heavy_mass_driver | 8 | 10,000 |
| Mk IV | planetary_mass_driver | 9 | 15,000 |

## UI

Mass driver card shown in colony screen when colony has an operational mass driver. Displays:
- Type and track length
- Ship/mirror max velocity
- Power draw
- Energy stored in capacitor
- Mirror segments available
- Lifetime mirrors launched
- Mirror launch cadence (calculated from energy/power)

## Files

- `src/colony/buildings.rs` — BuildingType variants, stat methods, physics helpers, `MassDriverShipEntry`, `mass_driver_ship_queue` on Colony
- `src/colony/simulation.rs` — `process_mass_driver()` tick logic (ship queue + mirror auto-launch)
- `src/colony/transfer.rs` — `compute_leg_delta_v_with_mass_driver()`, `LegResult` with departure/arrival burn fields
- `src/colony/trade.rs` — `TradeShipState::WaitingForMassDriver`, `TradeRoute.use_mass_driver`/`mass_driver_tier`
- `src/render/colony_ui.rs` — `render_mass_driver_card()`
- `src/render/trade_ui.rs` — Mass driver departure section in route creation panel
- `data/tech/tree.ron` — Tech nodes for Mk I-IV
