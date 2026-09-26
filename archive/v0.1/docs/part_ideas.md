# Game Ideas

## Engine Concepts

### Design Philosophy

Engines are categorized by size (nozzle diameter), fuel type, and purpose. Each engine has trade-offs between thrust, efficiency (ISP), mass, and gimbal range.

**Fuel Types:**
- **LOX/RP-1** - Kerosene, moderate ISP, high density, good for first stages
- **LOX/Methane (CH4)** - Balanced ISP and density, reusable-friendly
- **LOX/Hydrogen (LH2)** - Highest ISP, low density, best for upper stages

**Size Categories (1 grid square = 0.5m):**
- **Tiny** - 0.3-0.5m nozzle, 1 grid square wide (0.5m)
- **Small** - 1-1.5m nozzle, 3 grid squares wide (1.5m)
- **Medium** - 1.5-2.5m nozzle, 5 grid squares wide (2.5m)
- **Large** - 2.5-4.5m nozzle, 9 grid squares wide (4.5m)

---

## TINY Engines (1 grid wide = 0.5m)

*Small engines for probes, satellites, and precision maneuvering*

### Hummingbird (LOX/Methane)
- **Inspired by:** Small pressure-fed methalox concepts
- **Thrust:** 15 kN vac / 12 kN ASL
- **ISP:** 330s vac / 295s ASL
- **Mass:** 25 kg
- **Gimbal:** ±8°
- **Purpose:** Probe main engine, small lander, highly restartable
- **Character:** Efficient and delicate, good control authority for its size

### Gecko (LOX/RP-1)
- **Inspired by:** Rocket Lab Rutherford
- **Thrust:** 28 kN vac / 22 kN ASL
- **ISP:** 315s vac / 280s ASL
- **Mass:** 40 kg
- **Gimbal:** ±4°
- **Purpose:** Small stage clusters, high TWR for size
- **Character:** Designed for clusters with differential throttling, minimal gimbal

### Firefly (LOX/Hydrogen)
- **Inspired by:** Small hydrolox upper stage concepts
- **Thrust:** 8 kN vac / 5 kN ASL
- **ISP:** 445s vac / 360s ASL
- **Mass:** 30 kg
- **Gimbal:** None (fixed)
- **Purpose:** Deep space probes, high delta-V missions
- **Character:** Simple fixed nozzle, relies on RCS for attitude control

### Sparrow (LOX/RP-1)
- **Inspired by:** Throttleable descent engines
- **Thrust:** 20 kN vac / 16 kN ASL
- **ISP:** 305s vac / 270s ASL
- **Mass:** 35 kg
- **Gimbal:** ±15°
- **Purpose:** Small lander propulsion, precision hovering
- **Character:** Extreme gimbal range compensates for single-engine landing

---

## SMALL Engines (3 grid wide = 1.5m)

*Versatile engines for upper stages, small boosters, and medium craft*

### Wolf (LOX/RP-1)
- **Inspired by:** SpaceX Merlin 1D
- **Thrust:** 850 kN vac / 750 kN ASL
- **ISP:** 315s vac / 285s ASL
- **Mass:** 500 kg
- **Gimbal:** ±8°
- **Purpose:** First stage main engine, reusable booster core
- **Character:** Versatile workhorse with good gimbal for landing and ascent

### Falcon (LOX/Methane)
- **Inspired by:** Scaled-down Raptor
- **Thrust:** 600 kN vac / 520 kN ASL
- **ISP:** 355s vac / 320s ASL
- **Mass:** 450 kg
- **Gimbal:** ±12°
- **Purpose:** Reusable upper stage, propulsive landing
- **Character:** Excellent gimbal authority, optimized for powered descent

### Owl (LOX/Hydrogen)
- **Inspired by:** Aerojet RL-10 family
- **Thrust:** 180 kN vac / 110 kN ASL
- **ISP:** 460s vac / 370s ASL
- **Mass:** 280 kg
- **Gimbal:** ±4°
- **Purpose:** Upper stage, interplanetary injection
- **Character:** Expander cycle limits gimbal hardware, prioritizes efficiency

