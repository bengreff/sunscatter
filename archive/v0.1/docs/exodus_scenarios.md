# Exodus Mode Scenarios

## Overview

Exodus mode adds an existential threat with a precise countdown to Sunscatter. The player must build the infrastructure to survive before the timer reaches zero.

**Three game modes:**
- **Sandbox**: All tech unlocked, infinite funds, no time pressure
- **Pioneer**: The base game — open-ended exploration, colonization, no threat
- **Exodus**: A specific threat scenario with a countdown timer

**Design principles for Exodus threat scenarios (1-4):**
1. **Exact countdown.** Every threat scenario has a precise timer visible to the player from game start. No ambiguity about when doom arrives.
2. **Discrete events.** When the timer reaches zero (or a milestone), the game applies an instant state change. No gradual simulation of degrading conditions between milestones.
3. **Clear kill zones.** Each threat scenario defines a spatial region that becomes uninhabitable at T=0. You're either in the zone or you're not.
4. **Time warp has cost.** Every year warped is a year off the clock. This is the core tension of Exodus mode.

Scenario 5 (The Expedition) is an exploration scenario with no threat, no countdown, and no kill zone. It is a pure engineering and logistics challenge.

## Scenario Summary

| # | Name | Threat | Kill Zone | Default Countdown | Win Condition |
|---|---|---|---|---|---|
| 1 | The Fall | Asteroid impact | Earth surface | 100 years | Self-sustaining off-Earth colony |
| 2 | The Pyre | Supernova ejecta | ~50 ly sphere around Thyris | 315 years | Colony outside danger zone |
| 3 | The Seed | Black hole in the Sun | Solar system (no sunlight) | 170 years | Interstellar colony |
| 4 | The Cascade | Gamma-ray burst | Cone: ~80 ly wide at Earth, ~5,000 ly long | 500 years | Colony outside beam cone |
| 5 | The Expedition | None (exploration) | None | None | Reach the Laboratory at Sgr A* |

---

## Scenario 1: The Fall

**Threat:** A 120 km interstellar asteroid on a collision course with Earth.
**Default Countdown:** 100 years.
**Kill Zone:** Earth's surface. All complex surface life dies. The solar system is unaffected.
**Win Condition:** At least one self-sustaining colony off Earth (orbital, Lunar, Martian, or beyond).

### Setup

