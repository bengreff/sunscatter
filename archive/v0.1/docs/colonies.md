# Economy, Colonies & Supply Chain

---

## 1. Resources

Every resource in the game. All quantities in **kg**.

### Raw Resources (extracted by Mine or Atmospheric Collector)

| # | Resource | Description | Earth Price ($/kg) |
|---|----------|-------------|-------------------:|
| 1 | **Metal Ore** | Iron, aluminum, titanium bearing rock. Smelted into Structural Metal. | $5 |
| 2 | **Regolith** | Loose surface material from airless bodies. Source of He-3 on the Moon. | N/A |
| 3 | **Water** | Mined from ice deposits. Electrolyzed for LH2/LOX. Circulated (not consumed) by greenhouses. | $2 |
| 4 | **Lithium Ore** | Lithium-bearing minerals. Input for tritium breeding. | $50 |
| 5 | **Hydrocarbons** | Methane/ethane deposits (Titan surface lakes). Refined into Methane or RP-1. | $1 |
| 6 | **Atmospheric CO2** | Carbon dioxide from planetary atmospheres (Mars, Venus). | N/A* |
| 7 | **Gas Giant Atmosphere** | H2/He/He-3 mix scooped from Jupiter, Saturn, Uranus, Neptune. | N/A* |
| 8 | **Rare Earth Elements** | Neodymium, dysprosium, europium bearing minerals. Required for Electronics manufacturing. | $500 |
| 9 | **Uranium Ore** | Uranium-bearing minerals. Enriched via centrifuge cascade into fission reactor fuel. | $100 |

*Atmospheric resources only exist at colonies with atmospheres. Not purchasable.

### Refined Construction Resources (produced by Factory)

| # | Resource | Description | Earth Price ($/kg) |
|---|----------|-------------|-------------------:|
| 10 | **Structural Metal** | Aerospace-grade aluminum/titanium/steel alloys. Precision machined, heat-treated, inspected. Smelted from Metal Ore (5:1 ratio). | $100 |
| 11 | **High-Temp Alloys** | Inconel, tungsten, ceramic composites. Engine nozzles, heat shields. Single-crystal castings, vacuum-forged. From Metal Ore + Structural Metal. | $1,000 |
| 12 | **Electronics** | Flight-qualified avionics, rad-hardened computers, sensors, solar cells. MIL-spec manufacturing. From Structural Metal + HTA + REE. | $10,000 |
| 13 | **Superconductors** | YBCO tape, REBCO coils, cryocoolers. Magnets for fusion/antimatter/ion engines. Fabricated from Structural Metal + HTA + Electronics. | $50,000 |
| 14 | **Precision Instruments** | Magnetic confinement coils, particle beam optics, cryogenic seals. Sub-micron tolerance manufacturing. Earth-only until Electronics Mfg Tier 12. | $200,000 |

### Fuel Resources

| # | Resource | Earth Price ($/kg) | Earth Available | Notes |
|---|----------|-------------------:|:---:|-------|
| 15 | **RP-1** | $1 | Yes | Kerosene. Refined from hydrocarbons (4:1 ratio). |
| 16 | **Methane** | $2 | Yes | CH4. From hydrocarbons (~1:1 ratio), or Sabatier reaction (CO2 + LH2). |
| 17 | **Liquid Hydrogen (LH2)** | $6 | Yes | From water electrolysis. ~9:1 Water:LH2 (co-produced with LOX). |
| 18 | **LOX** | $0.50 | Yes | Liquid oxygen. From water electrolysis. ~1.1:1 Water:LOX (co-produced with LH2). |
| 19 | **Xenon** | $3,000 | Yes | Rare noble gas. Earth atmospheric extraction only. |
| 20 | **Deuterium** | $20 | Yes | Heavy hydrogen. Extracted from water (~0.016% by mass). |
| 21 | **Tritium** | $30,000 | Yes | Bred from lithium in fission reactors. 12.3-year half-life. Required for Nuclear Pulse Units (boosted fission). |
| 22 | **Enriched Uranium** | $15,000 | Yes | Reactor-grade U-235 enriched from Uranium Ore (100:1 ratio). Consumed by Fission Reactors. |
| 23 | **Nuclear Pulse Units** | $100,000 | Yes | Weapons-grade fissile material. Purchasable on Earth; buildable at colonies with Fission Reactor. |
| 24 | **Helium-3** | N/A | **No** | Lunar regolith (100,000:1 ratio) or gas giant atmosphere (10,000:1 ratio). |
| 25 | **Antimatter** | N/A | **No** | Equal parts antihydrogen + hydrogen by mass. Accelerator production only (hydrogen half from electrolysis). |

### Colony Consumables

| # | Resource | Description |
|---|----------|-------------|
| 26 | **Food** | Consumed at 0.5 kg/crew/day (dehydrated rations). Produced by Greenhouse (water circulated in closed loop). |

**Power** is tracked as a colony stat (kW generated vs. kW consumed), not a stockpiled resource.

---

## 2. Resource Availability by Body

Shows which raw resources can be extracted by Mine or Atmospheric Collector on each body.

### Mineable Resources (via Mine)

| Body | Metal Ore | Regolith | Water | Lithium Ore | Hydrocarbons | Rare Earth Elements | Uranium Ore |
|------|:---------:|:--------:|:-----:|:-----------:|:------------:|:-------------------:|:-----------:|
| Moon | Yes | Yes (He-3) | Yes (poles) | — | — | Yes | Yes (trace) |
| Mercury | Yes (rich) | Yes | — | — | — | Yes | Yes |
| Venus | Yes | — | — | — | — | — | — |
| Mars | Yes | — | Yes | Yes | — | Yes | Yes |
| Phobos | — | Yes | Yes (trace) | — | — | — | — |
| Deimos | — | Yes | Yes (trace) | — | — | — | — |
| Io | Yes | Yes | — | — | — | — | Yes (volcanic) |
| Europa | — | — | Yes (abundant) | — | — | — | — |
| Ganymede | Yes | Yes | Yes | — | — | — | Yes |
| Callisto | — | Yes | Yes | — | — | — | — |
| Titan | — | — | Yes | — | Yes (lakes) | — | — |
| Rhea | — | Yes | Yes | — | — | — | — |
| Iapetus | — | Yes | Yes | — | — | — | — |
| Dione | — | Yes | Yes | — | — | — | — |

### Atmospheric Resources (via Atmospheric Collector)

| Body | Resource Collected | Notes |
|------|-------------------|-------|
| Mars | Atmospheric CO2 | Thin atmosphere, unlimited supply |
| Venus | Atmospheric CO2 | Dense atmosphere, extreme surface conditions |
| Titan | — | N2/CH4 atmosphere, but surface Hydrocarbons are easier to mine |
| Jupiter | Gas Giant Atmosphere | Orbital scooping station. H2/He/He-3 mix. |
| Saturn | Gas Giant Atmosphere | Orbital scooping station. H2/He/He-3 mix. |
| Uranus | Gas Giant Atmosphere | Orbital scooping station. H2/He/He-3 mix. |
| Neptune | Gas Giant Atmosphere | Orbital scooping station. H2/He/He-3 mix. |

### Key Constraints
- **Electronics and Superconductors** cannot be mined. Must be manufactured in a Factory or shipped from Earth.
- **Precision Instruments** are Earth-manufactured only until Electronics Manufacturing Tier 12 unlocks the colony recipe. Required for Fusion Reactors, Particle Accelerators, and interstellar fusion/antimatter ship parts.
- **Rare Earth Elements** are only available in the inner system (Moon, Mars, Mercury). Outer system colonies must import REE or finished Electronics.
- **Xenon** is Earth-purchase only. Must be shipped to colonies via trade routes.
- **Nuclear Pulse Units** purchasable on Earth or manufactured at colonies with a co-located Fission Reactor (see §5).
- **Tritium** must be bred from Lithium Ore in a Factory with a co-located Fission Reactor.
- **Uranium Ore** available on Moon, Mercury, Mars, Io, and Ganymede. Must be enriched into Enriched Uranium for Fission Reactors. Early colonies buy Enriched Uranium from Earth; local enrichment requires Nuclear Engineering Tier 3.
- **He-3** only from lunar regolith extraction (slow) or gas giant atmospheric separation (fast).
- **Antimatter** only from Particle Accelerators (colony building).
- **Mercury has no accessible water.** Water must be shipped from other colonies for crew support and greenhouse operation.

---

## 3. Habitability Scores

0 = utterly uninhabitable, 100 = Earth. The score determines a **total cost multiplier** for Habitats and Greenhouses: `total_cost = base_cost × (200 - score) / 100`. This represents combined construction and the extra shielding, pressure vessels, insulation, and radiation protection needed for human-occupied structures.

Examples: Earth (100) = 1.0× base cost. Mars (30) = 1.7× base cost. Moon (15) = 1.85× base cost. Mercury (8) = 1.92× base cost. Ship/space station (0) = 2.0× base cost.

**Affects Habitats and Greenhouses ONLY** — resource costs, power draw, AND maintenance all scale by this multiplier. Automated industrial infrastructure (Mines, Factories, Solar Farms, Construction Robots, Particle Accelerators, Mass Drivers, Launchpads) uses base costs — these systems don't require the radiation shielding and life support that drives the multiplier.

| Body | Score | Avg Temp (K) | Radiation | Atmosphere | Notes |
|------|------:|-------------:|-----------|------------|-------|
| **Earth** | 100 | 288 | Low (magnetosphere) | 101 kPa breathable | Home base. No colony. |
| **Mars** | 30 | 210 | Moderate (no magnetosphere) | 636 Pa CO2 | Thin atm helps. Cold but manageable. Water ice. |
| **Titan** | 25 | 94 | Low (Saturn magnetosphere + thick atm) | 147 kPa N2/CH4 | No pressure vessels needed! But -179°C. |
| **Moon** | 15 | 220 (100-390) | High (no magnetosphere, no atm) | None | Close to Earth. Vacuum, radiation, temp swings. |
| **Callisto** | 15 | 134 | Moderate (outside Jupiter rad belt) | None | Least harsh Galilean moon. Cold, vacuum. |
| **Phobos** | 12 | 233 | Moderate | None | Accessible from Mars orbit. Vacuum. |
| **Deimos** | 12 | 233 | Moderate | None | Same as Phobos. |
| **Ganymede** | 10 | 110 | High (own magnetosphere helps) | Trace | Largest moon. Still cold and harsh. |
| **Rhea** | 10 | 73 | Low (outer Saturn system) | None | Cold, vacuum. Low radiation. |
| **Dione** | 10 | 87 | Low | None | Similar to Rhea. |
| **Mercury** | 8 | 440 (100-700) | Very High (no magnetosphere, close to Sun) | None | Extreme temp swings. Ideal for solar power. |
| **Iapetus** | 8 | 113 | Low | None | Distant, cold, vacuum. Water ice available. |
| **Europa** | 5 | 102 | Extreme (Jupiter radiation belt) | Trace | Abundant water ice, but radiation is brutal. |
| **Venus** | 3 | 737 | Moderate (thick atm shields radiation) | 9.2 MPa CO2 | 460°C, 92 atm. Nearly impossible surface colony. |
| **Io** | 2 | 130 | Extreme (Jupiter radiation belt) | Trace SO2 | Volcanic. Worst solid surface in solar system. |

**Gas giants** (Jupiter, Saturn, Uranus, Neptune) cannot have surface colonies. They get **orbital scooping stations** — functionally a colony but limited to atmospheric collection and He-3 separation. Establishment cost uses a fixed score of 20.

---

## 4. Colony Buildings

One size per building type. Scale by building multiples. All resource costs shown are **base costs** before the habitability multiplier.

**All output rates are base values (Tier 0)**. Technology upgrades improve each building type independently through 15 tiers — see Technology Upgrades at end of this section.

**Maintenance** = resources consumed per 30 days. If unmet, output degrades proportionally.

### Habitat
Houses crew. Each colony starts with one (from Colony Module).

| Stat | Value |
|------|-------|
| Crew capacity | +20 per habitat |
| Power draw | 10 kW |
| Build cost | 8,000 kg Metal, 1,000 kg Elec |
| Food capacity | 3,000 kg (300 days for full habitat) |
| Maintenance / 30 days | 40 kg Metal, 8 kg Elec |

Crew consume 0.5 kg Food/day each. 20 crew = 10 kg Food/day.

### Basic Greenhouse
Early-tech food production. Hydroponics/aeroponics.

