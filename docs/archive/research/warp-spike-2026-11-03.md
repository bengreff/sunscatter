# Warp invariance research (night 2026-11-03)

Mode: RESEARCH. Labels: [measured] [derived] [inferred] [guessed].

## 1. Answer in 10 lines

1. Yes. Warp invariance can be true by construction at 100,000x for these targets: in all four cases (and a coupled thermal plus battery craft) the scope 3.3 scheme gave bit-identical events and final states at 1x, 100x, 1,000x and 100,000x [measured].
2. The mechanism is canon's own rule made strict: the warp only decides how many steps run per frame; every step length and event epoch comes from the craft's state, and **a frame never splits a step** (allowing splits broke the bits in every case with a threshold).
3. The naive scheme reproduced the owner's exact failure: a chip stable at 100x overheats at 1,000x; a battery that survives at 1,000x runs flat at 100,000x; a relief valve cycles 663 times at 1x and 50 at 100,000x; a chute tears at 100x [measured].
4. Making the integrator implicit or substepping does not fix it; only taking the warp out of the physics does.
5. Accuracy needs exact events (roots inside the step, switch applied at the exact root): against references, events land within 0.000 to 0.15 s.
6. Cost for 10 live craft at 100,000x fits: 12 to 42 ms per craft-day for 40 to 200 thermal nodes, under 0.1 ms for tanks and batteries [measured]; the budget is 86 ms on one thread.
7. One 5,000-part craft fits only with a quiescence jump (periodic steady state, envelope following): 24 ms per day on a periodic orbit, 419 ms on a strongly drifting one, against 1,026 ms without [measured, path-graph network].
8. Chattering thermostats and valves are handled as sliding modes (hold at the set point): same heat and vented mass within 0.01 to 0.1 %, cost back to normal; the price is that click-by-click cycle counts come from a law.
9. Randomness stays identical through D105's existing fate thresholds keyed on (seed, launch, part, mode); per-frame dice give a different failure day at every warp [measured]; nothing environmental or broad is ever random.
10. Physics warp near a planet is capped by compute, not determinism (today 103 ms per tick for 1,000 parts); canon's "restrict the warp, never change the model" stays right there.


## 2. The mechanism: why naive warp breaks outcomes

A game frame lasts 1/60 s of real time. At warp W it covers W/60 s of simulated time: 1.67 s at 100x, 16.7 s at 1,000x, 28 minutes at 100,000x. Naive warp lets the frame's span become the physics step. Four separate things then change with the warp, and each one alone changes outcomes:

1. **Stability.** An explicit update of a node with time constant tau is stable only for steps under about 2 tau. A 2 J/K chip on a 0.6 W/K link has tau = 3.3 s. At 100x (1.67 s frames) it is stable; at 1,000x (16.7 s) every update overshoots by more than the last, and the "temperature" climbs until the overheat check fires. [measured: section 4.1, A_exp, overheat at +50 s at 1,000x, never at 100x]. This is literally the owner's example, and it happens in real thermal tools too: SINDA's explicit routine limits its step by the smallest node's capacitance-to-conductance ratio (CSGMIN) for this reason.
2. **Sampling of switches.** Thermostats, relief valves, load-shed logic and flight-computer triggers are only looked at when the step ends. A 28-minute step leaves a heater on for 28 minutes, a vent open for 28 minutes (the tank fell to 124 kPa instead of 283), and a battery's charge unchecked for most of an eclipse. [measured: 4.1 A_imp 8 switchings instead of 14; 4.2 A_imp 94 valve cycles instead of 663; 4.3 the battery runs flat]
3. **Sampling of inputs.** Sunlight, air and other inputs held at the frame's start apply for the whole frame. A frame that starts one second before eclipse gets 28 minutes of sunlight that never happened. [measured: 4.3, the 390 Wh battery sheds 15 times and survives at 1,000x, and is empty at +6.5 h at 100,000x]
4. **Bits.** Even a stable, accurate integrator whose step length is derived from the frame (A_sub) gives different round-off at each warp. Usually the difference is tiny (here up to 154 s on an event time), but any downstream threshold (a cycle count, a fate, a margin) can turn it into a different outcome. [measured: 4.1 A_sub; 4.2 B_split]

Making the integrator implicit fixes (1) only. Substepping fixes (1) and most of (2) and (3), never (4). **The only complete fix is that the warp never reaches the physics at all**: the sequence of steps, their lengths, and every event epoch must be a function of the craft's state and its inputs at their own epochs, and the frame must merely decide how many of those steps to run before drawing. This is canon already (D129: "the sim never reads the warp"; warp-invariance.md section 1), and today's sunscatter code holds to it on its lattices. The measured cases show that the remaining work is not this principle but the four things it needs to be cheap and right: exact events, laws that are exact over long steps, quiescence (periodic steady state), and sliding modes for chatter.


## 3. Candidate architectures compared

"Guarantee" means: what is identical between 100x and 100,000x. "Fit" is against canon (D119, D121, D127, D129, the libm-only rule, every craft runs its script, every failure has a cause).

