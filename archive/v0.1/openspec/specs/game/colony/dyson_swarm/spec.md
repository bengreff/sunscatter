# Dyson Swarm

A swarm of 1 km^2 reflective mirror segments orbiting the Sun at 0.1 AU. Launched from colony mass drivers, primarily from Mercury.

## Orbit: 0.1 AU

All mirrors go to a single fixed circular orbit at 0.1 AU from the Sun. No target selection.

| Metric | Value |
|---|---|
| Irradiance | 136,100 W/m^2 |
| Power per mirror (1 km^2) | ~129 GW |
| Mirror temperature | ~222 C (495 K) |

## Mirror Segment

| Parameter | Value |
|---|---|
| Deployed area | 1 km^2 |
| Film | 2 um aluminized CP-1, 3 g/m^2 |
| Structure + attitude control | 500 kg |
| Base total mass | 3,500 kg |
| Base sail loading | 3.5 g/m^2 |
| Base lightness number (beta) | 0.44 |

### Sail Technology Upgrades

Tech efficiency line `sail_technology` reduces mirror mass:

| Tier | Film density | sigma (g/m^2) | beta | Capability |
|---|---|---|---|---|
| 0 | 3.0 g/m^2 | 3.5 | 0.44 | Orbiting swarm |
| 5 | 1.5 g/m^2 | 2.0 | 0.77 | Displaced orbits |
| 10 | 0.75 g/m^2 | 1.25 | 1.22 | Statite |
| 15 | 0.38 g/m^2 | 0.88 | 1.74 | Dyson bubble |

Film density formula: `3.0 * 0.5^(tier/5)` g/m^2
Total loading: film + 0.5 g/m^2 (structure)
Beta: 1.53 / sigma (critical sail loading = 1.53 g/m^2)

## Mirror Production

Factory recipe `MirrorSegmentAssembly`:

| Input | Amount |
|---|---|
| StructuralMetal | 2,500 kg |
| HighTempAlloys | 500 kg |
| Electronics | 300 kg |
| Superconductors | 200 kg |
| **Total input** | **3,500 kg** |
| **Output** | **1 MirrorSegment** |

- Power: 500 kW
- Batch time: 48 hours
- Efficiency line: `construction`
- Tech gate: construction tier 3
- Resource type: `MirrorSegment` (stored in colony stockpile)

## Power Delivery

Mirrors reflect sunlight but this power is not directly usable. Collector stations at 0.1 AU convert reflected light to laser beams, which are received by colony buildings.

### Power Chain

```
Sun -> Mirrors (reflect) -> Collector Stations (PV -> laser) -> Receiver Arrays (-> electricity)
         eta_reflect=0.95      eta_collector=0.50                   eta_receiver=0.90
```

### Collection Efficiency

Mirrors must tilt to both maintain altitude (solar pressure) and reflect light to collectors. The tilt angle depends on lightness number beta:

```
eta_collection = beta / (1 + beta)
```

| beta | eta_collection | Notes |
|---|---|---|
| 0.44 (tier 0) | 31% | Low efficiency |
| 1.0 (statite) | 50% | Half power to collectors |
| 1.74 (tier 15) | 64% | High efficiency |

This is a continuous function with no step at the statite threshold.

### Occlusion

As mirrors accumulate, they begin shadowing each other. Total intercepted power converges to stellar luminosity:

```
P_intercepted = L_sun * (1 - e^(-N * A_mirror / A_sphere))
```

Where:
- `L_sun = 3.828 * 10^26 W` (solar luminosity)
- `A_mirror = 1,000,000 m^2` (1 km^2 per mirror)
- `A_sphere = 4 * pi * (0.1 AU)^2 ~ 2.812 * 10^21 m^2` (surface area at 0.1 AU)

At game-achievable scales (< 1M mirrors), occlusion is negligible but implemented for correctness.

### Collector Station

Thin-film photovoltaic array with integrated laser transmitter, deployed at 0.1 AU alongside mirrors. Uses rigid PV cells rather than thin-film reflective coating, so mass does not scale with sail_technology tier. Receives concentrated reflected sunlight from swarm mirrors, converts to electricity via PV cells, then beams laser power to receiver arrays at colony locations. Each station processes up to 500 GW input at 50% combined PV+laser efficiency.