| Stat | Value |
|------|-------|
| Food output | 0.5 kg/day (feeds 1 crew) at 100% water |
| Water capacity | 2,000 kg |
| Power draw | 50 kW |
| Build cost | 5,000 kg Metal, 3,000 kg Elec |
| Maintenance / 30 days | 25 kg Metal, 13 kg Elec |

### Advanced Greenhouse
Genetically modified crops and optimized growing systems. Unlocked later in tech tree.

| Stat | Value |
|------|-------|
| Food output | 2.5 kg/day (feeds 5 crew) at 100% water |
| Water capacity | 5,000 kg |
| Power draw | 50 kW |
| Build cost | 5,000 kg Metal, 3,000 kg Elec |
| Maintenance / 30 days | 25 kg Metal, 13 kg Elec |

Both greenhouse types are affected by the habitability multiplier on resource costs, power draw, and maintenance.

**Water**: Each greenhouse holds water that scales its food output linearly from 0% to 100% (e.g., a Basic Greenhouse with 1,000 kg of its 2,000 kg capacity produces 0.25 kg Food/day at Tier 0). Water is not consumed — it stays in the greenhouse indefinitely. The player can add or withdraw water at any time, trading food production for water availability elsewhere.

### Solar Farms
Solar panel installations. Three sizes. All output scales with distance from Sun: `output × (1 AU / distance)²`.

**Small Solar Farm** — Starter colony power. Enough for a few buildings.

| Stat | Value |
|------|-------|
| Power output | 10 MW @ 1 AU |
| Panel area | 0.037 km² (~190m × 190m) |
| Build cost | 10,000 kg Metal, 5,000 kg Elec |
| Maintenance / 30 days | 13 kg Metal, 5 kg Elec |

**Medium Solar Farm** — Growing colony power. Supports moderate industry.

| Stat | Value |
|------|-------|
| Power output | 100 MW @ 1 AU |
| Panel area | 0.37 km² (~600m × 600m) |
| Build cost | 100,000 kg Metal, 50,000 kg Elec |
| Maintenance / 30 days | 125 kg Metal, 50 kg Elec |

**Large Solar Farm** — Industrial-scale installation.

| Stat | Value |
|------|-------|
| Power output | 1 GW @ 1 AU |
| Panel area | 3.67 km² (~1.9 km × 1.9 km) |
| Build cost | 1,000,000 kg Metal, 500,000 kg Elec |
| Maintenance / 30 days | 1,250 kg Metal, 500 kg Elec |

**Physics basis**: Solar irradiance at 1 AU = 1,361 W/m². Panel efficiency = 40% (high-tech multi-junction photovoltaics; current lab record ~47%). Day/night factor = ×0.5 (average over planetary rotation). Large Solar Farm panel area = 3.67 km². Average output = 1,361 × 0.40 × 0.5 × 3.67×10⁶ = **1 GW at 1 AU** ✓. Smaller farms scale linearly (10× less area = 10× less power).

At Mercury (0.39 AU): Large = **6.6 GW**, Medium = 660 MW, Small = 66 MW. At Jupiter (5.2 AU): Large = 37 MW, Medium = 3.7 MW, Small = 370 kW. At Saturn (9.5 AU): Large = 11 MW, Medium = 1.1 MW, Small = 110 kW.

Large Solar Farms are the primary power source for Mercury antimatter production. A single Mercury Large Solar Farm produces as much power as 13 Fission Reactors.

**Maximum Large Solar Farms per body** (hard cap — cannot exceed surface area / 3.67 km²; smaller farms fit proportionally more):

| Body | Surface Area (km²) | Max Solar Farms | Max Power @ distance |
|------|-------------------:|----------------:|---------------------|
| Mercury | 74,800,000 | 20,380,000 | 134.5 PW (@ 0.39 AU) |
| Venus | 460,200,000 | 125,400,000 | 125.4 PW (@ 0.72 AU) |
| Moon | 37,900,000 | 10,327,000 | 10.3 PW (@ 1 AU) |
| Mars | 144,800,000 | 39,455,000 | 17.1 PW (@ 1.52 AU) |
| Phobos | 1,550 | 422 | 183 GW (@ 1.52 AU) |
| Deimos | 495 | 134 | 58 GW (@ 1.52 AU) |
| Io | 41,900,000 | 11,417,000 | 422 TW (@ 5.2 AU) |
| Europa | 30,100,000 | 8,201,000 | 303 TW (@ 5.2 AU) |
| Ganymede | 87,200,000 | 23,760,000 | 878 TW (@ 5.2 AU) |
| Callisto | 73,000,000 | 19,891,000 | 735 TW (@ 5.2 AU) |
| Titan | 83,300,000 | 22,698,000 | 251 TW (@ 9.5 AU) |
| Rhea | 7,300,000 | 1,989,000 | 22 TW (@ 9.5 AU) |
| Dione | 3,990,000 | 1,087,000 | 12 TW (@ 9.5 AU) |
| Iapetus | 6,700,000 | 1,825,000 | 20 TW (@ 9.5 AU) |

Note: Phobos and Deimos have meaningful limits. All major bodies have limits far beyond any practical need.

### Fission Reactor
Constant power output regardless of location. Essential for outer system colonies.

| Stat | Value |
|------|-------|
| Power output | 500 MW (constant) |
| Fuel consumed | 0.5 kg Enriched Uranium/day |
| Build cost | 200,000 kg Metal, 100,000 kg HTA, 100,000 kg Elec, 50,000 kg Super |
| Maintenance / 30 days | 750 kg Metal, 250 kg HTA |

Consumes Enriched Uranium continuously — without fuel supply, the reactor shuts down. Early colonies purchase Enriched Uranium from Earth ($15,000/kg); local enrichment from Uranium Ore requires Nuclear Engineering Tier 3. Also provides neutron flux required for Tritium Breeding recipe.

### Fusion Reactor
High-output power. Requires He-3 fuel supply.

| Stat | Value |
|------|-------|
| Power output | 5 GW (constant) |
| Fuel consumed | 3 kg He-3/day + 2 kg Deuterium/day |
| Build cost | 500,000 kg Metal, 200,000 kg HTA, 300,000 kg Elec, 400,000 kg Super, 200 kg PI |
| Maintenance / 30 days | 1,250 kg Metal, 500 kg HTA, 250 kg Super, 2 kg PI |

### Mine
Extracts one selected raw resource from the body's available deposits. Assign each mine to a specific resource.

| Stat | Value |
|------|-------|
| Output | 2,000 kg/day of assigned resource |
| Power draw | 100 kW |
| Build cost | 20,000 kg Metal, 5,000 kg HTA, 5,000 kg Elec |
| Maintenance / 30 days | 75 kg Metal, 13 kg HTA |

### Atmospheric Collector
Extracts atmospheric gases on bodies with atmospheres. On gas giants, operates as an orbital aerobraking scoop.

| Stat | Value |
|------|-------|
| Output | 10,000 kg/day of atmospheric resource |
| Power draw | 100 kW |
| Build cost | 15,000 kg Metal, 3,000 kg HTA, 8,000 kg Elec |
| Maintenance / 30 days | 50 kg Metal, 13 kg Elec |

### Factory
General-purpose processing facility. Assign one recipe at a time (see §5). Build multiple factories for different recipes or parallel throughput.

| Stat | Value |
|------|-------|
| Throughput | Processes one recipe batch at a time |
| Power draw | Varies by recipe (see §5) |
| Build cost | 50,000 kg Metal, 10,000 kg HTA, 30,000 kg Elec |
| Maintenance / 30 days | 125 kg Metal, 25 kg HTA, 25 kg Elec |

### Launchpad
Enables rocket launches from the colony. Required to send ships off-world.

| Stat | Value |
|------|-------|
| Capability | Standard rocket launches |
| Power draw | 10 kW (idle) |
| Build cost | 30,000 kg Metal, 5,000 kg HTA, 5,000 kg Elec |
| Maintenance / 30 days | 125 kg Metal, 25 kg HTA |

### Mass Driver (Mk I–IV)
Electromagnetic coilgun launch system. Airless bodies only (Moon, Mercury, Phobos, Deimos, asteroids). Launches uncrewed blueprint ships (1,000g) and Dyson swarm mirror segments (10,000g) using electrical power.

| Stat | Mk I | Mk II | Mk III | Mk IV |
|------|------|-------|--------|-------|
| Track length | 2 km | 10 km | 50 km | 200 km |
| Max payload | 10 t | 50 t | 200 t | 200 t |
| Ship max v | 6.3 km/s | 14 km/s | 31 km/s | 63 km/s |
| Mirror max v | 19.8 km/s | 44 km/s | 99 km/s | 198 km/s |
| Power draw | 10 MW | 100 MW | 1 GW | 100 GW |
| Tech era | 4 | 6 | 8 | 9 |

See `openspec/specs/game/colony/mass_driver/spec.md` for full details.

### Light Construction Robot
A single remotely operated construction drone for small or early-stage colonies. Lower throughput than a full Construction Robot fleet, but much cheaper to build and ship. Ideal for initial colony bootstrapping.

| Stat | Value |
|------|-------|
| Crew required | 1 per robot |
| Assembly rate | 5 tonnes/day (new construction) |
| Maintenance rate | 15 tonnes/day (repair/replacement work) |
| Power draw | 100 kW |
| Build cost | 3,000 kg Metal, 1,500 kg HTA, 5,000 kg Elec |
| Maintenance / 30 days | 15 kg Metal, 8 kg HTA, 15 kg Elec |

Suitable for colonies with 1-5 buildings. A single Light Robot can assemble a Mine (30t) in 6 days or a Habitat (9t) in under 2 days. Upgrade to full Construction Robots as the colony grows.

### Construction Robot
Remotely operated construction drones. Physically assembles colony buildings and megastructure segments from refined materials, and performs ongoing maintenance/repairs. Each unit represents a fleet of robotic constructors managed by one crew member.

| Stat | Value |
|------|-------|
| Crew required | 1 per robot |
| Assembly rate | 20 tonnes/day (new construction) |
| Maintenance rate | 60 tonnes/day (repair/replacement work) |
| Power draw | 500 kW |
| Build cost | 10,000 kg Metal, 5,000 kg HTA, 15,000 kg Elec |
| Maintenance / 30 days | 50 kg Metal, 25 kg HTA, 50 kg Elec |

Required for both constructing and maintaining colony buildings and megastructures. Without Construction Robots, new buildings cannot be assembled and existing buildings cannot be repaired. Assembly time = building total mass / (20 t/day × number of robots assigned) at Tier 0 tech. Maintenance throughput is 3× faster than new construction (replacing components is simpler than building from scratch). A robot can split its time between construction and maintenance.

### Science Lab
Research facility that extracts scientific knowledge from a body over time. Extracts 10× the body's landing discovery value (see §8) on a logarithmic exhaustion curve.

```
extracted(t) = 10 × landing_value × (1 − e^(−t/15))
```

At t=15yr: ~63% extracted. At t=50yr: ~96% extracted (near exhaustion).

Multiple labs on the same body share the exhaustion pool but extract faster: replace `t/15` with `N×t/15` where N = number of labs.

| Stat | Value |
|------|-------|
| Crew required | 5 |
| Power draw | 200 kW |
| Build cost | 30,000 kg Metal, 5,000 kg HTA, 20,000 kg Elec |
| Maintenance / 30 days | 100 kg Metal, 25 kg Elec |

| Body | Landing Value | Lab Total (10×) | After 50yr |
|------|-------------:|----------------:|-----------:|
| Moon | 100 | 1,000 | 964 |
| Mars | 133 | 1,330 | 1,282 |
| Titan | 279 | 2,790 | 2,690 |
| Exoplanet | 500 | 5,000 | 4,820 |

### Stockpile
Bulk material storage facility. Stores any resource type. No power draw and no maintenance — just a warehouse.

| Stat | Value |
|------|-------|
| Storage capacity | 500,000 kg per stockpile |
| Power draw | 0 |
| Build cost | 20,000 kg Metal |
| Maintenance / 30 days | None |

Build multiple stockpiles for more storage. Colonies without sufficient stockpile capacity cannot accept deliveries beyond their current storage — the player must build stockpiles before large shipments arrive. Not affected by the habitability multiplier.

### Food Storage
Refrigerated storage facility for preserved food supplies. Extends a colony's food buffer beyond what Habitats can hold, providing resilience against greenhouse disruptions or supply delays.

| Stat | Value |
|------|-------|
| Food capacity | 10,000 kg |
| Power draw | 5 kW |
| Build cost | 5,000 kg Metal, 500 kg Elec |
| Maintenance / 30 days | 10 kg Metal |