### Viper (LOX/RP-1)
- **Inspired by:** NK-33 (compact powerhouse philosophy)
- **Thrust:** 1100 kN vac / 950 kN ASL
- **ISP:** 325s vac / 295s ASL
- **Mass:** 700 kg
- **Gimbal:** None (fixed)
- **Purpose:** Dense high-thrust first stage, booster clusters
- **Character:** Trades gimbal for extreme thrust density, use in clusters or with fins

---

## MEDIUM Engines (5 grid wide = 2.5m)

*Main engines for medium-lift vehicles and heavy upper stages*

### Bear (LOX/Methane)
- **Inspired by:** Blue Origin BE-4
- **Thrust:** 2200 kN vac / 1900 kN ASL
- **ISP:** 345s vac / 310s ASL
- **Mass:** 1800 kg
- **Gimbal:** ±8°
- **Purpose:** Main first stage engine for medium-heavy lift
- **Character:** Reliable all-rounder with solid gimbal performance

### Eagle (LOX/Hydrogen)
- **Inspired by:** Aerojet RS-25 (scaled down)
- **Thrust:** 1400 kN vac / 1100 kN ASL
- **ISP:** 450s vac / 365s ASL
- **Mass:** 2200 kg
- **Gimbal:** ±10.5°
- **Purpose:** Hydrogen core stage, high-performance second stage
- **Character:** RS-25 heritage gimbal system, excellent control

### Panther (LOX/RP-1)
- **Inspired by:** NPO Energomash RD-180 (single chamber)
- **Thrust:** 2000 kN vac / 1750 kN ASL
- **ISP:** 335s vac / 305s ASL
- **Mass:** 2500 kg
- **Gimbal:** ±6°
- **Purpose:** Heavy-lift booster, high-thrust core
- **Character:** Moderate gimbal, relies on raw power over finesse

### Crane (LOX/Methane)
- **Inspired by:** SpaceX Raptor Vacuum variant
- **Thrust:** 1500 kN vac / 1200 kN ASL (throttles to 20%)
- **ISP:** 365s vac / 325s ASL
- **Mass:** 1400 kg
- **Gimbal:** ±15°
- **Purpose:** Large lander, precision powered descent
- **Character:** Maximum gimbal range for landing, lower thrust but supreme control

---

## LARGE Engines (9 grid wide = 4.5m)

*Heavy-lift engines for super-heavy vehicles and massive payloads*

### Mammoth (LOX/RP-1)
- **Inspired by:** Rocketdyne F-1
- **Thrust:** 7500 kN vac / 6700 kN ASL
- **ISP:** 305s vac / 265s ASL
- **Mass:** 8000 kg
- **Gimbal:** ±6°
- **Purpose:** Super-heavy first stage, moon rocket scale
- **Character:** Size limits gimbal range, but 6° at this thrust is plenty

### Whale (LOX/Hydrogen)
- **Inspired by:** RS-68 / J-2X concepts
- **Thrust:** 3500 kN vac / 2800 kN ASL
- **ISP:** 445s vac / 360s ASL
- **Mass:** 5500 kg
- **Gimbal:** ±8°
- **Purpose:** Heavy-lift hydrogen core, SLS-class vehicles
- **Character:** Good gimbal for hydrogen stage control

### Bison (LOX/Methane)
- **Inspired by:** Super-scaled Raptor concept
- **Thrust:** 5000 kN vac / 4300 kN ASL
- **ISP:** 350s vac / 315s ASL
- **Mass:** 4200 kg
- **Gimbal:** ±12°
- **Purpose:** Reusable super-heavy booster
- **Character:** Wide gimbal for booster landing and recovery

### Titan (LOX/RP-1)
- **Inspired by:** Large vacuum-optimized kerolox (M-1 concepts)
- **Thrust:** 4500 kN vac / 3200 kN ASL
- **ISP:** 340s vac / 290s ASL
- **Mass:** 5000 kg
- **Gimbal:** None (fixed)
- **Purpose:** Large upper stage, trans-lunar injection
- **Character:** Extended vacuum nozzle prevents gimbal, pair with RCS

