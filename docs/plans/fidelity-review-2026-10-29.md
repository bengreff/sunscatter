# Fidelity review: the ship model, 2026-10-29

> **Status:** review (FIDELITY agent, for the owner and the lead). The owner asked: "Is the ship model at an appropriate component-level fidelity? Brainstorm situations the current model might not realistically simulate. We need a good foundation." Read: D100–D116 and the decisions they cite (D032, D033, D061, D065, D066, D077–D079, D081–D083), `design/ship-model.md`, `features/{ship-systems,falcon9,parts}.md`, `open-issues.md`, `architecture.md`. Every "model now" below was checked in `crates/sim/src` (file references given); nothing was built or run.

**Verdict.** The split into parts, components and layers (D100) is at the right level, and in places beyond RO/RealFuels: each engine is its own state machine, each tank has its own pressurant, NPSP and settling, and each joint carries the exact rigid-body load. The weakness is not how finely things are split but how they are **coupled**. Several pieces of component state exist but feed nothing that should read them:

- a valve's position does not change any flow;
- where the liquid sits does not move the centre of mass;
- the sensors do not feed SAS;
- a dead bus does not stop hardware that is already running;
- a component's power draw does not heat its part;
- the feed pressure does not set a pressure-fed engine's thrust.

Some state is also kept twice (the mass, the heat of a tank wall), and the split of a vessel's state at a separation copies whatever it does not know about. Fix the couplings and the partitioning before adding more parts or failure modes, or every later feature inherits them.

## 1. The model as built

Status words per CLAUDE.md rule 7. "Lumped" means modelled as one value where the real thing has structure.

