# Laser Sails

Laser sails are a parallel interstellar propulsion system that uses external beamed power
instead of onboard fuel. A ground-based phased laser array at a colony pushes a
lightweight reflective sail, accelerating the vessel without consuming propellant.

**Core tradeoff vs reaction drives**: Zero fuel cost per mission, but requires colony
infrastructure and can only push *away* from the laser source. Reaction drives go anywhere;
laser sails create highways between established colonies.

**Fundamental limitation**: Photon momentum is inherently weak — 2P/c gives only 6.67
newtons per gigawatt. Laser sails compensate by carrying no fuel mass, making them ideal
for lightweight probes and repeated cargo runs where the infrastructure cost amortizes
over many missions.

---

## Added Content Summary

### Ship Parts

| Part | Category | Era | Size | Stowed Grid | Deployed Radius | Sail Area | Mass | Cost | Thermal Limit |
|------|----------|-----|------|-------------|-----------------|-----------|------|------|---------------|
| Probe Lightsail | Interstellar | 5 | Small | 3×5 | 500 m | 785,398 m² | 0.8 t | 300,000 | 25 kW/m² (460 K) |
| Expedition Lightsail | Interstellar | 6 | Medium | 5×7 | 4 km | 50.3 km² | 50 t | 2,500,000 | 1,000 kW/m² (700 K) |
| Grand Lightsail | Interstellar | 7 | XL | 13×15 | 12.5 km | 491 km² | 491 t | 10,000,000 | 5,700 kW/m² (1,000 K) |

All sails: areal density 1 g/m², reflectivity 0.999, no propellant, rectangle shape (stowed canister), irreversible deployment.

### Colony Building

| Building | Power Draw | Beam Power | Build Cost | Maintenance / 30d | Tech Unlock |
|----------|-----------|------------|------------|-------------------|-------------|
| Laser Emitter | 15 GW (active launch only) | 10 GW | 150k Metal, 30k HTA, 50k Elec, 20k Super (250 t) | 375 Metal, 75 HTA, 125 Elec, 50 Super | Laser Sail Propulsion (Era 5) |

N emitters combine into a phased array: total beam = N × 10 GW, aperture = 100 m × √N.

### Tech Tree

| Tech Node | Era | Cost | Prerequisites | Unlocks |
|-----------|-----|------|---------------|---------|
| Laser Sail Propulsion | 5 | 3,000 | Passive Shielding + Heavy Fission | Probe Lightsail, Laser Emitter |
| Advanced Laser Sails | 6 | 5,000 | Laser Sail Propulsion + Active Shielding | Expedition Lightsail |
| Interstellar Laser Highway | 7 | 7,000 | Advanced Laser Sails + Geodesic Shielding | Grand Lightsail |

Shield prerequisites reflect dust hazard: at 0.01c a 1 μg dust grain carries rifle-bullet
energy; at 0.1c, hand-grenade energy; at 0.5c, 11 GJ (small bomb).

---

## Gameplay Loop

### Phase 1: Flyby Probes (Era 5-6)

1. Build 10-100 laser emitters at a well-powered colony (Mercury ideal for solar)
2. Build an ultra-light probe (~1 t) with a Probe Lightsail — no engine, no fuel
3. Aim at target star, activate the array — probe accelerates for days, then coasts
4. At 100 emitters (1 TW), a 1.8 t probe reaches ~0.003c — a flyby of Alpha Centauri
   in ~1,400 years. Slow, but the first data from another star system
5. At 1,000 emitters (10 TW), the same probe reaches ~0.009c — 470 years to Alpha
   Centauri. Cheap reconnaissance at zero fuel cost

### Phase 2: One-Way Colony Seeds (Era 6-7)

1. Expand the array to thousands of emitters (fusion- or antimatter-powered colony)
2. Build a cargo vessel with Expedition Lightsail + small reaction drive for braking
3. Laser pushes the vessel to cruise speed, then it coasts to the target
4. At destination, the reaction drive decelerates into orbit
5. At 1,000 emitters, a 100 t cargo ship reaches ~0.017c (250 yr to Alpha Centauri)
6. At 10,000 emitters, it reaches ~0.07c (60 yr) — practical for colony seeding