---

## Summary Table

| Size | Engine | Fuel | Thrust (vac) | ISP (vac) | Mass | TWR | Gimbal |
|------|--------|------|--------------|-----------|------|-----|--------|
| **Tiny** | Hummingbird | CH4 | 15 kN | 330s | 25 kg | 61 | ±8° |
| | Gecko | RP-1 | 28 kN | 315s | 40 kg | 71 | ±4° |
| | Firefly | LH2 | 8 kN | 445s | 30 kg | 27 | None |
| | Sparrow | RP-1 | 20 kN | 305s | 35 kg | 58 | ±15° |
| **Small** | Wolf | RP-1 | 850 kN | 315s | 500 kg | 173 | ±8° |
| | Falcon | CH4 | 600 kN | 355s | 450 kg | 136 | ±12° |
| | Owl | LH2 | 180 kN | 460s | 280 kg | 66 | ±4° |
| | Viper | RP-1 | 1100 kN | 325s | 700 kg | 160 | None |
| **Medium** | Bear | CH4 | 2200 kN | 345s | 1800 kg | 125 | ±8° |
| | Eagle | LH2 | 1400 kN | 450s | 2200 kg | 65 | ±10.5° |
| | Panther | RP-1 | 2000 kN | 335s | 2500 kg | 82 | ±6° |
| | Crane | CH4 | 1500 kN | 365s | 1400 kg | 109 | ±15° |
| **Large** | Mammoth | RP-1 | 7500 kN | 305s | 8000 kg | 96 | ±6° |
| | Whale | LH2 | 3500 kN | 445s | 5500 kg | 65 | ±8° |
| | Bison | CH4 | 5000 kN | 350s | 4200 kg | 121 | ±12° |
| | Titan | RP-1 | 4500 kN | 340s | 5000 kg | 92 | None |

---

## Gimbal Balance Philosophy

**No Gimbal (Fixed) Engines:**
- **Firefly** - Simple deep space probe engine, mass savings, use RCS
- **Viper** - Extreme thrust density trade-off, designed for clusters
- **Titan** - Extended vacuum nozzle physically prevents gimbal

**Low Gimbal (±4-6°):**
- **Gecko** - Cluster-oriented, differential throttle for steering
- **Owl** - Expander cycle packaging constraints
- **Panther** - Brute force over finesse
- **Mammoth** - Physical size limits gimbal actuators

**Standard Gimbal (±8°):**
- **Hummingbird, Wolf, Bear, Whale** - Versatile all-rounders

**High Gimbal (±10-15°):**
- **Sparrow, Falcon, Crane, Bison** - Landing specialists need wide gimbal
- **Eagle** - Heritage RS-25 gimbal system

---

## Interstellar Systems

### Design Philosophy

Interstellar-scale systems span three technology tiers: **fission** (Orion-era, ~0.1c), **fusion** (Daedalus/Z-Pinch era, ~0.3c), and **antimatter** (Type 2 civilization, 0.5–0.85c). Each tier has engines, reactors, shields, and radiators. The design bottleneck shifts at each tier: fission is mass-constrained, fusion is thermally-constrained, antimatter is fuel-logistics-constrained.

---

### Interstellar Engines