An interstellar body — a 120 km carbonaceous asteroid, mass ~2.3 x 10^18 kg — is detected at ~400 AU, inbound at ~15 km/s. Orbital mechanics confirm collision with Earth in approximately 100 years. The object is too massive to deflect by any conceivable means (deflection would require ~10^26 J — the Sun's total output for one second). The impact date is known to within days from the moment the orbit is confirmed.

### Countdown Events

| Timer | Event | Game Effect |
|---|---|---|
| 100 years | Scenario begins. Asteroid confirmed on collision course. | Countdown starts. No gameplay effects yet. |
| 0 (impact) | Asteroid strikes Earth at ~25 km/s. | Earth becomes uninhabitable. See Execution below. |

No intermediate side effects. Earth is completely normal until impact. The asteroid is invisible to the naked eye until the final weeks.

### Execution (T=0)

The impact delivers ~4.5 x 10^26 J of kinetic energy — 1,000x the Chicxulub event that killed the dinosaurs. Effects are global and nearly instantaneous:

- **Hours 0-3:** Re-entering ejecta heats the upper atmosphere globally. Surface temperatures reach 200-500 C worldwide. Every combustible surface ignites.
- **Hours 3-24:** Global firestorm. The entire land surface burns.
- **Days 1-14:** Impact winter begins. Dust and soot block sunlight. Temperatures plummet to -20 to -40 C.
- **Months 1-12:** Near-total darkness. All surface photosynthesis ceases. Food chains collapse.

**Game effect:** Earth is marked "uninhabitable." Any population/infrastructure on Earth's surface is destroyed. Orbital stations, Lunar colonies, Martian colonies, and anything beyond are unaffected. If the player has a self-sustaining off-Earth colony, they win. If not, game over.

### What Survives

Deep-ocean hydrothermal vent life and subsurface bacteria survive. Earth's biosphere eventually recovers over millions of years. But human civilization on Earth is gone.

---

## Scenario 2: The Pyre

**Threat:** A Wolf-Rayet star 35 light-years from Earth goes supernova. The supernova light is the warning. The ejecta traveling at 0.1c is the extinction.
**Default Countdown:** 315 years (from supernova light arrival to ejecta arrival).
**Kill Zone:** ~50 ly radius sphere centered on the star Thyris. Ejecta at 0.1c delivers enough kinetic energy to superheat planetary atmospheres and sterilize surfaces.
**Win Condition:** At least one self-sustaining colony outside the 50 ly danger zone.

### Setup

**Thyris** is a WN8-type Wolf-Rayet star, 35 ly from Earth. Current mass ~12 solar masses, original mass ~35 solar masses. At apparent magnitude -3 to -4 (brighter than Venus), it is one of the brightest stars in the night sky. It is a **runaway star**, ejected from a distant OB association 2-3 million years ago when its binary companion went supernova.

The scenario begins when Thyris explodes. There is **no advance warning** — the supernova light arrives at Earth at the speed of light, announcing the event the instant it becomes observable. The star was known to be a Wolf-Rayet approaching end-of-life, but the specific collapse date was not predicted.

The supernova light is Event 1: the alert, with moderate damage. The real threat is the ejecta — a shell of superheated plasma expanding at ~0.1c (30,000 km/s). At 35 ly distance, the light arrives in 35 years; the ejecta arrives in 350 years. From Earth's perspective:

**Light arrives (game start) → ejecta arrives 315 years later.**

The ejecta velocity is measurable from the supernova's spectral line broadening within days of the light arriving. The arrival date is calculable immediately.

### Countdown Events

| Timer | Event | Game Effect |
|---|---|---|
| 315 years (game start) | **Event 1: Supernova light arrives.** Thyris brightens to mag -17 (brighter than the full Moon). Gamma and X-ray flash hits atmosphere. | Moderate biosphere damage. See Event 1 below. Countdown to ejecta begins. |
| 315 → 0 (ongoing) | **The Approach.** Ejecta shell visible as an expanding nebula, growing brighter and larger each year. | No additional game effects. The shockwave is visible in the sky, approaching. Its position can be tracked. |
| 0 | **Event 2: Ejecta arrives.** Wall of superheated plasma at 0.1c strikes the solar system. | Kill zone activated. Everything within 50 ly of Thyris is destroyed. See Event 2 below. |

### Event 1: The Flash (game start)

The supernova light arrives all at once. Thyris brightens to apparent magnitude -17 to -19 and remains visible for weeks. UV, X-ray, and gamma radiation bombard the upper atmosphere.

**Effects (moderate — survivable but damaging):**
- Ozone depletion: 40-50% globally within weeks. UV-B at the surface increases ~2x.
- Agriculture yields reduced. Exposed crops suffer DNA damage. Sunburn risk increases significantly.
- Phytoplankton stressed (marine food chain pressured but not collapsed).
- Cosmic ray flux increases ~100x over the following decades, sustaining ozone damage.

**Game effect:** Earth receives the "irradiated (moderate)" modifier. Agriculture yields reduced by ~30%. Outdoor operations have penalties. This is NOT extinction — it is pressure. Civilization continues but under worsened conditions. This modifier persists for the remainder of the game (cosmic rays sustain the damage for centuries).

This is the alert. Humanity now knows the ejecta is coming and has 315 years.

### Event 2: The Wall (T=0)

The ejecta front arrives — a shell of superheated plasma, heavy elements, and cosmic ray particles moving at 0.1c (30,000 km/s). At 35 ly, even the leading edge of the ejecta (~1% of the total ejected mass, ~0.1 solar masses) delivers ~65,000 kJ/m^2 of kinetic energy to anything in its path.

**Effects (extinction — unsurvivable on any planet surface):**
- The ejecta impacts the atmosphere at 0.1c. Kinetic energy deposited into the atmospheric column heats it by ~hundreds of degrees. The upper atmosphere superheats.
- Surface temperatures spike globally from atmospheric thermal radiation.
- Ozone is not merely depleted — the atmosphere is physically disrupted. Toxic nitrogen compounds saturate the air.
- Lethal radiation dose at the surface from secondary particles.
- Heavier, slower ejecta follows over subsequent decades, continuing the bombardment.

**Game effect:** All colonies within the ~50 ly kill zone are marked "destroyed." This is not "damaged" or "irradiated" — it is physically destroyed. Atmospheres are wrecked. Surfaces are sterilized. Colonies dependent on any planetary surface within 50 ly of Thyris are gone.

The kill zone is a sphere centered on Thyris, not on Earth. The ejecta expands radially in all directions. Systems closer to Thyris are hit first; systems at 50 ly are hit last (500 years after the supernova vs 350 years for systems at 35 ly). For game purposes, each system within 50 ly has its own ejecta arrival timer based on its distance from Thyris.

### Watching the Approach

The ejecta shell is visible. It is an expanding cloud of superheated gas, glowing in optical and infrared wavelengths. As it sweeps through interstellar space, it compresses and heats the interstellar medium, creating a visible shock front. From Earth, it appears as a growing, brightening nebula centered on the supernova remnant, expanding at a measurable rate.

Each year, the shell is ~0.1 ly closer. Humanity can watch it approach — track its position, refine arrival estimates, observe it engulf closer star systems. This creates unique dramatic tension: the threat is not abstract. It is visible in the sky, growing larger every year.

---

## Scenario 3: The Seed

**Threat:** A primordial black hole inside the Sun, consuming it from the core outward.
**Default Countdown:** 170 years.
**Kill Zone:** The entire solar system. When the Sun is consumed, all heat and light cease. Everything orbiting the remnant black hole freezes to -240 C within weeks.
**Win Condition:** At least one self-sustaining colony in another star system.

### Setup

A primordial black hole (PBH) — formed in the first second after the Big Bang, mass ~10^22 kg (roughly the mass of Ceres), Schwarzschild radius ~50 micrometers — was captured by the Sun and has settled in its core. It has been accreting solar matter via Bondi accretion for approximately 30 years before the scenario begins.

Helioseismology (analysis of the Sun's oscillation modes) detects an anomalous sound-speed deviation in the inner 5% of the solar radius. Follow-up observations — neutrino spectral distortion, repeated helioseismic measurements showing the anomaly is growing — lead to the diagnosis: a black hole in the Sun's core.

The accretion rate follows dM/dt proportional to M^2, giving the exact formula:

**M(t) = M_0 / (1 - alpha * M_0 * t)**

This has a finite-time singularity — the Sun is consumed at a precise, calculable date. Two mass measurements at different times determine M_0 and alpha, and the death date falls directly out of the equation. The countdown is mathematically exact.

**Countdown = 170 years from scenario start (200 years total from BH capture, 30 already elapsed).**

### Countdown Events

The Sun changes as the BH grows. Because 90% of the mass is consumed in the last 10% of the timeline, the effects are heavily back-loaded.

| Timer | Event | Game Effect |
|---|---|---|
| 170 years | Scenario begins. BH diagnosed, death date calculated. | Countdown starts. |
| 50 years | Solar luminosity +0.3%. | Negligible. Measurable by instruments only. |
| 20 years | Solar luminosity +2%. Sun measurably larger. | Mild warming: Earth temperature +0.5 C. Minor gameplay effect. |
| 5 years | Solar luminosity +20%. Sun visibly different — larger and slightly redder. | Significant warming. Polar ice melting. Agriculture stressed. |
| 1 year | Solar luminosity +500%. Sun dramatically swollen. | Severe overheating. Earth surface temperatures rising fast. Evacuation urgency. |
| 1 month | Wild luminosity swings. Sun visibly collapsing. | Catastrophic. Solar output unstable. |
| 0 | Sun consumed. Brief luminous transient (days), then darkness. | Solar system uninhabitable. See Execution. |

### Execution (T=0)

The remaining solar material falls into the BH over days to weeks. A brief luminous transient (10^6 to 10^9 times normal solar luminosity) flares and fades. Then: darkness.

The end state is a ~1 solar mass black hole. The planets continue orbiting it — their orbits are unchanged (same central mass). But there is no light and no heat. Earth's surface temperature drops to -240 C within weeks. The oceans freeze solid within months.

**Game effect:** The solar system is marked "dark." All colonies dependent on solar energy or surface conditions become uninhabitable. Any interstellar colony in another star system is unaffected. If the player has a self-sustaining interstellar colony, they win. If not, game over.

### Side Effect Implementation

The Seed is the only scenario with significant pre-event side effects. These should be implemented as discrete state changes at the milestone timers listed above:
- At 20 years remaining: apply "mild warming" modifier to Earth
- At 5 years remaining: apply "severe warming" modifier
- At 1 year remaining: apply "catastrophic" modifier (agriculture yield drops, population stress)
- At 0: apply "dark" — full extinction

Between milestones, conditions are stable. No gradual simulation needed.

---

## Scenario 4: The Cascade

**Threat:** A gamma-ray burst from the death of one of the most massive stars in the galaxy. Earth and hundreds of other star systems lie in the beam path.
**Default Countdown:** 500 years.
**Kill Zone:** A cone originating at Acheron (1,100 ly away), half-angle ~2 degrees, lethal out to ~5,000 ly from the source. At Earth's distance the cone is ~80 ly wide. Every biosphere in the cone is destroyed.
**Win Condition:** At least one self-sustaining colony outside the beam cone.

### Setup

**Acheron** is a ~320 solar mass star — a luminous blue variable / Wolf-Rayet hybrid, one of the most massive stars in the Milky Way. It sits approximately 1,100 light-years from Earth in a young OB association. Luminosity: ~8 million solar luminosities. Surface temperature: ~50,000 K. It has been burning through its hydrogen in approximately 2 million years and is approaching the end of its life.

At this mass, Acheron's helium core exceeds 133 solar masses — above the pair-instability supernova range. When it collapses, photodisintegration (gamma-ray photons breaking iron nuclei back into helium) absorbs the energy that would otherwise unbind the star. The core collapses directly into a black hole. With sufficient angular momentum, an accretion disk forms and launches relativistic jets: a gamma-ray burst.

Acheron is observed entering its terminal carbon-burning phase. For a star of this mass, carbon burning lasts ~500 years. The scenario begins when stellar models predict the remaining burning time.

Because both the observational light and the eventual GRB travel at the same speed (c), the countdown to GRB arrival equals the observed remaining burning time:

**Countdown = (observed remaining stellar lifetime) = 500 years.**

Spectropolarimetry and circumstellar nebula geometry determine Acheron's rotational axis — and thus the jet direction. Measurements confirm Earth's region lies within the beam cone (half-angle ~2 degrees). At 1,100 ly from Acheron, this cone is ~80 ly in diameter.

### Countdown Events

| Timer | Event | Game Effect |
|---|---|---|
| 500 years | Scenario begins. Acheron confirmed entering final carbon burning. Beam path includes Earth. | Countdown starts. |
| ~2 years | Neon burning observed (distinctive spectral shift). Countdown precise to months. | No direct effect. Urgency increases. |
| ~1 year | Oxygen burning observed. | No direct effect. |
| ~1 day | Silicon burning detected via neutrino spike. | Final warning. GRB arrives tomorrow. |
| 0 | Core collapse. GRB propagates at lightspeed. | Beam cone sterilized. See Execution. |

No side effects on Earth before T=0. Acheron's eruptions (luminous blue variable outbursts) are visible through telescopes but cause no damage at 1,100 ly.

### Execution (T=0)

The GRB delivers ~240,000 kJ/m^2 of gamma-ray fluence at Earth's distance (1,100 ly). The beam travels at lightspeed — there is zero interval between "it fired" and "it hit." Every system in the cone is struck simultaneously from its own reference frame.

This is **~4x more energy per square meter than The Pyre's ejecta** (65,000 kJ/m^2), and it is delivered as a concentrated electromagnetic pulse rather than a physical shockwave. The effects are more severe:

1. **Ozone annihilation (minutes):** The gamma-ray flux is so intense that nitrogen oxide production destroys 99%+ of the ozone layer within minutes, not hours. This is irreversible on any human timescale.
2. **Atmospheric heating (seconds to minutes):** At 240,000 kJ/m^2, the energy deposited in the upper atmosphere heats the stratosphere by hundreds of degrees. Unlike The Pyre's ejecta (which passes through), this energy is absorbed in place, creating a superheated atmospheric layer that radiates downward.
3. **Secondary radiation at ground level (during burst):** The extreme gamma flux generates electromagnetic particle cascades in the atmosphere. Secondary muons and other particles penetrate to ground level, delivering radiation doses that are dangerous even indoors. Deep underground shelters survive; surface buildings may not provide sufficient shielding during the burst itself.
4. **UV sterilization (years to decades):** Without ozone, solar UV-C reaches the surface at full lethal intensity. This is not a brief pulse — it persists for **5-15 years** until ozone chemistry slowly recovers. Every day of sunlight is a day of sterilizing radiation. All exposed plant life, phytoplankton, and surface organisms die. The marine and terrestrial food chains collapse completely.
5. **NOx haze (years):** Massive nitrogen dioxide production creates a brown haze blocking 50-70% of visible sunlight. Global temperatures plummet (impact-winter-like cooling) simultaneously with lethal UV — a combination of freezing cold and sterilizing radiation.

### Why The Cascade Is Worse Than The Pyre

The Pyre's ejecta is a **physical blow that passes.** The shell of plasma hits, deposits energy over minutes, and moves on. The atmosphere is heated and damaged, but the assault ends. A deep shelter could theoretically ride it out.

The Cascade's GRB inflicts a **chemical wound that keeps killing for a decade.** The gamma pulse is seconds long, but the ozone destruction it causes exposes the planet to lethal UV for 5-15 years continuously. There is no "riding it out" — you would need to remain underground with complete life support for over a decade, then emerge to a dead biosphere with no food chain, no agriculture, and toxic atmospheric chemistry. The sustained duration makes the Cascade categorically worse than the Pyre's instantaneous blow.

**Game effect:** All colonies inside the beam cone are marked "sterilized." This is permanent. Colonies outside the cone are completely unaffected — the beam is sharply defined. The cone is visible on the star map as a geometric kill zone.

**For implementation simplicity:** All systems within the beam cone are treated identically — sterilized at T=0. The actual fluence varies with distance from Acheron (higher near the source, lower further away), but it exceeds the lethal threshold everywhere within the cone out to ~5,000 ly. For game purposes, inside the cone = dead.

### Cone Geometry

The kill zone is a **cone**, not a cylinder. It originates at Acheron and expands outward with half-angle ~2 degrees:

| Distance from Acheron | Cone Diameter | Fluence (kJ/m^2) | Notes |
|---|---|---|---|
| 500 ly | ~35 ly | ~960,000 | Near source. Total sterilization. |
| 1,100 ly (Earth) | ~77 ly | ~241,000 | ~4x worse than The Pyre's ejecta |
| 2,000 ly | ~140 ly | ~73,000 | Comparable to Pyre ejecta |
| 3,000 ly | ~210 ly | ~32,000 | Still devastating |
| 5,000 ly (far edge) | ~350 ly | ~12,000 | Still 12x mass-extinction threshold |

The cone contains hundreds to thousands of star systems depending on stellar density along the beam path. The player must establish colonies **perpendicular to the beam axis** to escape the cone. At Earth's position, moving ~40 ly laterally (sideways relative to the beam direction) exits the cone.

The cone extends far beyond 5,000 ly at diminished intensity, but for game purposes, **5,000 ly from Acheron** is the hard boundary of the kill zone. Systems beyond this distance are safe.

---

## Scenario 5: The Expedition

**Threat:** None (exploration scenario).
**Default Countdown:** None.
**Kill Zone:** None.
**Win Condition:** Reach the Laboratory at Sagittarius A*, establish contact with the thinking ocean, begin the galactic survey.

> Full narrative design document: `docs/expedition_narrative.md`

### Setup

In 2031, the Square Kilometre Array detects a repeating radio source at 1.42 GHz (the hydrogen line) with a period of 174.8 days, originating from 13 AU from Sagittarius A* — the 4.15-million-solar-mass supermassive black hole at the center of the Milky Way. The signal has three properties that match no known astrophysical process: extraordinary period stability (parts per billion over a decade), a clean band-limited square wave pulse profile, and gravitational lensing consistent with a source deep inside the S-star cluster.

Sgr A*'s gravitational field amplifies the signal by a factor of ~10^4. Without that amplification, the source would be undetectable at 26,000 light-years. Something placed a transmitter where the black hole's gravity would broadcast it across the galaxy.

The confirmation comes from archival geology: a 174.8-day magnetic oscillation extracted from 2.6-billion-year-old banded iron formations in Western Australia. The same signal, recorded in stone. Whatever is broadcasting has been broadcasting for at least 2.6 billion years.

There is no countdown. The beacon's power source — unipolar induction from a metallic planetary core moving through Sgr A*'s magnetic field at 0.05c — requires no fuel. It will operate for millions of years yet. The motivation is not urgency but discovery: something built a permanent beacon at the most visible location in the galaxy, and it has been calling for longer than complex life has existed on Earth.

### What's There

The source is a super-Earth (3.2 M⊕, 1.4 R⊕, 1.6g) — later named **Crucible** — in a 175-day orbit at 13 AU from Sgr A*. It was placed there by an ancient civilization (the Builders) who used Sgr A* as a gravitational lens telescope — the most powerful observatory possible — to survey life across the galaxy.

The Builders found intelligence on many worlds and discovered a pattern: every civilization was limited by the biology that produced it. Single-origin biochemistry constrains what minds can think. They hypothesized that combining multiple independent biochemistries could break through this ceiling.

Approximately 150 million years ago, the Builders seeded the planet's deep ocean with organisms from 14 independent origins of life, each with a distinct genetic code and molecular architecture. They maintained the experiment through engineered infrastructure: mass drivers for orbital correction, resonance shepherd satellites for tidal heating, volatile reservoirs for atmospheric restoration, and hydrothermal vent networks for the deep biosphere.

Approximately 30-50 million years ago, an Sgr A* accretion flare killed the Builders. Their surface infrastructure was destroyed. But the deep ocean — shielded by kilometers of water and ice — survived. The experiment continued, unattended, for 30-50 million years.

In that time, the experiment produced what the Builders were searching for, in three stages:

1. **Self-directed evolution.** Fourteen competing biochemistries created selection pressure so extreme that organisms evolved the ability to direct their own mutations — targeted variation where needed, protected by error-correction from multiple lineages.

2. **Programmable molecular machinery.** Self-directed evolution across 14 substrates produced molecules that are not specialized but reconfigurable — general-purpose molecular tools that can catalyze any thermodynamically feasible reaction and interface with any carbon-based biology.

3. **Distributed cognition.** Programmable molecules enabled cross-lineage communication. The ocean's ecosystem became a distributed information-processing system — kilometers across, trillions of nodes, thinking in 14 molecular languages simultaneously. The ocean thinks. Slowly, but with computational breadth no centralized brain could achieve.

The Builders designed an experiment to create something beyond intelligence. They died 30-50 million years before it succeeded. The ocean has been thinking, alone, ever since.

### The Challenge

The source lies 26,000 light-years from Earth. There is no time pressure, but the distance demands the full arc of the tech tree:

- **Phase 1 — Local infrastructure.** Fusion drives at 0.05-0.1c enable colonization of nearby systems. Critical milestone: self-sustaining colonies that can manufacture ships and launch the next generation independently.
- **Phase 2 — Laser sail networks.** Dyson-scale energy infrastructure at established colonies powers laser arrays that accelerate sail-equipped vessels to 0.2-0.3c. Each waypoint colony becomes a node in a launch-and-brake network.
- **Phase 3 — Deep push.** Antimatter propulsion and relay stations push cruise velocities to 0.3-0.5c across thousands of light-years through the galactic bulge, where stellar density increases dramatically.

The journey requires a chain of waypoint colonies, each a self-sustaining civilization serving as fuel depot, relay station, and launch platform. The galactic center presents additional hazards: increasing stellar density, radiation, and gravitational perturbations from Sgr A*.

### Win Condition

Reach the Laboratory. Establish contact with the thinking ocean. Begin the galactic survey — combining the gravitational lens observatory with the ocean's distributed cognition to continue the Builders' search for life across the galaxy.

The Builders searched for a mind that could transcend the limitations of any single biology. They built an experiment to create one. They died before it succeeded. Humanity arrives 30 million years late and finds the answer waiting in the dark water — thinking, alone, ready to talk.

---

## Physics Reference

### Asteroid Impact (The Fall)

**Impactor:** 120 km diameter, density ~2,500 kg/m^3 (C-type carbonaceous), mass ~2.3 x 10^18 kg.

**Kinetic energy at 25 km/s:** KE = 0.5 * 2.3e18 * (25,000)^2 = 7.2 x 10^26 J.
- 1,000x the Chicxulub impact (10 km, ~4.2 x 10^23 J)
- ~7 trillion Hiroshima bombs
- Equal to ~2 seconds of total solar luminous output
- ~1/300,000 of Earth's gravitational binding energy (planet remains intact)

**Crater:** ~1,000-1,500 km diameter. Punches through Earth's crust (30-70 km) into the upper mantle. Comparable in area to western Europe.

**Ejecta and thermal pulse:** Trillions of tonnes of molten rock launched on ballistic arcs. Re-entering debris heats the upper atmosphere globally to incandescence within 1-3 hours. Radiant flux at 120 km scale is 10-100x the Chicxulub estimate of ~10 kW/m^2. Surface temperatures reach 200-500 C worldwide. All combustible material ignites.

**Impact winter:** Soot from the global firestorm plus dust from the crater block sunlight. Near-total darkness for months to years. Surface temperatures -20 to -40 C even in the tropics.

**Greenhouse aftermath:** After dust settles (years), CO2 from vaporized carbonate rock drives a +10-20 C greenhouse spike lasting centuries.

**Survival:** No complex surface life survives the thermal pulse + firestorm + winter sequence. Deep-ocean vent communities survive (independent of sunlight). Subsurface microbes unaffected. A sealed deep bunker with years of provisions is theoretically survivable but occupants emerge to a dead biosphere.

**Detection:** At 120 km diameter, the asteroid has absolute magnitude ~6-7. Visible at Jupiter distance with amateur telescopes. Easily detected at hundreds of AU with survey telescopes. At 15 km/s approach from 400+ AU, warning time is ~100 years. Orbital mechanics give an exact impact date within months of first observation.

**Deflection:** Impossible. No foreseeable technology can deliver 10^26 J to the asteroid. The DART mission deflected a 160 m body. Scaling to 120 km requires a mass ratio increase of ~10^12.

---

### Supernova Physics (The Pyre)

**Thyris — stellar properties:**
- Type: WN8 Wolf-Rayet (late nitrogen-sequence, with residual hydrogen)
- Current mass: ~12 solar masses
- Initial mass: ~35 solar masses (lost >60% via stellar winds)
- Luminosity: ~200,000 solar luminosities
- Surface temperature: ~35,000 K
- Apparent magnitude at 35 ly: -3 to -4 (brighter than Venus)
- Runaway star ejected from a distant OB association ~2-3 Myr ago by companion supernova

No real Wolf-Rayet stars exist within 100 ly of Earth. The nearest is Gamma Velorum at ~1,100 ly. Thyris is a fictional runaway — this is astrophysically plausible as WN8 stars are disproportionately runaways.

**Supernova type:** Type Ib (hydrogen-stripped, helium retained). No gamma-ray burst. Approximately spherically symmetric explosion.

**Energy budget:**
- Total: ~3 x 10^46 J (99% in neutrinos)
- Electromagnetic radiation: ~10^42 J (UV, visible, X-ray, gamma — Event 1)
- Ejecta kinetic energy: ~10^44 J (Event 2 — the real killer)

**Two-event structure — the physics:**

**Event 1 — Electromagnetic radiation (arrives at c, travel time = 35 years):**

Gamma and X-ray photons convert N2 and O2 in the stratosphere into nitrogen oxides (NOx), which catalytically destroy ozone. At 35 ly:
- Ozone depletion: 40-50% globally within weeks (Gehrels et al. 2003)
- UV-B increase: ~2x at the surface
- Cosmic ray flux increases ~100x over following decades, sustaining depletion
- Moderate biosphere damage: agriculture stressed, phytoplankton decline, cancer rates increase
- NOT an extinction event at this distance — civilization continues under degraded conditions

**Event 2 — Ejecta front (arrives at ~0.1c, travel time = 350 years):**

A core-collapse supernova ejects 5-15 solar masses of material. The fastest fraction (~1% of ejected mass, ~0.1 solar masses = 2 x 10^29 kg) is accelerated by the shock to ~0.1c (30,000 km/s). This leading edge arrives 350 years after the explosion, or **315 years after the light**.

**Ejecta energy at 35 ly:**
- Shell radius at 35 ly: r = 3.31 x 10^17 m
- Shell surface area: 4*pi*r^2 = 1.38 x 10^36 m^2
- Fast ejecta mass (~0.1 solar masses): 2 x 10^29 kg
- Surface mass density: 2e29 / 1.38e36 = 1.45 x 10^-7 kg/m^2
- Kinetic energy flux: 0.5 * 1.45e-7 * (3e7)^2 = **~65,000 kJ/m^2**

This is comparable to a gamma-ray burst at 1,100 ly. But the delivery mechanism is different — this is physical material impacting the atmosphere at 10% of lightspeed.

**Atmospheric heating from ejecta:**
- Earth's atmosphere has a column mass of ~10,000 kg/m^2
- Energy deposited per unit atmospheric mass: 6.5e7 J / 1e4 kg = ~6,500 J/kg
- Atmospheric temperature increase: delta-T = E / c_p = 6,500 / 1,000 = **~6.5 C average**
- But this average is misleading — the energy is deposited primarily in the upper atmosphere, which heats to extreme temperatures. The resulting thermal radiation cooks the surface.
- Additionally: the ejecta contains radioactive isotopes (Fe-60, Al-26), delivering sustained radiation for decades
- Heavier, slower ejecta (the bulk at 0.01-0.03c) follows over the next centuries, extending the bombardment

**Ejecta kill zone (where ejecta is lethal):**
Energy flux scales as 1/r^2 from the supernova. At 35 ly the flux is ~65,000 kJ/m^2. At other distances:

| Distance from Thyris | Ejecta energy flux | Arrival (years after SN) | Effect |
|---|---|---|---|
| 10 ly | ~800,000 kJ/m^2 | 100 yr | Total surface sterilization |
| 20 ly | ~200,000 kJ/m^2 | 200 yr | Total surface sterilization |
| 35 ly | ~65,000 kJ/m^2 | 350 yr | Atmosphere superheated, surface sterilized |
| 50 ly | ~32,000 kJ/m^2 | 500 yr | Severe — lethal surface radiation + heating |
| 75 ly | ~14,000 kJ/m^2 | 750 yr | Damaging but potentially survivable with shielding |
| 100 ly | ~8,000 kJ/m^2 | 1,000 yr | Survivable with preparation |

For game purposes, 50 ly is the kill zone boundary. Beyond 50 ly, the ejecta is still dangerous but not an automatic extinction.

**The ejecta is visible.** As the shell expands through interstellar space, it sweeps up and compresses the interstellar medium, creating a visible shock front — a supernova remnant. From Earth, this appears as a growing, brightening nebula expanding away from the supernova site. Its angular size increases measurably each year. When it engulfs a nearby star, the event is visible. Humanity watches the wall approach for 315 years.

**Arrival timing for different systems:** The ejecta is a spherical shell. It hits systems at different times based on their distance from Thyris. Systems closer to Thyris are hit first. This creates a rolling wavefront — systems wink out one by one as the shell passes through. For each system within 50 ly, the arrival time is simply: distance / 0.1c.

**Heliosphere interaction:** The ejecta at 0.1c vastly exceeds the solar wind's ability to deflect it. The heliosphere is crushed and overrun in seconds. It provides zero meaningful protection.

---

### Primordial Black Hole Physics (The Seed)

**Primordial black holes (PBHs):** Formed from density fluctuations in the first second after the Big Bang. Unlike astrophysical BHs (which require >3 solar masses from stellar collapse), PBHs can have any mass. PBHs above ~5 x 10^11 kg are stable against Hawking evaporation.

PBHs remain viable dark matter candidates in the "asteroid-mass window" (~10^17 to 10^22 kg). They are not ruled out observationally in this range.

**The Seed's PBH:** Mass ~10^22 kg (~5 x 10^-3 Earth masses, ~mass of Ceres). Schwarzschild radius ~50 micrometers. Emits no detectable radiation.

**Capture and settling:** The PBH, traveling at ~200 km/s, transited the Sun and lost kinetic energy via dynamical friction (gravitational interaction with solar plasma). After repeated transits on a decaying orbit, it settled to the Sun's core over ~10^4 years. The PBH was captured long before the game timeline begins.

**Bondi accretion formula:**

```
dM/dt = 4*pi*lambda * (G * M_BH)^2 * rho / c_s^3
```

Where (solar core values):
- rho = 1.5 x 10^5 kg/m^3 (core density)
- c_s = 5.1 x 10^5 m/s (core sound speed)
- lambda = 0.25 (adiabatic accretion parameter)
- G = 6.674 x 10^-11 m^3/(kg s^2)

This gives: **dM/dt = 1.58 x 10^-32 * M_BH^2 kg/s**

**M^2 scaling = hyperexponential growth.** The solution:

```
M(t) = M_0 / (1 - alpha * M_0 * t)
```

Where alpha = 1.58 x 10^-32 kg^-1 s^-1.

**Time to singularity:** t = 1 / (alpha * M_0)
- For M_0 = 10^22 kg: t = 6.33 x 10^9 seconds = **~200 years**

**90% of the Sun is consumed in the last 10% of the timeline.** The BH spends 180 years being nearly invisible, then 20 years showing obvious signs, then 1-2 years in catastrophic collapse.

**Accretion rate at different masses:**
| BH Mass | dM/dt | Fraction of solar luminosity |
|---|---|---|
| 10^22 kg (start) | 1.6 x 10^12 kg/yr | 10^-8 (undetectable) |
| 10^23 kg (20 yr left) | 1.6 x 10^14 kg/yr | 10^-4 (helioseismic anomaly) |
| 10^24 kg (2 yr left) | 1.6 x 10^16 kg/yr | 0.01 (visible luminosity change) |
| 10^25 kg (73 days left) | 1.6 x 10^18 kg/yr | 1.0 (equals solar luminosity) |
| 10^26 kg (7 days left) | 1.6 x 10^20 kg/yr | 100 (catastrophic) |

**Detection methods (ranked by earliest detection):**
1. **Helioseismology:** The Sun's oscillation modes are sensitive to core sound speed. A ~0.1% sound-speed anomaly in the inner 5% of the solar radius is detectable. First signal at BH mass ~10^22 kg.
2. **Neutrino flux:** B-8 neutrinos scale as T^18. Even a 1% core temperature increase changes the neutrino spectrum detectably. Second signal.
3. **Solar luminosity/radius:** Delayed by the Sun's thermal diffusion time. Detectable only in the last few decades.

**End state:** The Sun does not explode. There is no thermonuclear detonation (the Sun's hydrogen/helium don't detonate). The core material falls into the BH. A brief luminous transient (days to weeks) occurs as the final material spirals in. Then: a ~1 solar mass black hole. Planets continue orbiting. Temperature at Earth: -240 C within weeks.

---

### Gamma-Ray Burst Physics (The Cascade)

**Acheron — stellar properties:**
- Initial mass: ~320 solar masses (one of the most massive stars possible)
- Current mass: ~280 solar masses (wind losses)
- Type: Luminous blue variable / WN-type Wolf-Rayet
- Luminosity: ~8 million solar luminosities
- Temperature: ~50,000 K
- Distance: ~1,100 ly from Earth
- Lifetime: ~2 million years total. Currently in terminal phase.

Stars above ~260 solar masses (helium core >133 solar masses) skip pair-instability supernova. Photodisintegration of iron nuclei absorbs the energy that would unbind the star. The core collapses directly to a black hole. With angular momentum, an accretion disk + relativistic jet = GRB. This is above the "pair-instability gap" (65-133 solar mass helium cores, corresponding to ~130-250 solar mass initial masses, which are completely destroyed by pair-instability supernovae with no remnant and no jet).

**GRB properties:**
- Beamed gamma-ray energy: ~10^51 ergs = 10^44 J (the "standard energy reservoir" from Frail et al. 2001 — observed GRBs cluster around this value regardless of apparent brightness)
- Jet half-angle: ~2 degrees
- Duration of burst: seconds to minutes

**Beam cone geometry:**
- Beam solid angle: Omega = 2*pi*(1 - cos(2 deg)) = 0.00383 sr
- Cone radius at distance d: r = d * tan(2 deg)
- At 1,100 ly: diameter ~77 ly. At 5,000 ly: diameter ~350 ly.
- Number of star systems in cone at Earth's distance: dozens to hundreds

**Energy fluence calculation (corrected):**
- Beam cross-sectional area at distance d: A = Omega * d^2
- At 1,100 ly: A = 0.00383 * (1.04 x 10^19 m)^2 = 4.14 x 10^35 m^2
- Fluence: F = E_beamed / A = 10^44 J / 4.14 x 10^35 m^2 = **~241,000 kJ/m^2**

**Fluence falls as 1/r^2 within the beam** (the cone's cross-section grows as r^2, spreading the energy over a larger area). Fluence at various distances:

| Distance from Acheron | Fluence (kJ/m^2) | vs Pyre ejecta (65,000) | Effect |
|---|---|---|---|
| 500 ly | ~960,000 | 15x | Total atmospheric destruction |
| 1,100 ly (Earth) | ~241,000 | 3.7x | Complete biosphere sterilization |
| 2,000 ly | ~73,000 | 1.1x | Comparable to Pyre ejecta |
| 3,000 ly | ~32,000 | 0.5x | Surface sterilization |
| 5,000 ly | ~12,000 | 0.18x | 99%+ ozone loss, mass extinction |
| 10,000 ly | ~2,900 | — | 98%+ ozone loss |
| 17,000 ly | ~1,000 | — | Near-sterilization threshold |

**For game purposes, all systems within the cone out to 5,000 ly are treated as sterilized.** The fluence varies by ~80x across this range (960,000 at 500 ly vs 12,000 at 5,000 ly), but all values are far above the mass-extinction threshold of ~100 kJ/m^2. The variation doesn't matter gameplay-wise — everything in the cone dies.

**Ozone-destruction thresholds** (Thomas et al. 2005):
| Fluence (kJ/m^2) | Ozone depletion | Severity |
|---|---|---|
| 10 | 68% | Severe |
| 100 | 91% | Mass extinction |
| 1,000 | 98% | Near-sterilization |
| 10,000+ | 99%+ | Complete biosphere destruction |

**Atmospheric effects at 241,000 kJ/m^2 (Earth):**
- Energy deposited in atmospheric column (~10,000 kg/m^2): 241,000 kJ / 10,000 kg = **~24 kJ/kg**
- Average atmospheric temperature increase from energy deposition: **~24 C**
- But the energy is concentrated in the upper atmosphere (where gamma absorption occurs), heating the stratosphere by **hundreds of degrees**
- By comparison, The Pyre's ejecta deposits ~6.5 kJ/kg — the Cascade delivers ~4x more energy

**Cascade vs Pyre — why the Cascade is worse:**
The Pyre's ejecta (65,000 kJ/m^2 at 35 ly) is a physical shockwave — kinetic energy from plasma at 0.1c. It heats the atmosphere, sterilizes the surface, and passes through in minutes. The damage is severe but the assault ends.

The Cascade's GRB (241,000 kJ/m^2 at 1,100 ly) delivers more energy AND inflicts sustained damage:
- 4x more energy deposition → more atmospheric heating, more NOx production, more complete ozone destruction
- The ozone destruction persists for 5-15 years. Every day of sunlight is a day of lethal UV-C at the surface.
- The Pyre's damage is concentrated in a brief window (ejecta transit). The Cascade's damage is sustained for a decade+.
- The NOx haze simultaneously blocks visible light (cooling) while UV sterilizes — a uniquely lethal combination of cold and radiation

**GRB travel speed:** Lightspeed. Zero warning between emission and arrival. Every system in the cone is struck simultaneously from its own reference frame. The beam cannot be outrun, redirected, or detected before it arrives.

**Countdown mechanism:** The same as for any massive star — monitoring late nuclear burning stages. Both observational data and the GRB travel at c, so the observed remaining stellar lifetime equals the time until GRB arrival. For Acheron, carbon burning is predicted to last ~500 years, giving a 500-year countdown.

**Rotational axis measurement:** ~8-10 degree precision via spectropolarimetry. With a jet half-angle of ~2 degrees, early measurements cannot definitively confirm Earth is in the beam. For gameplay, the beam path is treated as confirmed from scenario start (additional measurement methods — circumstellar nebula geometry, binary orbital plane — have refined the axis).