### Phase 3: Bidirectional Highway (Era 7+)

1. Destination colony builds its own laser array
2. Outbound laser accelerates; destination laser decelerates (sail flips 180°)
3. Fast, fuel-free, repeatable transit — the "interstellar railroad"
4. Grand Lightsails carry crewed vessels and heavy cargo between established colonies
5. Reaction drives reserved for exploring new systems without laser infrastructure

### Deceleration

Since laser sails only push, deceleration requires one of:

- **Flyby**: No stopping. Probe zips past at cruise speed, gathers data in transit.
  The simplest option for early reconnaissance.
- **Reaction braking**: Carry a small engine + fuel to decelerate at the destination.
  The laser gets you up to speed for free; the engine handles the last part. This is
  the standard approach for colony seeder ships — a hybrid design.
- **Destination laser**: A colony at the target fires its laser array at the incoming
  vessel. The sail flips to face the destination laser, which pushes against it to
  decelerate. Requires established laser infrastructure at both ends. This is the
  endgame "highway" mode — fully fuel-free transit.

---

## Ship Parts

### Lightsails

All lightsails are stored as compact canisters in the editor. In flight, they deploy
into massive reflective mesh structures — hundreds of meters to kilometers across, but
massing only tonnes.

Category: Interstellar. Shape: Rectangle (stowed canister). No propellant.

| Part | Era | Stowed Grid | Size | Deployed Radius | Sail Area | Mass | Cost |
|------|-----|-------------|------|-----------------|-----------|------|------|
| Probe Lightsail | 5 | 3×5 | Small | 500 m | 785,398 m² | 0.8 t | 300,000 |
| Expedition Lightsail | 6 | 5×7 | Medium | 4 km | 50.3 km² | 50 t | 2,500,000 |
| Grand Lightsail | 7 | 13×15 | XL | 12.5 km | 491 km² | 491 t | 10,000,000 |

### Sail Material Progression

Each sail tier uses the best material available at its era. Higher tiers tolerate more
laser flux, enabling faster acceleration.

| Tier | Material | Reflectivity | Emissivity (ε) | T_max | Max Flux | Era |
|------|----------|-------------|----------------|-------|----------|-----|
| Conservative | Aluminum on polyimide | 0.999 | 0.005 | 460 K (190°C) | 25 kW/m² | 5 |
| Moderate | Aluminum on graphene + emissive backing | 0.999 | 0.04 | 700 K (430°C) | 1,000 kW/m² | 6 |
| Advanced | Photonic metamaterial + ceramic emitter | 0.999 | 0.05 | 1,000 K (730°C) | 5,700 kW/m² | 7 |

**Thermal equilibrium**: The sail absorbs (1−R) of incident flux and radiates from both
sides. At equilibrium: `flux_max = 2εσT⁴ / (1−R)`. Higher emissivity and temperature
allow dramatically higher flux tolerance.

The Advanced sail at 1,000 K glows dull orange-red during powered flight — visually
striking at map zoom.

**Common properties** (all tiers):
- Areal density: 1 g/m² (substrate + 20 nm aluminum reflective layer)
- Sheet thickness: ~0.5 μm (deployed as pre-tensioned mesh)
- Deployed shape: Circular mesh (rendered as translucent disc with radial spokes)
- Stowage: Accordion-folded mesh in pressurized canister; deployment is irreversible

**Optimal sail sizing**: Each sail is designed so its mass roughly equals its "matched"
payload mass. This maximizes terminal velocity for that payload class (derivation in
Physics Model section).

| Sail | Sail Mass | Matched Payload | Total at Optimum |
|------|-----------|-----------------|------------------|
| Probe | 0.8 t | ~1 t | ~1.8 t |
| Expedition | 50 t | ~50 t | ~100 t |
| Grand | 491 t | ~500 t | ~1,000 t |

### Sail Degradation

At interstellar velocities, dust grains in the interstellar medium punch through the
gossamer mesh. The sail degrades gradually — losing strands reduces effective reflective
area but does not cause catastrophic failure.