| Engine | Fuel | Ve (km/s) | Thrust (MN) | Fuel Flow (kg/s) | Engine Mass (t) | Total Reaction Power | Dir. Eff. | Waste Heat |
|---|---|---|---|---|---|---|---|---|
| Orion Pulse | U-235/Pu-239 + Li-6D | 40 | 60 | 1,500 | 7,000 | 3 TW | 98% | 60 GW |
| Daedalus – Stage 1 | D + He-3 (1:1) | 10,000 | 7 | 0.7 | 4,500 | 35 TW | 97% | 1.05 TW |
| Daedalus – Stage 2 | D + He-3 (1:1) | 10,000 | 2 | 0.2 | 2,000 | 10 TW | 97.5% | 250 GW |
| Z-Pinch (Probe) | D + He-3 (1:1) | 8,000 | 0.8 | 0.1 | 600 | 4 TW | 96% | 160 GW |
| Z-Pinch (Advanced p-B11) | p + B-11 (1:11) | 20,000 | 5 | 0.25 | 2,500 | 25 TW | 98% | 500 GW |
| AM-Cat Fusion | D + He-3 + μg anti-H | 30,000 | 4 | 0.133 | 2,000 | 60 TW | 99.5% | 300 GW |
| Antimatter Torch (0.5c) | H + anti-H (1:1) | 150,000 | 2 | 0.0133 | 800 | 300 TW | 99.9% | 300 GW |
| Gamma Conversion (0.85c) | H + anti-H (1:1) + pair stage | 255,000 | 1.3 | 0.0051 | 1,800 | 300 TW | 99.95% | 150 GW |

### Interstellar Engine Dimensions

Dimensions are based on the physical reaction area requirements of each propulsion concept, derived from engineering studies (BIS Project Daedalus, General Atomics Project Orion, LLNL VISTA, Penn State ICAN-II, NASA MSFC Z-Pinch, Keane & Zhang beam-core antimatter). All engines are orbital-construction-only parts.

The density (kg/grid-sq) varies dramatically by engine type — open magnetic cage nozzles (Daedalus) are mostly empty space, while compact antimatter containment systems (AM Torch) pack heavy superconducting magnets and shielding into a small volume:

| Engine | Mass (t) | Width (grid) | Height (grid) | Width (m) | Height (m) | Top Width (grid) | Density (kg/sq) |
|---|---|---|---|---|---|---|---|
| Antimatter Torch | 800 | 8 | 12 | 4.0 | 6.0 | 6 | 8,333 |
| Z-Pinch (Probe) | 600 | 10 | 18 | 5.0 | 9.0 | 6 | 3,333 |
| Z-Pinch (Advanced) | 2,500 | 16 | 26 | 8.0 | 13.0 | 10 | 6,010 |
| AM-Cat Fusion | 2,000 | 22 | 16 | 11.0 | 8.0 | 12 | 5,682 |
| Gamma Conversion | 1,800 | 34 | 26 | 17.0 | 13.0 | 14 | 2,036 |
| Daedalus S2 | 2,000 | 44 | 34 | 22.0 | 17.0 | 16 | 1,337 |
| Daedalus S1 | 4,500 | 60 | 48 | 30.0 | 24.0 | 20 | 1,563 |
| Orion Pulse | 7,000 | 70 | 52 | 35.0 | 26.0 | 30 | 1,923 |

**Real-world dimension references:**

| Engine | Real Diameter | Real Length | Source | Game Scale |
|---|---|---|---|---|
| Daedalus S1 | ~55m (bell) | ~45m | BIS 1978 study | ~55% |
| Daedalus S2 | 40m (bell) | ~30m | BIS 1978 (documented) | ~55% |
| Orion Pulse | 41m (plate) | ~30m | General Atomics interplanetary | ~85% |
| Gamma Conversion | 26m (reflector coil) | ~18m | VISTA/LLNL study | ~65% |
| AM-Cat Fusion | 8–10m (SiC shell) | ~7m | ICAN-II Penn State | ~110% |
| Z-Pinch (Advanced) | ~5m (nozzle) | ~10m | NASA MSFC Adams 2012 | ~160% |
| Z-Pinch (Probe) | ~3m (nozzle) | ~6m | NASA MSFC Adams 2012 | ~170% |
| Antimatter Torch | 3m (solenoid) | ~4m | Keane & Zhang 2012 | ~130% |

Note: Compact engines (Z-Pinch, AM Torch, AM-Cat) are slightly larger than real-world references to account for game-scale power conditioning, fuel handling, and support systems not detailed in the original studies. Large engines (Daedalus, Orion) are slightly compressed from real-world scale for editor practicality.