| Parameter | Value |
|---|---|
| Mass | 5,000 kg |
| Max input power | 500 GW |
| PV + laser efficiency | 50% |
| Deployment time | 2 days |
| Launch accel | 1,000g (ship-class, not mirror-class) |

Waste heat rejection: 250 GW via phononic metamaterial radiators (2,577 kg at 97 MW/kg, 6,000 K operating temp). Remaining 2,423 kg for solar-pumped laser core, concentrator optics, and structure.

Factory recipe `CollectorStationAssembly`:

| Input | Amount |
|---|---|
| StructuralMetal | 2,000 kg |
| HighTempAlloys | 800 kg |
| Electronics | 1,200 kg |
| Superconductors | 700 kg |
| PrecisionInstruments | 300 kg |
| **Total** | **5,000 kg** |
| **Output** | **1 CollectorStation** |

- Power: 500 kW
- Batch time: 48 hours
- Efficiency line: `construction`
- Tech gate: `swarm_power_delivery` efficiency line (gated at tier 0, requires `swarm_power` node)

### Receiver Array

Colony building that receives laser power from collector stations and converts to electricity.

| Parameter | Value |
|---|---|
| Max laser input | 50 GW |
| Laser -> electricity | 90% |
| Max power output | 45 GW (45,000,000 kW) |
| Build cost | 100,000 kg Metal + 20,000 kg HTA + 30,000 kg Elec + 10,000 kg Super |
| Maintenance/30d | 250 kg Metal + 50 kg HTA + 75 kg Elec + 25 kg Super |
| Tech gate | `swarm_power` node (Era 7, requires fusion_power) |

### End-to-End Power Formula

```rust
let filling = N_mirrors * MIRROR_AREA / A_SPHERE_01AU;
let intercepted = L_SUN * (1.0 - (-filling).exp());     // occlusion
let reflected = intercepted * 0.95;                       // mirror efficiency
let collected = reflected * (beta / (1.0 + beta));        // collection efficiency
let collector_limited = collected.min(N_collectors * 500e9); // collector capacity
let laser_power = collector_limited * 0.50;               // PV + laser
// Per colony: min(laser_power, N_receivers * 50e9) * 0.90 = electricity
```

### Receiver Allocation (v1)

Each colony draws from the shared laser pool up to its receiver capacity. No per-colony allocation — each colony independently caps at `N_receivers * 50 GW * 0.90`. Slightly generous if total receiver capacity exceeds laser power.

### Mass Driver Launch Priority

1. Ships (trade route queue)
2. Collector stations (1,000g accel)
3. Mirror segments (10,000g accel)

## Transfer Physics (Mercury -> 0.1 AU)

```
Mercury orbit:          r1 = 0.387 AU, v_circ = 47,876 m/s
Target orbit:           r2 = 0.1 AU,   v_circ = 94,182 m/s
From Mercury surface:   v_launch = 17,712 m/s (within Mk I mirror range)
```

### Solar Pressure Circularization

No kick stage. Solar radiation pressure at 0.1 AU:
```
Characteristic accel (base):  a_c = beta * 5.93 mm/s^2 = 2.61 mm/s^2
Scaled to 0.1 AU (x100):     a_c = 0.261 m/s^2
Circularization dv:           24,552 m/s
Time:                         ~1.1 days
```

Deployment time constant: `MIRROR_DEPLOY_TIME_S = 1.1 * 86400 = 95,040 s`

## Game State

### DysonSwarm struct (per-star, keyed by star body index)

```rust
// Game struct holds:
pub dyson_swarms: HashMap<usize, DysonSwarm>  // keyed by star body index

pub struct DysonSwarm {
    pub mirror_count: u64,                    // Operational mirrors
    pub deploying: Vec<DeployingMirror>,       // In-transit mirrors
    pub collector_count: u64,                  // Operational collector stations
    pub deploying_collectors: Vec<DeployingMirror>,  // In-transit collectors
}
```