**Degradation model:**

    degradation_per_ly = 0.5 × (v/c)²

where v is the ship's speed and the result is fractional area loss per light-year of travel.
Degradation accumulates continuously during flight while the sail is deployed.

    sail_integrity = 1 − Σ(0.5 × β² × Δd_ly)

| Cruise Speed | Loss / ly | After 4.2 ly | After 10 ly |
|-------------|-----------|--------------|-------------|
| 0.01c | 0.005% | 0.02% | 0.05% |
| 0.05c | 0.13% | 0.5% | 1.3% |
| 0.10c | 0.5% | 2.1% | 5% |
| 0.30c | 4.5% | 19% | 45% |
| 0.50c | 12.5% | 53% | 100%+ |

**Gameplay effects:**
- Degradation reduces effective sail area, proportionally reducing thrust from laser arrays
- At 100% degradation the sail is destroyed and cannot receive thrust
- Displayed in flight UI as "Sail Integrity: XX%"
- Faster launches = more degradation = weaker deceleration capability at destination
- Creates a strategic tradeoff: push harder for speed, or preserve the sail for braking

**After acceleration**: The sail can be jettisoned to stop degradation (and reduce mass
for reaction braking). But a jettisoned sail cannot be used for destination-laser
deceleration. The player must choose: keep the sail (for the highway) or dump it (for
engine braking with less mass).

---

## Colony Building: Laser Emitter

A single building type. Build as many as you want — all emitters at a colony
automatically combine into a phased laser array.

**Placement**: No-atmosphere bodies only (atmospheres scatter the beam). This naturally
channels players to Mercury (best solar power) and the Moon.

| Stat | Value |
|------|-------|
| **Beam power** | 10 GW per building |
| **Power draw** | 15 GW per emitter during active launch; 0 when idle — 67% wall-plug efficiency |
| **Build cost** | 150,000 kg Metal, 30,000 kg HTA, 50,000 kg Elec, 20,000 kg Super |
| **Total build mass** | 250,000 kg (250 t) |
| **Maintenance / 30d** | 375 kg Metal, 75 kg HTA, 125 kg Elec, 50 kg Super |
| **Tech unlock** | Laser Sail Propulsion (Era 5) |
| **Habitability affected** | No |

**Phased array scaling**: N emitters combine coherently. The physical spread of emitters
across the colony body's surface determines the effective aperture.

- Total beam power: P = N × 10 GW
- Effective aperture: D = 100 m × √N

The aperture grows as √N because each new emitter adds baseline to the phased array.
Both power AND beam focus improve as you add emitters — a natural reward for investment.

### Colony Power Context

Each emitter draws 15 GW — comparable to a Fission Reactor's entire output (500 MW)
times 30. The power infrastructure is typically 10-50× more expensive than the emitters
themselves. Power is the real bottleneck.

| N Emitters | Beam Power | Aperture | Power Draw | Power Source Options |
|------------|------------|----------|------------|---------------------|
| 1 | 10 GW | 100 m | 15 GW | 30 Fission Reactors (500 MW each) |
| 10 | 100 GW | 316 m | 150 GW | 30 Fusion Reactors (5 GW each) |
| 100 | 1 TW | 1 km | 1.5 TW | 300 Fusion Reactors |
| 1,000 | 10 TW | 3.16 km | 15 TW | 3,000 Fusion or ~2,300 Solar @ Mercury |
| 10,000 | 100 TW | 10 km | 150 TW | Antimatter-era colony power |

**Mercury advantage**: Large Solar Farms produce 6.6 GW at Mercury (0.39 AU) vs 1 GW
at Earth. Mercury's full solar capacity is 134.5 PW (20.38M farms), enough for 8.97M
emitters — an 89.7 PW beam with a 299 km aperture.

**Opportunity cost**: Colony power used for the laser array cannot simultaneously run
mines, factories, or particle accelerators. During a launch (days to weeks), the colony
dedicates its power budget to the array. Between launches, the power infrastructure
serves normal colony operations.

---

## Tech Tree

Three new nodes on the Beamed Propulsion branch, sharing prerequisites with the shield
and power branches:

| Tech Node | Era | Cost | Prerequisites | Unlocks |
|-----------|-----|------|---------------|---------|
| Laser Sail Propulsion | 5 | 3,000 | Passive Shielding + Heavy Fission | Probe Lightsail, Laser Emitter (building) |
| Advanced Laser Sails | 6 | 5,000 | Laser Sail Propulsion + Active Shielding | Expedition Lightsail |
| Interstellar Laser Highway | 7 | 7,000 | Advanced Laser Sails + Geodesic Shielding | Grand Lightsail |

Each tier unlocks both a new sail part AND the sail material technology needed to survive
at higher speeds. The shield prerequisites ensure the ship itself can handle the dust
environment at the speeds the new material enables.

---

## Physics Model

### Radiation Pressure and Doppler Thrust Reduction

A photon beam of power P striking a stationary perfect reflector exerts force:

    F_static = 2P / c

At 10 GW: F = 2 × 10¹⁰ / 3 × 10⁸ = 66.7 N. Photon momentum is extremely weak.

**Doppler reduction is the dominant relativistic effect.** When the sail moves away at
velocity v, incoming photons are redshifted (less energy, lower arrival rate). Reflected
photons are redshifted again (double Doppler shift). The exact force:

    F(v) = (2P / c) × (1 − β) / (1 + β)        where β = v/c

| Ship Speed | β | Force | % of Static | KE Delivery (% of P) |
|------------|---|-------|-------------|----------------------|
| 0 | 0 | 2P/c | 100% | 0% |
| 3,000 km/s | 0.01 | 1.96P/c | 98.0% | 2.0% |
| 15,000 km/s | 0.05 | 1.81P/c | 90.5% | 9.1% |
| 30,000 km/s | 0.10 | 1.64P/c | 81.8% | 16.4% |
| 90,000 km/s | 0.30 | 1.08P/c | 53.8% | 32.3% |
| 124,000 km/s | 0.414 | 0.83P/c | 41.4% | 34.3% (peak) |
| 150,000 km/s | 0.50 | 0.67P/c | 33.3% | 33.3% |

**Peak KE delivery is 34.3% at β = 0.414.** Above this speed, the Doppler penalty on
force exceeds the gain from higher velocity — the beam is increasingly wasted. This is
a fundamental ceiling on photon propulsion efficiency.

Despite this, laser sails are worthwhile because:
1. The "fuel" (photons) is generated from colony power — no production or transport of
   physical propellant
2. Solar/fission/fusion power is renewable and effectively unlimited
3. Colony power is "stranded" — laser arrays convert stranded power into ship velocity

### Constants

```
c      = 2.998 × 10⁸ m/s          Speed of light
λ      = 1.064 × 10⁻⁶ m          Nd:YAG laser wavelength
1 ly   = 9.461 × 10¹⁵ m
1 AU   = 1.496 × 10¹¹ m
σ      = 5.670 × 10⁻⁸ W/m²/K⁴   Stefan-Boltzmann constant
```

### Beam Divergence and Critical Distance

A diffraction-limited laser beam with aperture diameter D has angular divergence:

    θ = 1.22 × λ / D

The beam radius at distance d:

    r_beam(d) = 1.22 × λ × d / D

The **critical distance** d₀ is where the beam radius equals the sail radius. Inside
d₀, the sail intercepts 100% of the beam. Beyond d₀, intercepted power falls as 1/d².

    d₀ = r_sail × D / (1.22 × λ)

| Sail | Aperture (N emitters) | d₀ (AU) | d₀ |
|------|-----------------------|---------|-----|
| Probe (500 m) | 316 m (N=10) | 0.81 | 1.22 × 10¹¹ m |
| Probe (500 m) | 1 km (N=100) | 2.57 | 3.85 × 10¹¹ m |
| Probe (500 m) | 3.16 km (N=1000) | 8.14 | 1.22 × 10¹² m |
| Expedition (4 km) | 1 km (N=100) | 20.6 | 3.08 × 10¹² m |
| Expedition (4 km) | 3.16 km (N=1000) | 65.1 | 9.75 × 10¹² m |
| Expedition (4 km) | 10 km (N=10000) | 206 | 3.08 × 10¹³ m |
| Grand (12.5 km) | 3.16 km (N=1000) | 203 | 3.04 × 10¹³ m |
| Grand (12.5 km) | 10 km (N=10000) | 643 | 9.63 × 10¹³ m |
| Grand (12.5 km) | 31.6 km (N=100000) | 2,034 | 3.04 × 10¹⁴ m |