**Visual shape concepts:**
- **Orion Pulse** (70×52, top 30): Anvil-shaped. Massive 35m steel pusher plate at bottom (full width, ~4,000t of plate alone), shock absorber column with compression springs above, pulse unit magazine, narrower mount adapter at top. Unique silhouette — wider at bottom than top.
- **Daedalus S1/S2** (60×48 / 44×34): Reactor housing with hemispherical reaction dome, feeding into an open magnetic cage nozzle — full-width copper superconducting coil ring bands connected by steel longerons with transparent gaps between rings. The defining visual of an interstellar fusion engine.
- **Z-Pinch Probe/Advanced** (10×18 / 16×26): Tall narrow cylindrical body with prominent Z-pinch coil rings and electrodes along the full length. Converging section feeds into an open magnetic cage nozzle. Industrial, utilitarian. Tall aspect ratio reflects tube confinement geometry.
- **AM-Cat Fusion** (22×16): Purple Penning trap antimatter storage at top, blue-steel fusion reactor with ion beam injector nubs and viewports, copper containment ring, then open magnetic cage nozzle. Squat and wide — the SiC shell dominates the silhouette.
- **Antimatter Torch** (8×12): Purple antimatter feed system at top, then the "birdcage" magnetic bottle — cylindrical open cage of superconducting coil rings with magnetic mirrors at both ends. Short exhaust section below. The most compact interstellar engine — extremely dense for its size.
- **Gamma Conversion** (34×26): Three distinct sections — purple annihilation reactor dome, blue pair production chamber with viewports, then massive 17m parabolic reflector with curved panel lines, focal point structure, and magnetic collimation coils near exit. The only interstellar engine without an open cage nozzle — the solid reflector is its defining visual.

**Engine waste heat notes:**
- Fission/nuclear pulse (2–4% of total): Neutron radiation scatters broadly, bulky shielding absorbs significant heat.
- Fusion (2.5–3%): Magnetic confinement directs most charged particles into exhaust, but neutrons and bremsstrahlung leak.
- Antimatter (0.05–0.1%): Annihilation products are charged and magnetically directable. Very little leaks into structure.

---

### Power Generation

Reactors are separate from engines. Power output is scaled to realistic specific power values, extrapolated from real-world fission reactor performance (Virginia-class submarine ~175 kW/t) with advancement factors per tier.

| Reactor | Power Output | Waste Heat | Mass | Fuel | Fuel Rate (100%) | Conversion Eff. | Specific Power |
|---|---|---|---|---|---|---|---|
| Fission (Small) | 500 MW | 214 MW | 250 t | U-235 | 0.032 kg/hr | 70% | 2 MW/t |
| Fission (Large) | 1.6 GW | 686 MW | 800 t | U-235 | 0.103 kg/hr | 70% | 2 MW/t |
| Fusion (Small) | 10 GW | 6.7 GW | 400 t | D + He-3 | 0.17 kg/hr | 60% | 25 MW/t |
| Fusion (Large) | 30 GW | 20 GW | 1,200 t | D + He-3 | 0.51 kg/hr | 60% | 25 MW/t |
| Antimatter (Small) | 800 GW | 141 GW | 800 t | H + anti-H | 37.6 g/hr | 85% | 1,000 MW/t |
| Antimatter (Large) | 2,500 GW | 441 GW | 2,500 t | H + anti-H | 118 g/hr | 85% | 1,000 MW/t |

**Specific power tiers:** Fission 2 MW/t → Fusion 25 MW/t (12.5×) → Antimatter 1,000 MW/t (40×). Total scaling fission→antimatter = 500×, conservative vs theoretical energy density advantage of ~1,125× (80 TJ/kg fission vs 90 PJ/kg antimatter).

**Reactor–shield pairings:**
- Fission Large (1.6 GW): Powers FRES Small at 0.1c (1 GW)
- Fusion Small (10 GW): Powers FRES Small up to ~0.2c, or Geodesic Small at any speed
- Fusion Large (30 GW): Powers FRES Medium up to ~0.24c, or any Geodesic at any speed
- AM Small (800 GW): Powers FRES Large up to ~0.5c
- AM Large (2,500 GW): Powers FRES Large up to ~0.85c (2,456 GW needed)