10,000 kg feeds 20 crew for 1,000 days (~2.7 years). Build multiple for larger colonies or longer buffers. Not affected by the habitability multiplier.

### Technology Upgrades

All building output rates above are **base values (Tier 0)**. Each building type has an independent technology line with 15 upgrade tiers. Research improves output by **+11% per tier** (multiplicative), giving approximately **4.8× total improvement** from Tier 0 to Tier 15.

| Tier | Cumulative Multiplier |
|------|----------------------|
| 0 (base) | 1.00× |
| 3 | 1.37× |
| 5 | 1.69× |
| 8 | 2.30× |
| 10 | 2.84× |
| 12 | 3.50× |
| 15 (max) | 4.78× |

**Technology Lines** (15 tiers each, chained after a prerequisite tech tree node — see §10):

| Tech Line | Prereq Node | Affects | Base → Tier 15 | Recipe Unlocks |
|-----------|-------------|---------|----------------|----------------|
| **Mining** | 2.6 Colony Foundations | Mine output rate | 2,000 → ~10,000 kg/day | — (throughput only) |
| **Metallurgy** | 2.6 Colony Foundations | Metal Smelting + Alloy Forging throughput | 400 → ~2,000 SM/day | T5: Alloy Forging |
| **Electronics Manufacturing** | 3.8 Extended Crewed | Electronics + Superconductor fabrication throughput | 8 → ~38 Elec/day | T6: Superconductor Fab |
| **Agriculture** | 2.3 Advanced Crewed | Greenhouse food output | 0.5 → ~2.4 kg/day (Basic), 2.5 → ~12 (Advanced) | — (throughput only) |
| **Chemical Processing** | 2.6 Colony Foundations | Electrolysis, Sabatier, hydrocarbon + fuel refining | All fuel recipes ×4.8 | T3: Sabatier, Kerosene Refining; T8: Deuterium Extraction |
| **Atmospheric Science** | 3.6 Heavy Fission | Atmospheric Collector output rate | 10,000 → ~50,000 kg/day | — (throughput only) |
| **Nuclear Engineering** | 3.6 Heavy Fission | Uranium Enrichment, Tritium Breeding, NPU Assembly throughput | 0.5 → ~2.5 Tritium/day | T3: Uranium Enrichment; T5: NPU Assembly |
| **Isotope Extraction** | 6.3 Fusion Power | He-3 processing (regolith and gas giant) | 0.2 → ~1 He-3/day (regolith), 1 → ~5 (gas giant) | T5: Gas Giant He-3 |
| **Construction** | 2.6 Colony Foundations | Robot assembly/maintenance rates, ship part build speed | 20 → ~100 t/day assembly | T8: Ship Part Mfg |
| **Precision Manufacturing** | Elec Mfg Tier 12 | Factory Precision Instruments recipe throughput | Unlocked at Electronics Mfg Tier 12 | — (single recipe) |
| **Life Support** | 2.6 Colony Foundations | Habitat + Greenhouse maintenance cost reduction | 1.0× → 0.21× maintenance | — (throughput only) |

Each tier costs Science points (see §8 Science system). Cost per tier: `base × tier^1.7`. Tech lines are independent — the player prioritizes based on strategy.

**Effect on late-game calculations**: The Mk IV analysis (§4 Particle Accelerators) and He-3 production figures assume Tier 13-15 technology. Early colonies operate at base rates and scale up over decades.

### Particle Accelerators

Four tiers, unlocked progressively through the tech tree. Each tier is a superconducting ring accelerator; larger rings enable higher particle energies and more efficient pair production.

**Physics basis**: Creating 1 kg of antihydrogen requires minimum `2mc² = 1.8×10¹⁷ J` (pair production creates matter + antimatter; you keep the antimatter). Conversion efficiency = fraction of input electrical energy that becomes antimatter rest mass. Output formula: `kg/day = (Power_W × 86,400 × efficiency) / 1.8×10¹⁷`.

**Fuel composition**: Antimatter annihilation requires equal parts matter and antimatter. Production rates below are **antihydrogen output** — the bottleneck. Each kg of antihydrogen is paired with 1 kg of hydrogen (trivially available from water electrolysis) to make 2 kg of usable Antimatter fuel. The "Antimatter" resource on ships represents the combined fuel stored in a single tank.

#### Mk I — Research Accelerator
First antimatter experiments. 10 km superconducting ring — comparable to a large Earth collider. Proves the concept, produces milligrams for laboratory use. Enough for AM-Cat fuel after months of accumulation.

| Stat | Value |
|------|-------|
| Ring circumference | 10 km |
| Conversion efficiency | 0.1% |
| Power draw | 50 GW |
| Antimatter output | 0.024 g/day (8.76 g/year) |
| Total mass | 5,000 tonnes |
| Build cost | 1,500,000 kg Metal, 500,000 kg HTA, 1,000,000 kg Elec, 2,000,000 kg Super, 500 kg PI |
| Maintenance / 30 days | 12,500 kg Metal, 5,000 kg HTA, 7,500 kg Elec, 12,500 kg Super, 5 kg PI |

Power source: ~8 Mercury Large Solar Farms, or 10 Fusion Reactors, or 100 Fission Reactors.

#### Mk II — Production Accelerator
100 km ring. First practical production unit — 10× more efficient than Mk I. Can fuel AM-Cat missions after ~1 year of accumulation. A major surface installation visible from orbit.

| Stat | Value |
|------|-------|
| Ring circumference | 100 km |
| Conversion efficiency | 1% |
| Power draw | 500 GW |
| Antimatter output | 2.4 g/day (876 g/year) |
| Total mass | 50,000 tonnes |
| Build cost | 15,000,000 kg Metal, 5,000,000 kg HTA, 10,000,000 kg Elec, 20,000,000 kg Super, 5,000 kg PI |
| Maintenance / 30 days | 125,000 kg Metal, 50,000 kg HTA, 75,000 kg Elec, 125,000 kg Super, 50 kg PI |

Power source: ~76 Mercury Large Solar Farms, or 100 Fusion Reactors.