### Thrust Profile

The complete force law combines Doppler, beam divergence, and sail degradation:

    F(d, v) = (2P / c) × ((1 − β) / (1 + β)) × min(1, (d₀/d)²) × sail_integrity

**Near-field (d ≤ d₀)**: Full beam intercepted. Thrust drops only from Doppler.

**Far-field (d > d₀)**: Beam has spread beyond the sail. Intercepted power falls as
(d₀/d)². Combined with Doppler, thrust drops rapidly.

### Thermal Limit

The sail's material constrains the maximum laser flux it can sustain. When the array
could deliver more power than the sail can handle, the array modulates down to keep the
sail at its thermal limit.

**Thermal-limited acceleration**: When an array has excess power (common for large arrays
on small sails), the sail operates at constant rest-frame flux = flux_max. This gives
constant proper acceleration:

    α = 2 × flux_max × A_sail / (m × c)

As the ship speeds up, Doppler reduces the received flux in the rest frame, allowing the
array to increase emitted power without overheating the sail. This sustains constant
proper acceleration until either:

1. Beam divergence drops intercepted power below the thermal limit (most common)
2. The array runs out of total power

The **transition distance** d_transition is where the array reaches full power. Beyond
this, thrust drops as 1/d². Before it, constant proper acceleration.

**Thermal threshold**: The thermal limit dominates when N × 10 GW > flux_max × A_sail:

| Sail Tier | flux_max × A_sail | Threshold N |
|-----------|-------------------|-------------|
| Probe (Conservative) | 19.6 GW | 2 |
| Expedition (Moderate) | 50.3 TW | 5,030 |
| Grand (Advanced) | 2.80 PW | 280,000 |

The Probe is thermally limited for essentially all configurations. The Expedition is
thermally limited only above ~5,000 emitters. The Grand is never thermally limited for
arrays below 280,000 emitters (only at Mercury mega-array scale).

### Terminal Velocity (Non-Relativistic Approximation)

Ignoring both Doppler and thermal limits, total work equals kinetic energy:

    v_NR = sqrt(8 × P × d₀ / (m × c))

This is an upper bound. In practice:
- Doppler reduces thrust at speed (significant above 0.1c)
- Thermal limits cap near-field thrust for large arrays on small sails
- Sail degradation reduces area over the journey

### Optimal Sail Sizing

For a given payload mass, the optimal sail has mass equal to the payload:

    m_payload = ρ × π × r_sail² = m_sail

At the optimum, half the total mass is sail and half is payload. The three sail parts
are sized at the optima for ~1 t, ~50 t, and ~500 t payloads.

### Sail Degradation Model

Interstellar dust degrades the sail proportionally to speed² and distance:

    Δ(integrity) = −0.5 × β² × Δd_ly

where Δd_ly is distance traveled in light-years. Effective sail area = A_sail × integrity.
Degradation accumulates during flight while the sail is deployed. The sail can be
jettisoned to halt degradation (but cannot then be used for destination-laser braking).

---

## Performance Tables

### Probe Lightsail (r = 500 m, Conservative material, 25 kW/m²)

Payload: 1 t, total mass 1.8 t. **All entries are thermally limited** — the Conservative
material constrains near-field thrust regardless of array size. More emitters extend the
range (d_transition) rather than increasing peak acceleration.

Thermal-limited acceleration: 0.073 m/s² (0.0074g) — constant for all N.
Velocity scales as √N in the thermal-limited regime.