| Subsystem | Component level (built) | Lumped | Missing | Status | Code |
|---|---|---|---|---|---|
| Propulsion, engines | One state machine per engine (off → starting → running → tail-off → off/failed). Ignitions are counted (ground TEA-TEB on the pad). Steady map F = τF_vac − pAₑ. Each engine sits at its own mount and has its own feeds. Gas ingestion and cavitation stop it; flameout when dry; the pad's release rule; D114's throttle-close. | Gimbal: one deflection for all engines, at the thrust-weighted mount, with the smallest range. Mixture ratio fixed. Throttle reaches its target in the same tick (`step_engine`: `Running => target`). | Per-engine gimbal; roll by differential gimbal (a test asserts none: `vessel/attitude.rs:603`); hard starts; chill-in; spin-start helium; inlet conditions acting on thrust and mixture ratio; propellant utilisation. | playable | `systems/propulsion.rs` |
| Plumbing, tanks | Per tank: liquid mass and temperature, ullage moles and temperature; pressure = ideal gas + vapour; relief; boil-off; NPSP with the acceleration head and the line's drop; settling as a 1-D slug along the tank axis (Bond number, capillary dispersal). | Tank as an equivalent cylinder (`geometry`: length = V/area, domes ignored for height and head). Heat into the liquid from its own film and radiation model. | Valves acting on flow (`valves` are not read by `plumbing.rs`); slosh; lateral liquid motion; line transients; stratification; freezing (liquid temperature clamped to [freezing, critical], `keep_physical`). | playable | `systems/plumbing.rs` |
| Pressurant | Bottles blow down isentropically and exchange heat with their liquid; choked lines; helium heated while an engine is lit; the ullage expands adiabatically. | A perfect regulator: each tank is held at its loading pressure. | Regulator as a component with failure modes. Cold-gas thrust falling with bottle pressure (`draw_block` removes moles at constant thrust and Isp). Pressure-fed thrust from feed pressure. | playable | `plumbing.rs::pressurise`, `systems/ship.rs::draw_block` |
| Structure, joints | Every joint's force and moment, exact for the rigid body. Equivalent line load against the ultimate and buckling at temperature; a pressurised section is relieved by its gas and liquid head. Snap → a split. | Air loads shared out per part along the slender axis (the totals are exact; the split is an approximation). | Capacity for point attachments (engine mounts, legs, grid-fin hinges, COPV struts: ratio 0, never break); thruster forces among the loads; fatigue. | playable | `systems/structure.rs`, `vessel/ship_loads.rs` |
| Mass, inertia | Dry parts as shells; components as solids; bottles' and ullage gas as point masses; recomputed each tick. | Each tank's liquid is a solid cylinder filling from its base, rigid with the tank (`craft/mass.rs::with_contents`). It ignores the plumbing's slug drift and does not rotate freely in roll. | The liquid's real position (coast, flip, settling); slosh mass. | playable (dry) / skeleton (liquids) | `craft/mass.rs` |
| Aero | Cells with regimes from subsonic to free-molecular (D070, D085); lattice fins with the transonic dip; per-part forces for the joints. | Shares per part; the frontal drag of the lattice fins. | Plume–flow interaction (supersonic retropropulsion); base heating; wind and gusts; the nosecone's aero while hinged. | playable | `sim::aero`, `systems/fins.rs` |
| Thermal | Skin cells and interior nodes per part (D065); material limits; time at temperature on interior nodes. | Engine heat into the nozzle cells by static design shares (D077), so an engine that is off still heats its nozzle. Node capacities from the scalar `propellant` (`aerothermal.rs::engine_and_nodes`). Cabin temperature: the mean of the capsule's skin cells (`systems/ship.rs::cabin_k`). | Components' dissipation into their nodes (`node_heat.fill(0.0)`); tank liquids as heat sinks in the network; ablation (PICA-X has data, nothing uses it); coolant loops and radiators; eclipse swing on rails (orbit averages). | playable (skin) / skeleton (internal) | `vessel/aerothermal.rs`, `sim::thermal` |
| Electrical | Buses; batteries with charge and I²R heating; body-mounted solar cells; the umbilical while clamped; shedding by priority; bus voltage derived. | Batteries as one capacity each; a lost cell takes 25 %. | Battery temperature limits; cells per skin cell; albedo. A load's loss only gates starts and functions: a running engine, the gimbal and SAS need no power. | playable | `systems/power.rs` |
| Data, sensors | `Knowledge` from working, powered, linked sensors: the truth shown to the sensor's precision, a dash when nothing measures it. | Each quantity has one precision. | Control reading it (SAS, holds, the pad's health check, estimates all read the truth); GNSS outages; IMU drift; relative navigation (range, range-rate, bearing to a target: no `Quantity` for it); chamber pressure and turbopump speed. | playable (display) / skeleton (information model) | `systems/data.rs` |
| RCS | Translation allocated from nozzle geometry; settling fires; feeds from bottles or bipropellant tanks. | Rotation as per-axis authority (`craft/ship.rs::rcs_authority`): pure couples, no cross-coupling or translation side effects; duty is continuous. | Minimum impulse bits; RCS on rails (D078: not counted); plume impingement. | playable | `systems/rcs.rs` |
| Mechanisms | Actuator rates; configurations (D103); legs, grid fins, nosecone; a lost actuator stops its mechanism; cycles counted. | The grid-fin hydraulics is a power draw only. | Hydraulic fluid; the legs' self-levelling. | playable | `systems/mechanisms.rs`, `craft/configure.rs` |
| Separation, staging | Separation impulse along the joint's normal; momentum exact; the stage sequence and functions; stages run again (D114); the liquids jolted. | The impulse is instantaneous (the pushers' stroke is not flown). | **No capsule–trunk separation joint** (`data/craft/falcon9-crew/craft.ron` has 3 joints); thrust termination on abort; booster pass-through inside 1 s of MECO (open issue); pyroshock. | playable | `vessel/split.rs`, `craft/state.rs` |
| Landing, contact | Spring-damper contact points; crush plateau; impact per point; stance and tip rules. | Crush cores recover after a touchdown. | Water (D037: oceans are solid at sea level, `body/mod.rs:175`); moving ground (droneship); legs' strut capacity. | playable (land) | `sim::contact` |
| Crew, life support | Cabin partial pressures per BVAD; ECLSS removal rates; acceleration windows by axis against NASA-STD-3001. | ECLSS holds 295 K and 101.3 kPa from unlimited N₂/O₂ and unlimited cooling. Acceleration taken at the centre of mass. | **D116's doses**: the code records injuries and alarms, and the crew never die (`systems/crew.rs`). O₂/N₂ tanks; leaks; suits; Brinkley; radiation. | playable (air) / skeleton (limits) | `systems/crew.rs` |
| Failures, wear | Fates (counter-based, seeded); hazards per hour, start or cycle with wear; Larson–Miller; Miner; bursts and Kinney–Graham blasts that cascade; history. | Vibration by class (20 g rms at mounts, ×0.6 per joint; 5 g buffet). | Physical outcomes for *valve stuck*, *leak* and *gimbal stuck* (they set a flag or "function lost", `failure/mod.rs:247–266`); blast from an uncontained engine (only bursts make blasts); hazards on the lattice; hard starts; pyroshock; fragments. | playable | `systems/failure/` |