#### Mk III — Industrial Accelerator
1,000 km ring — spans a major arc of a small planet (~6.5% of Mercury's circumference). 10× more efficient than Mk II. The workhorse for serious antimatter stockpiling. Multiple Mk IIIs can be built on the same body.

| Stat | Value |
|------|-------|
| Ring circumference | 1,000 km |
| Conversion efficiency | 10% |
| Power draw | 5 TW |
| Antimatter output | 240 g/day (87.6 kg/year) |
| Total mass | 500,000 tonnes |
| Build cost | 150,000,000 kg Metal, 50,000,000 kg HTA, 100,000,000 kg Elec, 200,000,000 kg Super, 50,000 kg PI |
| Maintenance / 30 days | 1,250,000 kg Metal, 500,000 kg HTA, 750,000 kg Elec, 1,250,000 kg Super, 500 kg PI |

Power source: ~758 Mercury Large Solar Farms.

With 10 Mk IIIs on Mercury: 876 kg/year antihydrogen = 1,752 kg/year Antimatter fuel. After 50 years: ~88 tonnes fuel. Enough for first Antimatter Torch probe missions.

#### Mk IV — Planetary Ring Accelerator
Planet-encircling accelerator ring. The ultimate megastructure. 2.5× more efficient than Mk III, and 10× more power throughput per km (enabled by the massive ring's extreme beam energy). Multiple rings can be built per body — concentric rings at different altitudes or parallel rings at different latitudes. Each ring has the full circumference of the body.

**All stats scale linearly with the planet's circumference.**

| Stat | Value (per km of circumference) |
|------|-------|
| Conversion efficiency | 25% |
| Power draw | 50 GW/km |
| Antimatter output | 6 g/day/km (2.19 kg/year/km) |
| Mass | 500 tonnes/km |
| Build cost per km | 150,000 kg Metal, 50,000 kg HTA, 100,000 kg Elec, 200,000 kg Super, 50 kg PI |
| Maintenance per km / 30 days | 250 kg Metal, 75 kg HTA, 125 kg Elec, 250 kg Super, 0.05 kg PI |

**Mk IV on key bodies:**

| Body | Circumference | Output (kg/day) | Output (t/year) | Power (TW) | Ring Mass (t) |
|------|-------------:|-----------:|------------:|----------:|-------------:|
| **Mercury** | 15,330 km | 92 | 33.6 | 766.5 | 7,665,000 |
| Moon | 10,916 km | 65.5 | 23.9 | 545.8 | 5,458,000 |
| Mars | 21,300 km | 127.8 | 46.6 | 1,065 | 10,650,000 |

Mercury is the optimal location: closest to the Sun means Solar Farms produce 6.6× more power than at 1 AU, making the enormous power requirements feasible. The Moon or Mars would require impractical numbers of Fusion Reactors for power.

#### Mercury Mk IV — Full Industrial Analysis

Mercury circumference: **15,330 km**. Output: **92 kg/day antihydrogen = 33.6 tonnes/year antihydrogen = 67.2 tonnes/year Antimatter fuel**. Over 50 years of operation: **~3,360 tonnes of Antimatter fuel**.

*All production calculations below assume Tier 15 technology (see §4 Technology Upgrades).*

##### Direct Construction Targets

| Component | Count | Metal (t) | HTA (t) | Elec (t) | Super (t) | PI (t) |
|-----------|------:|----------:|--------:|---------:|---------:|-------:|
| Mk IV Ring | 1 (15,330 km) | 2,300,000 | 766,500 | 1,533,000 | 3,066,000 | 766.5 |
| Solar Farms | 116,000 | 116,000,000 | — | 58,000,000 | — | — |
| **Subtotal** | | **118,300,000** | **766,500** | **59,533,000** | **3,066,000** | **766.5** |

**Power**: 766.5 TW. At Mercury (0.39 AU), each Solar Farm produces 6.6 GW → **~116,000 Solar Farms**. Panel area: ~430,000 km² ≈ 0.6% of Mercury's surface — an area the size of California.

##### Refining Chain — Working Backwards

Each refined resource requires inputs from earlier stages. The full dependency cascade:

**Superconductors** (3 kg Metal + 1 kg HTA + 0.5 kg Elec → 1 kg Super):
3,066,000 t Super needs: 9,198,000 t Metal + 3,066,000 t HTA + 1,533,000 t Elec

**Total Electronics** = 59,533,000 + 1,533,000 = **61,066,000 t**

**Electronics** (1.25 kg Metal + 0.25 kg HTA + 0.125 kg REE → 1 kg Elec):
61,066,000 t Elec needs: 76,333,000 t Metal + 15,267,000 t HTA + ~7,633,000 t REE

**Total HTA** = 766,500 + 3,066,000 + 15,267,000 = **19,100,000 t**

**Alloy Forging** (6.67 kg Ore + 1.33 kg Metal → 1 kg HTA):
19,100,000 t HTA needs: 127,377,000 t Ore + 25,423,000 t Metal

**Total Structural Metal** = 118,300,000 + 9,198,000 + 76,333,000 + 25,423,000 = **229,254,000 t**

**Metal Smelting** (5 kg Ore → 1 kg Metal):
229,254,000 t Metal needs: 1,146,270,000 t Ore

**Total Metal Ore** = 1,146,270,000 + 127,377,000 = **1,274,000,000 t** (~1.27 billion tonnes)

##### Full Resource Budget

| Resource | For Construction | For Refining | **Total to Produce** |
|----------|----------------:|--------------:|--------------------:|
| Metal Ore | — | 1,274,000,000 t | **1,274,000,000 t** (mining) |
| Structural Metal | 118,300,000 t | 110,954,000 t | **229,254,000 t** |
| High-Temp Alloys | 766,500 t | 18,333,000 t | **19,100,000 t** |
| Electronics | 59,533,000 t | 1,533,000 t | **61,066,000 t** |
| Superconductors | 3,066,000 t | — | **3,066,000 t** |
| Precision Instruments | 766.5 t | — | **766.5 t** |
| Rare Earth Elements | — | ~7,633,000 t | **~7,633,000 t** (mining) |

##### Factory & Mine Requirements (30-year production phase)

30 years = 10,950 days of continuous operation.

| Recipe | Total Output | Per Factory Per Day | **Factories Needed** |
|--------|------------:|-------------------:|-----------:|
| Metal Smelting | 229,254,000 t | 2 t | **10,500** |
| Alloy Forging | 19,100,000 t | 0.15 t | **11,600** |
| Electronics Mfg | 61,066,000 t | 0.04 t | **139,500** |
| Superconductor Fab | 3,066,000 t | 0.005 t | **56,000** |
| Precision Instruments | 766.5 t | 0.001 t | **73** |
| **Total Factories** | | | **217,673** |

| Mining | Total Ore | Per Mine Per Day | **Mines Needed** |
|--------|----------:|----------------:|-------:|
| Metal Ore | 1,274,000,000 t | 10 t | **11,600** |
| Rare Earth Elements | ~7,633,000 t | 10 t | **~70** |

**Electronics manufacturing is the critical bottleneck** — 139,500 factories (64% of all industry) producing 40 kg/day each. This is what makes the Mk IV a true planetary-scale project. Precision Instruments add only 73 factories (<0.04%) — trivial compared to the Electronics bottleneck.

##### Construction & Assembly

Total mass to physically assemble:

| Component | Mass (t) |
|-----------|----------:|
| Mk IV Ring | 7,665,000 |
| Solar Farms (116,000 × 1,500 t) | 174,000,000 |
| Factories (217,673 × 90 t) | 19,591,000 |
| Mines (11,600 × 30 t) | 348,000 |
| **Total** | **201,597,000** |

At 100 t/day per Construction Robot over 30 years: **~185 Construction Robots** needed for assembly alone.

Additionally, Construction Robots must maintain all operational infrastructure (§4). Maintenance load at full scale:

| Infrastructure | Maintenance mass / 30 days |
|----------------|---------------------------:|
| 116,000 Solar Farms | 203,000 t |
| 217,673 Factories | 38,093 t |
| 11,600 Mines | 1,021 t |
| **Total** | **~242,100 t** |

At 300 t/day maintenance throughput per robot (9,000 t per 30-day cycle): **~27 robots** dedicated to maintenance at full scale.

**Peak (Phase C): ~210 Construction Robots** (~185 construction + ~27 maintenance), requiring **210 crew**. Mercury habitability = 8, so the 11 Habitats needed for this crew cost 1.92× base in resources, power, and maintenance.

**Operational (Phase D): ~27 Construction Robots** (maintenance only), requiring **27 crew**.

##### Infrastructure Self-Cost

The factories and mines themselves consume resources to build:

| Infrastructure | Metal (t) | HTA (t) | Elec (t) |
|----------------|----------:|--------:|---------:|
| 217,673 Factories | 10,884,000 | 2,177,000 | 6,530,000 |
| 11,600 Mines | 232,000 | 58,000 | 58,000 |
| 210 Construction Robots | 2,100 | 1,050 | 3,150 |
| **Total** | **11,114,100** | **2,235,050** | **6,589,150** |

This adds ~10% overhead on top of direct production targets. Handled during the bootstrap phase — early factories build more factories before final production begins.

##### Power Budget

| Consumer | Power |
|----------|------:|
| Mk IV Accelerator | 766.5 TW |
| Factories (217,673) | ~70 GW |
| Mines (11,600) | ~1.2 GW |
| Construction Robots (210) | ~105 MW |
| **Total** | **~766.6 TW** |

Factory and mine power is negligible (<0.01%) compared to the accelerator. The 116,000 Solar Farms are sized for the accelerator — everything else runs on rounding error.

##### Bootstrapping Timeline

Mercury already has substantial industry from Mk I–III accelerators during Phases 7-9 (~5,000 factories, ~1,000 mines, ~8,000 Solar Farms).

**Phase A — Seed Expansion (Years 0-5):**
Divert existing Mercury industry + ship ~5,000 factory kits from Earth/other colonies. Starting Electronics fabs: ~10,000. Nearly all production dedicated to building more factories and mines.

**Phase B — Exponential Growth (Years 5-15):**
Electronics fabs self-replicate. One fab produces 14,000 kg Elec/year; a new fab costs 30,000 kg Elec. With ~70% of output dedicated to expansion:
- Growth rate: ~33%/year
- Doubling time: ~2.1 years
- From 10,000 → 139,500 fabs in ~8 years

Other factory types and mines scale in parallel, gated by Electronics output. Metal smelters and HTA forges are much faster to ramp (lower Elec cost relative to throughput). Solar Farms are built continuously alongside factory expansion.

**Phase C — Full Production (Years 15-45):**
All ~217,700 factories and 11,600 mines operational. 30 years of sustained output produces the full material budget. ~210 Construction Robots (210 crew) assemble the Ring and Solar Farms while maintaining existing infrastructure. Ring segments brought online progressively — partial production begins before full ring completion.

**Phase D — Operational (Year 45+):**
Mk IV Ring online. 92 kg/day antimatter production begins. Industry shifts to maintenance mode — most factories shut down or repurpose. ~27 Construction Robots (27 crew) remain permanently for infrastructure maintenance. Ongoing Solar Farm maintenance: 145,000 t Metal + 58,000 t Elec per month (requires ~2,500 Metal Smelting factories + ~50,000 Electronics fabs permanently).

**Total: ~45 years** from Mk IV program start to operational ring.

##### Scale Check

| Metric | Value |
|--------|-------|
| Total Metal Ore mined | 1.27 billion tonnes |
| Mercury's estimated iron content | ~10¹⁹ tonnes |
| Fraction consumed | ~0.00000001% |
| Solar Farm area | 430,000 km² (0.6% of surface) |
| Factory footprint (est.) | ~2,200 km² (0.003% of surface) |
| Total Mercury surface used | <1% |

Mercury has essentially unlimited raw material. The constraint is **manufacturing throughput**, not resources. The entire Mk IV program consumes a vanishingly small fraction of Mercury's iron-rich crust — the planet could sustain this level of industry for millions of years.

#### Fusion vs Solar — Antimatter Production Viability

Can a Mk III accelerator (5 TW, 240 g AM/day) be powered by fusion instead of solar? This matters for star systems without a Mercury-like inner planet.

##### Solar Approach (Mercury)

758 Solar Farms at 6.6 GW each = 5 TW.

| Resource | Amount |
|----------|-------:|
| Structural Metal | ~908,000 t |
| High-Temp Alloys | ~50,000 t |
| Electronics | ~479,000 t |
| Superconductors | ~200,000 t |

No ongoing fuel cost. Requires rocky planet close to star with high solar flux.

##### Fusion Approach (Gas Giant System)

*Assumes Tier 15 technology throughputs.*

1,000 Fusion Reactors at 5 GW each = 5 TW. Plus fuel infrastructure:

- **600 Atmospheric Collectors** + **600 He-3 Separation Factories** → 3,000 kg He-3/day
- **1 Water Mine** + Deuterium Extraction Factory → 2,000 kg Deuterium/day

| Resource | Amount |
|----------|-------:|
| Structural Metal | ~715,000 t |
| High-Temp Alloys | ~263,000 t |
| Electronics | ~438,000 t |
| Superconductors | ~600,000 t |

Ongoing fuel: 3,000 kg He-3 + 2,000 kg Deuterium per day, forever. Requires gas giant He-3 source + rocky body for construction.

##### Comparison

| Factor | Solar (Mercury) | Fusion (Gas Giant) |
|--------|-----------------|-------------------|
| Metal | ~908,000 t | ~715,000 t |
| HTA | ~50,000 t | ~263,000 t (5×) |
| Electronics | ~479,000 t | ~438,000 t |
| Superconductors | ~200,000 t | ~600,000 t (3×) |
| Fuel cost | None | 3,000 kg He-3 + 2,000 kg D/day |
| Location | Inner rocky planet | Gas giant system |

**Mercury solar is clearly better in our solar system** — less HTA, far fewer Superconductors, and zero ongoing fuel cost.

**Fusion is viable in other star systems** where gas giants exist but no inner rocky planet has high solar output. At Jupiter distance (5.2 AU), Solar Farms produce only 37 MW each — powering 5 TW would need ~135,000 Solar Farms, an absurd number. Fusion wins decisively at outer system distances.

**Bottom line**: Solar is the Mercury-specific optimal. Fusion is the portable option for antimatter production anywhere gas giants provide He-3.

---

## 5. Factory Recipes

All processing is done on the standard **Factory** building. Each Factory runs one recipe at a time, processing batches continuously. Inputs are consumed from colony inventory; outputs are added to colony inventory.

### Metal Processing

**Metal Smelting**
| | |
|-|-|
| Input | 1,000 kg Metal Ore |
| Output | 200 kg Structural Metal |
| Time | 12 hours |
| Power | 150 kW |
| Waste | 800 kg slag (gangue minerals, impurities, silicates — discarded) |

**Alloy Forging**
| | |
|-|-|
| Input | 200 kg Metal Ore + 40 kg Structural Metal |
| Output | 30 kg High-Temp Alloys |
| Time | 24 hours |
| Power | 250 kW |
| Waste | 210 kg (failed castings, machining swarf, impurity rejects — discarded) |
| Requires | Metallurgy Tier 5. Early colonies must import HTA from Earth. |

### Advanced Manufacturing

**Electronics Manufacturing**
| | |
|-|-|
| Input | 10 kg Structural Metal + 2 kg High-Temp Alloys + 1 kg Rare Earth Elements |
| Output | 8 kg Electronics |
| Time | 24 hours |
| Power | 300 kW |
| Waste | 5 kg (etching residue, failed wafers, solder dross — discarded) |

**Superconductor Fabrication**
| | |
|-|-|
| Input | 6 kg Structural Metal + 2 kg High-Temp Alloys + 1 kg Electronics |
| Output | 2 kg Superconductors |
| Time | 48 hours |
| Power | 500 kW |
| Waste | 7 kg (rejected tape, contaminated substrate, test specimens — discarded) |
| Requires | Electronics Manufacturing Tier 6. Must import Superconductors until then. |

**Precision Instruments Manufacturing**
| | |
|-|-|
| Input | 50 kg Electronics + 10 kg Superconductors + 5 kg High-Temp Alloys |
| Output | 1 kg Precision Instruments |
| Time | 120 hours (5 days) |
| Power | 500 kW |
| Waste | 64 kg (rejected sub-micron components, calibration test articles, contaminated optics — discarded) |
| Notes | Requires Electronics Manufacturing Tier 12. Tier 0: 0.2 kg/day/factory. Tier 15: ~0.96 kg/day/factory. |

### Water Processing

**Electrolysis**
| | |
|-|-|
| Input | 200 kg Water |
| Output | 22 kg LH2 + 178 kg LOX |
| Time | 8 hours |
| Power | 50 kW |
| Mass balance | 200 kg in → 200 kg out ✓ |

**Deuterium Extraction**
| | |
|-|-|
| Input | 10,000 kg Water |
| Output | 2 kg Deuterium + 1,098 kg LH2 + 8,900 kg LOX |
| Time | 120 hours (5 days) |
| Power | 100 kW |
| Mass balance | 10,000 kg in → 10,000 kg out ✓ |
| Notes | Deuterium is ~0.016% of water by mass. Byproduct LH2/LOX is usable. Requires Chemical Processing Tier 8. |

### Atmospheric Processing

**Sabatier Reaction** (Mars, Venus — requires Atmospheric CO2)
| | |
|-|-|
| Input | 88 kg Atmospheric CO2 + 16 kg LH2 |
| Output | 32 kg Methane + 72 kg Water |
| Time | 12 hours |
| Power | 75 kW |
| Mass balance | 104 kg in → 104 kg out ✓ (CO2 + 4H2 → CH4 + 2H2O) |
| Notes | Produces Water as byproduct, which can be electrolyzed back to LH2 — partially closed loop. Requires Chemical Processing Tier 3. |

### Hydrocarbon Processing

**Methane Purification** (Titan — requires Hydrocarbons)
| | |
|-|-|
| Input | 200 kg Hydrocarbons |
| Output | 180 kg Methane |
| Time | 4 hours |
| Power | 30 kW |
| Waste | 20 kg ethane/propane impurities (discarded) |
| Notes | Titan's surface lakes are nearly pure methane/ethane. Simple purification. |

**Kerosene Refining** (Titan — requires Hydrocarbons)
| | |
|-|-|
| Input | 400 kg Hydrocarbons |
| Output | 100 kg RP-1 |
| Time | 12 hours |
| Power | 75 kW |
| Waste | 300 kg light hydrocarbon byproducts (discarded) |
| Notes | Complex processing of light hydrocarbons into kerosene-grade fuel. Requires Chemical Processing Tier 3. |

### Nuclear Processing

**Uranium Enrichment**
| | |
|-|-|
| Input | 100 kg Uranium Ore |
| Output | 1 kg Enriched Uranium |
| Time | 48 hours |
| Power | 500 kW |
| Waste | 99 kg depleted uranium tails (discarded) |
| Notes | Centrifuge cascade concentrates U-235 from ~0.7% to reactor-grade ~4%. Requires Nuclear Engineering Tier 3. Must import Enriched Uranium from Earth until then. |

**Tritium Breeding** (requires co-located Fission Reactor for neutron flux)
| | |
|-|-|
| Input | 20 kg Lithium Ore |
| Output | 1 kg Tritium |
| Time | 48 hours |
| Power | 200 kW |
| Waste | 19 kg irradiated lithium residue (discarded) |
| Notes | Fission Reactor must be present on the colony. Lithium Ore mineable on Mars. |

**Nuclear Pulse Unit Assembly** (requires co-located Fission Reactor for fissile material breeding)
| | |
|-|-|
| Input | 100 kg Structural Metal + 40 kg High-Temp Alloys + 0.5 kg Tritium |
| Output | 1 Nuclear Pulse Unit (~50 kg) |
| Time | 240 hours (10 days) |
| Power | 500 kW |
| Waste | ~90 kg machining swarf, shielding offcuts, and test components (discarded) |
| Notes | Boosted fission device. Fission Reactor must be present for fissile material breeding. Tritium bred from Lithium Ore (Mars). Requires Nuclear Engineering Tier 5. Must import NPUs from Earth until then. |

### He-3 Extraction

**Regolith He-3 Extraction** (Moon — requires Regolith)
| | |
|-|-|
| Input | 20,000 kg Regolith |
| Output | 0.2 kg Helium-3 |
| Time | 24 hours |
| Power | 500 kW |
| Waste | ~20,000 kg spent regolith (returned to surface) |
| Notes | Heats regolith to release solar-wind-implanted He-3. Extremely low yield (100,000:1 ratio). Requires 10 Mines to feed one Factory (Mine produces 2,000 kg/day, recipe needs 20,000 kg). |

**Gas Giant He-3 Separation** (Jupiter, Saturn, Uranus, Neptune — requires Gas Giant Atmosphere)
| | |
|-|-|
| Input | 10,000 kg Gas Giant Atmosphere |
| Output | 1 kg Helium-3 |
| Time | 24 hours |
| Power | 200 kW |
| Waste | ~9,999 kg bulk H2/He vented |
| Notes | Cryogenic separation of He-3 from bulk H2/He. 1 Atmospheric Collector feeds 1 Factory exactly (10,000 kg/day). **5× more efficient than lunar extraction per factory.** Requires Isotope Extraction Tier 5. |

### Food Production

Handled by **Greenhouse** buildings directly (not a Factory recipe). Basic Greenhouse: 0.5 kg Food/day at Tier 0 (at full 2,000 kg water). Advanced Greenhouse: 2.5 kg Food/day at Tier 0 (at full 5,000 kg water). Output scales linearly with water fill level and Agriculture tech tier (see §4).

### Ship Part Manufacturing

Any Factory can build ship parts by consuming the part's resource cost (Metal, HTA, Elec, Super as defined in §7) from colony inventory. Production time = 120 hours per 1,000 kg of part mass at Tier 0 (minimum 40 hours for small parts). Improved by Construction technology. Requires Construction Tier 8 — colonies need real construction expertise before building ships locally.

### Recipe Throughput Summary (Tier 0 base rates)

| Recipe | Factory Output per Day | Mines/Collectors Needed |
|--------|----------------------|------------------------|
| Metal Smelting | 400 kg Metal (2 batches) | 1 Mine (on Metal Ore) |
| Alloy Forging | 30 kg HTA | <1 Mine |
| Electronics Mfg | 8 kg Electronics | Needs Metal + HTA + REE supply chain |
| Superconductor Fab | 1 kg Super (0.5 batches) | Needs Metal + HTA + Elec supply chain |
| Precision Instruments | 0.2 kg PI | Needs Elec + Super + HTA; Elec Mfg Tier 12 |
| Electrolysis | 66 kg LH2 + 534 kg LOX (3 batches) | <1 Mine (on Water) |
| Deuterium Extraction | 0.4 kg Deuterium | 1 Mine (on Water) |
| Sabatier Reaction | 64 kg Methane (2 batches) | 1 Atmo Collector (excess) |
| Methane Purification | 1,080 kg Methane (6 batches) | 1 Mine (on Hydrocarbons, excess) |
| Uranium Enrichment | 0.5 kg Enriched Uranium (0.5 batches) | <1 Mine (on Uranium Ore) |
| Tritium Breeding | 0.5 kg Tritium (0.5 batches) | <1 Mine (on Lithium Ore) |
| Pulse Unit Assembly | 0.1 units | Needs Metal + HTA supply chain |
| He-3 Regolith | 0.2 kg He-3 | **10 Mines** (on Regolith) |
| He-3 Gas Giant | 1 kg He-3 | 1 Atmospheric Collector |

All throughputs improve with technology tiers (see §4 Technology Upgrades). At Tier 15: ~4.8× base rates.

---

## 6. Colony Establishment & Management

### Colony Module (Ship Part)

A special cargo part that establishes a colony when landed on a body. Pre-fabricated, deployable surface installation — the minimum viable colony.

| Stat | Value |
|------|-------|
| Total mass | 15,000 kg |
| Category | Cargo |

**Contents when deployed:**

| Component | Metal (kg) | HTA (kg) | Elec (kg) | Mass (kg) | Notes |
|-----------|----------:|--------:|---------:|---------:|-------|
| Habitat (20 crew) | 8,000 | — | 1,000 | 9,000 | Pressurized, shielded, life support |
| Food supply | — | — | — | 3,000 | 300 days for 20 crew (dehydrated) |
| Solar array (100 kW) | 100 | 50 | 350 | 500 | Deployable panels |
| Storage (100,000 kg) | 2,000 | — | — | 2,000 | Unpressurized material storage |
| Deployment hardware | 500 | — | — | 500 | Mechanisms, anchoring, cabling |
| **Total** | **10,600** | **50** | **1,350** | **15,000** | |

**Earth cost:** 10,600 × $100 + 50 × $1,000 + 1,350 × $10,000 + 3,000 × $50 (food) = **$14.8M**

The Colony Module's Habitat is pre-fabricated for the destination and provided at **base cost (1.0×)**, bypassing the habitability multiplier. All subsequent Habitats built on-site pay the full multiplier for that body.

The Colony Module does NOT include a Construction Robot — the first robot must be delivered on a separate mission (pre-assembled Light Construction Robot: 9,500 kg, $51.8M at Earth prices; or full Construction Robot: 30,000 kg, $156M). Until a robot arrives, the colony cannot build anything new. The 300-day food supply provides a buffer for follow-up missions.

Land a ship carrying a Colony Module on any body without an existing colony → colony is established. The module is consumed and becomes the initial infrastructure.

### Colony Lifecycle

**Phase 1: Dependent** — Colony needs everything shipped from Earth. No local manufacturing. Regular supply missions for Food, Metal, Electronics. Money drain.

**Phase 2: Growing** — Local mining and basic refining (Metal Smelting, Electrolysis). Food from Greenhouses. Still needs Electronics (or REE if inner system), Precision Instruments shipped in.

**Phase 3: Self-Sufficient** — Produces Electronics locally (inner system) or imports REE (outer system). Still needs Precision Instruments for fusion/antimatter infrastructure.

**Phase 4: Exporting** — Colony produces surplus resources (He-3, fuel, construction materials) and ships them to other colonies or sells for income.

**Phase 5: Fully Independent** — Precision Instruments self-sufficiency achieved (Electronics Mfg Tier 12+). Colony operates without Earth shipments.

### Management Model

Colony management should be **simple and hands-off** once stabilized:
- Set production priorities (which recipes to run)
- Queue building construction
- Assign Mines to resources
- Monitor supply/demand balance

The player's attention should be on **flying rockets and establishing new routes**, not spreadsheet management. Colonies that have sufficient buildings and supply chains should run themselves indefinitely.

### Maintenance

Colony buildings consume maintenance resources (per §4) every 30 days. **Construction Robots are required to perform repairs** — maintenance resources are consumed, but robots must be available to install them. Without enough robots, buildings degrade even if resources are stockpiled.

Habitat and Greenhouse maintenance costs scale with the habitability multiplier (§3), same as their build costs — harsher environments require more upkeep on life support, shielding, and pressure vessels. All other buildings use base maintenance costs.

If maintenance resources or robot capacity are insufficient:
- Building output drops proportionally to shortfall
- Colony doesn't catastrophically fail — degrades gracefully
- Player has time to send resupply missions or build more robots

---

## 7. Ship Part Resource Costs

All costs in **kg**. Derived from part dry mass using category-specific material breakdowns.

### Material Breakdown by Category

| Category | Metal % | HTA % | Elec % | Super % | PI % |
|----------|--------:|------:|-------:|--------:|-----:|
| Fuel Tanks | 90 | 5 | 5 | 0 | 0 |
| Chemical Engines | 50 | 35 | 15 | 0 | 0 |
| Nuclear Thermal Engines | 40 | 30 | 15 | 15 | 0 |
| Ion/Hall Thrusters | 25 | 15 | 35 | 25 | 0 |
| MPD Thrusters | 25 | 15 | 30 | 30 | 0 |
| Fusion Engines | 25 | 15 | 20 | 38 | 2 |
| Antimatter Engines | 20 | 10 | 15 | 50 | 5 |
| Command Pods | 50 | 10 | 40 | 0 | 0 |
| Probe Cores | 20 | 10 | 70 | 0 | 0 |
| Crew Quarters | 60 | 10 | 30 | 0 | 0 |
| Decouplers | 85 | 10 | 5 | 0 | 0 |
| Fairings | 90 | 5 | 5 | 0 | 0 |
| Nose Cones | 80 | 15 | 5 | 0 | 0 |
| Heat Shields | 25 | 70 | 5 | 0 | 0 |
| Parachutes | 60 | 25 | 15 | 0 | 0 |
| Solar Panels | 20 | 10 | 70 | 0 | 0 |
| Batteries | 30 | 5 | 65 | 0 | 0 |
| RTG | 25 | 40 | 35 | 0 | 0 |
| Small Fission Reactors | 45 | 25 | 20 | 10 | 0 |
| Large Fission Reactors | 40 | 20 | 20 | 20 | 0 |
| Fusion Reactors | 25 | 15 | 18 | 40 | 2 |
| Antimatter Reactors | 15 | 10 | 15 | 55 | 5 |
| Whipple Shields | 95 | 5 | 0 | 0 | 0 |
| FRES Shields | 25 | 10 | 25 | 40 | 0 |
| Geodesic Shields | 15 | 5 | 25 | 55 | 0 |
| RCS Thrusters | 40 | 35 | 25 | 0 | 0 |
| Cargo Containers | 95 | 0 | 5 | 0 | 0 |
| Greenhouses (ship) | 50 | 5 | 45 | 0 | 0 |

Earth cost = `(Metal × $100) + (HTA × $1,000) + (Elec × $10,000) + (Super × $50,000) + (PI × $200,000)`. Aerospace-grade pricing reflects flight-qualified manufacturing, inspection, and certification. Raw resources (ores, water, fuels) remain at commodity prices — the expensive part is the hardware, not the propellant. This heavily incentivizes reusable ship designs.

### Chemical Engines

| Part | Mass (kg) | Metal | HTA | Elec | Earth Cost |
|------|----------:|------:|----:|-----:|-----------:|
| Hummingbird | 25 | 13 | 9 | 4 | $50,300 |
| Gecko | 40 | 20 | 14 | 6 | $76,000 |
| Firefly | 30 | 15 | 11 | 5 | $62,500 |
| Wolf | 500 | 250 | 175 | 75 | $950,000 |
| Falcon | 450 | 225 | 158 | 68 | $860,500 |
| Wren | 150 | 75 | 53 | 23 | $290,500 |
| Owl | 280 | 140 | 98 | 42 | $532,000 |
| Viper | 700 | 350 | 245 | 105 | $1,330,000 |
| Bear | 1,800 | 900 | 630 | 270 | $3,420,000 |
| Eagle | 2,200 | 1,100 | 770 | 330 | $4,180,000 |
| Panther | 2,000 | 1,000 | 700 | 300 | $3,800,000 |
| Crane | 1,400 | 700 | 490 | 210 | $2,660,000 |
| Mammoth | 8,000 | 4,000 | 2,800 | 1,200 | $15,200,000 |
| Whale | 5,500 | 2,750 | 1,925 | 825 | $10,450,000 |
| Bison | 4,200 | 2,100 | 1,470 | 630 | $7,980,000 |
| Titan | 5,000 | 2,500 | 1,750 | 750 | $9,500,000 |

### Nuclear Thermal Engines

| Part | Mass (kg) | Metal | HTA | Elec | Super | Earth Cost |
|------|----------:|------:|----:|-----:|------:|-----------:|
| Salamander | 2,500 | 1,000 | 750 | 375 | 375 | $23,350,000 |
| Basilisk | 7,000 | 2,800 | 2,100 | 1,050 | 1,050 | $65,380,000 |
| Wyvern | 15,000 | 6,000 | 4,500 | 2,250 | 2,250 | $140,100,000 |

### Electric Propulsion

| Part | Mass (kg) | Metal | HTA | Elec | Super | Earth Cost |
|------|----------:|------:|----:|-----:|------:|-----------:|
| Moth (Ion) | 12 | 3 | 2 | 4 | 3 | $192,300 |
| Cicada (Ion) | 25 | 6 | 4 | 9 | 6 | $394,600 |
| Tern (Hall) | 8 | 2 | 1 | 3 | 2 | $131,200 |
| Albatross (Hall) | 120 | 30 | 18 | 42 | 30 | $1,941,000 |
| Mako (MPD) | 300 | 75 | 45 | 90 | 90 | $5,452,500 |
| Orca (MPD) | 1,200 | 300 | 180 | 360 | 360 | $21,810,000 |

### Interstellar Engines (tonnes)

| Part | Mass (t) | Metal | HTA | Elec | Super | PI |
|------|----------:|------:|----:|-----:|------:|---:|
| Orion Pulse | 3,500 | 1,750 | 1,225 | 525 | — | — |
| Daedalus S1 (Fusion) | 2,000 | 500 | 300 | 400 | 760 | 40 |
| Daedalus S2 (Fusion) | 1,000 | 250 | 150 | 200 | 380 | 20 |
| Z-Pinch Probe (Fusion) | 300 | 75 | 45 | 60 | 114 | 6 |
| Z-Pinch Advanced (Fusion) | 1,200 | 300 | 180 | 240 | 456 | 24 |
| AM-Cat Fusion | 600 | 120 | 60 | 90 | 300 | 30 |
| Antimatter Torch | 400 | 80 | 40 | 60 | 200 | 20 |
| Gamma Converter | 900 | 180 | 90 | 135 | 450 | 45 |

Interstellar engine masses are engine-only — radiators and reactors are separate ship parts. Must be built at colonies with Factories.

### Interstellar Reactors (tonnes)

| Part | Mass (t) | Power | Fuel | Metal | HTA | Elec | Super | PI |
|------|----------:|------:|------|------:|----:|-----:|------:|---:|
| Prometheus (Fission) | 125 | 500 MW | — | 50 | 25 | 25 | 25 | — |
| Vulcan (Fission) | 400 | 1.6 GW | — | 160 | 80 | 80 | 80 | — |
| Stellarator (Fusion) | 200 | 10 GW | He-3 + D | 50 | 30 | 36 | 80 | 4 |
| Tokamak (Fusion) | 600 | 30 GW | He-3 + D | 150 | 90 | 108 | 240 | 12 |
| Penning Reactor (AM) | 400 | 800 GW | — | 60 | 40 | 60 | 220 | 20 |
| Ixion Reactor (AM) | 1,250 | 2.5 TW | — | 188 | 125 | 188 | 688 | 63 |

Ship reactor power outputs match their RON file definitions. Reactor masses are reactor-only — radiators are separate ship parts.

### Interstellar Shields (tonnes)

| Part | Mass (t) | Metal | HTA | Elec | Super |
|------|----------:|------:|----:|-----:|------:|
| Whipple S | 150 | 143 | 8 | — | — |
| Whipple M | 600 | 570 | 30 | — | — |
| Whipple L | 2,400 | 2,280 | 120 | — | — |
| FRES S | 150 | 38 | 15 | 38 | 60 |
| FRES M | 375 | 94 | 38 | 94 | 150 |
| FRES L | 1,200 | 300 | 120 | 300 | 480 |
| Geodesic S | 400 | 60 | 20 | 100 | 220 |
| Geodesic M | 1,000 | 150 | 50 | 250 | 550 |
| Geodesic L | 3,000 | 450 | 150 | 750 | 1,650 |

### Command Pods & Probe Cores

| Part | Mass (kg) | Metal | HTA | Elec | Earth Cost |
|------|----------:|------:|----:|-----:|-----------:|
| Small Cmd Pod | 2,000 | 1,000 | 200 | 800 | $8,300,000 |
| Medium Cmd Pod | 4,000 | 2,000 | 400 | 1,600 | $16,600,000 |
| Small Inline Ctrl | 1,000 | 500 | 100 | 400 | $4,150,000 |
| Medium Inline Ctrl | 2,500 | 1,250 | 250 | 1,000 | $10,375,000 |
| Large Inline Ctrl | 5,000 | 2,500 | 500 | 2,000 | $20,750,000 |
| XL Inline Ctrl | 12,000 | 6,000 | 1,200 | 4,800 | $49,800,000 |
| Tiny Probe | 20 | 4 | 2 | 14 | $142,400 |
| Small Probe | 50 | 10 | 5 | 35 | $356,000 |
| Medium Probe | 100 | 20 | 10 | 70 | $712,000 |
| Large Probe | 200 | 40 | 20 | 140 | $1,424,000 |

### Crew Quarters

| Part | Mass (kg) | Metal | HTA | Elec | Earth Cost |
|------|----------:|------:|----:|-----:|-----------:|
| Small Quarters | 1,500 | 900 | 150 | 450 | $4,740,000 |
| Medium Quarters | 4,000 | 2,400 | 400 | 1,200 | $12,640,000 |
| Large Quarters | 10,000 | 6,000 | 1,000 | 3,000 | $31,600,000 |

### Fuel Tanks (all 90/5/5 — Metal/HTA/Elec)

| Part | Mass (kg) | Metal | HTA | Elec | Earth Cost |
|------|----------:|------:|----:|-----:|-----------:|
| Tank 1×1 | 35 | 32 | 2 | 2 | $25,200 |
| Tank 1×2 | 70 | 63 | 4 | 4 | $50,300 |
| Tank 1×4 | 140 | 126 | 7 | 7 | $89,600 |
| Tank 1×8 | 280 | 252 | 14 | 14 | $179,200 |
| Tank 3×1 | 105 | 95 | 5 | 5 | $64,500 |
| Tank 3×2 | 210 | 189 | 11 | 11 | $139,900 |
| Tank 3×4 | 420 | 378 | 21 | 21 | $268,800 |
| Tank 3×8 | 840 | 756 | 42 | 42 | $537,600 |
| Tank 5×1 | 175 | 158 | 9 | 9 | $114,800 |
| Tank 5×2 | 350 | 315 | 18 | 18 | $229,500 |
| Tank 5×4 | 700 | 630 | 35 | 35 | $448,000 |
| Tank 5×8 | 1,400 | 1,260 | 70 | 70 | $896,000 |
| Tank 5×16 | 2,800 | 2,520 | 140 | 140 | $1,792,000 |
| Tank 9×1 | 315 | 284 | 16 | 16 | $204,400 |
| Tank 9×2 | 630 | 567 | 32 | 32 | $408,700 |
| Tank 9×4 | 1,260 | 1,134 | 63 | 63 | $806,400 |
| Tank 9×8 | 2,520 | 2,268 | 126 | 126 | $1,612,800 |
| Tank 9×16 | 5,040 | 4,536 | 252 | 252 | $3,225,600 |
| Tank 13×1 | 455 | 410 | 23 | 23 | $294,000 |
| Tank 13×2 | 910 | 819 | 46 | 46 | $587,900 |
| Tank 13×4 | 1,820 | 1,638 | 91 | 91 | $1,164,800 |
| Tank 13×8 | 3,640 | 3,276 | 182 | 182 | $2,329,600 |
| Tank 13×16 | 7,280 | 6,552 | 364 | 364 | $4,659,200 |

Hydrogen tanks, Xenon tanks: same 90/5/5 breakdown. See previous version for full listing.

### Structural, Aerodynamic, Electrical, RCS, Cargo

All use the category formulas above applied to part dry mass. See material breakdown table for percentages.

---

## 8. Economy

### Company

**Starting cash**: $25,000,000

On Earth, all resources are purchased with money at listed $/kg rates. On colonies, actual resources are consumed from inventory.

**Costs**: Ship construction (resource costs × Earth prices from §7), R&D budget (see Science below), colony supply missions.

#### Contracts

| Contract | Payout | Repeatable | Notes |
|----------|-------:|:---:|-------|
| Suborbital Payload | $500K | Yes | Reach 100 km altitude with payload |
| Orbital Payload | $2M | Yes | Stable Earth orbit with payload |
| Lunar Orbit Payload | $4M | Yes | Stable lunar orbit with payload |
| Lunar Surface Delivery | $6M | Yes | Land payload on Moon |
| Mars Orbit Payload | $8M | Yes | Reach Mars orbit with payload |
| Mars Surface Delivery | $12M | Yes | Land payload on Mars |
| Suborbital Tourism | $1M/tourist | Yes | 100 km, return safely (crew capacity + parachute) |
| Orbital Tourism | $3M/tourist | Yes | Complete orbit, return safely (+ heat shield) |
| Lunar Tourism | $10M/tourist | Yes | Orbit Moon, return safely |
| Material Return | Market price | Yes | Ship colony resources to Earth (sells at §1 $/kg) |

**Early game flow**: Default parts build $200–500K probe rockets. Suborbital payloads net ~$300K profit and the suborbital milestone (25 sci) provides early research momentum. With $5M/yr R&D (~45 sci/yr) plus milestones, orbit is reachable in ~2 years (140 sci for nodes 1.1–1.3). Orbital and geostationary milestones then inject 75 more science immediately. Tourism requires crewed spaceflight (Node 1.6). Late-game income shifts to material returns from colonies.

### Science

Science points unlock tech tree nodes (§10) and efficiency upgrade tiers (§4). Three sources:

#### Discovery Science — One-Time Rewards

**Earth Milestones** — awarded once each during early spaceflight:

| Milestone | Science | Trigger |
|-----------|--------:|--------|
| First suborbital flight | 25 | Reach 100 km altitude |
| First Earth orbit | 50 | Complete stable orbit |
| Geostationary altitude | 25 | Reach 35,786 km altitude |

**Total Earth milestones: 100 science**

**Per-Body Discoveries** — awarded once per body per achievement (orbit, landing). Scaled by distance from Earth.

```
Orbit reward  = 50 + 30 × ln(1 + distance_AU)
Landing reward = 100 + 80 × ln(1 + distance_AU)
```

| Body | Dist (AU) | Orbit | Landing | Total |
|------|----------:|------:|--------:|------:|
| Moon | 0.003 | 50 | 100 | 150 |
| Venus | 0.28 | 57 | 120 | 177 |
| Mars | 0.52 | 63 | 133 | 196 |
| Phobos | 0.52 | 63 | 133 | 196 |
| Deimos | 0.52 | 63 | 133 | 196 |
| Mercury | 0.61 | 64 | 136 | 200 |
| Jupiter | 4.2 | 99 | — | 99 |
| Io | 4.2 | 99 | 231 | 330 |
| Europa | 4.2 | 99 | 231 | 330 |
| Ganymede | 4.2 | 99 | 231 | 330 |
| Callisto | 4.2 | 99 | 231 | 330 |
| Saturn | 8.5 | 117 | — | 117 |
| Titan | 8.5 | 117 | 279 | 396 |
| Rhea | 8.5 | 117 | 279 | 396 |
| Iapetus | 8.5 | 117 | 279 | 396 |
| Dione | 8.5 | 117 | 279 | 396 |
| Uranus | 18.2 | 139 | — | 139 |
| Neptune | 29.1 | 152 | — | 152 |

Gas giants: orbit only (no landing). Exoplanets: flat **500 orbit + 500 landing = 1,000/body**.

**Total solar system discovery: ~4,630 science** (100 Earth milestones + 4,530 per-body)
**3 exoplanet systems × ~8 bodies: ~24,000 science**

#### R&D — Money to Science

R&D draws from a global exhaustion pool of **50,000 science**. Your annual budget determines the base rate, but effective output diminishes as cumulative R&D science increases — early research yields breakthroughs, late research hits diminishing returns.

```
base_rate = 25 × ln(1 + budget_dollars / 1,000,000)
effective_rate = base_rate × (1 - cumulative_earned / 50,000)
```

Set in $1M/year increments. Flows evenly over time.

| Annual Budget | Base Rate | @ 0 earned | @ 10,000 earned | @ 22,000 earned |
|--------------:|----------:|-----------:|----------------:|----------------:|
| $1M | 17/yr | 17/yr | 14/yr | 9/yr |
| $10M | 60/yr | 60/yr | 48/yr | 34/yr |
| $100M | 115/yr | 115/yr | 92/yr | 64/yr |
| $500M | 156/yr | 156/yr | 125/yr | 87/yr |
| $1B | 173/yr | 173/yr | 138/yr | 97/yr |

| Milestone | Cumulative R&D Science | Effective Rate (@ $1B) |
|-----------|----------------------:|----------------------:|
| Year ~50 | ~7,000 | 86% of base |
| Year ~100 | ~14,000 | 72% of base |
| Year ~200 | ~22,000 | 56% of base |
| Year ~500 | ~38,000 | 24% of base |
| Theoretical max | 50,000 | 0% (exhausted) |

Realistic trajectory over 200 years (ramping from $1M to $1B): **~22,000 R&D science**. Over the full 600-year game: **~42,000**.

#### Lab Building — Colony Science Extraction

Science Labs (§4) extract 10× the body's landing discovery value over time on a logarithmic exhaustion curve. See §4 Science Lab for building stats and extraction formula.

**Solar system labs (14 bodies, 50yr): ~27,000 science**
**Exoplanet labs (3 systems × 8 bodies, 50yr): ~115,000 science**

#### Total Science Budget

| Source | @ 200yr | Full Game (~600yr) |
|--------|--------:|---------:|
| Solar system discovery | 4,630 | 4,630 |
| Exoplanet discovery (3 systems) | 24,000 | 24,000 |
| R&D (50,000 cap) | 22,000 | ~42,000 |
| Solar system labs (14 bodies) | 27,000 | ~28,000 |
| Exoplanet labs (24 bodies) | 115,000 | ~120,000 |
| **Total** | **~192,600** | **~218,600** |

Tech tree total cost: **~192,990**. Completable with full colonization + sustained R&D. The R&D exhaustion pool provides ~25,000 science of headroom over the full game — the player doesn't need perfect optimization of every source (see §10).

### Trade Routes & Logistics

Every colony and station is a node. Routes connect nodes. Transfers are computed (not simulated) using Lambert solvers for realistic delta-v and flight time. Ships use normal blueprints with computed cargo capacity.

**What the system computes (realistic):**
- Delta-v requirements between bodies (Hohmann minimum + Lambert for faster transfers)
- Flight time (from Lambert solution — more delta-v = shorter time)
- Fuel consumption (Tsiolkovsky equation from ship blueprint)
- Launch windows (synodic periods — delta-v cost varies with orbital alignment)
- Cargo capacity (max payload mass that still allows enough delta-v for the route)

**What the system abstracts away:**
- No orbit simulation for trade ships — just timers and resource transfers
- No manual piloting, staging, maneuver nodes, or time warp
- No rendered ships in transit (just map icons with progress bars)

#### Route Network

Any colony or station with a Launchpad (§4) can serve as a route endpoint. Routes connect any two nodes — no manual flight prerequisite. The player creates routes directly from the logistics panel by selecting source and destination.

#### Transfer Mechanics

Delta-v and flight time are computed from orbital mechanics:

- **Hohmann transfer** = minimum delta-v, longest flight time. Used as the baseline.
- **Lambert solver** = spend more delta-v for a faster transfer. The player controls the tradeoff via a speed slider (economical ↔ express).
- **Launch windows**: Delta-v cost varies with orbital alignment. The system computes the next optimal window (lowest delta-v) and shows how delta-v increases for off-window launches. Interplanetary windows repeat on synodic periods (e.g., Earth–Mars ≈ 780 days).
- **Surface launch/landing delta-v**: Includes gravity losses and aerobraking factors on top of orbital delta-v.
  - **Gravity loss factors** (applied to launch delta-v): Airless bodies 1.1×, thin atmosphere (Mars) 1.15×, thick atmosphere (Earth, Venus, Titan) 1.3×.
  - **Landing delta-v**: Airless bodies = 1.0× orbital velocity (full propulsive braking). Atmospheric bodies = 0× (free landing via aerobraking) — but the ship blueprint **must include a parachute part**. The system validates parachute presence and rejects the route if the ship lacks one for atmospheric landings.
  - **Atmosphere threshold**: Bodies with trace atmospheres (Io, Europa, Ganymede) are treated as **airless** for launch and landing purposes — trace SO2 or tenuous exospheres provide no meaningful aerobraking. Only bodies with significant surface pressure (Earth, Mars, Venus, Titan) count as atmospheric.

#### Ship Selection

Ships for trade routes are selected from saved blueprints (designed in the normal editor):

- **Delta-v calculation**: Same Tsiolkovsky equation used in the editor stats display (`Δv = Isp × g₀ × ln(wet/dry)`). Thrust-weighted average Isp of all engines.
- **Cargo capacity**: Maximum payload mass where the ship's total delta-v still covers the route's delta-v requirement. Computed automatically — the player sees a "max cargo" number.
- **Crew capacity**: From command pods and crew quarters in the blueprint.
- **Crewed or uncrewed**: Probe core = uncrewed cargo ship (no crew needed). Command pod = crewed (must assign crew from source colony).
- **Ship construction cost**: From Earth = money (resource costs × Earth $/kg prices from §7). From a colony = actual resources consumed from colony inventory (Metal, HTA, Elec, Super per §7 material breakdowns).

#### Route Creation Flow

1. **Pick endpoints**: Select source colony and destination colony from the logistics panel.
2. **Pick ship**: Choose a saved blueprint. System displays: total ship delta-v, required route delta-v, max cargo capacity, crew capacity, and a go/no-go indicator.
3. **Choose transfer speed**: Slider from economical (Hohmann minimum) to express (higher delta-v, shorter flight time). Shows the delta-v vs. flight time tradeoff in real-time.
4. **Add intermediate stops** (optional): Insert refueling waypoints to enable longer routes (see Multi-Hop Routes below).
5. **Set cargo manifest**: Select which resources to load and how much (up to max cargo capacity).
6. **Set crew** (if crewed ship): Assign crew members from source colony's population.
7. **Review cost summary**: Total cost in money (Earth) or resources (colony). Includes ship construction (if building new) and fuel.
8. **Launch timing**: Launch immediately, or schedule for optimal window (system shows next window date and delta-v savings).

#### Multi-Hop Routes

Refueling at intermediate colonies enables longer journeys that a single ship couldn't complete in one leg:

- A route is a sequence of **legs**, each with its own delta-v requirement and flight time.
- At each stop, the ship refuels from the colony's fuel stockpile. The correct propellant type (Kerolox/Methalox/Hydrolox per §1) must be available.
- Delta-v is validated **per-leg** — the ship only needs enough delta-v for the current leg after each refueling.
- Example: **Earth → Moon (refuel) → Mars** — the ship needs Earth-to-Moon delta-v for the first leg, then after refueling at the Moon colony, only Moon-to-Mars delta-v for the second leg. A ship that couldn't reach Mars directly from Earth can make the trip with a lunar refueling stop.

#### Fleet Management

Ships are persistent objects that exist at specific locations:

- A ship that arrives at a colony is **stationed** there and can fly again (reusable).
- Multiple ships can operate simultaneously on different routes.
- Ships in transit are shown as map icons with progress bars (not rendered or simulated).
- **Fleet panel** shows all ships: current location (or in-transit with ETA), state (stationed / in transit / under construction), assigned route, cargo contents, and crew.

#### Automation

Two launch trigger modes, selectable per route:

**Window-based** (interplanetary routes): The system monitors orbital alignment and launches automatically when conditions are met. Two options:
- Launch at every transfer window (synodic period)
- Launch whenever Lambert delta-v drops below a player-set threshold (e.g., "launch whenever Δv < 4,000 m/s")

**Frequency-based** (same-body or surface-to-orbit routes): Player sets a launch frequency (e.g., every 30 days). System auto-launches on schedule.

Both modes validate before each launch:
- Sufficient fuel at source colony
- Cargo resources available (or minimum stockpile threshold met)
- Ship is stationed and ready at source

If validation fails, the route pauses with a notification explaining the shortfall.

**Configurable per route:**
- Cargo manifest per direction (different cargo outbound vs. return)
- Minimum stockpile thresholds (don't ship resources below a reserve level)
- Priority (when multiple routes compete for the same ship or resources, higher priority launches first)

#### Route Summary Display

Each route shows a summary like this:

```
Earth → Moon → Mars
  Ship: "Cargo Hauler Mk3" (probe core, 8,200 m/s Δv)
  Cargo: 15,000 kg Electronics
  Leg 1: Earth → Moon    | 5,100 m/s | 3 days
  Leg 2: Moon → Mars     | 3,100 m/s | 186 days  [refuel at Moon]
  Total: 189 days | Next window: Year 12, Day 94
  Status: ● Waiting for window
```

#### Key Trade Flows

Reference showing the primary import/export relationships that drive trade routes:

- **Earth → colonies**: Precision Instruments, Xenon
- **Inner system → outer system**: Rare Earth Elements, finished Electronics
- **Mars → NPU-producing colonies**: Lithium / Tritium
- **Titan → everywhere**: Hydrocarbons, Methane, RP-1
- **Gas giants → inner system**: He-3
- **Everyone → Mercury**: Water
- **Mercury (late) → everywhere**: Antimatter

---

## 9. Progression Timeline

### Phase 1: Chemical Era (Year 0-8)
Chemical rockets (Kerolox/Methalox/Hydrolox). Earth contracts fund operations. First orbit, Moon flyby, Moon landing. Build basic rockets for $500k–$2M.

### Phase 2: Lunar Colony (Year 5-15)
Establish Moon colony (land Colony Module, ~10t + habitability overhead). Begin mining Metal Ore and Water. Ship Electronics from Earth. Nuclear thermal engines unlocked — efficient transfers to Moon and Mars.

### Phase 3: Inner System (Year 10-35)
Mars colony established. Electric propulsion unlocked (ion/Hall/MPD). Launch first electric probes toward Jupiter (~5-year transit) and Saturn (~8 years). Automate Earth→Moon and Earth→Mars supply routes. Begin He-3 mining from lunar regolith.

### Phase 4: Outer System Exploration (Year 25-65)
Electric probes arrive at Jupiter and Saturn systems. MPD missions to Uranus (~15-year transit) and Neptune (~20 years). Establish Callisto colony (moderate habitability, outside Jupiter's radiation belt). Long electric transit times — the player develops inner system infrastructure while probes coast. Research fusion propulsion — a decades-long effort.

### Phase 5: Fusion Era (Year 60-120)
Fusion engines unlocked (~Year 60, reflecting realistic fusion propulsion timeline). Gas giant He-3 scooping stations (1 kg/day at Tier 0, up to ~5 kg/day at advanced tech — dramatically faster than lunar extraction). Abundant fusion fuel enables:
- **Z-Pinch Probe**: First interstellar probes. 300t engine, built at outer system colony. Miniprobe swarms at ~0.03c. Alpha Centauri flyby in ~145 years. Data return begins the exploration of other star systems.
- Fast interplanetary travel. Jupiter in weeks, not months.

### Phase 6: Fusion Interstellar (Year 100-250)
The main interstellar era — the longest phase of the game. Fusion ships colonize the nearest stars over 100+ years before antimatter technology matures.
- **Daedalus-class flyby**: 2-stage vessel (S1: 2,000t, Ve=10,600 km/s + S2: 1,000t, Ve=9,210 km/s per BIS study) plus radiators, reactors, shields, and payload. Built at outer system colony near gas giant He-3 supply. ~0.12c flyby — Alpha Centauri in ~36 years. Requires 100+ gas giant scooping stations running for years.
- **Z-Pinch Advanced colony ship**: 1,200t engine (Ve=20,000 km/s). With braking capability, cruises at ~0.1c. Alpha Centauri in ~50 years. First stellar colony established via fusion.
- Multiple star systems colonized with fusion before antimatter becomes practical.
- Interstellar colonies managed across multi-decade communication delays.
- **Precision Instruments challenge**: Interstellar colonies have no Earth supply route at multi-light-year distances. Colony ships must carry initial PI stockpile and prioritize bootstrapping to Electronics Mfg Tier 12 for local PI production.

### Phase 7: Antimatter Research (Year 150-200)
Mk I Particle Accelerator (10 km ring) at Mercury, powered by ~8 Large Solar Farms. Produces 0.024 g/day antihydrogen — laboratory experiments and first AM-Cat fuel tests after months of accumulation. Begin Mercury solar infrastructure buildup.

### Phase 8: Antimatter Production (Year 180-280)
Mk II (100 km ring, ~76 Large Solar Farms) online. Producing 2.4 g/day = 876 g/year antihydrogen. AM-Cat engines (600t, Ve=30,000 km/s) unlocked — upgrade to existing fusion routes, ~0.15c with braking, using only 0.24% antimatter by fuel mass. First Mk III (1,000 km ring, ~758 Large Solar Farms) under construction.

### Phase 9: Industrial Mercury (Year 250-400)
Multiple Mk III accelerators operational, each producing 87.6 kg/year antihydrogen. With 10 Mk IIIs: ~876 kg/year antihydrogen = ~1,750 kg/year Antimatter fuel. Antimatter Torch (400t, Ve=150,000 km/s) unlocked ~Year 300. First Torch probe missions — 0.2c+ flyby, Alpha Centauri in ~22 years. Full Torch colony ships require decades of further stockpiling. Begin Mk IV planetary ring construction (~50-year build).

### Phase 10: Planetary Ring (Year 350-600+)
Mk IV accelerator ring encircles Mercury (15,330 km circumference). Powered by ~116,000 Large Solar Farms covering ~430,000 km² of Mercury's surface. Produces **92 kg/day antihydrogen = 67.2 tonnes/year Antimatter fuel**. Ring operational ~Year 400.

**Gamma Converter missions**: 900t engine, Ve=255,000 km/s (0.85c). First missions ~Year 420+ after decades of Mk IV production. Vessels at 0.3–0.5c for multi-star exploration and colonization. The true endgame.

---

## 10. Tech Tree

Tech tree nodes unlock parts, buildings, and capabilities. Each node costs Science points (see §8). Nodes have prerequisite nodes that must be researched first. For efficiency upgrade lines, see §4 Technology Upgrades.

### Default Unlocked Parts

Available from game start (no research required):

Gecko, Tank 1×1, Tank 1×2, Tank 1×4, Tank 1×8, Tiny Probe Core, NC-1 Nose Cone, AE-FF0 Fairing, TD-1 Decoupler

### ERA 1: EARLY CHEMICAL (25–200 science)

**1.1 Basic Rocketry** — 25 sci — Prereqs: none
- Hummingbird, Firefly, NC-1R, NC-1L, Small Parachute, HS-1, Battery Z-1

**1.2 Structural Engineering** — 40 sci — Prereqs: [1.1]
- TD-12, TT-38K Radial, AE-FF1, NC-3, NC-3R, NC-3L

**1.3 Small Kerolox** — 75 sci — Prereqs: [1.2]
- Wolf, Viper, Tank 3×1, Tank 3×2, Tank 3×4, Tank 3×8, HS-3, Small Probe Core

**1.4 Small Methalox** — 100 sci — Prereqs: [1.2]
- Falcon

**1.5 Small Hydrolox** — 150 sci — Prereqs: [1.2]
- Wren, Owl

**1.6 Crewed Spaceflight** — 200 sci — Prereqs: [1.3]
- Small Cmd Pod, Small Inline Ctrl, Large Parachute, RV-1 RCS, RV-1 RCS Mirror, SP-3, Tiny Cargo

**Era 1 total: 590 science**

### ERA 2: ADVANCED CHEMICAL (250–500 science)

**2.1 Medium Launch Vehicles** — 250 sci — Prereqs: [1.3, 1.4]
- Panther, Bear, Tank 5×1, Tank 5×2, Tank 5×4, Tank 5×8, Tank 5×16, TD-25, AE-FF2, NC-5, NC-5R, NC-5L, HS-5, Medium Probe Core

**2.2 Medium Hydrolox** — 300 sci — Prereqs: [1.5, 2.1]
- Eagle, Crane

**2.3 Advanced Crewed Systems** — 350 sci — Prereqs: [1.6, 2.1]
- Medium Cmd Pod, Medium Inline Ctrl, Small Quarters, RV-3 RCS, RV-3 RCS Mirror, SP-6, Battery Z-3, Small Cargo, Small Greenhouse (ship)
- Buildings: Basic Greenhouse

**2.4 Heavy Lift** — 400 sci — Prereqs: [2.1]
- Mammoth, Titan, Tank 9×1, Tank 9×2, Tank 9×4, Tank 9×8, Tank 9×16, TD-50, AE-FF3, NC-9, NC-9R, NC-9L, HS-9

**2.5 Large Methalox & Hydrolox** — 500 sci — Prereqs: [2.2, 2.4]
- Whale, Bison

**2.6 Colony Foundations** — 500 sci — Prereqs: [2.3]
- Colony Module (ship part)
- Buildings: Habitat, Mine, Factory, Small Solar Farm, Stockpile, Light Construction Robot, Construction Robot, Food Storage, Launchpad

**Era 2 total: 2,300 science**

### ERA 3: NUCLEAR & ELECTRIC (600–1,500 science)

**3.1 Nuclear Thermal Propulsion** — 600 sci — Prereqs: [2.5]
- Salamander, LH2-T, LH2-S

**3.2 Advanced NTR** — 900 sci — Prereqs: [3.1]
- Basilisk, Wyvern, LH2-M, LH2-L

**3.3 Ion Propulsion** — 800 sci — Prereqs: [2.3]
- Moth, Tern, Xe-1, PB-NUK RTG

**3.4 Advanced Electric Propulsion** — 1,200 sci — Prereqs: [3.3, 3.5]
- Cicada, Albatross, Xe-3

**3.5 Compact Fission Reactors** — 1,000 sci — Prereqs: [3.1]
- Ember, Hearth

**3.6 Heavy Fission** — 1,500 sci — Prereqs: [3.5]
- Crucible, Xe-5, Xe-9
- Buildings: Fission Reactor, Atmospheric Collector

**3.7 Super-Heavy Launch** — 1,200 sci — Prereqs: [2.4]
- Tank 13×1, Tank 13×2, Tank 13×4, Tank 13×8, Tank 13×16, TD-100, AE-FF4

**3.8 Extended Crewed Missions** — 1,000 sci — Prereqs: [2.6]
- Large Inline Ctrl, Medium Quarters, SP-12, Battery Z-5, Medium Cargo, Medium Greenhouse (ship), Large Probe Core
- Buildings: Medium Solar Farm

**Era 3 total: 8,200 science**

### ERA 4: INTERPLANETARY (1,500–2,500 science)

**4.1 MPD Propulsion** — 2,000 sci — Prereqs: [3.4, 3.6]
- Mako, Orca

**4.2 Deep Space Habitation** — 2,500 sci — Prereqs: [3.8]
- XL Inline Ctrl, Large Quarters, SP-24, Battery Z-9, Battery Z-13, Large Cargo, Large Greenhouse (ship)
- Buildings: Advanced Greenhouse, Mass Driver Mk I, Large Solar Farm

**4.3 Science Laboratory** — 1,500 sci — Prereqs: [2.6]
- Buildings: Science Lab

**Era 4 total: 6,000 science**

### ERA 5: NUCLEAR PULSE (3,500–5,000 science)

**5.1 Nuclear Pulse Propulsion** — 4,000 sci — Prereqs: [3.2, 3.6]
- Orion Pulse Drive

**5.2 Interstellar Fission Reactors** — 5,000 sci — Prereqs: [3.6, 5.1]
- Prometheus, Vulcan

**5.3 Passive Shielding** — 3,500 sci — Prereqs: [5.1]
- Whipple Shield S, Whipple Shield M, Whipple Shield L

**Era 5 total: 12,500 science**

### ERA 6: FUSION (4,000–7,000 science)

**6.1 Fusion Propulsion — Probe** — 4,000 sci — Prereqs: [5.2]
- Z-Pinch Probe, Daedalus S2, Fusion Sphere S

**6.2 Fusion Propulsion — Full Scale** — 6,000 sci — Prereqs: [6.1]
- Z-Pinch Advanced, Daedalus S1, Fusion Sphere M, Fusion Sphere L

**6.3 Fusion Power** — 5,000 sci — Prereqs: [6.1]
- Stellarator
- Buildings: Fusion Reactor

**6.4 Advanced Fusion Power** — 7,000 sci — Prereqs: [6.3, 6.2]
- Tokamak

**6.5 Active Shielding** — 4,000 sci — Prereqs: [5.3, 5.2]
- FRES Shield S, FRES Shield M, FRES Shield L

**Era 6 total: 26,000 science**

### ERA 7: ANTIMATTER RESEARCH (5,000–8,000 science)

**7.1 Antimatter-Catalyzed Fusion** — 8,000 sci — Prereqs: [6.2, 6.3]
- AM-Cat Fusion engine
- Buildings: Particle Accelerator Mk I

**7.2 Antimatter Production** — 8,000 sci — Prereqs: [7.1]
- Buildings: Particle Accelerator Mk II

**7.3 Geodesic Shielding** — 5,000 sci — Prereqs: [6.5, 6.4]
- Geodesic Shield S, Geodesic Shield M, Geodesic Shield L

**Era 7 total: 21,000 science**

### ERA 8: ANTIMATTER ENGINES (8,000–12,000 science)

**8.1 Antimatter Torch Drive** — 12,000 sci — Prereqs: [7.2, 7.3]
- Antimatter Torch
- Buildings: Particle Accelerator Mk III

**8.2 Antimatter Power** — 8,000 sci — Prereqs: [7.2, 6.4]
- Penning Reactor

**8.3 Advanced Antimatter Power** — 12,000 sci — Prereqs: [8.2]
- Ixion Reactor

**Era 8 total: 32,000 science**

### ERA 9: ENDGAME (10,000–15,000 science)

**9.1 Gamma-Ray Photon Drive** — 15,000 sci — Prereqs: [8.1, 8.3]
- Gamma Converter

**9.2 Planetary Ring Accelerator** — 10,000 sci — Prereqs: [8.1]
- Buildings: Particle Accelerator Mk IV

**Era 9 total: 25,000 science**

### Efficiency Upgrade Lines

Each line: 15 tiers in a chain after a prerequisite node. Cost per tier: `base × tier^1.7`. See §4 Technology Upgrades for what each line affects.

Beyond throughput scaling (+11%/tier), some tiers gate new factory recipes — making tier investment feel qualitatively meaningful rather than pure number-going-up. Lines without recipe gates (Mining, Agriculture, Atmospheric Science, Life Support, Precision Mfg) reward the player through throughput alone.

| Tech Line | Base | Total (15 tiers) | Prereq Node | Recipe Gates |
|-----------|-----:|-----------------:|-------------|--------------|
| Mining | 8 | 4,800 | 2.6 Colony Foundations | — |
| Metallurgy | 8 | 4,800 | 2.6 Colony Foundations | T5: Alloy Forging |
| Electronics Mfg | 10 | 6,000 | 3.8 Extended Crewed | T6: Superconductor Fab |
| Agriculture | 6 | 3,600 | 2.3 Advanced Crewed | — |
| Chemical Processing | 6 | 3,600 | 2.6 Colony Foundations | T3: Sabatier, Kerosene; T8: Deuterium |
| Atmospheric Science | 8 | 4,800 | 3.6 Heavy Fission | — |
| Nuclear Engineering | 12 | 7,200 | 3.6 Heavy Fission | T3: Uranium Enrichment; T5: NPU Assembly |
| Isotope Extraction | 12 | 7,200 | 6.3 Fusion Power | T5: Gas Giant He-3 |
| Construction | 8 | 4,800 | 2.6 Colony Foundations | T8: Ship Part Mfg |
| Precision Mfg | 15 | 9,000 | Elec Mfg Tier 12 | — |
| Life Support | 6 | 3,600 | 2.6 Colony Foundations | — |

**Total efficiency upgrades: ~59,400 science**

### Cost Summary

| Era | Nodes | Science |
|-----|------:|--------:|
| 1 Early Chemical | 6 | 590 |
| 2 Advanced Chemical | 6 | 2,300 |
| 3 Nuclear & Electric | 8 | 8,200 |
| 4 Interplanetary | 3 | 6,000 |
| 5 Nuclear Pulse | 3 | 12,500 |
| 6 Fusion | 5 | 26,000 |
| 7 Antimatter Research | 3 | 21,000 |
| 8 Antimatter Engines | 3 | 32,000 |
| 9 Endgame | 2 | 25,000 |
| Efficiency Upgrades | 165 | 59,400 |
| **Total** | **204** | **~192,990** |

vs full-game budget of ~218,600 — completable with ~25,000 science headroom from the R&D exhaustion pool.

---

*All numbers are starting points for balancing. Prices, production rates, and progression pacing should be tuned during implementation.*