| N Emitters | Power | Aperture | d₀ (AU) | v/c | Trip 4.2 ly | Degradation at arrival |
|------------|-------|----------|---------|-----|-------------|----------------------|
| 10 | 100 GW | 316 m | 0.81 | 0.0009 | 4,700 yr | <0.1% |
| 100 | 1 TW | 1 km | 2.57 | 0.003 | 1,400 yr | <0.1% |
| 1,000 | 10 TW | 3.16 km | 8.14 | 0.009 | 470 yr | <0.1% |
| 10,000 | 100 TW | 10 km | 25.7 | 0.030 | 140 yr | 0.2% |

The Probe is cheap and slow — ideal for reconnaissance of many star systems in parallel.
At 0.009c (N=1000), a probe fleet surveys nearby stars for only the cost of the sails.

### Expedition Lightsail (r = 4 km, Moderate material, 1,000 kW/m²)

Payload: 50 t, total mass 100 t. Thermally limited above ~5,000 emitters. Entries below
this threshold show power-limited performance (v_NR).

| N Emitters | Power | Aperture | d₀ (AU) | v/c | Trip 4.2 ly | Degradation | Notes |
|------------|-------|----------|---------|-----|-------------|-------------|-------|
| 100 | 1 TW | 1 km | 20.6 | 0.003 | 1,400 yr | <0.1% | Power-limited |
| 1,000 | 10 TW | 3.16 km | 65.1 | 0.017 | 250 yr | <0.1% | Power-limited |
| 5,000 | 50 TW | 7.07 km | 146 | 0.054 | 78 yr | 0.6% | At threshold |
| 10,000 | 100 TW | 10 km | 206 | 0.070 | 60 yr | 1.0% | Thermal-limited |

At N=5,000+, the Moderate material caps acceleration at 3.35 m/s² (0.34g). Above this,
more emitters extend range but don't increase peak thrust.

### Grand Lightsail (r = 12.5 km, Advanced material, 5,700 kW/m²)

Payload: 500 t, total mass 991 t. **Never thermally limited** for arrays below 280,000
emitters. All entries show power-limited performance.

| N Emitters | Power | Aperture | d₀ (AU) | v_NR/c | Corrected v/c | Trip 4.2 ly | Degradation |
|------------|-------|----------|---------|--------|---------------|-------------|-------------|
| 100 | 1 TW | 1 km | 64.4 | 0.0017 | — | 2,500 yr | <0.1% |
| 1,000 | 10 TW | 3.16 km | 203 | 0.010 | — | 440 yr | <0.1% |
| 10,000 | 100 TW | 10 km | 643 | 0.054 | — | 78 yr | 0.6% |
| 100,000 | 1 PW | 31.6 km | 2,034 | 0.30 | ~0.25c | 17 yr | 13% |

### Crewed Interstellar Vessel (Grand Lightsail + Mercury Array)

Reference case: a crewed ship with reaction engines for deceleration, pushed by a full
Mercury laser array.

**Ship**: 10,000 t total (structure, crew, Gamma Converter engine, antimatter fuel for braking)
**Sail**: Grand Lightsail (12.5 km radius, 491 t)
**Total**: 10,491 t
**Array**: Full Mercury (89.7 PW beam, 299 km aperture, 8.97M emitters)

At this scale, the Grand sail IS thermally limited (threshold: 280k emitters, array has
8.97M). The array modulates to ~3% power at launch, ramping up as the ship accelerates.

**Thermal-limited acceleration**: 18.6 MN / 10,491 t = **0.18g** (comfortable for crew).

**Flight profile**:

| Phase | Distance | Duration | Speed | Notes |
|-------|----------|----------|-------|-------|
| Constant acceleration (0.18g) | 0.95 ly | 3.4 yr | 0 → 0.53c | Array ramps from 3% to 100% power |
| Diminishing thrust | 0.55 ly | ~1 yr | 0.53 → ~0.6c | Array at full power, beam diverging |
| Coast | 2.7 ly | ~4.5 yr | 0.6c | Sail integrity: ~85% at arrival |
| Deceleration (Gamma Converter) | at destination | ~5 yr | 0.6c → 0 | Mass ratio 2.0, jettison sail first |
| **Total** | **4.2 ly** | **~14 yr** | | |

The sail operates at **1,000 K** (727°C) throughout the acceleration phase — the full
25 km diameter disk glows dull orange-red.