**Physics basis:**
- Fission (70% eff.): Advanced gas-core fission with Brayton cycle. ~10× improvement over current naval reactor specific power. U-235 releases ~80 TJ/kg.
- Fusion (60% eff.): D+He-3 ICF or magnetic confinement with direct electrostatic conversion. D-He3 releases ~350 TJ/kg. Aneutronic reaction enables direct charged-particle capture.
- Antimatter (85% eff.): Controlled p-antiproton annihilation with magnetic nozzle capture of charged pion/muon decay products. 90 PJ/kg of total reactant. Fuel rate splits 50/50 matter/antimatter.

---

### Shielding Systems

| Shield | Size | Diameter | Mass | Rated Max Velocity | Power @0.1c | @0.3c | @0.5c | @0.85c |
|---|---|---|---|---|---|---|---|---|
| Passive Whipple | Small | 50 m | 300 t | 0.1c | 0 | — | — | — |
| Passive Whipple | Medium | 100 m | 1,200 t | 0.1c | 0 | — | — | — |
| Passive Whipple | Large | 200 m | 4,800 t | 0.1c | 0 | — | — | — |
| Active FRES | Small | 50 m | 300 t | 0.85c (power-limited) | 1 GW | 27 GW | 125 GW | 614 GW |
| Active FRES | Medium | 100 m | 750 t | 0.85c (power-limited) | 2 GW | 54 GW | 250 GW | 1,228 GW |
| Active FRES | Large | 200 m | 2,400 t | 0.85c (power-limited) | 4 GW | 108 GW | 500 GW | 2,456 GW |
| Geodesic Deflector | Small | 50 m | 800 t | 0.85c | 4 GW | 4.3 GW | 4.8 GW | 6.2 GW |
| Geodesic Deflector | Medium | 100 m | 2,000 t | 0.85c | 8 GW | 8.6 GW | 9.6 GW | 12.5 GW |
| Geodesic Deflector | Large | 200 m | 6,000 t | 0.85c | 16 GW | 17.1 GW | 19.1 GW | 25.0 GW |

**Shield types:**
- **Passive Whipple:** Physical multi-layer barrier. Zero power, heavy, 0.1c max. Ablative protection only.
- **Active FRES (Field-Reinforced Electromagnetic Shield):** Electromagnetic particle deflection. Power scales as v³ (flux × kinetic energy). Formula: P = P_base × (v/0.1c)³, size ratio 1:2:4 GW base. Light but power-hungry at high speeds. Requires antimatter reactor above ~0.3c.
- **Geodesic Deflector:** Exotic-matter toroidal ring generates controlled spacetime curvature. Particles follow curved geodesics around the ship. Power scales as P = P_base × (1 + 0.775v²), nearly flat with velocity. Deflects everything universally — charged particles, neutral atoms, dust, even photons. Requires exotic matter only producible by antimatter-scale industrial processes. 2.5–3× heavier than FRES but far more power-efficient at high speed. Crossover vs FRES at ~0.16c — below that FRES is cheaper, above that Geodesic is cheaper. Fusion reactors can power Geodesic at any speed; high-speed FRES requires antimatter reactors.

**FRES — How It Works:**
The FRES generates an intense magnetic bubble around the ship using a stack of high-temperature superconducting solenoid coils. Charged particles entering the field experience the Lorentz force and are deflected along curved trajectories away from the ship. The critical problem is that the interstellar medium is mostly neutral hydrogen — invisible to electromagnetic fields. The FRES solves this with a forward-facing pre-ionization array: a grid of UV laser emitters that bathes the incoming particle stream in hard ultraviolet, stripping electrons from neutral atoms and converting them to ions that the magnetic field can then deflect. Power scales as v³ because particle flux increases linearly with velocity while kinetic energy per particle increases as v², requiring proportionally stronger fields. Components from fore to aft: (1) **pre-ionization emitter array** — forward-facing UV laser grid that ionizes incoming neutrals; (2) **superconducting solenoid stack** — the primary field generators, multiple copper-wound HTS coils separated by open structural bays, each coil producing a segment of the overall magnetic dipole; (3) **electrostatic deflection electrodes** — charged discs at intervals between coils that provide supplementary electrostatic deflection for residual ions; (4) **cryocooler radiator fins** — small heat-rejection panels on the sides that keep the superconductors at operating temperature; (5) **power distribution bus** — high-current superconducting cables running the length of the stack connecting to the ship's reactor.