Each star has its own independent swarm. Colonies look up their star via `SolarSystem::parent_star(body_index)` and access the corresponding swarm.

### DeployingMirror
```rust
pub struct DeployingMirror {
    pub arrival_time: f64,  // Sim time when operational
}
```

### Colony Fields
- `mass_driver_energy_j: f64` — Energy accumulated in mass driver capacitor
- `mirrors_launched: u64` — Lifetime mirrors launched from this colony
- `receiver_power_kw: f64` — Actual receiver array power output (kW), computed by simulation
- `receiver_laser_power_kw: f64` — Total laser power available to this colony's receivers (kW), computed by simulation. Used in the power card to explain receiver saturation (laser-limited vs receiver-limited tooltip).

## Deployment Flow

```
Colony Factory -> Mass Driver -> 0.1 AU Orbit
   Build MirrorSegment    Launch at 17.7 km/s    Arrives on Hohmann,
   from resources          (auto, power-limited)  solar pressure
                                                  circularizes in ~1 day
                                                  -> Mirror operational

Colony Factory -> Mass Driver -> 0.1 AU Orbit
   Build CollectorStation  Launch at 1000g        Arrives, deploys in 2 days
   from resources          (after ships,           -> Collector operational
                            before mirrors)
```

## Swarm Scaling

| Mirrors | Area | Power | Time @ Mk III (141/day) | Time @ Mk IV (14,141/day) |
|---|---|---|---|---|
| 100 | 100 km^2 | 12.9 TW | 17 hours | 10 seconds |
| 1,000 | 1,000 km^2 | 129 TW | 7.1 days | 1.7 hours |
| 10,000 | 10,000 km^2 | 1.29 PW | 71 days | 17 hours |
| 100,000 | 100k km^2 | 12.9 PW | 1.9 years | 7.1 days |

## Save/Load

`dyson_swarms: HashMap<usize, DysonSwarm>` is serialized in SaveGame with `#[serde(default)]`. Old saves with a single `dyson_swarm` field are migrated on load: if the new `dyson_swarms` map is empty but the old field has data, it is inserted under `sun_index`.

## UI

Dyson swarm cards are shown on the **colony overview** screen, grouped by star. Each star section shows its colonies followed by its dyson swarm card (if any). The per-colony screen does not show the swarm card.

The card is organized into three sections:

**Swarm Status** — always shown:
- Mirrors: count + total area (km^2)
- Collectors: count
- In transit: mirrors and/or collectors (yellow, shown when > 0)

**Power Chain** — shown when collectors exist (operational or in-transit):
- Reflected power (GW/TW/PW)
- Collection efficiency (from beta), with tooltip showing per-step efficiencies (mirror reflectivity, collector PV+laser, receiver conversion)
- Laser power available

**Technology** — always shown:
- Sail loading and lightness number (combined on one line: `"X.XX g/m^2  |  beta: X.XX"`)
- Statite capability status (green, shown when beta >= 1.0)

## Files

- `src/colony/dyson_swarm.rs` — DysonSwarm struct, physics functions, sail tech, power delivery
- `src/colony/buildings.rs` — MirrorSegmentAssembly + CollectorStationAssembly recipes, ReceiverArray building, Colony mass driver fields
- `src/colony/resources.rs` — MirrorSegment + CollectorStation ResourceTypes
- `src/colony/simulation.rs` — process_mass_driver() (ships > collectors > mirrors), swarm power in colony power balance
- `src/bodies.rs` — `SolarSystem::parent_star()` helper to find a body's star
- `src/render/colony_ui.rs` — render_dyson_swarm_card() (pub(super), reused by colony_overview_ui)
- `src/render/colony_overview_ui.rs` — star-grouped layout with dyson swarm cards
- `src/save.rs` — DysonSwarm save/load with old-format migration
- `src/game.rs` — dyson_swarms: HashMap<usize, DysonSwarm> field on Game struct
- `data/tech/tree.ron` — sail_technology + swarm_power_delivery efficiency lines, swarm_power node
