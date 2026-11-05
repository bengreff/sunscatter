# Research: Rocket Lab Electron vs SpaceX Falcon 1 — first calibration vehicle

Status: complete, first pass (2026-11-04). Primary sources pulled and parsed directly (Electron PUG v8.0, Falcon 1 PUG Rev. 7, Spaceflight Now Flight 4 timeline); everything else is tagged per the trust scale below.

Owner's calibration order (scope-2026-11.md §3.1, answer 7): "Electron or Falcon 1 (small and well documented), then Falcon 9 + Dragon rebuilt from the same types, then a probe." This doc decides between the two and lays out the part/component breakdown (owner's definitions, §3.1) for the one chosen.

Data trust tags: **P** published (source given), **D** derived (method given), **E** estimate (basis given), **U** unknown.

---

## 1. Recommendation

**Calibrate on Rocket Lab Electron first, keep Falcon 1 as the fallback/comparison.**

1. Electron has a current, maintained, free public primary source: the Payload User's Guide (PUG), now at v8.0 (2025/2026), with a predecessor v7.0 — both pulled and parsed for this doc. Falcon 1's only primary source is a single 2008 PDF (Rev. 7) for a vehicle retired in 2009; it is static and smaller.
2. Electron has ~99 flights (95 successes, 9 of them HASTE suborbital) as of late 2026, so there are dozens of real missions, webcasts and post-flight write-ups to cross-check against. Falcon 1 flew 5 times total, 2 successes (Flight 4 "RatSat", Flight 5 "RazakSAT") — thin statistics, and no orbital Falcon 1 flight was crewed or carried a public-telemetry science payload.
3. Electron's PUG gives a published nominal flight profile with nine named timed events (liftoff to kick-stage separation) and a payload-performance chart (mass vs. altitude vs. inclination) — more structured flight data than Falcon 1's PUG, which gives one generic axial-acceleration-vs-time plot and a flight-event table, not a full altitude/velocity profile.
4. Electron's key component — the electric-pump-fed Rutherford with brushless DC motors and jettisoned battery packs — is the more novel system graph exercise for sunscatter: it forces us to model a power graph (battery → motor → pump) feeding a fluid graph (propellant feed), which is exactly the cross-domain coupling the engine-family work (scope §3.1, decision 30) needs a first worked example of. Falcon 1's Merlin 1C/Kestrel are conventional turbopump/pressure-fed engines, already closer to what Falcon 9 (the next calibration target) will need, so Electron is the better *first* step and Falcon 1 is a useful second data point for the same part types without duplicating effort.
5. Electron's tanks are linerless CFRP, both stages, with a stated (if round) 40% mass saving over aluminium (PUG) — a cleaner test of the derived-mass-from-geometry-and-material path (scope §3.1) than Falcon 1's conventional aluminium monocoque tanks, which sunscatter will need anyway for Falcon 9 later.
6. Electron is simpler to place as PARTS: 2 stages + kick stage + fairing + payload adapter, all data-sheeted at 1.2 m max diameter — small enough to hand-check every derived mass against the PUG's round numbers.
7. Both vehicles are "well documented" per the owner's framing (scope §3.1, answer 7), so neither choice reopens or contradicts canon; this is new ground, not a revisit.
8. Risk: Rocket Lab publishes fewer internal numbers than SpaceX did for Falcon 1 (no public dry-mass-per-stage, no tank wall thickness, Curie's propellant is still undisclosed) — more E/U tags than Falcon 1 would need. This is an acceptable trade for richer flight validation data and the more interesting component.
9. Recommendation stands even accounting for memory's "dates are ceilings" — this is not a deadline call, it is a data-availability and interest call, and Electron wins on both axes independent of schedule.
10. Net: Electron primary, full part/component breakdown in §2; Falcon 1 secondary, lower-depth breakdown in §4, specifically to cross-validate the generic `tank`, `engine`, `fairing` and `stage separation` types once Electron's are built.

## 2. Part breakdown — Electron

Sources for this section unless noted: [E1] Rocket Lab Electron Payload User's Guide v8.0 (PUG8, 2025/26, rocketlabcorp.com); [E2] PUG v7.0; [E3] Wikipedia "Rocket Lab Electron"; [E4] Wikipedia "Rutherford (rocket engine)"; [E5] Wikipedia/HandWiki "Curie (rocket engine)". Overall vehicle: length 18 m, max diameter 1.2 m, 2 stages + Kick Stage, lift-off mass 13,000 kg, material "Carbon Fiber Composite/Monocoque", propellant LOX/RP-1 (all **P**, [E1] "Electron Launch Vehicle Parts" overview page).

### 2.1 PART: First-stage tank/structure assembly
*Shape:* cylinder, 1.2 m diameter, ~12.1 m length (**D/E**, [E3] gives first-stage height 12.1 m; not itemised in the PUG). *Material:* linerless carbon-composite common-bulkhead tanks for LOX and RP-1 (**P**, [E1]/[E2] "Electron utilizes advanced carbon composite technologies throughout the launch vehicle structures, including all of Electron's propellant tanks... linerless common bulkhead tanks for LOx and RP-1"). Dry mass: not published (**U** — PUG gives no stage dry mass; see §5 gaps).

**Components (fluid graph unless noted):**
- LOX tank volume and RP-1 tank volume: not individually published (**U**). First-stage total propellant mass is not published either; only stage 2's (2,000 kg, **P**) is given. Total liftoff mass 13,000 kg minus kick stage (40 kg dry + propellant, **U**) minus stage 2 (dry + 2,000 kg propellant) minus payload gives stage-1 wet mass by subtraction (**D**, needs stage dry masses first — circular without more data, flagged in §5).
- Common bulkhead between LOX and RP-1 tanks (**P**, construction stated, no geometry).
- Tank pressurisation: PUG does not state the method for stage 1 explicitly; Rocket Lab's engine description implies a pressurant system feeds pump inlet pressure (**E**, inferred from the stage-2 text "a pressurant system is used to provide enough pump inlet pressure to safely operate" [E1], likely common to both stages).
- Interstage: separation system "Pneumatic Pusher" (**P**, [E1] spec block).

### 2.2 PART: Rutherford sea-level engine ×9 (one part each, per owner's example in scope §3.1)
*Shape:* single rigid body, 0.25 m diameter (**P**, [E4] infobox), engine-only dry mass 35 kg (**P**, [E4], thrust-to-weight ratio 72.8 corroborates: 24.9 kN / (35 kg × 9.80665) ≈ 72.6, **D** check). *Material:* regeneratively-cooled thrust chamber, injector, pumps and main propellant valves are laser-powder-bed-fused (DMLS) metal — alloy not stated (**P** for the process, [E4]; **U** for the alloy).

**Components:**
| Component | Function | Parameters | Mass | Graph |
|---|---|---|---|---|
| Thrust chamber + regen-cooled nozzle | Combustion, cooling | chamber pressure ≈3 MPa (**E**, secondary sources, no SpaceX/Rocket Lab primary figure found), mixture ratio ≈2.6 (**E**), thrust 24.9 kN SL per engine (**D**, 224.3 kN total ÷ 9, [E3]; PUG rounds to "5,600 lbf (24 kN)" [E1]), Isp 311 s SL (**P**, [E1]) | part of 35 kg dry mass, not separated (**U**) | fluid + thermal |
| Dual brushless DC electric pump motors | Drive LOX and RP-1 pumps | 37 kW (50 hp) each at 40,000 rpm (**E**, secondary/blog-level sourcing — not found in a Rocket Lab primary document; treat as unverified until a primary source is found) | **U** | power → fluid (the exact coupling sunscatter needs to model) |
| LOX + RP-1 centrifugal pumps | Raise propellant to chamber pressure | **U** (no published flow rate, head, or efficiency numbers beyond "90% efficiency" clause for the electric cycle vs. ~50% for gas-generator, **E**, [E1] "can achieve 90% efficiency") | **U** | fluid |
| Main propellant valves ×2 (LOX, RP-1) | Start/stop/throttle flow | **U** | **U** | fluid + data (actuation) |
| Electromechanical thrust-vector actuators ×2 axes | Gimbal the engine for pitch/yaw | **P** that TVC is electromechanical ([E1]); range/rate **U** | **U** | power + load + data |
| Igniter | Start combustion | not described in any source found (**U**) | **U** | data |

### 2.3 PART: First-stage battery pack(s) (power source for stage-1 Rutherfords)
Not broken out as a separate part in the PUG; stage 1 is described as drawing on "a lithium polymer battery" system collectively able to deliver "over 1 MW (1,300 hp) of electric power" across the 9 engines (**E**, secondary sourcing, order-of-magnitude only: 9 × 2 × 37 kW ≈ 666 kW matches roughly if the 1 MW figure includes margin/peak draw — **D** cross-check, not exact). Chemistry: lithium polymer (**P**, [E1]). Count, voltage, cell count, pack mass: **U**.

### 2.4 PART: Second-stage tank/structure assembly
*Shape:* cylinder, 1.2 m diameter, 2.4 m length (**E**, [E3]; not itemised in PUG). *Material:* same linerless carbon-composite common-bulkhead construction as stage 1 (**P**, [E1]). Propellant load ≈2,000 kg LOX+RP-1 (**P**, [E1] "The 1.2 m diameter second stage has approximately 2,000 kg of propellant on board"). Dry mass: **U**.

**Components:**
- Three High Voltage Batteries (HVBs): two power the LOX/RP-1 pumps "until depletion", a third takes over for the remainder of the burn; the first two are then jettisoned to cut mass (**P**, [E1] "During the second stage burn, two HVBs power the electric pumps until depletion, when a third HVB takes over... Upon depletion, the first two HVBs are jettisoned from Electron to reduce mass and increase performance in flight"). Individual HVB mass, chemistry details beyond "lithium polymer": **U**. Battery jettison event timed at L+388 s in the PUG's example flight profile (**P**, [E1], §3).
- Pressurant system: feeds pump inlet pressure (**P**, text only, no bottle count/volume/pressure, **U**).
- Cold-gas RCS for roll control during stage-2 burn (**P**, [E1]; thruster count/thrust **U** for this specific set — not the Kick Stage's six, which are specified separately in §2.6).

### 2.5 PART: Rutherford Vacuum engine ×1 (second stage)
Identical internals to the sea-level Rutherford except a larger expansion-ratio nozzle (**P**, [E1] "The Stage 1 and Stage 2 Rutherford engines are identical, with the exception of a larger expansion ratio nozzle for Stage 2"). Thrust 25.8 kN / 5,800 lbf vacuum (**P**, [E1]), Isp 343 s (**P**, [E1]). Burn time ≈373 s, derived from the PUG's own flight-profile timestamps: ignition L+162 s, cutoff L+535 s (**D**, [E1] §"Example Flight Profile"); PUG's prose separately claims "approximately five minutes" (300 s) for the burn (**P**, [E1]) — the two don't quite agree (373 s vs. 300 s), logged as a model-input mismatch rather than silently picking one (per scope §3.1's rule on published-vs-derived conflicts).

### 2.6 PART: Kick Stage (third stage / Photon bus precursor)
*Shape:* disc/short cylinder, 1.2 m diameter, 405 mm height (**P**, [E1] "Kick Stage Specifications"). Dry mass 40 kg (**P**, [E1]). Material: carbon composite (**P**, [E1]).

**Components:**
| Component | Function | Parameters | Mass | Graph |
|---|---|---|---|---|
| Curie engine | Main orbit-raising/circularisation burn | pressure-fed, additively manufactured, re-startable, storable bipropellant (**P**, [E1]/[E5]); propellant chemistry unpublished — Rocket Lab calls it a "green" bipropellant (**U**, [E5]); Isp ≈320 s (**E**, secondary, not in the PUG); main-engine thrust is not stated separately from the RCS figure in the PUG (ambiguity, flagged §5) | **U** (included in 40 kg dry mass or not — unclear) | fluid + thermal |
| Cold-gas RCS thrusters ×6, 2 pods | Attitude hold and low-rate pointing before payload separation | thrust 120 N (**P**, [E1] — though this number sits under the Kick Stage spec block and may be a per-pod or per-thruster total rather than Curie's main-engine thrust; see §5) | **U** | fluid + load |
| Composite propellant tanks (integrated with Curie into one module) | Store Curie's bipropellant | "high propellant mass fraction," scalable (**P**, [E1]); volumes **U** | **U** | fluid |
| Tank pressurisation system | Pressurise Curie's feed | **U** | **U** | fluid |
| SEIP (Standard Electrical Interface Panel) | Payload power/data/command interface, sits on top of Kick Stage | **P**, [E1] | **U** | data + power |

### 2.7 PART: Payload fairing (×2 halves)
*Shape:* composite split clam-shell, 2.5 m length, 1.2 m diameter, 44 kg total mass (both halves) (**P**, [E1] "The Fairing" spec block). Separation: pneumatic unlocking + springs, each half rotates on a hinge away from the payload (**P**, [E1]). Acoustic protection: foam sheets (**P**, [E1]). Fairing separation timed at L+184 s, altitude ≈126 km in the PUG's example profile (**P**, [E1]).

### 2.8 PART: Payload adapter / dispensers (mission-dependent, not core vehicle)
Options published: Rocket Lab's own Maxwell CubeSat dispenser series; RUAG PAS (Payload Adapter System); Planetary Systems Corporation Motorized Light Band (MLB), 8–38 inch bolt-circle diameters; PSC canisterized CubeSat dispenser, 2 kg/U capability, non-pyrotechnic (**P**, [E1], "Separation Systems" section). Masses of individual adapters: **U** (mission-specific, not tabulated).

### 2.9 System-graph notes for Electron (for the generic part/component model)
- **Power graph**: battery pack(s) → brushless DC motor controller → motor → pump shaft, repeated per engine per stage; plus the mid-burn HVB handover on stage 2 is a live power-graph topology change during flight (switch, then jettison a now-dead node) — a good first test of the power solver handling a node disappearing mid-burn.
- **Fluid graph**: pump → main valve → injector → chamber, LOX and RP-1 separately, joined at the injector; tank pressurant line feeds pump inlet, parallel to the main feed line.
- **Data graph**: engine controllers, SEIP, and presumably a flight computer are mentioned narratively but never specified (**U** across the board) — this is the thinnest graph Rocket Lab publishes.
- **Load graph**: thrust transmitted through the TVC actuator mount into the tank/structure part; fairing and Kick Stage separations are discrete load-path breaks at published times.
- **Thermal graph**: regenerative cooling (RP-1 through the chamber) is the only thermal path stated; no temperatures or flow splits given.

## 3. Performance and flight data — Electron

### 3.1 Masses
| Quantity | Value | Tag | Source |
|---|---|---|---|
| Lift-off mass | 13,000 kg | P | [E1] overview spec block |
| Stage 2 propellant mass | ≈2,000 kg | P | [E1] "approximately 2,000 kg of propellant on board" |
| Kick Stage dry mass | 40 kg | P | [E1] Kick Stage spec block |
| Fairing mass (both halves) | 44 kg | P | [E1] Fairing spec block |
| Rutherford engine dry mass | 35 kg each | P | [E4] Wikipedia infobox |
| Stage 1 / Stage 2 dry mass, stage 1 propellant mass | — | U | not published anywhere found; see §5 |
| Nominal payload capacity | 200 kg to 500 km SSO; up to 300 kg to LEO | P | [E1] overview + payload section ("boosting... total payload lift capacity up to 300 kg / 660 lbm") |

### 3.2 Thrust and Isp
| Quantity | Value | Tag | Source |
|---|---|---|---|
| Stage 1 thrust, per engine, sea level | 5,600 lbf (24.9 kN) | P | [E1] Stage 1 spec block |
| Stage 1 thrust, total, sea level (×9) | 224.3 kN (lift-off) to 234 kN (vacuum) | D/P | [E3] Wikipedia infobox, consistent with 9 × 24.9 kN ≈ 224 kN |
| Stage 1 Isp, sea level | 311 s | P | [E1] Stage 1 spec block |
| Stage 2 thrust, vacuum | 5,800 lbf (25.8 kN) | P | [E1] Stage 2 spec block |
| Stage 2 Isp, vacuum | 343 s | P | [E1] Stage 2 spec block |

### 3.3 Burn times and staging (PUG "Example Flight Profile", all **P**, [E1])
| Event | Time | Altitude |
|---|---|---|
| Liftoff | L+0 s | — |
| Stage 1 separation | L+155 s | ≈78 km |
| Stage 2 engine ignition | L+162 s | ≈82 km |
| Fairing separation | L+184 s | ≈126 km |
| Battery (HVB) jettison | L+388 s | — |
| Stage 2 engine cutoff | L+535 s | orbit reached |
| Stage 2 separation (from Kick Stage) | L+539 s | — |
| Kick Stage ignition, Kick Stage engine cutoff, payload separation | mission-dependent | — |

Derived burn times (**D**, from the above): stage 1 burn ≈155 s; stage 1/stage 2 gap (coast + ignition) ≈7 s; stage 2 burn ≈373 s (ignition to cutoff) — this conflicts with the PUG's own prose claim of "approximately five minutes" (300 s, **P**, §2.4 text) for the stage-2 burn; both are published by Rocket Lab in the same document, logged as a mismatch rather than picked silently, per scope §3.1.

### 3.4 Trajectory / environment data
- Payload performance curves (mass vs. circular altitude, 40°/60°/80°/SSO inclinations): payload mass ranges ≈150–290 kg across 350–1,350 km altitude (**P**, [E1] "Electron Performance Curves" graph — read off the published chart, not a table, so treat as **E** precision even though the source is **P**).
- Elliptical-orbit performance (perigee 180 km, inclination 39°, launched due east from Māhia): payload mass ≈285–313 kg across apogee 300–800 km (**P/E** same caveat, [E1]).
- Orbit injection accuracy, typical 500 km SSO mission: inclination ±0.15°, perigee ±15 km, apogee ±15 km (**P**, [E1]).
- Attitude/deployment rates at separation: attitude ±5°, rate ±1.5°/s (**P**, [E1]).
- Max predicted acceleration envelope at the payload plate: axial envelope roughly −4 g to +8 g (compression positive), lateral roughly ±2.5 g (**P** source, **E** precision — read off the "Electron Acceleration MPE" chart, no tabulated values given) [E1].
- Max predicted shock response spectrum at the payload plate (Q=10): 50 g at 100 Hz, 700 g at 900 Hz, 700 g at 10,000 Hz (**P**, [E1] table).
- Launch-site inclination ranges: LC-1 Māhia 39°–120°; LC-2 Wallops 38°–60° (**P**, [E1]).

### 3.5 Flight heritage and real-mission telemetry
As of this research, Electron has flown ≈99 missions (95 successes, including 9 HASTE suborbital flights) since first flight 25 May 2017, with first orbital success 21 January 2018 ("Still Testing") (**P/E**, [E3] Wikipedia, which cites Rocket Lab's own mission pages). More than 50 satellites deployed as of the PUG's own count (**P**, [E1]). This gives dozens of public webcasts (Rocket Lab streams every launch with live telemetry overlays: altitude, velocity, downrange distance) as a source of actual flight-vs-nominal comparison, but no source found in this pass tabulates a specific mission's webcast telemetry (altitude/velocity vs. time) into text — that data exists only inside the video streams themselves (Rocket Lab's own YouTube/webcast archive) and would need to be read off-screen frame by frame, or requested from Rocket Lab directly, to get a numeric trajectory. This is logged as a gap (§5) rather than invented.

Notable missions with published qualitative detail: Flight 16 "Return to Sender" (20 Nov 2020) — first controlled ocean splashdown of a recovered stage 1, via parachute (**P**, [E3]); "Running Out of Fingers" (6 Dec 2019) — first guided re-entry telemetry from stage 1 using S-band and onboard flight computer (**P**, NASASpaceflight.com); a recovered Rutherford from a later catch attempt passed 5 full-duration hot-fire tests and reflew (**P**, [E3]).

## 4. Lower-depth breakdown — Falcon 1

Source unless noted: [F1] SpaceX Falcon 1 Launch Vehicle Payload User's Guide, Rev. 7, D000973, 2008 (PUG, pulled and parsed for this doc); [F2] Spaceflight Now Falcon 1 Flight 4 timeline (28 Sep 2008); [F3] Wikipedia "Falcon 1".

### 4.1 Overview (Table 2-1, Falcon 1 2006–2010 column, all **P** [F1])
| Quantity | Value |
|---|---|
| Length | 70 ft (21.3 m), both stages with fairing & interstage |
| Diameter | 5.5 ft (1.68 m) both stages; fairing 5 ft (1.52 m) |
| Stage 1 dry mass | 3,000 lb (1,361 kg) |
| Stage 1 usable propellant | 47,380 lb (21,487 kg) |
| Stage 2 dry mass | 1,200 lb (544 kg) |
| Stage 2 usable propellant | 8,900 lb (4,037 kg) |
| Fairing mass | 320 lb (145 kg), aluminium skin & stringer, biconic |
| Stage 1 structure | Monocoque, aluminium |
| Stage 2 structure | Monocoque, aluminium-lithium |
| Fairing diameter (stated separately in text) | 1.54 m |

Liftoff mass (**D**, sum of the above + Falcon 1's own stated dry/propellant figures): ≈1,361 + 21,487 (stage 1) + 544 + 4,037 (stage 2) + 145 (fairing) + payload (up to 1,010 kg) ≈ 27,500–28,600 kg depending on payload — no single "liftoff mass" figure found stated directly in the PUG; this is a derived sum (**D**), not picked from a published total.

### 4.2 Engines (Table 2-1, all **P** [F1])
| Quantity | Stage 1: Merlin 1C | Stage 2: Kestrel |
|---|---|---|
| Feed system | Turbopump, gas-generator cycle | Pressure-fed |
| Thrust | 78,000 lbf SL (347 kN) | 6,900 lbf vacuum (30.7 kN) |
| Isp (vacuum) | 300 s | 317 s |
| Attitude control, pitch/yaw | Hydraulic TVC | Electro-mechanical actuator TVC |
| Attitude control, roll | Turbopump exhaust | Cold-gas thrusters |
| Nominal burn time | 169 s | 418 s |
| Shutdown | Burn to depletion | Predetermined velocity cutoff |
| Restart capable | No | Yes |
| Tank pressurisation | Heated helium | Heated helium |
| Stage separation | Explosive bolts + pneumatic pushers | Marmon clamp + pneumatic pushers |

Cross-check against a different secondary figure found in search (not the PUG): Merlin 1C vacuum thrust ≈400 kN / 90,000 lbf quoted elsewhere (**E**, Wikipedia-sourced search snippet) — the PUG only states the 78,000 lbf sea-level figure for the Falcon 1 configuration, so that (**P**) figure is the one used here; the vacuum figure is a different, uncited claim and is not reconciled in this pass.

### 4.3 Mass properties constraint on payload (Table 3-1, **P** [F1])
Payload mass up to 1,010 kg (2,200 lb); CG offset from centreline and from the separation plane constrained per charts (Figures 3-1/3-2, not digitised here); payload stiffness requirement: axial and lateral fundamental frequency > 25 Hz.

### 4.4 Flight timeline — Flight 4 "RatSat", 28 Sep 2008 (first Falcon 1 orbital success) (**P**, [F2] Spaceflight Now timeline, built from the SpaceX webcast)
| Event | Time | Note |
|---|---|---|
| Liftoff | T+0:00 | Merlin 1C, 78,000 lbf SL |
| Max-Q | T+1:08 | |
| MECO (stage 1 cutoff) | T+2:37 | |
| Stage separation | T+2:42 | widened to 5 s gap after Flight 3's collision (was 1.5 s) |
| Stage 2 ignition | T+2:45 | Kestrel ramps to 6,900 lbf |
| 100 km (space boundary) | T+2:48 | |
| Fairing jettison | T+3:15 | |
| Orbital velocity reached | T+9:34 | 7.5 km/s |
| SECO | T+9:39 | orbit: perigee 205 mi (330 km), apogee 425 mi (684 km), inclination 9° |
| Loss of signal | T+10:18 | ground station horizon |

RatSat payload: 165 kg dummy mass simulator (**P**, [F3]/search). This flight is the better of the two Falcon 1 successes to validate against, since Flight 5 (RazakSAT) is less documented in open sources found this pass.

### 4.5 Quasi-static load factors (Table 3-6, **P** [F1])
| Flight event | Axial g (steady ± dynamic) | Lateral g |
|---|---|---|
| Ground handling | 0.5 | 2.0 |
| Liftoff | 1.2 ± 0.4 (0.8/1.6) | 0.50 |
| Max qα | 2.0 ± 1.0 (3.0/1.0) | 0.75 |
| Stage 1 burnout | 6.4 ± 1.25 (7.7/5.2) | 0.75 |
| Stage 2 ignition | 3.2±0.25 to 6.0±0.25 | 0.25 |
| Stage 2 burnout | 4.5±0.5 to 6.5±0.5 | 0.25 |

This is meaningfully richer than Electron's PUG, which gives only an envelope chart (§3.4) rather than one named value per flight event — Falcon 1's PUG is the better source for *validating a load-timeline model against discrete named events*, even though Electron wins on breadth of other data (flight count, performance curves, timeline granularity).

Vehicle bending/axial modes are also tabulated at three flight times (liftoff, max Q at 73.5 s, burnout at 168.1 s) — 1st bending ~5–13 Hz, 2nd bending ~14.5–27 Hz, axial ~16–43 Hz (**P**, [F1] Table 3-7) — a structural-model validation point Electron's PUG does not offer at all.

### 4.6 Not pursued to Electron's depth (left as U for this doc, would need a dedicated pass if Falcon 1 becomes primary)
Tank wall thickness and construction method; helium COPV count/volume/pressure; Merlin 1C turbopump internals (impeller stages, shaft speed, power); Kestrel's pressure-fed tank pressure and feed-line sizing; avionics/flight computer make; fairing separation mechanism detail beyond "explosive bolts + pneumatic pushers" (stage) and unspecified for the fairing itself.

## 5. Gaps and how we'd derive them

Ordered roughly by how much it blocks a first calibration build.

1. **Electron stage 1 and stage 2 dry masses.** Not published anywhere found. Derivation: build each tank as linerless CFRP (density ≈1,550–1,600 kg/m³ for a typical T-800/T-1000 epoxy layup, **E** generic material, not Rocket-Lab-specific) at a wall thickness set by MEOP and a CFRP hoop-stress allowable (the exact worked method the task names as the model case) — tank diameter 1.2 m is fixed (**P**), length split between stage 1 (≈12.1 m, **E**) and stage 2 (≈2.4 m, **E**) is the free variable to calibrate against the one number we do have (stage 2 propellant 2,000 kg, **P**, which fixes stage-2 tank volume given LOX/RP-1 densities, a **D** check on the assumed length). Sum 9 × 35 kg Rutherford engines (**P**) + estimated structure + TVC + avionics mass and compare the total to the only hard anchor, lift-off mass 13,000 kg (**P**) minus stage 2 propellant minus kick stage minus fairing minus a nominal payload; any gap becomes the part's unmodelled mass, exactly as scope §3.1 and the Falcon 9 data sheet precedent (`docs/vehicles/falcon9.md` §1.2) already do for Falcon 9.
2. **Rutherford's internal component split.** 35 kg total dry mass per engine is published; the chamber, pumps, motors, valves and TVC actuators that make it up are not individually weighed in any public source. Derivation: hold the motor power (37 kW × 2, **E**-tagged, unverified against a primary source) and burn time fixed, use typical brushless DC motor power density (≈3–5 kW/kg for small high-speed aerospace motors, **E** generic) to bound motor mass, then let pump + chamber + valve mass be "the rest" — flagged explicitly as a model error if a better primary source turns up (e.g. a Rocket Lab patent filing, which was searched for but not found with public claims/drawings in this pass).
3. **Battery mass and count, both stages.** Chemistry (lithium polymer) and a qualitative draw pattern (9 engines share stage-1 batteries; stage 2 has 3 HVBs, 2 jettisoned) are published; mass, voltage, cell count and watt-hour capacity are not. Derivation (the exact method the task names): take peak electrical power (9 × 2 × 37 kW ≈ 666 kW for stage 1, **D** from the unverified 37 kW figure) × burn time (155 s stage 1, **P**) = energy required; divide by a lithium-polymer pack specific energy at high discharge rate (≈100–150 Wh/kg at the C-rates a <3-minute full-depletion burn implies, **E** generic lithium-polymer figure, not Rocket-Lab-specific) to get a first-pass pack mass, then calibrate down against the fact that Rocket Lab explicitly jettisons stage-2 batteries specifically "to reduce mass", which implies the pack mass is non-trivial relative to total stage-2 mass (**E** qualitative constraint only).
4. **Curie engine specifics** (propellant chemistry, main-engine thrust, chamber pressure). Rocket Lab calls it a "green" bipropellant and has never named it; Isp ≈320 s circulates in secondary sources without a primary citation found. The PUG's own Kick Stage spec block gives only "THRUST 120 N" against "6 reaction control thrusters (RCS) (2 pods)" — it is not clear from the document whether 120 N is the RCS total, per-thruster, or (confusingly) meant to stand in for Curie's main thrust, since no separate main-engine thrust line exists in that table. This ambiguity should be resolved by contacting Rocket Lab or finding a clearer secondary source (e.g. a conference paper) before encoding it as **P**; for now treat Curie's main thrust as **U** and the 120 N as the RCS pod figure only.
5. **Tank wall thickness / CFRP layup for both Electron stages.** MEOP (maximum expected operating pressure) is not published for either tank. Derivation: back out MEOP from the stated "heated helium" pressurant text ([E1]) plus a typical LOX/RP-1 pump-fed feed-pressure range (≈2–4 bar tank pressure is typical for a pump-fed stage, since the pump — not tank pressure — does the work raising pressure to chamber level, **E** generic launch-vehicle practice), then size wall thickness from hoop stress = MEOP × radius / thickness against a CFRP allowable (a T-800-class layup allowable ≈1,500 MPa ultimate hoop, with a safety factor **E**, not found specific to Rocket Lab) — this is the task's own worked example and is the right next step once stage dry masses (gap 1) need splitting between tank and other structure.
6. **Real-mission telemetry (altitude/velocity vs. time from an actual flight, not the PUG's generic profile).** Rocket Lab's webcasts carry a live telemetry overlay for every one of ~99 flights, but no text source found in this pass transcribes one into numbers. Next step: pull a specific mission's webcast VOD (Rocket Lab's own YouTube channel) and read the on-screen altitude/velocity/downrange ticker at a handful of timestamps, or check whether NASASpaceflight's L2 forum (subscriber-only, not accessed here) has already done this for a mission of interest.
7. **Falcon 1 Merlin 1C vacuum thrust discrepancy.** The PUG states 78,000 lbf sea-level only (no vacuum figure in that table); a ~400 kN (90,000 lbf) vacuum figure appears in uncited secondary sources. Needs a primary SpaceX source (a later Falcon 9 Merlin-1C-lineage document, or the original AIAA papers by Bjelde/Dinardi found in search but not fetched in this pass) to resolve before it's used.
8. **Falcon 1 avionics, TVC hydraulics detail, and COPV sizing.** Listed in §4.6; not pursued to Electron's depth in this pass since Falcon 1 is the secondary vehicle — worth a dedicated follow-up only if Falcon 1's engines (Merlin 1C/Kestrel) get modelled ahead of Falcon 9's Merlin 1D for a smoother generic-engine-type calibration path.

## Sources

- [E1] Rocket Lab, *Electron Payload User's Guide*, v8.0, rocketlabcorp.com/assets/Rocket-Lab-Electron-Payload-User-Guide-8.0.pdf (pulled and parsed with pdftotext for this doc).
- [E2] Rocket Lab, *Electron Payload User's Guide*, v7.0, rocketlabcorp.com/assets/Electron-Payload-User-Guide-7.0-v6.pdf.
- [E3] Wikipedia, "Rocket Lab Electron" (accessed 2026-11-04).
- [E4] Wikipedia, "Rutherford (rocket engine)" (accessed 2026-11-04).
- [E5] Wikipedia/HandWiki, "Curie (rocket engine)" (accessed 2026-11-04).
- [F1] SpaceX, *Falcon 1 Launch Vehicle Payload User's Guide*, Rev. 7, D000973, 2008, georing.biz/usefull/Falcon1UsersGuide.pdf (pulled and parsed with pdftotext for this doc).
- [F2] Spaceflight Now, Falcon 1 Flight 4 launch timeline, spaceflightnow.com/falcon/004/080927timeline.html (accessed 2026-11-04).
- [F3] Wikipedia, "Falcon 1" / "RatSat" (accessed 2026-11-04).
- NASASpaceflight.com, "Catch Me If You Can" (Flight 10, "Running Out Of Fingers" re-entry telemetry coverage) — nasaspaceflight.com/2022/11/catch-me-if-you-can/.
- Various secondary web search snippets (Wikipedia-sourced search summaries, blog-level sources) tagged **E** throughout where no primary document could be fetched and parsed directly in this pass; these should be upgraded or dropped once a primary source is found.