**Geodesic Deflector — How It Works:**
The Geodesic Deflector exploits general relativity: a toroidal ring of exotic matter (negative energy density) generates controlled spacetime curvature that bends the trajectories of all incoming particles — charged, neutral, dust, even photons — along geodesics that curve smoothly around the ship. Unlike the FRES, which can only push charged particles, the Geodesic warps spacetime itself, so nothing can pass through straight regardless of charge or mass. The power requirement is nearly flat with velocity because the deflector maintains a static spacetime metric rather than actively fighting particle momentum — the curvature simply exists and particles passively follow it. The exotic matter is produced only by antimatter-scale industrial processes (pair-production cascades in controlled annihilation reactors) and slowly decays, requiring periodic replenishment. Components: (1) **exotic matter containment torus** — the defining element, a toroidal pressure vessel containing stabilized exotic matter that generates the spacetime curvature; the torus has a distinctive visual signature from Cherenkov-like glow as virtual particle pairs interact with the negative energy field; (2) **metric stabilization coils** — superconducting windings wrapped directly around the torus surface that maintain the precise curvature geometry and prevent metric oscillations; (3) **gravimetric sensor pods** — small monitoring instruments positioned around the torus perimeter that continuously measure local spacetime curvature and feed corrections to the stabilization array; (4) **exotic matter injector ports** — replenishment valves on the torus for topping off decayed exotic matter; (5) **power conditioning unit** — converts reactor output into the specific high-frequency waveforms required by the stabilization coils; (6) **structural support cage** — rigid framework holding the torus precisely centered on the ship axis.

---

### Radiator Systems

Power radiated per m² (double-sided): Q = 2εσT⁴. Doubling operating temperature gives 16× rejection per unit area.

| Radiator | Tech Era | Operating Temp | Emissivity | Mass | Heat Rejected | Specific Rejection |
|---|---|---|---|---|---|---|
| Heat Pipe Panel | Fission | 1,200 K | 0.92 | 8 kg/m² | 216 kW/m² | 27 kW/kg |
| Liquid Metal Droplet | Fusion | 2,500 K | 0.95 | 3 kg/m² | 4.2 MW/m² | 1.4 MW/kg |
| Phononic Metamaterial | Antimatter | 6,000 K | 0.99 | 1.5 kg/m² | 145 MW/m² | 97 MW/kg |

Each tier is ~20× better specific rejection than the last.

**Radiator technologies:**
- **Heat Pipe Panel:** Sodium/lithium heat pipes in carbon-carbon composite sheets. Limited by working fluid and carbon sublimation point.
- **Liquid Metal Droplet Array:** Tin or lithium droplets sprayed into vacuum, radiating freely, magnetically recollected. No solid structure at the radiating surface eliminates melting point limits.
- **Phononic Metamaterial Emitter:** Engineered atomic lattices channel thermal phonons into coherent radiative emission. Strong-force-scale bonding maintains structure at stellar photosphere temperatures. Self-repairing lattice.

**Radiator burden by ship class:**

| Ship | Total Waste Heat | Radiator Tier | Area Needed | Radiator Mass | % of Dry Mass |
|---|---|---|---|---|---|
| Orion + Fission Large | 61 GW | Heat Pipe | 282,000 m² | 2,259 t | ~15% |
| Daedalus S1 + Fusion Large | 1,070 GW | Liquid Droplet | 255,000 m² | 764 t | ~11% |
| AM Torch + AM Small | 441 GW | Phononic | 3,041 m² | 4.6 t | <0.2% |

---

### Endgame Ship Design Pattern

The optimal high-speed interstellar ship uses two operating modes:

1. **Burn phase:** Antimatter engine + antimatter reactor + FRES (maximum power, expensive fuel, short duration)
2. **Cruise phase:** Engine off, geodesic deflector active, fusion reactor (low power, cheap fuel, years-long duration)

Example — Alpha Centauri sprint at 0.85c (5.5-year cruise):
- **FRES Small + AM Small reactor for shield:** 614 GW continuous → 29 g/hr antimatter → ~1.4 tonnes antimatter for shield alone
- **Geodesic Small + Fusion Small reactor:** 6.2 GW → 0.11 kg/hr D+He-3 → ~5.2 tonnes D+He-3, zero antimatter during cruise

The antimatter budget becomes acceleration/deceleration only. Cruise is fusion-powered.

---

### Electricity System

Conventional electricity for pods, avionics, and engine startup. Stored in watt-hours (Wh), generated and consumed in watts (W). Separate from interstellar reactor power (GW-TW scale) — reactors are not part of the conventional electricity grid.

#### Batteries

5,000 Wh capacity and 25 kg per grid square. Thin stackable banks.

| Part | Size | Grid | Mass | Capacity | Cost |
|---|---|---|---|---|---|
| Battery Bank Z-1 | Tiny | 1×1 | 25 kg | 5,000 Wh | 200 |
| Battery Bank Z-3 | Small | 3×1 | 75 kg | 15,000 Wh | 500 |
| Battery Bank Z-5 | Medium | 5×1 | 125 kg | 25,000 Wh | 1,000 |
| Battery Bank Z-9 | Large | 9×1 | 225 kg | 45,000 Wh | 2,000 |
| Battery Bank Z-13 | XL | 13×1 | 325 kg | 65,000 Wh | 3,500 |
| Radial Battery | Radial | 1×1 | 25 kg | 5,000 Wh | 300 |

#### Solar Panels

Power at 1 AU, scales as 1/r² with distance from Sun.

| Part | Size | Grid | Mass | Power @1AU | Cost |
|---|---|---|---|---|---|
| OX-1 Fixed Panel | Tiny | 1×2 | 10 kg | 300 W | 500 |
| OX-3 Fixed Panel | Small | 1×3 | 25 kg | 1,000 W | 1,500 |
| OX-5 Tracking Panel | Medium | 1×3 | 50 kg | 2,500 W | 4,000 |
| OX-9 Tracking Array | Large | 1×4 | 100 kg | 4,500 W | 8,000 |

#### RTG

| Part | Size | Grid | Mass | Power | Cost |
|---|---|---|---|---|---|
| PB-NUK RTG | Radial | 1×2 | 80 kg | 300 W | 10,000 |

Constant output regardless of distance from Sun. Ideal for outer solar system missions.

#### Power Consumers

| Component | Power Draw | Notes |
|---|---|---|
| Small Pod | 200 W | Life support, avionics |
| Medium Pod | 500 W | Life support, avionics |
| SAS (active) | 100 W | When active |

#### Interstellar Engine Startup Power

Electric ignition sequence — one-time draw from batteries per ignition. Engines past the Orion drive require electrical startup to initiate their reaction.

| Engine | Startup Power |
|---|---|
| Z-Pinch Probe | 50,000 Wh |
| Z-Pinch Advanced | 75,000 Wh |
| AM-Cat Fusion | 150,000 Wh |
| AM Torch | 200,000 Wh |
| Gamma Conversion | 300,000 Wh |
| Daedalus S2 | 500,000 Wh |
| Daedalus S1 | 1,000,000 Wh |

#### Reactor Startup Power

Reactors require a one-time electrical charge to initiate. Batteries must provide this before the reactor can begin generating power.

| Reactor | Startup Power |
|---|---|
| Fission Small | 30,000 Wh |
| Fission Large | 50,000 Wh |
| Fusion Small | 100,000 Wh |
| Fusion Large | 200,000 Wh |
| AM Small | 300,000 Wh |
| AM Large | 500,000 Wh |

#### Cold-Start Chain

Batteries → Reactor startup → Reactor recharges batteries → Engine startup → Cruise