| # | Architecture | Guarantee across warps | Cost at 100,000x | Complexity | Failure modes | Fit with canon |
|---|---|---|---|---|---|---|
| 1 | Frame step = physics step (KSP-style background updates; A_exp, A_imp) | none | lowest | trivial | the owner's case, measured in all four cases | violates D129 |
| 2 | Substeps whose length comes from the frame (Orbiter's subsampling; A_sub) | outcomes usually, bits never | moderate | low | thresholds near a margin flip; cycle counts drift | violates D129 (bits) |
| 3 | One fixed global tick for everything (deterministic lockstep: Factorio, RTS games, Fiedler's accumulator) | bit-identical by construction | proportional to simulated time / tick: 5 million 20 ms ticks per real second per craft at 100,000x | low | cost; events land on tick epochs | canon for live flight (D127); unaffordable for coast warp |
| 4 | **State-chosen steps, frames never split a step** (block power-of-two steps as in N-body codes; scope 3.3 rule 1) | **bit-identical by construction** (measured, all cases) | set by the dynamics: 12 ms per craft-day at 40 nodes | moderate | step control must read only state; display needs interpolation between steps | this is D129 made general |
| 5 | 4 + **exact events** (roots located inside the step, epochs on an integer grid, the switch applied at the exact root; rule 4) | events identical and accurate (0.000 to 0.12 s against the references) | small: about 17 re-steps per event | moderate | Zeno chatter (needs 7); grid rounding bias if the switch is applied at the rounded epoch | closes audit section 7 point 3 |
| 6 | 4 + **closed-form or exponential laws** for near-linear domains (rule 2; D119) | exact for the frozen-coefficient model | tiny: 0.004 to 0.07 ms per craft-day | low per law | coefficients that depend on state drift unless steps are capped (measured: 1,289 s over 9 months uncapped, 11 s at a 1 s cap) | D119, D121 "one law per subsystem" |
| 7 | 5 + **sliding modes** (Filippov equivalent dynamics) for zero-deadband switches | identical, and outcome within 0.01 % of the resolved chatter | removes the chatter cost (553 ms to 12 ms per day) | moderate | the count of switchings is replaced by a law; entering and leaving the mode must be events | new; consistent with D129 (chosen by state) |
| 8 | 4 + **periodic-steady-state jump** (quiescence certificate, rule 3) | identical (decided by state) | 40x to 1,200x cheaper on periodic coasts | moderate | needs k-orbit cycles and a tolerance; only valid while the forcing is periodic; the craft's software state must be periodic too | new; makes the 5,000-part offline target |
| 9 | 8 + **envelope following** (Petzold 1981 multirevolution: integrate one cycle every N cycles and extrapolate the slow drift) | identical | close to 8 under slowly drifting forcing (beta angle, distance from the Sun) | high | extrapolation error; needs its own error control | new; the general form of 8 [not measured] |
| 10 | Multirate integration (Gear-Wells 1984; MRI-GARK, Sandu 2019) inside a live craft | identical if the partition and the ratio come from state | saves the fast domain's rate on the slow ones | high | coupling error between rates; order reduction | fits D121 "each part at its own rate" for the live tier |
| 11 | Co-simulation master with fixed communication points (FMI; Broman et al. 2013 determinate composition) | identical if the master's step and the order of exchanges are fixed | communication overhead | moderate | a one-step lag on two-way couplings (a modelling error, the same at every warp) | the domain scheduler of scope 3.2 is one of these; case 4.5 is a lookahead version |
| 12 | Quantised state systems (QSS1-3, LIQSS; Kofman and Junco 2001): each variable steps when it has changed by a quantum | identical if ties are ordered | excellent for sparse, loosely coupled systems; poor for stiff tightly coupled networks (QSS1 oscillates; LIQSS needed) | high | event storms on stiff links and on chatter | interesting for the data and power networks [not measured] |
| 13 | **Event-exclusion certificates** (novel here): a cheap bound that proves no threshold can be reached over a span (for an M-matrix thermal network, the comparison principle: a node driven by its worst-case inputs from below its limit stays below it), allowing one long exact step | identical (decided by state) | one bound per span | moderate | conservative bounds give no saving; must cover every event function | new; the safety net under 6 and 8 [not measured] |
| 14 | Outcome distributions / "on rails with nothing running" (KSP stock for unloaded craft) | trivially identical | zero | low | nothing really runs | rejected by the owner (answer 40) |

What the experiment says about the five hypotheses of scope 3.3:
- **Rule 1 (warp never an input) holds and is sufficient for bit-identity**, provided one sub-rule is added: **a frame never splits a step**. B_split (frames may cut a step) loses bit-identity in every case with a threshold (1e-5 to 1e-3 relative), and keeps it only to about 1e-12 for purely linear closed forms. Display between steps comes from interpolation or the closed form, read-only (Fiedler's accumulator with interpolation, applied to a state-chosen step instead of a fixed one).
- **Rule 2 (exact long steps) is about cost and accuracy, not invariance.** Long and short steps do not agree to round-off once a coefficient depends on the state; they agree to the method's error. So "merging" steps must also be decided by the state, never by the warp.
- **Rule 3 (quiescence) is the cost lever** for big craft, and the measured form is a periodic-steady-state jump with k-orbit cycles and a tolerance. A literal "steady state" rarely exists in orbit; a periodic one usually does.
- **Rule 4 (exact events) is needed for accuracy and for cycle counts**, and must apply the switch at the exact root (then cover the rest of the grid unit in the new mode), or a bias accumulates (measured on the tank).
- **Rule 5 (seeded randomness) already exists in canon** as D105's fates (section 5).


## 4. Worked examples (the 4 cases)

All numbers in this section are [measured] on the M2 Pro (load 5 to 6) with the prototype in `warp-spike/` unless labelled. "Bit-identical" means equal FNV hashes of every f64 of the final state and every event (kind and integer-ms epoch). Raw outputs: `warp-spike/out/*.jsonl`, analyser `warp-spike/analyse.py`.

**The four cases in one table** (B = the scope 3.3 scheme; details and numbers follow):

| Case | Reproducible at every warp (bit-identical) | Not reproducible, or only approximately | The trade-off |
|---|---|---|---|
| Radiator and electronics box, heater thermostat | every heater switching epoch, every temperature, the heater energy, any overheat | against other correct integrators: event times to about 0.1 s; a very tight thermostat's exact click count | cost grows with the number of switchings; tight deadbands become a hold-at-set-point mode (same energy within 0.01 %); quiet coasts are copied from a certified cycle (error about 1e-4 K) |
| Cryogenic tank, 9 months, relief valve | every crack and reseat epoch (1,326 of them), vented mass, liquid left | across code versions: valve epochs move by seconds after months (11 s at a 1 s step cap) | analytic phases are exact only for frozen coefficients, so a state-dependent coefficient needs a step cap; a valve with no blowdown becomes a regulating mode |
| Battery near a shed threshold | every shed, restore and empty epoch, to the ms, and equal to the exact solution | nothing measured | none in cost (microseconds per day); the law must be solved for its threshold epochs rather than checked per step |
| Chute on a pressure trigger during entry | mortar epoch, riser load, tear or not, touchdown epoch and speed | nothing measured | the 20 ms tick is fixed, so warp in atmosphere is limited by compute: a large craft cannot go faster than its ticks |

### 4.1 Radiator panel and electronics box through eclipses (thermal network, heater thermostat)

Model: a chip (2 J/K, 5 W) on a board (400 J/K, 25 W) in a box (4,000 J/K, MLI, 40 W heater, thermostat 267.5 to 272.5 K), a strap to a 1 m2, 3 kg aluminium radiator split into a chain of nodes, T^4 radiation to space, a 92 min orbit with 35 min of eclipse. 40 nodes (also 200, 1,000, 5,000 for cost). One simulated day unless stated. Reference: explicit RK4 at a 16 ms fixed step (5.4 million steps). The chip's time constant is about 3 s and the radiator chain's about 3 s at 40 nodes, so the network is stiff, like every real spacecraft thermal model with small nodes.

| Scheme | 1x | 100x | 1,000x | 100,000x |
|---|---|---|---|---|
| A_exp (one explicit step per frame) | 14 heater events, chip max 298.0 K | 14 events, chip max 298.0 K, event times off by up to 2.7 s | **overheat at +50 s** (third frame: the chip's update overshoots and grows; chip 311.7 K and climbing when the failure fires) | **overheat in the first frame** |
| A_imp (one backward-Euler step per frame) | 14 events | 14 events, off by 2.7 s | 14 events, off by 39 s, heater 64.07 Wh | **8 events instead of 14**, heater 74.1 Wh (+17 %), box min 266.9 K |
| A_sub (60 s substeps, length set by the frame) | 14 events | off by 3.1 s | off by 39 s | off by 154 s, heater 64.15 Wh |
| **B** (scope 3.3) | **14 events, hash 35512bf2...** | **same hash** | **same hash** | **same hash** |
| Reference | 14 events, heater 63.36 Wh, chip max 297.97 K | | | |

- **The owner's exact failure, reproduced.** A_exp is stable at 100x and overheats at 1,000x. Nothing about the craft changed; the frame got 10x longer than the chip's stability limit (2/lambda = about 6.7 s). This is the mechanism of section 2.
- **Making the integrator stable does not fix it.** A_imp never blows up, but at 100,000x the thermostat is only looked at every 28 minutes, so the heater runs whole frames: 8 switchings instead of 14 and 17 % more heater energy. On a craft with a marginal battery that is a different mission (see 4.3).
- **Careful substepping does not fix it either.** A_sub is accurate (154 s worst event error at 100,000x) but is never bit-identical across warps, because the substep lengths are derived from the frame. The outcomes happen to agree here; near any threshold they would not.
- **B is bit-identical at 1x, 100x, 1,000x and 100,000x** (P2 met), and against the reference every event is the same kind in the same order, at most 0.118 s off, heater energy 63.36 vs 63.36 Wh, chip max 297.97 vs 297.97 K (P3 met).
- **B's cost** per simulated day, one craft, at 100,000x: 12 ms at 40 nodes, 42 ms at 200, 243 ms at 1,000, 1,026 ms at 5,000 (about 5,450 steps a day at every size: the step count is set by the dynamics, not the node count; about 38 ns per node per step on a tree network). At 1x the same run costs 20 ms, the extra 8 ms being 5.2 million empty frames.
- **B_split (frames may cut a step):** not bit-identical (P6 as expected): 8e-7 to 1.6e-5 relative final-state difference, event times moving by up to 0.16 s. All four warps still agree with the reference. So the rule "a frame never splits a step" is what buys bit-identity; the integrator alone does not.

**Periodic steady state (the quiescence certificate), 30 days.** The orbit forcing is periodic, so after a few orbits the craft repeats itself. A section is taken at every eclipse entry. The first finding: **the steady state here is a 2-orbit cycle, not a 1-orbit one** (the heater fires every other orbit), so a certificate must look for k-orbit cycles (k = 1 to 4 tried). The second finding: **the discrete map never repeats exactly** (state gap floors at about 1e-4 K and event offsets jitter by 1 to 9 ms, from ms-quantised event epochs and grid alignment that drifts relative to the orbit), so the certificate needs a tolerance (here 3e-4 K and 10 ms). With it:

| 30 days, 100,000x | Without the jump | With the jump |
|---|---|---|
| 40 nodes | 361 ms, 164,566 steps | **9.3 ms**, 4,805 steps (458 of 469 orbits jumped) |
| 5,000 nodes | 29.4 s | **0.71 s** (24 ms per simulated day) |
| Against the reference (40 nodes) | 468 events, max 0.120 s off | 468 events, max 0.118 s off, heater 2,135 vs 2,134 Wh |
| Across warps | bit-identical | bit-identical (1x, 1,000x, 100,000x) |

The jump is decided by the state only, so it happens at the same epoch at every warp. What it gives up: the 458 jumped orbits are copies of the certified cycle (events replicated at their offsets), not integrated; the error is bounded by the gap times 1/(1 - contraction), about 1e-4 K here [derived]. It only applies while the forcing is truly periodic; a drifting beta angle needs re-certification every N orbits (envelope following, section 3).

**Chattering thermostat (P5).** Shrinking the deadband multiplies the events and the cost; the zero-deadband limit is handled as a Filippov sliding mode (the heater holds the box at its setpoint with the exact power needed, a continuous law chosen by the state when both sides of the switch point into it).

| Deadband | Events per day | B cost per day | Heater energy (ref) | Across warps |
|---|---|---|---|---|
| 5 K | 14 | 12 ms | 63.36 (63.36) Wh | bit-identical |
| 0.5 K | 174 | 16 ms | 71.63 (71.62) Wh | bit-identical |
| 0.05 K | 1,670 | 61 to 72 ms | 70.18 (70.08) Wh; the ref, at 16 ms steps, counts 6 fewer switchings | bit-identical |
| 0.005 K | 16,676 | 495 to 553 ms | 70.13 (70.16) Wh | bit-identical |
| 0 (sliding mode) | 32 (entries and exits) | 12 ms | 70.14 (70.15) Wh, 0.01 % | bit-identical |

So P5 is met by the sliding-mode treatment: the cost goes back to 12 ms and the energy is within 0.01 %. The trade-off is that the **switching count** of a tight thermostat is not a robust quantity: it is reproducible across warps in B, but a different correct integrator (the reference) counts a different number (1,664 vs 1,670). If a failure is ever caused by "number of relay cycles", that count must come from a law (cycles = duty x time / period) rather than from counting simulated chatter.


### 4.2 Cryogenic tank on a 9-month coast with a cycling relief valve

Model: 600 kg of liquid hydrogen, heat leak 3 W at 1 AU falling as 1/r^2 to 1.52 AU (held constant over each day: a slow input on a fixed daily epoch grid), self-pressurisation dP/dt = Q/C(m) with C growing with the ullage, a relief valve that cracks at 300 kPa and reseats at 280 kPa, choked venting (mdot proportional to P). Both phases have closed forms, and so do the crack and reseat epochs. 273 days, 663 valve cycles. Reference: RK2 at 10 ms with each crossing bisected to 1 microsecond (2.36 billion steps, 22 s).

| Scheme | 1x | 100x | 1,000x | 100,000x |
|---|---|---|---|---|
| A_exp | 663 cycles, events up to 2,559 s off the reference | **659 cycles** | **627 cycles** | **50 cycles**, the tank ends at 124 kPa instead of 283 (the vent runs a whole 28-minute frame each time, far below the reseat point) |
| A_imp | 663 cycles | 658 | 627 | **94 cycles**, ends at 206 kPa |
| **B** (closed form, open-phase steps capped at 1.024 s) | **663 cycles, hash 09f732d8...** | same | **same** | **same** |
| Reference | 663 cycles, 104.3 kg vented, 495.7 kg left | | | |

- **What is reproducible.** In B, every crack and reseat epoch, all 1,326 of them, is bit-identical at every warp, and so are the vented mass and the liquid left. Boil-off totals are nearly the same in every scheme (104.2 to 105.2 kg) because energy is conserved: the heat leak has to leave as vapour whatever the valve does. **The valve's cycle count is not**: naive schemes give 663, 659, 627 or 50 cycles. If valve-seat wear is ever a modelled failure cause (it should be: relief valves do wear), A would let a valve fail at 1x and survive at 100,000x.
- **Exactness depends on the model you solve exactly.** The first B version solved each open phase as one analytic step with C frozen at its start. It was bit-identical across warps but drifted 1,289 s (21 minutes) from the reference over 9 months, with the same 663 cycles in the same order. The error is first order in the step: capping the open-phase step gave 170 s (16 s cap), 43.5 s (4 s cap), 11.4 s (1 s cap). Cost at the 1 s cap: 20 ms for the whole 9 months. Lesson: "analytic" is only exact for the frozen-coefficient model; a coefficient that changes with the state (here C(m)) needs its own step cap, chosen by state.
- **Event epochs on a grid.** Rounding an event up to the 1 ms grid and switching there biases every cycle a little (the valve closes up to 1 ms late, 0.17 Pa low, and the next rise takes up to 0.5 s longer). The fix that keeps bits and accuracy: advance to the exact root in the old mode, switch, then cover the rest of the grid unit in the new mode. Epochs stay integers, the physics is not biased.
- **B_split:** not bit-identical (1.9e-4 relative at 1,000x vs 1x, 1.4e-3 at 100,000x), because a split moves an event's grid rounding, and the tank's state is sensitive to where the valve switched. P6's expectation (<= 1e-12 for exact linear laws) is **not met** once there is a threshold downstream: closed forms commute only up to round-off, and a threshold turns round-off into a ms shift.

**Chattering valve (P5).** Shrinking the blowdown (hysteresis):

| Hysteresis | Cycles in 9 months | B cost | Vented (ref) | Across warps |
|---|---|---|---|---|
| 20 kPa | 663 | 20 ms | 104.3 (104.3) kg | bit-identical |
| 2 kPa | 6,624 | 22 to 30 ms | 104.2 (104.2) kg, events within 12.5 s | bit-identical |
| 200 Pa | 66,235 | 45 to 54 ms | 104.2 (104.2) kg, within 20.7 s | bit-identical |
| 20 Pa | 662,350 | 270 ms | 104.2 (104.2) kg; the reference counts 2 cycles fewer | bit-identical |
| 0 (regulating, sliding mode) | 1 entry | **0.08 ms** | 104.2 kg | bit-identical |

The sliding mode (pressure held at the set point, venting exactly the boil-off Q/L) is chosen by the state at the moment the valve first cracks, so it happens identically at every warp, and it costs nothing. P5 met.

### 4.3 Battery, solar array and loads near a load-shed threshold

Model: a 700 W array in sunlight, 250 W essential and 150 W payload loads, 95 % efficiency each way, constant-power charging until a taper limit (then exponential), the payload shed below 20 % charge and restored above 50 %. The same 92/35 min orbit. Every piece and every threshold epoch is closed form (linear or exponential). 3 days. Reference: RK2 at 10 ms with crossings bisected to 1 microsecond.

Two batteries: 400 Wh (the lowest charge comes within 0.66 Wh of the 80 Wh shed line, 0.8 % margin) and 390 Wh (it sheds).

| | Scheme | 1x | 100x | 1,000x | 100,000x |
|---|---|---|---|---|---|
| 400 Wh | A_exp | no shed, min 80.67 Wh | no shed, 80.71 | no shed, 80.90 | **12 sheds**, min 4.8 Wh |
| 400 Wh | **B** | no shed, min 80.66 Wh | **same bits** | **same bits** | **same bits** |
| 400 Wh | Reference | no shed, min 80.66 Wh | | | |
| 390 Wh | A_exp | 15 sheds, 0.018 s off | 15, 1.7 s off | 15, 35 s off | **battery empty at +6.5 h: craft lost** |
| 390 Wh | **B** | 15 sheds, **0.000 s off the reference** | **same bits** | **same bits** | **same bits** |

- **The battery version of the owner's sentence:** with A, a craft that sheds its payload and survives at 1,000x **runs flat and dies at 100,000x**. The cause is mundane: a 28-minute frame samples "in sunlight" at its start and charges the battery for sunlight it never had (the eclipse is 35 minutes long).
- **B is exact here**, not just invariant: every shed and restore epoch equals the bisected reference to the ms, because the laws are exactly integrable between events. Cost: 185 steps for 3 days, 11 microseconds at 100,000x (0.004 ms per simulated day).
- **B_split:** 5.5e-13 to 7.3e-12 relative and the same event list. This is about P6's expectation for exact linear laws (5.5e-13 met; 7.3e-12 just above the 1e-12 line), and it is the only case where splitting is harmless, because no threshold sat within round-off of a crossing.
- Today's sunscatter power law decides shedding by what the battery can give over the step (`crates/sim/src/systems/power.rs:192`), so 20 ms ticks and 60 s lattice steps shed at different charges. B's version (the empty or shed epoch solved from the law, E/P_net) removes that dependence.

### 4.4 A parachute deploying on a barometric trigger during entry (physics-warp tier)

Model: a 100 kg probe (CdA 0.5 m2) from 40 km at 1,500 m/s, 30 degrees down, exponential atmosphere. A static-pressure sensor with a 0.2 s lag; the flight computer samples it at 10 Hz on its own lattice and fires the mortar above 20 kPa; the canopy inflates 0.5 s later over 1.5 s to CdA 20 m2; the riser tears above 12 kN; touchdown above 12 m/s is a crash. B: RK4 on the fixed 20 ms tick (canon's `TICK`), the 10 Hz controller on the tick lattice, the tear and the touchdown bisected inside the tick to the ms. Reference: the same model at a 1 ms step.

| Scheme | 1x | 4x (physics warp) | 100x | 1,000x | 100,000x |
|---|---|---|---|---|---|
| A_exp (one RK4 step per frame, the computer once per frame) | mortar 0.055 s late, riser peak 9,521 N | 0.35 s late, **riser 9,944 N** (+6 %) | **mortar fires at +3.3 s, hypersonic; riser torn at +5 s; crash at 57 m/s** | torn at +33 s, the state goes non-finite | the probe "lands" in the first frame, no deploy |
| **B** | mortar +85.700 s, riser 9,379 N, touchdown +979.945 s at 8.84 m/s | **same bits** | **same bits** | **same bits** | **same bits** |
| Reference (1 ms) | mortar +85.700 s, riser 9,379.3 N, touchdown +979.945 s at 8.84 m/s | | | | |

- **The mechanism at 100x is the sensor, not the chute.** A frame of 1.67 s is 8 times the sensor's 0.2 s lag; the explicit update of the sensor state overshoots, the computer reads a pressure that is not there, and fires the mortar at about 1,500 m/s near 38 km (the riser then sees 115 kN). Every failure here has a modelled cause, but in A the cause is the frame length.
- **Even at physics warp (4x), A is not invariant**: the riser load is 6 % higher than at 1x. With a riser rated 5 % above nominal, the chute would hold at 1x and tear at 4x.
- **B is invariant and matches the reference** to the ms. The controller samples on its own lattice (10 Hz from the epoch it started), never at frame ends.
- **Cost is the limit of this tier, not determinism.** B costs 7.4 ms for the 980 s descent of a point mass. For real craft, sunscatter measured a live tick on entry at 17 ms for 300 parts and 103 ms for 1,000 parts (`docs/technical/scale-performance.md`): at 50 ticks a second that is at most 1.2x real time for 300 parts and 0.19x for 1,000 [derived]. Physics warp near a planet is therefore capped by compute, and the canon rule (warp restricted, model unchanged) is the right one there.
- B_split: 1.3e-5 to 1.6e-5 relative, same events.

### 4.5 Two domains coupled under one scheduler (multi-rate)

Case 1's heater is made a load on case 3's battery (412 Wh: the lowest charge is 82.88 Wh against an 82.4 Wh shed line, 0.6 % margin). The thermal network takes adaptive implicit steps of 1 to 65 s; the battery takes closed-form steps from event to event (up to 57 minutes). The scheduler: the thermal domain leads by one of its own steps, then the battery catches up to the thermal domain's epoch with the heater power that held over that step. Every heater switching is a load change at its exact epoch, and the order of operations depends only on the two states. This is a conservative (lookahead) synchronisation in the sense of parallel discrete-event simulation, with the event-producing domain leading.

| 3 days | 1x | 100x | 1,000x | 100,000x |
|---|---|---|---|---|
| A_imp + A_exp coupled per frame | no shed, min 82.89 Wh | min 82.96 | min 83.28, heater 211.5 Wh | **battery empty at +6.5 h** |
| **B, one scheduler** | no shed, min 82.88 Wh, heater 209.3 Wh, 46 events | **same bits** | **same bits** | **same bits** |

Cost: 35 ms for 3 days (11.6 ms per simulated day) at 100,000x, almost all of it the thermal network.

### 4.6 Cost against the 100,000x budget (P4)

Budget [derived]: at 100,000x a simulated day passes in 0.864 s of real time. For 10 craft on one thread that is 86 ms per craft-day; with 7 worker threads (D127), 605 ms per craft-day.

| Domain, one craft, B at 100,000x | ms per simulated day [measured] |
|---|---|
| Thermal, 40 nodes (tree), 5 K deadband | 12 |
| Thermal, 200 nodes | 42 |
| Thermal, 1,000 nodes | 243 |
| Thermal, 5,000 nodes | 1,026 |
| Thermal, 5,000 nodes, with the periodic-steady-state jump (30 days) | 24 |
| Thermal, 40 nodes, with the jump | 0.31 |
| Thermal, 40 nodes, 0.005 K deadband (chattering, resolved) | 495 to 553 |
| Thermal, 40 nodes, zero deadband (sliding mode) | 12 |
| Tank, 9 months, 1 s open-phase cap | 0.07 |
| Tank, 20 Pa blowdown (662,000 cycles, resolved) | 1.0 |
| Battery and loads | 0.004 |
| Thermal + battery coupled | 11.6 |
| Entry with chute, point mass, 20 ms ticks | 7.4 per 980 s descent (650 per simulated day if it were all live) |

Verdict on P4:
- **10 live craft at 100,000x on coast warp: met** with margin for networks up to about 500 nodes per craft without any jump (10 x 42 ms = 0.42 s per day at 200 nodes, one thread), and for any size with the jump while the craft is periodic [derived from the measured rows].
- **One 5,000-part craft at 100,000x offline: met only with the periodic-steady-state jump** (24 ms per day on a periodic orbit, 419 ms per day on a strongly drifting one with envelope following, section 4.7; without it, 1.03 s per day on one thread, so about 84,000x: just short of the target on one thread, met on 2 threads if the network can be split) [measured / inferred]. Note that a real 5,000-part craft's thermal network is not a path: sunscatter's own block Gauss-Seidel on its tree-plus-links network costs about 10 ns per unknown per sweep with 3 to 10 sweeps (`scale-performance.md`), similar to the 38 ns per node per step measured here [inferred].
- **Physics warp near a planet: not met and not meetable for real craft**, by compute alone (section 4.4). This is where canon already restricts warp.
- **Chattering resolved is the cost risk**: 40x more expensive at a 0.005 K deadband. The sliding-mode treatment removes it.

### 4.7 The quiescence jump under drifting forcing (envelope following)

Real orbits are not exactly periodic: the beta angle drifts, so the eclipse changes length. Test: case 1 over 30 days with the sunlit part of each orbit growing by 2 s per orbit (the eclipse shrinks from 35 to 19.5 minutes; the heater stops being needed after about 110 orbits). Reference: B without any jump.

Two findings first:
- **Phase-locking the step lattice to the orbit removes the certificate's noise floor.** With the adaptive power-of-two steps aligned to absolute time, the state at the eclipse-entry section never repeats better than about 1e-4 K (section 4.1). Aligning the steps to the start of the current sunlit or eclipsed phase instead (still decided by state and schedule only) makes the discrete solution repeat to 2e-6 K, a 100x lower floor [measured]. The two lattices agree with each other to 0.089 s on events.
- **The exact jump (forcing identical every orbit) never engages under drift** [measured: 0 orbits jumped], so a drifting coast pays full price unless the jump extrapolates.

Envelope following (after Petzold 1981's multirevolution methods): when the state's second difference over three k-orbit cycles is below a tolerance and the event pattern drifts linearly, jump at most M cycles, extrapolating the state, the event offsets and the heater energy linearly; then integrate again to re-certify. 40 nodes, M = 32 cycles:

| Event-offset tolerance | Orbits jumped of 469 | Cost (30 days) | Events against the reference | Heater energy (ref 744.85 Wh) |
|---|---|---|---|---|
| no jump | 0 | 299 ms | 200, exact | 744.85 |
| 5 ms | 193 | 184 ms | 200, exact (the jumps fall where the heater is idle) | 744.85 |
| 20 ms | 325 | 93 ms | 200, worst 135 s off | 745.84 (+0.13 %) |
| 100 ms | 392 | 51 ms | **228: the extrapolation kept the heater cycling after it really stopped** | 820.11 (+10 %) |

Every row is bit-identical between 1,000x and 100,000x (the jump is decided by the state). So this is a pure accuracy-against-cost dial, with a sharp failure mode: **extrapolating across a bifurcation** (here the heater ceasing to fire). The guard is an event-exclusion test over the jumped span (architecture 13): before jumping, prove that no event function can change its pattern within it, or shorten the jump.

At 5,000 nodes with the 5 ms tolerance: 26.8 s without the jump, **12.6 s with it for 30 days, i.e. 419 ms per simulated day on one thread**, inside the 864 ms budget even on this strongly drifting orbit [measured].

## 5. Randomness

**Canon already has the right mechanism** (D105, `crates/sim/src/systems/failure/fates.rs`): each statistical failure mode of each part draws once a threshold E = -ln u, with u from a counter-based hash of (the save's seed, the vessel's launch epoch, the part's id, the mode); the mode accumulates its hazard H (rate x time, wear, damage) and fails when H >= E. This is the first-passage form of an exponential failure law, and it is the same idea as Random123's counter-based generators (Salmon et al. 2011): the random number is a pure function of a key, never of how many numbers were drawn before. Nothing here needs reopening; the research only says what it implies under warp.

**Why it is warp-invariant, and the naive alternative is not** [measured, `warp-spike/rng/rng_demo.py`, a part with a 10-day mean life over 30 days]:

| Seed | Roll a die every frame (p = 1 - e^(-lambda h)): 100x | 1,000x | 100,000x | Fate threshold (any warp) |
|---|---|---|---|---|
| 1 | 1.9 d | 2.6 d | 25.7 d | 9.27 d |
| 3 | 2.7 d | 11.0 d | 0.4 d | 18.36 d |
| 5 | 0.004 d | 0.04 d | 0.6 d | 23.07 d |
| 7 | survives | 20.6 d | 23.3 d | 14.03 d |

Both have the right statistics (the fate gives P(fail within 30 d) = 0.948 over 2,000 seeds; theory 0.950), but only the fate gives the same craft the same day at every warp. The random number decides how much the part can take; the physics decides when that is reached.

**Rules that keep it identical across warps:**
1. Key every draw on (seed, stable entity id, mode or process, and an index that comes from the physics: the launch, the k-th arrival, the n-th start). Never on the step index, the frame index, the thread, or the order of evaluation. Then D127's parallel workers can run in any order.
2. Accumulate H on the canonical steps of section 3 (state-chosen), and find the failure epoch as an event: the root of H(t) = E, in closed form where the rate is constant over the step (t = (E - H)/lambda). Today H only accumulates on live ticks (audit section 5: "no failure off live ticks"); a coasting computer never ages. That is a gap, not a warp break.
3. Repeated random processes (micrometeoroid hits, single-event upsets, a relay's random sticking) use the same trick with a sequence of thresholds: the k-th event happens when the integrated rate since the (k-1)-th crosses E_k = -ln u_k, u_k keyed on (seed, entity, process, k). A rate that depends on the state (radiation flux, temperature) is handled by the same integral (a time change), so the arrivals still sit at physically caused epochs.
4. Manufacturing variation (a valve's leak rate, a cell's capacity within tolerance, a flaw's size) is drawn once at build, keyed on (seed, craft serial, part id, parameter), and stored. It is a parameter of the physics, not an event.

**What must never be random** (the owner: "it cannot be anything with a broad effect"):
- The environment seen by more than one craft (solar flux, space weather, atmosphere density). If it varies, it is one seeded deterministic time series keyed on the epoch only, shared by every craft, never per craft or per step.
- Anything in the integrator, the scheduler, or the choice of step, tier or model.
- A failure without a cause. The hazard rate is the cause (wear, temperature, dose, cycles); the fate only fixes the margin.
- Draws whose count depends on the step count. A "roll per step" changes outcomes whenever a code or model change changes the step sequence, even at a fixed warp.

**Reproducibility trade-offs, plainly:**
- Same save, same inputs at the same epochs: the same failures at the same epochs at every warp (by construction).
- Relaunching the same craft at a different epoch: different fates (canon keys on the launch epoch). This is intended: a repeat mission is not a replay.
- Renaming a part id changes its fate. Ids used as keys must be stable across edits of unrelated parts.
- A model or code change that changes H changes when a fate is reached, but not the fate itself, so outcomes move smoothly rather than being re-rolled.


## 6. Recommendation for design session 2 (time and warp)

**Recommendation: keep D129 exactly as decided and make it a scheduler contract with five clauses, each backed by a test in the `warp_course.rs` style.** None of this reopens canon; it is how canon becomes cheap enough for the 2026-11 targets.

1. **The step contract.** Every domain advances only in steps whose ends come from its own state, its scheduled inputs (orders, eclipse boundaries, daily inputs) and its own lattice. A frame never splits a step; it runs the whole steps that end at or before its target and shows the rest by read-only interpolation or closed form. Merging or lengthening steps is a decision of the state, never of the warp. (Measured: bit-identical in every case; splitting breaks it, 1e-5 to 1e-3.)
2. **The event contract.** Every threshold (thermostat, relief, shed, overheat, a fate H = E, a sensor trigger) is an event function of its domain, located inside the step, applied at the exact root, with the rest of the grid unit covered in the new mode; epochs are sunscatter's `Epoch` (integer seconds plus fraction), so no new time type is needed. (Measured: 0.000 to 0.12 s against references; rounding the switch instead biased a 9-month tank.)
3. **The law contract** (D121's "one law per subsystem over any interval", made precise). Each law states its exact or approximate form, its step-size controller from state only (an error estimate where it is not exact), and its caps where coefficients move with the state. Closed forms for batteries, tank phases, consumables; linearly implicit or exponential steps for thermal networks.
4. **Chatter by sliding modes.** A switch whose both sides push into its surface (a zero-deadband thermostat, a relief valve with no blowdown, a regulator) becomes a continuous regulating mode, entered and left as events. Wear counted in cycles comes from a law (duty, period), not from counting simulated chatter.
5. **Quiescence by periodic-steady-state jumps** for inactive coast warp: sections at a forcing boundary, k-orbit cycles, a phase-locked step lattice, envelope following with a tolerance, and (to build next) an event-exclusion test over each jump. The craft's software (D128 layers 1 and 2) must be part of the certified state: a program waiting on a periodic condition is fine, one counting iterations is not.

**Tiers** (scope 3.3), all chosen by state as canon requires: physics warp keeps the fixed 20 ms tick with sub-rates inside (multirate, D121), and is capped by compute (section 4.4), not by determinism; coast warp runs clauses 1 to 4; inactive coast warp adds clause 5 and the coarse laws of D119.

**Domains under one scheduler** (scope 3.2): conservative lookahead as in case 4.5. The domain that produces events leads by one of its own steps; dependent domains catch up to its epoch with the inputs that held over that step; ties are ordered by (epoch, domain index, event sequence). Two-way couplings take a one-step lag (deterministic) or an iteration inside the step; which one per coupling is a design choice for the session.

**Open questions for the owner (plain words):**
1. Thermostats and relief valves with tiny or zero deadband: is it acceptable that they become "hold at the set point" when they would otherwise switch thousands of times an hour, so the craft gets the same heat and vented mass but not a literal click-by-click record? (Recommended: yes, with wear computed from a cycle law.)
2. Inactive coast warp: is it acceptable that a quiet craft's orbits are copied from a certified cycle (with a recorded error bound, here about 1e-4 K and under 0.13 % of heater energy) instead of every orbit being integrated? Every craft still runs; its script still sees every event at its epoch. (Recommended: yes, behind the event-exclusion guard.)
3. How exact must a 9-month figure be across code versions, rather than across warps? Across warps it is bit-exact. Across versions, a better integrator moves a relief-valve epoch by seconds after months (measured: 11 s over 9 months at a 1 s cap). Is that the right kind of difference to accept?
4. Physics warp near a planet is limited by compute (a 1,000-part craft costs 103 ms per 20 ms tick today). Is it acceptable that warp there falls to what the cores can carry, rather than the model being simplified?
5. Should stochastic processes beyond D105's fates (micrometeoroids, single-event upsets) exist in v0.2 at all? If yes, they follow section 5's rules.

**What in today's code would have to change** (read-only survey at 9922c93; the audit's own list in `docs/technical/warp-invariance.md` sections 4, 5 and 7 remains the authority):
- `crates/sim/src/systems/power.rs:119-122`: shedding judged on "what the batteries can give over the step" (charge / dt) makes the outcome depend on the step length. Replace with the closed-form law and the shed and empty epochs as events (case 4.3).
- `crates/sim/src/systems/ship.rs:64-68` and the tick-length `power::step_ship` and `crew::step` calls after it: the slow plumbing covers "the time since it last stepped" while power and crew step by the tick only (audit section 5). One law over any interval.
- `crates/sim/src/systems/mod.rs:48` (`STEP = 60 s` systems lattice): becomes the maximum of a state-chosen step with events inside it, rather than the place events land.
- `crates/sim/src/vessel/aerothermal.rs:64,74` (`STEP_CHANGE_K`, `SUN_SAME` skip): today's skip is a heuristic quiescence that errs up to 1.3 K (scale-performance.md section 7). Replace with error-controlled steps plus the periodic-steady-state certificate with its recorded bound.
- `crates/sim/src/thermal/mod.rs:250,272` (`step`, `step_to`): keep the block Gauss-Seidel solve; add an embedded error estimate for state-only step control and the event functions (overheat, thermostats) located inside steps.
- `crates/sim/src/vessel/mod.rs:470` (`advance_grounded`): a landed vessel's thermal network is never stepped (audit).
- `crates/sim/src/systems/failure/mod.rs` and `fates.rs`: keep the fates; accumulate hazard on the coast and landed steps too, and find the failure epoch as the root of H(t) = E.
- `crates/sim/src/vessel/helm.rs`, `crates/game/src/commands.rs`, `crates/sim/src/vessel/hold.rs`, `crates/sim/src/vessel/bubble.rs`, `crates/game/src/state.rs` (rotation inputs only at physics warp): the audit's D1, D2, D3 and D6 divergences, unchanged by this research.


## 7. Sources and number labels

**Primary and technical sources** (read or checked tonight; "recalled" means from prior knowledge, not re-read tonight):
- M. Hochbruck, A. Ostermann, "Exponential integrators", Acta Numerica 19 (2010) 209-286, doi:10.1017/S0962492910000048. [PDF](https://www.math.kit.edu/ianm3/lehre/semjahnke2012w/media/exp-int-acta-numerica.pdf)
- J. G. Verwer, E. J. Spee, J. G. Blom, W. Hundsdorfer, "A second-order Rosenbrock method applied to photochemical dispersion problems", SIAM J. Sci. Comput. 20 (1999) 1456-1480 (the ROS2 method used in the spike; recalled).
- C. W. Gear, D. R. Wells, "Multirate linear multistep methods", BIT 24 (1984) 484-502. [link](https://link.springer.com/article/10.1007/BF01934907)
- A. Sandu, "A class of multirate infinitesimal GARK methods", SIAM J. Numer. Anal. 57 (2019) 2300-2327, doi:10.1137/18M1205492. [arXiv](https://arxiv.org/abs/1808.02759)
- D. Broman et al., "Determinate composition of FMUs for co-simulation", EMSOFT 2013. [PDF](https://people.kth.se/~dbro/papers/broman-et-al-2013-fmu-co-simulation.pdf); C. Gomes et al., "Co-simulation: state of the art" (2017). [arXiv](https://arxiv.org/abs/1702.00686)
- E. Kofman, S. Junco, "Quantized-state systems: a DEVS approach for continuous system simulation", Trans. SCS 18(3) (2001) 123-132. [ACM](https://dl.acm.org/doi/10.5555/609891.609893)
- K. H. Johansson, M. Egerstedt, J. Lygeros, S. Sastry, "On the regularization of Zeno hybrid automata", Systems and Control Letters (1999); "Simulation of Zeno hybrid automata", CDC 1999. [PDF](http://people.eecs.berkeley.edu/~sastry/pubs/OldSastryALL/Johansson.Simulation.1999.pdf). A. F. Filippov, Differential Equations with Discontinuous Righthand Sides (1988; recalled).
- L. R. Petzold, "An efficient numerical method for highly oscillatory ordinary differential equations", SIAM J. Numer. Anal. 18 (1981) 455-479 (multirevolution / quasi-envelope). [Semantic Scholar](https://www.semanticscholar.org/paper/An-efficient-numerical-method-for-highly-ordinary-Petzold/d3083fc76725780781976ef8ad839331d775e8ce)
- J. Makino, block (power-of-two) individual time steps for N-body codes (1991; recalled), as discussed in F. I. Pelupessy et al., "N-body integrators with individual time steps from hierarchical splitting" (2012). [arXiv](https://arxiv.org/abs/1205.5668)
- J. K. Salmon, M. A. Moraes, R. O. Dror, D. E. Shaw, "Parallel random numbers: as easy as 1, 2, 3", SC '11, doi:10.1145/2063384.2063405 (Philox, Threefry: counter-based generators). [ACM](https://dl.acm.org/doi/10.1145/2063384.2063405)
- G. Fiedler, "Fix your timestep!" (fixed step, accumulator, interpolation for display). [link](https://gafferongames.com/post/fix_your_timestep/)
- Factorio, Friday Facts #188 "Bug, bug, desync" (deterministic lockstep). [link](https://factorio.com/blog/post/fff-188); Factorio wiki, Desynchronization. [link](https://wiki.factorio.com/Desynchronization)
- Rapier, determinism guide (the `enhanced-determinism` feature: cross-platform determinism on strict IEEE 754 targets). [link](https://rapier.rs/docs/user_guides/rust/determinism)
- C&R Technologies, SINDA/FLUINT (explicit routines limited by the smallest capacitance-to-conductance ratio, CSGMIN; implicit forward-backward and steady-state routines; recalled from the user manual). [overview](https://www.crtech.com/sites/default/files/product-files/SindaFluint.pdf). ESATAN-TMS uses the same lumped-parameter node/conductor formulation (recalled).
- M. Schweiger, "Spacecraft simulation and visualisation with Orbiter 2006" (time-step subsampling; "orbit stabilisation" that disables numerical integration at high time acceleration). [PDF](https://trajectory.estec.esa.int/Astro/3rd-astro-workshop-presentations/Spacecraft%20Simulation%20and%20Visualisation%20with%20Orbiter%202006.pdf)
- Principia (KSP mod): vessel histories and predictions integrated by its own integrators, independent of the game's frames. [repo](https://github.com/mockingbirdnest/Principia), [concepts](https://github.com/mockingbirdnest/Principia/wiki/Concepts). The claim that its integration is frame-independent is inferred from its design, not verified in code tonight.
- Kerbalism: background simulation of unloaded vessels "coherent regardless of warp speed" via an order-less solver at arbitrary step lengths. [wiki](https://github.com/Kerbalism/Kerbalism/wiki/TechGuide-~-Background-Simulation). sunscatter's limbo.md records per-frame sunlight sampling as a known Kerbalism problem; I did not verify which is current.
- sunscatter (read-only, 9922c93): `docs/plans/scope-2026-11.md` section 3.3; `docs/technical/warp-invariance.md`, `limbo.md`, `frame-order.md`, `scale-performance.md`; decisions D105, D119, D121, D127, D128, D129; `crates/sim/src/systems/failure/fates.rs`, `power.rs`, `ship.rs`, `systems/mod.rs`, `vessel/aerothermal.rs`, `thermal/mod.rs`, `vessel/mod.rs`.

**Number labels.** Every number in sections 4.1 to 4.7 and in the randomness table is **[measured]** by the prototype on the M2 Pro (Mac14,9) at load averages of 2 to 11 (other users' work), single-threaded release builds; wall times vary by perhaps 20 % with load, event lists and hashes do not. Numbers marked [derived] are arithmetic on measured or stated values (the 0.864 s budget, stability limits 2 tau, the 1.2x and 0.19x live-tick ceilings from sunscatter's measured 17 ms and 103 ms ticks). [inferred]: the comparison of the spike's 38 ns per node per step with sunscatter's solver, and how a real 5,000-part network (not a path graph) would scale. [guessed]: none of the conclusions rest on a guess; the model parameters (chip 2 J/K, 3 W heat leak, 700 W array, 12 kN riser) are illustrative choices, picked to put each case near its margin, and say so.

**Limits of the experiment.** Each model is small and illustrative. The thermal network is a path graph (tridiagonal solve), not sunscatter's tree-plus-links network. Cross-machine bit identity was not tested (the spike uses Rust's std `exp`/`ln`; canon routes them through libm). The live tier was a point mass. Cross-craft interactions (the audit's D1 and D3) were not modelled.

**Artefacts.** Prototype: `/Users/ben/director/logs/night-2026-11-03/warp-spike/` (`src/harness.rs` frame loop and schemes; `src/thermal.rs`, `tank.rs`, `battery.rs`, `chute.rs`, `coupled.rs`; `analyse.py`; `rng/rng_demo.py`); raw runs in `warp-spike/out/*.jsonl`. Rerun, e.g.: `cargo build --release && ./target/release/warp-spike c1 40 5 1 0 B,A_exp,A_imp,A_sub 1,100,1000,100000`.


## Appendix B. Results against the pre-registered criteria

| Criterion | Verdict | Evidence |
|---|---|---|
| P1: A fails | **met** (all four cases) | case 1: A_exp stable at 100x, overheats at 1,000x; case 2: 663 / 659 / 627 / 50 valve cycles; case 3: survives at 1,000x, battery empty at 100,000x; case 4: chute torn and crash at 100x, riser +6 % at 4x |
| P2: B bit-identical across 1x, 100x, 1,000x, 100,000x | **met** in every case and variant | equal hashes, identical event lists (sections 4.1 to 4.7) |
| P3: B matches the reference | **met** for cases 1, 3, 4 (0.000 to 0.145 s); **case 2 met only after a change**: the uncapped closed form drifted 1,289 s in 9 months; with a 1 s open-phase cap, 11.4 s (tolerance 10 s: marginally missed, and the reference itself has ms-level event rounding) | 4.2 |
| P4: <= 86 ms per craft-day at 100,000x | **met** for networks up to a few hundred nodes; **5,000 nodes met only with the periodic-steady-state jump** (24 ms periodic, 419 ms drifting, against 1,026 ms without) | 4.6, 4.7 |
| P5: chattering within budget, invariant, within 1 % | **met** with sliding modes (thermostat 0.01 %, tank vented mass equal to 0.1 kg) | 4.1, 4.2 |
| P6: B_split not bit-identical; <= 1e-12 for exact linear laws | **partly**: battery 5.5e-13 (390 Wh, met) and 7.3e-12 (400 Wh, just above); tank 1.9e-4 to 1.4e-3 (not met: a threshold turns round-off into a ms shift) | 4.2, 4.3 |

## Appendix A. Pre-registered experiment criteria
Written at 20:45 local, before any prototype code existed. Not edited after the runs (results go in section 4).

Harness: a frame loop at 60 frames per real second; each frame's span is warp/60 simulated seconds (1x: 16.7 ms; 100x: 1.67 s; 1e3x: 16.7 s; 1e5x: 1,667 s). Time in B is an integer count of grid units (1/1024 s).

Schemes:
- A_exp: one explicit Euler step per frame (step = frame span), thresholds checked at frame ends, inputs (sunlight etc.) sampled at frame start. The Kerbalism-style background update.
- A_imp: the same with backward Euler (stable at any step). The "fixed the blow-up" naive scheme.
- A_sub: substeps of at most h_max, but the substep length is span/ceil(span/h_max), so the step sequence depends on the warp. The "careful" naive scheme.
- B: scope 3.3. Steps chosen from the state only, power-of-two lengths aligned to the grid (block time steps), frames only decide how many whole steps run (a frame never splits a step), thresholds located inside the step by bisection on the step's own solution and quantised up to the grid, analytic or exponential laws where the subsystem is (near) linear.
- B_split: B, but each frame end truncates the step in progress (a frame CAN split a step). Measures hypothesis 2's "long and short steps agree to round-off".

Pre-registered criteria:
- P1 (A fails): in at least one case, A gives a different discrete outcome (a failure, a load shed, a chute rip, or the relief cycle count off by more than 1 %) between 100x and 1e3x, or between 1x and 1e5x. Expected: A_exp on case 1 is stable at 100x and diverges ("overheats") at 1e3x.
- P2 (B is invariant): in every case, B at 1x, 100x, 1e3x and 1e5x gives the identical event list (kind, order, integer grid epoch) and a bit-identical final state (equal hashes of the f64 bits). Any difference is a failure of the hypothesis.
- P3 (B is right): B's discrete outcomes equal a converged fine-step reference. Event times within 1 s (cases 1, 3), within 10 s (tank relief), the chute within one controller sample; final temperatures within 0.5 K, state of charge within 0.5 %, liquid mass within 0.1 %.
- P4 (B is cheap enough): B's wall time per simulated day for one craft at 1e5x is at most 86 ms (so 10 craft fit 0.86 s on one thread; 602 ms per craft-day with 7 threads). Case 1 is measured at 40 and 200 nodes; a 5,000-part craft is extrapolated and labelled inferred.
- P5 (chattering): with the relief valve's and the thermostat's deadbands shrunk toward zero, B's event count and cost are recorded. Success if a treatment chosen by state (sliding mode or a periodic-orbit jump) keeps the cost within P4, stays bit-identical across warps, and keeps the outcome (vented mass, heater energy) within 1 % of the fully resolved run.
- P6 (splitting): B_split is expected NOT bit-identical; for exactly solved linear laws (battery, the tank's closed phase) the difference is expected at or below 1e-12 relative, for the stepped nonlinear thermal network possibly above it.