**With destination laser (highway mode)**: No braking fuel needed. Ship mass drops to
~2,000 t (sail + payload). Acceleration jumps to **0.95g**. The sail must survive with
enough integrity for deceleration (~85% at 0.6c over 4.2 ly — viable).

---

## Energy Economics

### Laser Efficiency

During a launch, the colony's power grid feeds the laser array continuously:

    Colony energy = P_draw × t_total = (N × 15 GW) × t_total

Of this energy:
- 67% becomes coherent beam (wall-plug to beam)
- Of the beam energy, only 2β(1−β)/(1+β) becomes ship KE

**Net efficiency (colony power to ship KE):**

| Ship speed (v/c) | Beam eff. | Photon-to-KE | Net wall-plug-to-KE |
|-------------------|-----------|-------------|---------------------|
| 0.01 | 67% | 2.0% | 1.3% |
| 0.05 | 67% | 9.1% | 6.1% |
| 0.10 | 67% | 16.4% | 11.0% |
| 0.30 | 67% | 32.3% | 21.6% |
| 0.414 | 67% | 34.3% | 23.0% (maximum) |

Peak net efficiency is only 23%. At typical game speeds (0.01-0.05c), only 1-6% of
consumed colony power becomes ship kinetic energy. But the energy source is renewable
solar/fission/fusion power, not a consumable.

### Comparison: Laser Launch vs Antimatter Production

**Example: Getting a 100 t payload to 0.05c**

*Laser sail (Expedition + 5,000 emitters):*
- Power draw: 75 TW continuous
- Firing duration: ~2 days total
- Colony energy consumed: 1.3 × 10¹⁹ J
- Efficiency: ~0.4%

*AM-Cat Fusion drive (same 100 t payload):*
- Total mass: 1,155 t (100t payload + 600t engine + 455t fuel)
- Antimatter needed: 1.1 kg, Mk III production: 4.6 days, energy: 2.0 × 10¹⁸ J
- Efficiency: much higher per joule

The AM-Cat uses ~6× less energy. But:
- The AM ship masses 1,155 t vs 100 t for the sail ship
- Each AM mission requires producing antimatter (days of accelerator time)
- The laser approach costs nothing per mission after the array is built
- The 10th laser launch costs only the sail; the 10th AM launch costs fuel + engine

### Why Laser Sails Despite Low Efficiency

1. **Stranded power has zero opportunity cost during launch windows.** Between launches,
   that power runs industry. During a launch, it's redirected to the array.
2. **No supply chain.** Laser sails require solar farms + emitters. That's it.
3. **Per-mission marginal cost is near zero.** The sail itself is cheap. The 100th
   mission costs the same as the 1st.
4. **Probes are the killer app.** A 1.8 t laser-sail probe costs 300k credits and zero
   fuel. For survey missions to dozens of star systems, laser probes are orders of
   magnitude cheaper than reaction drive alternatives.

---

## Comparison with Reaction Drives

### Reaction Drive Performance

| Drive | Era | Payload | Total Mass | Δv | Trip 4.2 ly |
|-------|-----|---------|------------|-----|-------------|
| Z-Pinch Probe | 6 | 50 t | 1,050 t | 0.029c | 145 yr |
| Z-Pinch Advanced | 7 | 200 t | 4,200 t | 0.073c | 57 yr |
| AM-Cat Fusion | 8 | 200 t | 2,400 t | 0.110c | 38 yr |
| AM Torch | 9 | 200 t | 1,200 t | 0.347c | 12 yr |
| Gamma Converter | 9 | 200 t | 2,200 t | 0.500c | 8.4 yr |

### Laser Sail Performance

| Sail + Array | Era | Payload | Ship Mass | v/c | Trip 4.2 ly | Fuel Cost |
|-------------|------|---------|-----------|------|-------------|-----------|
| Probe + 100 | 5 | 1 t | 1.8 t | 0.003 | 1,400 yr | 0 |
| Probe + 1,000 | 6 | 1 t | 1.8 t | 0.009 | 470 yr | 0 |
| Expedition + 1,000 | 7 | 50 t | 100 t | 0.017 | 250 yr | 0 |
| Expedition + 10,000 | 8 | 50 t | 100 t | 0.070 | 60 yr | 0 |
| Grand + 10,000 | 8 | 500 t | 991 t | 0.054 | 78 yr | 0 |
| Grand + 100,000 | 9 | 500 t | 991 t | 0.25 | 17 yr | 0 |
| Grand + Mercury array | 7+ | 10,000 t | 10,491 t | 0.6 | 14 yr* | 0 |