Doc drift to fix by whoever owns `parts.md`: its "Not done" still says engines start and stop at once and nothing checks loads. RUNTIME and SYSTEMS2 have since built both.

## 2. Scenarios it would get wrong

Rank for D100's first goal (a whole Crew mission): **A** breaks or falsifies the mission, **B** gives the wrong numbers or outcome in it, **C** later or an edge case. The scenarios are grouped by mission phase and ranked within each group.

### Pad and ascent

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 1 | Pad abort on SuperDracos | The stack's thrust is terminated. All 8 SuperDracos run ~5–6 s and Dragon reaches 100 mph in 1.2 s (falcon9.md §2.3). | "Launch abort" separates at the trunk ring and lights the SuperDracos (`craft.ron:225`), but **the nine Merlins keep running**. SuperDraco thrust ignores the feed pressure, and Dragon's tanks load at 1.7 MPa against a 6.9 MPa chamber (`craft.ron:196–199`), which could not feed them. The abort has not been flown in a test. | This is the crew's survival case, and D115 counts a mismatch with the real vehicle as a bug. | A |
| 2 | Engine-out at T+79 s (CRS-1) | The controller shuts the engine down; the others gimbal and burn longer; the vehicle re-trims and reaches orbit. | Benign shutdown (statistical, 2.2e-3/h). The combined gimbal trims the thrust offset with no per-engine limit. Nothing re-trims or re-plans (the adaptive functions of D115 are not built). | Engine-out is the reference failure of the vehicle. | B |
| 3 | Uncontained engine failure | Turbopump fragments; the octaweb's bays shield the neighbours, more or less. | The engine part is destroyed. There is no blast or neighbour damage: only tank and bottle bursts make blasts (`failure/physical.rs::bursts`). | This decides whether a failure cascades (D105). | B |
| 4 | Lateral load, wind shear at max-Q | The design case: q·α from a gust, with launch-commit limits on the upper winds. | No wind exists; α comes only from steering. The joints peak at 0.70 at 4° α. | The structural failure path exists but nothing in the environment ever drives it. | B |
| 5 | Strut or point failure (CRS-7) | A COPV strut's rod end fails at 2,000 lbf at 3.2 g; the bottle breaks loose and the stage bursts. | Point attachments have no capacity (ratio 0, never break). | The real Falcon 9 loss cannot happen. | B |
| 6 | COPV over-pressure, AMOS-6 | Solid oxygen in the overwrap's buckles, then a rupture, then a LOX breach. | Bursts exist, but nothing can over-pressurise a tank: the regulator is perfect, valves gate nothing, and liquids are clamped above freezing. The AMOS-6 test reaches it only through a blast cascade. | D105's "physical triggers" are mostly unreachable. | B |
| 7 | Stuck valve | A prevalve stuck closed starves the engines; a vent stuck open loses pressure. | `StuckOpen/StuckClosed` set `valve.stuck/open`, which nothing reads. | A failure mode that does nothing contradicts D105 ("modes have outcomes"). | B |
| 8 | Leak (Starlink 9-3) | Fatigue cracks a sense line; LOX leaks and over-cools the igniter feed; the MVac hard-starts. | `Leak` becomes "function lost". There is no mass flow or cooling, and no hard-start model. | The record's one Block 5 mission failure cannot be reproduced. | C |
| 9 | Roll on ascent | The outer Merlins' differential gimbal. | Zero roll from the gimbal; any roll comes from the 200 N nitrogen nozzles. Nothing disturbs roll today (no misalignment, no aero roll), so the gap does not show. | It shows the moment a disturbance or a stuck gimbal arrives. | C |
| 10 | Stuck gimbal | One engine is stuck at an angle; the others trim. | "Function lost" only. The shared gimbal keeps steering every engine. | Needs per-engine gimbals. | C |
| 11 | Throttle bucket at max-Q | The throttle moves at a finite rate. | It steps in one tick. | Small on ascent; matters for landing (#16). | C |
| 12 | Hard start, off-nominal inlet | It damages or destroys the engine. | Every start ramps cleanly when its inlet is settled and above NPSP. | D115 lists hard starts as next. | C |

### Staging and second stage

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 13 | Separation soon after MECO | Separation waits ~3 s for the tail-off. | Inside 1 s the booster passes through the second stage (open issue). The push is an instant impulse, and the interstage is not hit by the MVac plume. | The player can stage early by hand. | B |
| 14 | MVac start after a coast | Nitrogen settling, then the start. | Modelled, with the slug moving along the axis only. Lateral rates (a slow tumble) are projected onto the axis. | Good enough for Crew; a tumble needs lateral liquid motion. | C |

### Booster return

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 15 | The flip and coast after separation | The liquids float and slosh; the centre of mass and the inertia move. | The liquid's centre of mass stays at each tank's base (`with_contents`) while the plumbing's slug drifts elsewhere. The liquid is rigid with the tank, which overstates roll inertia. | Attitude control during the flip and the entry burn. | B |
| 16 | Landing burn (hoverslam) | Throttle at a finite rate, the slosh of a nearly empty tank, a guided touchdown at ~2 m/s on a moving deck. | The throttle steps in one tick. No slosh. The ocean is solid at sea level, with no droneship and no deck motion. Crush cores recover, so a bounce lands soft again. Legs have no strut capacity. | The booster landing is D100's second objective. Today it lands too easily. | B |
| 17 | Entry burn, supersonic retropropulsion | The plume pushes the shock out, drag and heating fall, and the burn protects the base. | Air drag and heating are computed as if there were no plume. Engine heat uses static shares. There is no base heating, so flight 108's "boot" failure cannot happen. | The reason the entry burn exists is missing; descent drag and heating are wrong. | B |
| 18 | Grid fins in the hot flow | The titanium fins glow on entry; the hydraulic pump draws ~20 kW. | The pump is a power draw and a statistical stall; the fins' heating is not modelled. | Minor for Crew. | C |
| 19 | Booster relight in vacuum | Cold gas settles the propellant (boostback, on RTLS). | Its pods have no aft nozzle, so a start waits until drag settles the tanks at ~40 km (open issue). | Needed only for RTLS. | C |

### Orbit, rendezvous, docking

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 20 | Docking contact with the ISS | Soft capture, attenuation, then hard capture; the two craft become one vessel. | There is no ISS vessel and no port mechanics. Craft do not merge; the bubble only gives hull spring-dampers. Knowledge has no relative quantities (range, range-rate, bearing). | D100's third objective; nothing exists. | A |
| 21 | Phasing (~10 burns over a day) | Planned burns run under warp. | `PlanError::PartsUnsupported`: every burn is live and by hand. RCS for attitude holds on rails is not counted (D078). | It is already queued; the rendezvous budget is wrong without it. | A |
| 22 | Fine translation near the port | Draco pulses with minimum impulse bits; firings for rotation also translate. | Duty is continuous; rotation is pure couples with no side translation. | Approach dispersion and propellant use. | B |
| 23 | Long coast, or 210 days docked | Consumables are used up; heaters and coolant loops run on power; hazards accrue with time. | Boil-off, batteries and solar run on the 60 s lattice. **Failure hazards and time at temperature do not** (`systems/ship.rs::lattice` has no failure step). ECLSS draws unlimited O₂/N₂ and cooling. Dragon's propellant tanks are held by heaters that draw no power (`plumbing.rs::heat_in` returns 0). MMH and NTO cannot freeze. | A week's mission ends with nothing used and nothing worn. | B |
| 24 | Power brown-out | Undervoltage makes computers drop out, controllers safe the engines, and heaters stop. | Shedding exists. A shed computer only blocks starts: running engines, gimbal, SAS and heaters carry on. | Failures do not propagate (D105). | B |
| 25 | Sensor failure and what the craft then knows | Losing the IMUs means no attitude solution, and SAS cannot hold. | Knowledge shows a dash, but SAS, the holds, the estimates and the pad's release check read the truth. | D106/D115 hold only for the display, not for control. | B |
| 26 | Coast heat with eclipses | The skin swings with every sunset and sunrise. | Orbit-averaged skin temperatures on rails (open issue); components produce no heat. | Battery and propellant temperatures. | C |
| 27 | Radiation, single-event upsets | Dose to the crew and electronics. | None (open question 2). | Later (D116 has the rule ready). | C |

### Entry and splashdown

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 28 | Trunk jettison before the deorbit burn | The capsule enters without the trunk (falcon9.md §4.5). | **No separation joint between the capsule and the trunk**: the capsule enters with its trunk, its fins and its solar cells. | The entry configuration is wrong. | A |
| 29 | Splashdown | Water impact (slamming, added mass), flotation and uprighting, crew loads per Brinkley. | Solid sea (D037): rigid contact with an impact limit per point. There is no Brinkley model, and the crew limits are injuries (alarms), not D116's doses. | D100's final objective; crew survival is not judged. | A |
| 30 | Heat-shield ablation | PICA-X recedes and pyrolyses; the backface stays cool. | No ablation: the shield radiates up to its 3,000 K service limit, so mass loss and the soak inward are wrong. Dragon's lifting trim (its CoM offset) is untested. | Entry heating of parts; whether the heat shield holds. | B |
| 31 | Parachute opening loads | An opening-shock overshoot (Knacke's Cx); the lead canopy takes ~1.9× the mean. | No overshoot, an even share across the cluster, reefing marked U (D079). | Whether a canopy tears, and the out-of-chutes case. | B |
| 32 | Plasma blackout and navigation | No GNSS through the plasma; navigation by the IMU alone. | The comms blackout exists (`sim::comms`); GNSS outages and dead reckoning do not. | Covered by #25. | C |
| 33 | Crew loads in a tumbling abort | Each seat feels α×r and ω²r. | The centre of mass's acceleration is used (`systems/ship.rs`: `crew::step(…, Some(env.accel))`). | Abort cases. | C |
| 34 | Cabin leak | Dragon can land with a 6.35 mm leak; suits protect the crew. | No leaks: the total is held from unlimited N₂. | D115 has it next. | C |

### Across the vehicle

| # | Scenario | What really happens | What the model does now (checked) | Why it matters | Rank |
|---|---|---|---|---|---|
| 35 | Cold-gas blowdown | Thrust and Isp fall with bottle pressure and temperature. | Full thrust to the last mole. | Flip time, settling time. | C |
| 36 | Propellant residuals and utilisation | The mixture ratio is trimmed to burn both propellants out together. | Fixed ratio; the first propellant to run out flames the engine out. | Landing margin. | C |
| 37 | An engine that is off | It heats nothing (D115). | It heats its nozzle by the static shares (D077); the revision is queued. | Heat on entry and long coasts. | C |
| 38 | Splits carry systems state | Each piece keeps its own. | The crew's exposure history, the buses' records, the realised gimbal direction and `pressure_regulated` are copied to every piece (`systems/runtime.rs::partition`, `..self.clone()`). The grid-fin deflections were too, and misaligned, until f41f229 fixed that one field by hand today. | See §3, gap F1. | B |

## 3. Foundation gaps (in the order to fix)

- **F1. State partition at splits, and its inverse.**
  - `ShipState` and `Runtime` are lists aligned by index with `ShipDef`. They are split by filtering on the part, and every field not named is copied whole (`..self.clone()`).
  - `refit` silently rebuilds the lists as loaded when their lengths disagree, so state can be lost without a sign.
  - Each new field makes this worse. Docking needs the inverse (a merge), and there is none.
  - Fix: every item of state keyed by its owner's id, and a partition and merge that every field must implement (no default copy), so the compiler and a coverage test catch a missing one.
- **F2. One mass truth.**
  - The mass is held twice: in `Vessel::propellant`, a scalar (also the coast segment's starting mass, scaled back over every tank uniformly by `scale_to`), and in the ship state's tanks.
  - The thermal nodes' capacities come from the scalar.
  - Where the liquid sits (the slug drift) is not read by the mass model.
  - Slosh, boil-off by tank and the flip all need one per-tank liquid state that the mass model, the thermal network and the coast law all read.
- **F3. Control reads the truth.** SAS, the holds, the pad's health check, the estimates and the landing prediction read the true state, not `Knowledge`. D115's adaptive functions (engine-out re-trim, launch-commit, the navigation filter) need a flight-software layer whose inputs are `Knowledge` and the computers' health, and whose outputs are `ShipCommand`s. Building those functions first means writing them against the truth and then rewriting them.
- **F4. Failure outcomes have no physical hooks.** *Valve stuck*, *leak* and *gimbal stuck* collapse into a flag or "function lost". `Runtime::lost` and `unpowered` are keyed by strings, with ~70 linear searches by id across `systems/`. Valves must act on the plumbing, a leak must be a flow source or sink, a gimbal must be per engine, and the handles must be typed.
- **F5. One owner per heat.**
  - A tank's liquid takes heat from the plumbing's own film and radiation model, beside the thermal network's cells on the same wall.
  - Components put nothing into their nodes.
  - Engine heat goes by static shares.
  - The cabin follows the mean of its skin cells.
  - The liquids should be node heat sinks in the network, and components' power should go into their nodes (with D077's queued revision).
- **F6. Lattice versus live.**
  - Live ticks run structure, failures and crew acceleration.
  - The lattice runs the plumbing, power and the cabin air only.
  - Hazards and time at temperature therefore stop the moment a craft goes on rails. RCS for holds on rails is not counted, and planned burns are refused.
  - Fix: one table, per subsystem, of what it does in each phase: a continuous law or an event on the lattice, warp-invariant and tested chunked = single pass.
- **F7. Thrust lumped across engines.** `ShipThrust` already keeps a magnitude per engine but only one direction and one gimbal. It needs a vector and a gimbal state per engine (roll, engine-out, a stuck gimbal, joint loads).
- **F8. Duplicated rules (rule 5).** `ShipState::apply`'s instant ignite and deploy, and `craft::engine::ship_thrust`, duplicate the engine rule (open issue). Clean them up alongside F4.
- **F9. Missing environment owners.** Water (D037), moving ground (a droneship), winds, and the ISS as a vessel. These are not part of the ship model, but two A scenarios (#20 docking, #29 splashdown) and the booster's landing (#16) wait on them.

Determinism is in good shape: the fates are counter-based, and there is no hash-map iteration in the systems (the one `HashMap`, in `craft/cells.rs`, is only looked up). Orders apply at epochs and the warp tests exist. Keep it so as F1–F7 land: partition and merge must be bit-exact, and new lattice laws need their chunked = single-pass test.

## 4. Recommendations

1. **Partition and merge by construction (F1).** Key every `ShipState` and `Runtime` item by its owner's id. Give each field an explicit split and merge, with the global fields listed by name. Crew data goes with the cabin, the fins' deflections with their parts, and the buses with theirs. *Acceptance:* a state with every field non-default, split at stage separation and at Dragon separation, puts each item in exactly one piece. `merge(partition(s, k), partition(s, ¬k)) == s`, bit for bit. The grid fins (fixed by hand in f41f229) become one case of the rule.
2. **The liquids as state the mass model reads (F2).** Each tank's liquid centroid comes from the slug drift (later a slosh pendulum, Abramson SP-106). Remove `scale_to`'s uniform scaling. Make the ship state the only propellant truth for a craft of parts, and take the thermal nodes' capacity from the tanks. *Acceptance:* in a 30 s booster flip on cold gas, the centre of mass moves by the hand-computed amount (±1 cm). After a day's coast with boil-off, only the venting tank loses mass, and the vessel's propellant equals the ship's consumables bit for bit in 1× frames and in one jump.
3. **The capsule's entry configuration.** Add the capsule–trunk separation joint and a "Trunk jettison" stage before the deorbit burn. *Acceptance:* the capsule's piece weighs 8,400 kg plus its remaining propellant (the craft file's check), runs on its 4 × 2.5 kWh with the trunk's cells gone, and the trunk becomes debris. A test entry from 7.8 km/s reaches drogue deployment near 5.5 km at ~156 m/s (falcon9.md §4.5) with the shield intact and debug mode off. Ablation follows as its own step, against PICA-X's data.
4. **Flight software reads Knowledge (F3), before D115's adaptive functions.** *Acceptance (table tests):*
   - with every IMU lost, SAS will not hold ("no attitude reference");
   - with the stage computer unpowered, its running engines are safed by their controllers (the behaviour sourced, or the class default marked E);
   - the pad's release waits on the engine controllers' `EngineThrust` readings, so a lost controller aborts;
   - debug mode is unchanged.
5. **Physical plumbing failures, and feed pressure (F4).** Valves gate flow; a leak is a flow; the regulator is a component with modes; a pressure-fed engine's thrust follows its inlet pressure; cold gas blows down. Fix Dragon's loading pressure to the real regulated value (D115). *Acceptance:*
   - a LOX prevalve stuck closed flames the nine Merlins out when the downcomer runs dry (~2,700 kg at 1,890 kg/s ≈ 1.4 s, D);
   - a helium regulator stuck open bursts the LOX tank at its rated burst pressure, at the time the ideal-gas fill gives;
   - the SuperDracos cannot run at 1.7 MPa.
6. **Abort scenarios as tests (after 5).** "Launch abort" also terminates the stack's thrust (the AFSS and the stage 2 pyrovalve, falcon9.md §5.1). *Acceptance:*
   - from the pad, the eight SuperDracos give the published 100 mph in 1.2 s, Dragon clears the stack, and the crew stay under their dose thresholds;
   - an in-flight abort at max-Q (the January 2020 test) separates cleanly, and the booster left behind breaks up under its aero loads, as the real one did.
7. **Per-engine thrust vectors and gimbals (F7; queued in D115).** *Acceptance:*
   - a CRS-1 replay (an outer engine out at T+79 s) reaches MECO with every engine within ±5°;
   - roll is held by differential gimbal with the stage 2 thrusters off;
   - a gimbal stuck at 2° is trimmed by the others;
   - the joint loads from the per-engine vectors match a hand calculation.
8. **Splashdown and D116's doses.** Replace injuries with D116's doses and add Brinkley on the seat axes. Model water as a surface with a water-impact model at each contact point: slamming pressure, added mass, buoyancy. *Acceptance:* D116's table tests. At ~7 m/s Dragon floats; its peak deceleration lies in a sourced range (Apollo or Orion water-impact data, marked E until checked); Brinkley's index stays under NASA-STD-3001's low-risk limit. With one main out the crew still survive, a case Dragon is designed for.
9. **The systems on the lattice (F6), with planned burns for parts (queued).** Failure hazards and time at temperature move onto the 60 s lattice, RCS for holds on rails is counted, and ECLSS O₂ and N₂ become tanks. *Acceptance:* a 7-day coast gives the same bits in one jump and in 60 fps frames; Dragon's O₂ falls by 4 × 0.82 kg a day; battery and ECLSS hazards accrue; the 210-day docked limit becomes visible.
10. **Docking (F9, after 1).** The ISS as a vessel, IDSS soft and hard capture as a mechanism, the merge from recommendation 1, and relative-navigation quantities in `Knowledge`. *Acceptance:* Dragon closing at 0.1 m/s captures and merges, and undocking gives back the two states bit for bit. The forces at capture stay within the IDSS's published contact limits (marked E until sourced).

Order: 1 and 2 first (the foundation; every later item adds state). Then 3 and 4 (cheap, and they unblock the entry and the meaning of sensors). Then 5 → 6, 7, 8, 9 (9 alongside the queued planned burns) and 10. The supersonic-retropropulsion model for the entry burn (#17) and ablation (#30) come after these, once the booster's return and the entry are flown end to end.