*Includes deceleration time via Gamma Converter.

### Summary

**Laser sails are best for:**
- Lightweight reconnaissance probes (unmatched cost-per-mission)
- Repeated cargo runs between established colonies (zero marginal fuel cost)
- Inner-system colonies with abundant solar power (Mercury, Venus orbit)
- Bidirectional highways between established colonies (endgame)

**Reaction drives are best for:**
- First contact with new star systems (no infrastructure needed)
- Missions requiring deceleration without a destination laser
- Players who want speed NOW without building colony infrastructure

**The strategic picture**: Laser sails are infrastructure; reaction drives are expeditions.
A mature interstellar civilization uses both — laser highways for routine traffic between
established colonies, reaction drives for exploration at the frontier.

---

## Implementation Notes

### No Body Occlusion

Laser beams are not blocked by celestial bodies. At interstellar distances, the angular
size of any solar system body is negligible. The phased array can steer the beam
regardless of orbital geometry.

### Physics Integration

During **physics warp** (1x-10x): Apply laser thrust as an external force each tick.

    F_vec = F_magnitude × normalize(ship_pos − laser_body_pos)

    F_magnitude = (2P / c) × ((1 − β) / (1 + β)) × min(1, (d₀/d)²) × sail_integrity

where β = v_radial/c (velocity component along the beam direction) and d is the distance
from the laser array to the ship.

Thermal limit: if computed flux > flux_max for the sail material, reduce effective P so
that the flux equals flux_max. This caps F_magnitude at:

    F_max = 2 × flux_max × A_sail × sail_integrity / c

During **on-rails warp** (100x+): Pre-compute the boost trajectory by numerical
integration of the relativistic equation of motion, including thermal limits and
degradation. Transition to Keplerian propagation once thrust drops below 0.1% of peak.

### Sail Deployment

The sail must be **deployed** before it can receive thrust (activation similar to staging).
Once deployed, the sail cannot be stowed — deployment is irreversible. The sail's
orientation automatically tracks to face the active laser source.

If multiple laser arrays are in range, only one can push the sail at a time (the player
selects which). The sail faces the selected array.

### Sail Degradation

Update sail integrity each physics tick:

    integrity -= 0.5 × β² × (Δd / 9.461e15)

where Δd is distance traveled in meters this tick. Clamp integrity to [0, 1]. Display
in the flight HUD alongside ship velocity and orbit info. At integrity = 0, the sail
part is marked as destroyed (no longer provides thrust or renders as deployed).

### Rendering

**Stowed (editor + pre-deployment)**: Small rectangular canister. Rendered with
Interstellar category color.

**Deployed (flight view)**: Large translucent disc with radial spoke lines and concentric
ring pattern. Oriented perpendicular to the laser beam direction. Visible at map-view
zoom levels.

**Thermal glow**: When the laser is active, tint the sail disc to reflect its operating
temperature. The Grand Lightsail at 1,000 K should glow dull orange-red. Scale the glow
intensity with the ratio of current flux to flux_max.

**Degradation visual**: As sail integrity drops, fade the disc opacity and add gaps/holes
to the mesh pattern. Below 50% integrity, the sail is visibly tattered.

**Beam line**: While the laser is active, render a faint dotted or dashed line from the
colony body to the sail, with subtle glow.

### Editor Integration

Lightsail parts display in the parts palette:

- Deployed radius and sail area
- Sail mass and areal density (1 g/m²)
- Material tier and max flux
- "Stowed — deploys in flight" label

The delta-v calculator cannot account for laser thrust (it depends on external
infrastructure), so sail-equipped vessels show "Laser sail — Δv depends on array" in
place of a delta-v number for that component.
