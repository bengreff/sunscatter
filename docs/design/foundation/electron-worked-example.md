# Worked example: Electron in the part schema (stress test)

Status: Director draft for design session 1, 2026-11-04. Not canon. It applies the settled rules 1-15 and the TYPE field table in [part-schema.md](part-schema.md) to Rocket Lab Electron (expendable configuration, with Kick Stage and fairing), using [electron-falcon1.md](research/electron-falcon1.md) (cited as R, with its section) and [type-catalog.md](research/type-catalog.md) (cited as C). Section 5 (gaps) is the point of the exercise.

Conventions. Tags: **P** published, **D** derived (method given), **E** estimate, **U** unknown. Parameter class: **G** geometry, **Pr** production, **O** operational (rule 4). Fields are F1-F15 (scope 5.1). Mount load classes are a Director proposal for rule 12: **L1** up to 20 kN/m ring line load, **L2** up to 60 kN/m, **L3** up to 150 kN/m; point mounts give a rated force. Where a rule forces a choice, the choice is made and the gap it exposes is numbered in section 5 (G#).

Two derived anchors used throughout (all **D** from R 3.2/3.3):
- Rutherford SL mass flow = 24.9 kN / (311 s x 9.80665) = 8.16 kg/s per engine (LOX 5.90, RP-1 2.27 at MR 2.6 E). Vac: 25.8 kN / (343 x 9.80665) = 7.67 kg/s.
- Per-engine pump hydraulic power at Pc 3 MPa (E), pump rise 4.5 MPa LOX and 6 MPa fuel (E, fuel adds regen and injector drops): LOX 5.90/1141 x 4.5e6 = 23 kW, RP-1 2.27/810 x 6e6 = 17 kW. At pump efficiency 0.6 (E) that is 38 + 28 kW shaft, consistent with the 2 x 37 kW motor figure (E). The 3 MPa and 37 kW estimates support each other.

## 1. The parts

33 part instances of 19 part entries (35 if G1 is resolved by splitting the common-bulkhead tanks). Groups (editor only, no physics): Stage 1 = P1-P5, Stage 2 = P6-P12, Kick Stage = P13-P17, Fairing = P18.

### P1. Stage-1 propellant tank (x1)
- Type `tank.cylindrical`, design "Electron S1 tank". Class nodes: tankage, ground-launch, small regime (up to ~1.5 m); alternative designs: common bulkhead, linerless composite. Materials: CFRP intermediate-modulus/epoxy (cryo-rated, linerless), Al fittings and polar bosses (E).
- One part for two fluid volumes (LOX forward, RP-1 aft, order E), forced choice, see G1.

| Parameter | Class | Value | Tag, source |
|---|---|---|---|
| outer diameter | G | 1.2 m | P, R 2 |
| barrel length | G | ~8.6 m | D: 11.4 t propellant (G12) at LOX 1141, RP-1 810 kg/m3 = 9.0 m3 + 3% ullage over 1.131 m2, less dome volume credit |
| dome shape | G | 2:1 ellipsoidal | E |
| common bulkhead station | G | ~5.9 m from fwd | D from the LOX/RP-1 volume split at MR 2.6 |
| wall layup, thickness | Pr | ~2.5 mm, quasi-isotropic + hoop | D: hoop t = MEOP x r / (allowable / SF) = 0.4 MPa x 0.6 m / (1500/2.5 MPa) = 0.4 mm strength-sized; buckling and handling set the floor, so ~2-3 mm E |
| resin grade, ply count | Pr | U | U |
| ullage pressure setpoints | O | 0.3-0.4 MPa | E, R 5 gap 5 |
| fill fractions | O | 0-1 per volume | - |
| dry mass | derived | ~200 kg | D: CFRP 1580 kg/m3 x shell area ~37 m2 x 2.5 mm = 146 kg + bulkhead, rings, fittings; no P figure (R 5 gap 1) |

Mounting surfaces:
| Surface | Kind, size | Load | Ports |
|---|---|---|---|
| S1 forward ring | ring 1.2 m | L2 | data/power connector (pass-through harness to stage 2), pressurant vent line (E) |
| S2 aft ring | ring 1.2 m | L3 | LOX outlet, RP-1 outlet, pressurant inlets x2, data/power connector |
| S3 umbilical plate (added by a component) | radial plate | point | GSE: LOX fill/drain, RP-1 fill/drain, He fill, purge, power, data (G7) |

Components:
| Component | Type | Param/designed | Region | Graphs |
|---|---|---|---|---|
| LOX feed tunnel through RP-1 volume | `line.tube_rigid` | param | volume, centreline (E) | fluid, thermal |
| pressurant diffusers x2 | `diffuser.pressurant` (not in C) | param | dome, forward of each volume | fluid |
| anti-vortex/anti-slosh baffles x2 | `baffle.antislosh` | param | volume, near outlets | fluid, load |
| vent/relief valves x2 | `valve.relief_pressure` | param | forward domes | fluid, data |
| pressure transducers x2 | `pressure_transducer` | param | domes | fluid, data |
| temperature/level sensors x2 | `level_sensor_cryo` | param | volume | fluid, data |
| fill/drain valves x2 | `valve.cryogenic_shutoff` | param | aft wall | fluid, data |
| umbilical plate + separation | `coupling.quick_disconnect_fluid` + surface-changing component (rule 9) | param | wall | fluid, power, data |
| power/data harness raceway | `wiring_harness.power_bundle` + data bundle | param (routed) | wall exterior | power, data |

### P2. Stage-1 thrust structure (x1)
- Type `structure.thrust_structure`, design "Electron 9-engine". Class node: thrust structure, small, multi-engine pattern. Materials: CFRP, AM or machined Al/Ti engine mount fittings (E).
- Parameters: diameter 1.2 m G (P); engine pattern 1 centre + 8 ring G (P, 9 engines); height ~0.9 m G (E); skin thickness Pr (U); dry mass ~60 kg (E).
- Surfaces: S1 forward ring 1.2 m L3 (ports: LOX, RP-1, 2 pressurant, power, data); S2-S10 engine gimbal mounts (not a rule-12 kind, G4), rated 30 kN point (D: 24.9 kN SL rising to 26 kN vac, x1.15 margin); ports per gimbal mount: LOX, RP-1, HV power, data. S11-S12 COPV hard points (radial, point 2 kN E).
- Components: stage-1 battery packs (`battery.cell_pack`, param; count U, 4 E, region: bay volume; power, thermal), battery management units (`battery.management_unit` x4, param; power, data), HV distribution (`avionics.power_distribution_unit` as a component here, G17), feed manifolds splitting LOX and RP-1 nine ways (`line.tube_rigid` x18, param; fluid), prevalves x2 (`valve.ball`, param; fluid, data), gimbal bearings x9 (`gimbal_bearing_ring`, param; load), EMA anchors (G4).
- Battery sizing (D from E): 9 x 74 kW shaft / 0.9 motor+inverter = 740 kW electrical x 155 s = 115 MJ = 32 kWh; at 120 Wh/kg (E, high-rate Li-polymer) = ~265 kg. This is the largest single uncertainty in stage-1 dry mass.

### P3. Rutherford sea-level engine (x9)
- Type `engine.liquid_bipropellant`, design "Rutherford SL". Class nodes: liquid biprop, small thrust (10-50 kN); alternative design: electric-pump-fed cycle. Materials: AM nickel superalloy for chamber, injector, pump housings, valves (alloy U, Inconel 718 class E); process node: laser powder bed fusion (P for the process, R 2.2).

| Parameter | Class | Value | Tag, source |
|---|---|---|---|
| thrust, SL | rated | 24.9 kN | P, R 3.2 |
| Isp SL | rated | 311 s | P |
| chamber pressure | G (design point) | ~3 MPa | E, supported by the power check above |
| mixture ratio | O (range Pr) | 2.6 nominal | E |
| expansion ratio | G | U (~15-20 E for SL) | U |
| exit diameter | G | 0.25 m | P (R 2.2 gives engine diameter) |
| dry mass | derived | 35 kg | P, calibrates the derived sum |
| throttle range | O | U | U (needed, G12) |
| gimbal range | G | U (+-5 deg E) | U |

- Surfaces: S1 gimbal mount to P2 (articulated, G4), ports LOX in, RP-1 in, HV power in, data. S2 nozzle exit (no mount; aero and plume surface only).
- Components, the engine boundary (detail in G8):

| Component | Type | Param/designed | Region | Graphs |
|---|---|---|---|---|
| LOX e-pump | `pump.electric_centrifugal` (not in C, G10) | designed | pump region, aft of injector | power, fluid, data, thermal, load (rotor momentum) |
| RP-1 e-pump | same type, own design | designed | pump region | same |
| motor inverters x2 | `power.motor_inverter` (not in C) | designed or bought | engine volume or P2 (U) | power, data, thermal |
| main valves x2 (LOX, RP-1) | `valve.poppet` or `valve.ball` (U) | param | between pumps and injector | fluid, data, power |
| igniter | `igniter.*` (U, method unpublished) | param | injector face | data, power, (fluid if chemical) |
| EMAs x2 (pitch, yaw) | `electromechanical_actuator` | param | spans P2 and P3 (G4) | power, data, load |
| feed lines + flex hoses | `line.flex_hose` x2 | param | gimbal crossing | fluid, load |
| chamber Pc + temp sensors, motor tachometers | `pressure_transducer`, `tachometer` | param | chamber, pumps | data |
| engine controller | `avionics.subsystem_controller` as a component (G17) | designed | engine volume (E) | data, power |
| combustion device: injector + regen chamber + nozzle | the tabulated core (G8) | defined geometry + table | fixed | fluid (as a boundary node), thermal, load |

### P4. Interstage (x1, stays with stage 1)
- Type `structure.interstage`. Class: interstage, small. Material CFRP. Params: diameter 1.2 m G (P); length ~1.5 m G (E, must clear the vac nozzle of P8); mass ~25 kg (E).
- Surfaces: S1 aft ring to P1 S1, ring 1.2 m L2, direct match, bolted (rule 9). S2 forward ring to P7 S1, ring 1.2 m L2, made a separable surface by a component; ports: in-flight disconnect for power/data (severed at separation).
- Components: pneumatic pushers (`separation_system` pusher, count U, 4 E; load, fluid: needs gas, from P5 or its own bottle U), release mechanism (`separation_bolt` or pneumatic latch, U; load, data), pass-through harness (power, data). Separation: the pusher set severs S2 and imparts the push (P, R 2.1 "Pneumatic Pusher").

### P5. Stage-1 pressurant COPV (x1, count U)
- Type `tank.pressurant_copv`. Class: COPV, small. Materials: Al or Ti liner (U), carbon overwrap. Params: volume ~40 L G (E), MEOP ~30 MPa G (E), helium (P that a pressurant system exists, R 2.4; gas U, "heated helium" is Falcon 1's). Mass ~8 kg (E).
- Placement U (inside the LOX tank is common practice, G2). Placed here on P2 hard points S11-S12 to avoid nesting.
- Surfaces: S1 hard-point pair, ports: He out, data (pressure, temperature).
- Components: regulator (`regulator.pressure`, param; fluid, data), isolation solenoid valve (`valve.solenoid`; fluid, power, data), burst disk (`valve.burst_disk`), pressure transducer.

### P6. Stage-2 propellant tank (x1)
- Same type and class nodes as P1, design "Electron S2 tank" (a second design; similarity discount, scope 5.2).
- Params: diameter 1.2 m G (P); barrel ~1.4 m G (D: 2,000 kg (P) = 1.27 m3 LOX + 0.69 m3 RP-1, + 3% ullage, less domes); wall ~2 mm Pr (E); dry mass ~45 kg (D, same method as P1).
- Surfaces: S1 forward ring 1.2 m L1 to P12; S2 aft ring 1.2 m L2 to P7; ports as P1 (LOX, RP-1 outlets; pressurant in; harness). Components: as P1 minus the GSE plate (stage-2 fill goes through the P1 umbilical and the P4 pass-through, E).

### P7. Stage-2 aft skirt / engine mount (x1)
- Type `structure.thrust_structure`, design "single engine". Params: diameter 1.2 m G; height ~0.4 m G (E). Surfaces: S1 aft ring to P4 S2 (separable, see P4); S2 forward ring to P6 S2, L2; S3 one gimbal mount (30 kN point); S4-S5 radial hard points for the roll RCS (P11).
- Components: feed lines, prevalves x2, gimbal bearing, EMA anchors; the stage-2 pressurant regulator.

### P8. Rutherford Vacuum (x1)
- Same type as P3, design "Rutherford Vac": identical internals, larger expansion ratio (P, R 2.5). Thrust 25.8 kN vac, Isp 343 s (P). Burn 373 s by timeline vs "about five minutes" by prose (P vs P, R 2.5; G12). Expansion ratio U (~40-70 E). Nozzle extension cooling U (regen or radiative, G8).
- Same components as P3, with the same e-pump and valve designs (component designs are shared across the two engine designs, rule 1).

### P9. High Voltage Battery, HVB (x3)
- Type `power.battery_module_housing` (a part, because two of the three separate as their own bodies, rule 7; G3). Class: battery housing, jettisonable, small. Materials: CFRP or Al housing (U).
- Params: energy ~2.9 kWh each G/Pr (D: 74 kW x 373 s / 0.9 = 8.5 kWh across three, if the third covers a third of the burn; E for the split); mass ~25 kg each (E at 120 Wh/kg, cell pack plus housing).
- Surfaces: S1 radial hard-point set on P12 (E placement), ports: HV power, data (BMS), and a jettison-severable connector.
- Components: cell pack (`battery.cell_pack`, param; power, thermal), BMS (param; power, data), separation component (spring push-off + release, `separation_system`, U mechanism; load). The HVB handover is a switch in P12's PDU (power graph topology change), then the release fires (P, jettison at L+388 s, R 3.3).

### P10. Stage-2 pressurant COPV (x1, count U)
- As P5, smaller (~10 L E). Placement U; on P7 hard points (E). Also supplies the stage-2 roll RCS (E, shared gas).

### P11. Stage-2 roll RCS thruster (x2, count U)
- Type `thruster.rcs_cold_gas`. Class: cold-gas thruster, small. Params: thrust U (~5-10 N E), throat G. Surface: radial hard point on P7, ports: gas in, power, data. Components: solenoid valve (param; fluid, power, data). P that roll is controlled by cold gas during the stage-2 burn (R 2.4).

### P12. Stage-2 avionics bay (x1)
- Type `structure.equipment_bay`. Class: equipment bay, small. Material CFRP. Params: diameter 1.2 m G; height ~0.3 m G (E). Placement and content E (R 2.9: the data graph is the thinnest Rocket Lab publishes).
- Surfaces: S1 aft ring to P6 S1 (L1, bolted); S2 forward ring to P13 (separable, mechanism U); S3 fairing base ring, the same 1.2 m station as S2 but carrying P18 outboard (G5); S4-S9 HVB hard points; antenna patches (surface components).
- Components (all inside the bay's volume, so components by rule 7; G17): flight computer (`avionics.flight_computer_unit` as a component, designed/bought; data, power, thermal), IMU (designed/bought; data), GNSS receiver + antennas (bought; data, power), S-band telemetry transmitter + antennas (bought; data, power), flight termination system (U, autonomous E; data, power; G21), PDU with HVB handover switch (param; power, data), harness (routed), kick-stage separation actuator (U).

### P13. Kick Stage structure (x1)
- Type `structure.equipment_bay` or a stage-bus class (no fitting type in C; G19). Class node: kick stage bus, small. Material CFRP (P). Params: diameter 1.2 m G, height 0.405 m G, dry mass 40 kg (all P, R 2.6, but whether 40 kg includes Curie, tanks and RCS is unclear: G18).
- Surfaces: S1 aft ring to P12 S2, ring 1.2 m L1, separable (component U, at L+539 s P); S2 payload interface: payload adapter standard (MLB bolt circle, 8-38 in P) with SEIP connector ports (power, data, P); S3 Curie mount (fixed, no gimbal E); S4-S5 RCS pod hard points; S6-S8 internal hard points for the Curie tanks and COPV (G2).
- Components: kick-stage computer, battery pack, radio, GNSS, harness, SEIP (P), payload separation command channel.

### P14. Curie engine (x1)
- Type `engine.liquid_bipropellant`, design "Curie". Class: liquid biprop, very small thrust; alternative design: pressure-fed. Materials: AM metal (P process, alloy U). Propellant: undisclosed "green" bipropellant (U, G15). Thrust U (the PUG's 120 N is probably the RCS figure, R 5 gap 4). Isp ~320 s (E).
- Surface: fixed mount to P13 S3, ports: fuel in, ox in, power, data. Components: bipropellant valves x2 (`valve.solenoid`), igniter or hypergolic start (U), injector/chamber/nozzle core (tabulated, G8), heater (`heater.resistive_patch`; thermal, power).

### P15. Curie propellant tanks (x2, fuel and oxidiser, E)
- Type `tank.spherical` or `tank.conformal` (shape U; "composite... integrated with Curie into one module", P R 2.6). One pressurised volume each, so parts (rule 10), though inside P13's volume (rule 7; G2). Materials: composite (P). Components: PMD or bladder (`screen.capillary_pmd`, U), pressure transducer, fill/drain valve.

### P16. Kick-stage pressurant COPV (x1, E)
- As P5, ~3 L (E). Feeds P15 and the RCS (E).

### P17. Kick-stage RCS pod (x2)
- Type: a pod part with three thruster components (P: 6 thrusters, 2 pods, cold gas, R 2.6). Forced choice against C's "each thruster a part" (G16). Params: thrust 120 N (P, but which quantity is ambiguous). Surface: radial hard point on P13, ports gas, power, data. Components: thrusters x3 (`thruster.rcs_cold_gas` as a component), solenoid valves x3.

### P18. Fairing half (x2)
- Type `structure.fairing_half`. Class: fairing, small; alternative design: hinged clamshell. Materials: CFRP sandwich (P composite; core U), acoustic foam (P). Params: length 2.5 m G, diameter 1.2 m G, mass 22 kg each (D: 44 kg total P / 2, R 2.7); ogive profile G (U); foam thickness Pr (U).
- Surfaces: S1 base half-ring (180 deg arc of 1.2 m) on P12 S3 (G5); S2 longitudinal seam to the other half (no rule-12 kind, G5); S3 hinge at the base (articulated, G4).
- Components: pneumatic unlocking latches (P; load, fluid or pyro-free gas U), separation springs (P; load), hinge (G4), acoustic foam sheets (param or a cell field, see G20), vent ports (E).
- Separation: the latches unlock S1 and S2, springs rotate each half on its hinge away, the hinge then releases (P, R 2.7). Fairing separation at L+184 s, ~126 km (P).

### P19. Payload adapter (x1, mission-dependent)
- Type `structure.payload_adapter` with a separation component, or the separation system itself as the part (e.g. a Motorized Light Band, P option, R 2.8). Surfaces: S1 to P13 S2 (payload adapter standard), S2 to the payload (same standard, a different size possible). Components: band, motors or springs, separation connectors.

### Attachment summary
| Joint | How | Separates by |
|---|---|---|
| P1-P2, P1-P4, P6-P7, P6-P12 | direct ring match, bolted | - |
| P3 x9 - P2, P8 - P7 | gimbal mount (not a rule-12 kind) | - |
| P4 - P7 (stage sep) | ring made separable by components on P4 | P4 pneumatic pushers + release, L+155 s |
| P12 - P13 (S2 / kick sep) | ring made separable (component U) | U, L+539 s |
| P18 - P12, P18 - P18 | half-ring + seam + hinge | P18 latches + springs, L+184 s |
| P9 - P12 | hard points made separable | P9 release, L+388 s (two of three) |
| P13 - P19 - payload | adapter part (standard surfaces) | P19's band |
| P1 - ground | umbilical plate added by a component | plate's own release at T-0 (G7) |

Counts. Part instances 33 (19 entries). Component instances, counting each listed line once per host instance: P1 16, P2 ~45, P3 ~16 x 9 = 144, P4 ~7, P5 4 x 3 COPVs = 12, P6 14, P7 ~8, P8 14, P9 3 x 3 = 9, P11 1 x 2 = 2, P12 ~12, P13 ~7, P14 5, P15 3 x 2 = 6, P17 6 x 2 = 12, P18 ~6 x 2 = 12, P19 ~4: about **320 component instances of about 50 component types**, of which about 8 types are designed or bought (e-pump, inverter, engine controller, flight computer, IMU, GNSS, radio, FTS) and the rest parametric.

## 2. System graphs

### Power
1. P2 stage-1 cell packs (x4 E) > BMUs > P2 HV PDU > 9 gimbal-mount ports > each P3: inverter x2 > e-pump motors x2. Also > EMAs x2, engine controller, igniter (low-voltage branch, through a DC-DC converter, not listed in R, E).
2. P9 HVB A, B (parallel) > P12 PDU > P6 harness > P7 > P8 inverters > e-pump motors; EMAs. At depletion the PDU switches to HVB C (topology change), then A and B separate (L+388 s P).
3. P12 avionics LV bus (source U: a small LV pack or a DC-DC from the HVBs, E) > flight computer, IMU, GNSS, S-band, FTS, valves and sensors across stages 1 and 2 (through P4's pass-through until L+155 s).
4. P13 kick-stage pack > kick computer, radio, Curie valves and heater, RCS solenoids, SEIP > payload.
5. GSE > P1 umbilical > ground power to all buses until T-0 (G7).

### Fluid
1. LOX: P1 forward volume > feed tunnel > S2 aft ring port > P2 manifold > prevalve > 9 ways > flex hose > LOX e-pump (inlet pressure from ullage pressure + head, checked against the pump's NPSH-required curve) > main valve > table boundary (injector LOX manifold).
2. RP-1: P1 aft volume > P2 manifold > prevalve > 9 ways > RP-1 e-pump > main valve > table boundary (regen jacket inlet; G8).
3. Pressurant: P5 > regulator > isolation valve > P2/P1 ports > diffusers in both P1 volumes. Stage 2: P10 > regulator > P6 diffusers, and > P11 thrusters. Kick: P16 > P15 tanks, > P17 thrusters.
4. Stage-2 propellant: P6 > P7 > P8 pumps > valves > table. Kick: P15 > valves > P14 table.
5. Separation gas: pneumatic pushers on P4 and latches on P18 need a gas source (U; own small bottles E, a fluid graph island each).
6. GSE and purge: P1 umbilical: LOX and RP-1 fill/drain, He fill to P5 and through P4 to P10 (E), N2 or He purge of the P2 engine bay and the pump cavities (E), LOX conditioning/bleed back through the umbilical (E). All severed at T-0.
7. Vents: P1 and P6 relief valves to ambient (fluid sinks to the environment, not a port).

### Data
1. P12 flight computer (master) > P12 data bus > P4 pass-through > P2 > 9 engine controllers (each > 2 inverters (speed commands), 2 main valves, igniter, 2 EMAs; < tachometers, Pc and temperature sensors).
2. Flight computer < IMU, GNSS; > S-band transmitter; > FTS (or FTS independent, U); > PDU switching; > P11 solenoids; > separation commands (P4 release, P9 releases, P18 latches, P13 release).
3. Flight computer < tank pressure and level sensors (P1, P6), COPV sensors (P5, P10), BMU status.
4. After L+539 s: P13 kick computer is master > Curie valves, RCS solenoids, SEIP (payload power and separation).
5. Stage 1 after L+155 s has no computer in this breakdown (expendable; the recovery variant adds one, out of scope).

### Thermal (main paths)
1. Regen: chamber wall heat > RP-1 in the jacket > injector (inside the table, G8). Output to the thermal graph: jacket outlet temperature, wall temperature margin, heat into the engine part's structure cells.
2. Motor and inverter loss: ~10% of 740 kW = ~74 kW for 155 s in stage 1 (D from E). Sink U: propellant-cooled motors (heat into the LOX/RP-1 flow before the pump outlet, a fluid-thermal link) or thermal mass (E: propellant cooling is the plausible design).
3. Battery loss: ~5% of 740 kW (E) = ~37 kW into the cell packs, adiabatic over a 155 s burn; the cell temperature limit is a real constraint on burn time.
4. LOX tank: boil-off through the uninsulated (E) CFRP wall on the pad, replaced through the umbilical until T-0; the common bulkhead conducts between LOX and RP-1 (RP-1 freezing margin).
5. Ascent aero heating on P18 (and P1/P6 after fairing separation at 126 km is small); P18 foam is acoustic, not thermal.
6. Nozzle: SL nozzle regen (E); the vac nozzle extension radiates to space (E). P4 has separated 7 s before ignition, so there is no interstage heating.
7. Kick stage: Curie heaters, battery heaters, radiative balance on P13 for a multi-orbit mission.

### Load paths (thrust to payload)
1. Each P3 chamber > gimbal bearing (P2 component) + EMA reaction (P2 anchor, P3 actuator) > P2 gimbal surfaces S2-S10 > P2 cone > P2 S1 ring (L3).
2. > P1 S2 aft ring > P1 wall (pressure-stabilised CFRP; capacity depends on ullage pressure, G9) > P1 S1 forward ring (L2).
3. > P4 aft ring > P4 shell > P4 forward ring (separable, held by the release) > P7 > P6 > P12.
4. At P12 the path splits: (a) P12 S3 > P18 halves (fairing drag and inertia, until L+184 s); (b) P12 S2 > P13 > P13 S2 > P19 > payload.
5. Point loads: P9 HVBs at P12 hard points; COPVs at P2/P7 hard points (inertia x up to 8 g axial, P envelope R 3.4).
6. Check value at stage-1 burnout (D from E): mass above P4 ~2,650 kg x 6.6 g (234 kN / ~3,600 kg) = 172 kN, ring line load 172 / (pi x 1.2) = 46 kN/m, inside L2.

## 3. Type > Design > Unit, two worked cases

### 3.1 Stage-1 tank (P1)
- **Type `tank.cylindrical`** (data composing the cylinder + domes shape builder):
  - nodes: tankage ground-launch small; designs common bulkhead and linerless composite (each opens parameter options: bulkhead station, liner yes/no); materials CFRP (allowable, density, permeation rate and microcracking threshold at 90 K as level-driven ranges, F5) and Al.
  - parameter ranges as functions of field levels: diameter 0.3-6 m (sanity cap by class, rule 10; predefined 1.2 m offered), length up to a slenderness limit set by F5 level, wall thickness by MEOP and material, bulkhead stations.
  - slots: diffusers (forward dome of each volume), outlets (aft domes), sensors (domes, wall), baffles (volume), feed tunnel (volume, centreline), umbilical plate (wall, adds a surface), heaters and insulation (wall).
  - domain models: pressure vessel (hoop, axial, buckling with pressure stabilisation), two fluid nodes with ullage, thermal cells on the wall and bulkhead, aero surface after fairing separation.
  - failure modes: burst (whole part, stored energy = pV/(gamma-1) per volume), buckle (split at weakest ring, later), bulkhead reversal (new for common bulkheads: RP-1 side overpressure), leak by permeation (linerless).
  - recipe template: CFRP prepreg (kg), resin, Al fittings, labour hours as functions of area and ply count.
  - broken state: 3-6 segments + both volumes' pressure release.
- **Design "Electron S1 tank"** (output of a design program):
  - fixed G: 1.2 m, ~8.6 m barrel, 2:1 domes, bulkhead at ~5.9 m, outlet and slot positions.
  - rated performance with spread: MEOP 0.4 MPa (E), proof factor, burst pressure mean and sigma, buckling load at 0 and at operating pressure, permeation leak rate, rated pressure cycles (relevant for reuse).
  - recipe: kg of each material, labour, tooling (mandrel) reused by later units.
  - production choices: ply count within tolerance, resin grade, component designs (which valves, sensors), insulation none, routing waypoints (rule 14).
- **Unit (serial 0042, say)**:
  - drawn at manufacture (seeded): burst strength factor, void content and its flaw sites (hidden until inspection), permeation multiplier, wall thickness deviation map.
  - state: two fill levels and temperatures, ullage pressures, wall temperature cells, pressure cycle count, accumulated damage (microcracks), inspection record.

### 3.2 Rutherford LOX e-pump (component in P3)
- **Type `pump.electric_centrifugal`** (complex, designed; rule 2):
  - nodes: electric-pump cycle (engine design node), high-speed BLDC motor, centrifugal pump small high-speed, plus the material nodes it uses (AM nickel alloy, NdFeB, copper).
  - parameters and ranges: shaft power 1-500 kW and speed limits as functions of F8 (motor power density), F5 (bearing and rotor stress limits), F2 (pump efficiency); propellant compatibility (LOX needs oxygen-safe materials).
  - behaviour model: dimensionless pump map (head and efficiency vs flow coefficient), NPSH-required curve, motor torque-speed with copper and iron losses, rotor inertia. In the power graph it is a load; in the fluid graph a pressure source; in the load graph a torque on P3 (spin-up and run reaction torque, and gyroscopic momentum, rule 7).
  - slots it fits: engine pump region, envelope by power class.
  - failure modes: cavitation (inlet below NPSH-required), bearing seizure, winding over-temperature, LOX ignition at a rub (an energetic failure that can take the engine), seal leak between propellant and motor cavity.
  - recipe: AM housing and impeller, magnets, copper, bearings, seals, labour.
- **Design "RLOXP-1"** (from a design program or bought; rule 2):
  - fixed: impeller diameter and blade geometry, motor stack length and pole count, so a fixed pump map and motor constants.
  - rated: design point ~5.9 kg/s at 4.5 MPa rise, ~40,000 rpm, ~37 kW (all E), efficiency mean and sigma, rated run life (seconds of firing, starts) with spread.
  - production choices: bearing grade, magnet grade, seal material, which inverter design it pairs with (separate component, G10).
- **Unit**: drawn: efficiency deviation (+-1-2% E), bearing life constant, rotor imbalance, a hidden AM porosity flaw (probability set by the process node level). State: speed, winding and bearing temperatures, cumulative run seconds and starts, bearing wear.

## 4. Nodes implied

Class nodes are per size regime and per alternative design (rule 15). "2030" says whether a 2030 start would have the node in the world; "player" says whether a new company plausibly holds it at the start (G14).

| # | Node | Kind | Fields | 2030 world | Player at start |
|---|---|---|---|---|---|
| 1 | Tankage, ground-launch, small (to ~1.5 m) | class, size | F5 | yes | yes |
| 2 | Common-bulkhead tank | class, alt design | F5, F3 | yes | yes |
| 3 | Linerless composite cryotank | class, alt design | F5, F3, F6 | yes (Rocket Lab) | no (proprietary, G14) |
| 4 | COPV, small | class, size | F5, F6 | yes | yes (or buy) |
| 5 | Small sphere/conformal tank (in-space) | class, size | F5 | yes | yes |
| 6 | Thrust structure, small, multi-engine | class, size | F5 | yes | yes |
| 7 | Interstage, small | class, size | F5 | yes | yes |
| 8 | Fairing, small | class, size | F2, F5 | yes | yes |
| 9 | Hinged clamshell fairing | class, alt design | F5 | yes | yes |
| 10 | Equipment bay, small | class, size | F5 | yes | yes |
| 11 | Kick stage bus (G19) | class | F5, F1 | yes | yes |
| 12 | Jettisonable battery housing | class, alt design | F5, F4 | yes (Rocket Lab) | partly |
| 13 | Liquid biprop engine, small (10-50 kN) | class, size | F4, F2 | yes | yes |
| 14 | Liquid biprop, very small (<1 kN) | class, size | F4 | yes | yes |
| 15 | Electric-pump-fed cycle | class, alt design | F7, F8, F4 | yes (Rocket Lab) | no (G14) |
| 16 | Pressure-fed cycle | class, alt design | F4 | yes | yes |
| 17 | Cold-gas thruster, small | class | F2 | yes | yes |
| 18 | Payload adapter, small standard | class | F5 | yes | yes (buy) |
| 19 | CFRP intermediate-modulus/epoxy | material | F5 | yes | yes |
| 20 | Cryo-rated linerless resin system | material | F5, F3 | yes | no |
| 21 | AM nickel superalloy (Inconel 718 class, E) | material | F5, F6 | yes | yes |
| 22 | Aluminium alloys (fittings) | material | F5 | yes | yes |
| 23 | Titanium alloy (COPV liners, fittings) | material | F5 | yes | yes |
| 24 | High-rate Li-polymer cells | material | F4 | yes | yes |
| 25 | NdFeB magnets | material | F7 | yes | yes |
| 26 | Acoustic foam | material | F2, F5 | yes | yes |
| 27 | LOX/RP-1 propellant pair | material/propellant | F4, F3 | yes | yes |
| 28 | Green storable bipropellant (Curie stand-in) | material/propellant | F4 | yes (U chemistry, G15) | no |
| 29 | Laser powder bed fusion | process | F6 | yes | yes |
| 30 | Automated fibre placement / winding | process | F6 | yes | yes |
| 31 | High-speed BLDC motor | component | F8, F7 | yes | yes |
| 32 | High-power motor inverter | component | F7 | yes | yes |
| 33 | Small high-speed centrifugal pump | component | F2 | yes | yes |
| 34 | Cryogenic valves | component | F3, F5 | yes | yes |
| 35 | Solenoid valves | component | F8 | yes | yes |
| 36 | Pressure regulator | component | F2 | yes | yes |
| 37 | Electromechanical actuator | component | F8, F5 | yes | yes |
| 38 | Igniter (method U) | component | F4 | yes | yes |
| 39 | Pressure, temperature, level sensors | component | F7 | yes | yes |
| 40 | Battery management unit | component | F7 | yes | yes |
| 41 | Flight computer | component | F15, F7 | yes | yes (buy) |
| 42 | IMU | component | F1, F7 | yes | yes (buy) |
| 43 | GNSS receiver | component | F8, F1 | yes | yes (buy) |
| 44 | S-band transmitter | component | F8 | yes | yes (buy) |
| 45 | Autonomous flight termination | component | F15 (G21) | yes | yes (buy) |
| 46 | Pneumatic pusher / latch separation | component | F5 | yes | yes |
| 47 | Spring and hinge mechanisms | component | F5 | yes | yes |
| 48 | Light-band separation system | component | F5 | yes | yes (buy) |

48 nodes: 18 class, 10 material/propellant, 2 process, 18 component. Field coverage (nodes touching each field): F5 24, F4 9, F7 8, F2 6, F8 6, F3 5, F6 5, F1 3, F15 2; F9-F14 none (expected for a chemical launcher). Not given nodes (level-only, G13): lines, hoses, harness, baffles, diffusers, fasteners, burst disks.

## 5. Gaps

Each: what breaks, which rules, and a proposal (Director). The ten that most need the owner are listed at the end.

**G1. Common bulkhead vs "one pressurised volume is one part".** Rule 10 makes the LOX and RP-1 volumes two parts. Rule 15 names "common dome" as an alternative design of one tank class, and physically it is one wound structure with one shared wall that fails as one thing (bulkhead reversal). Two parts would need a "shared wall" surface kind that does not exist; one part breaks rule 10. *Proposal:* amend rule 10 to "one pressure-vessel structure is one part; it may hold several fluid volumes", with the structure's failure modes covering each volume and the shared wall.

**G2. Parts inside parts.** Rule 7 says anything inside a part's volume is a component; rule 10 and the catalog make COPVs and propellant tanks parts. Electron has COPVs (placement U, often inside the LOX tank) and Curie's tanks sit inside the Kick Stage's 405 mm structure. There is no internal mount surface kind, and no rule for a part hosted inside another's volume (routing keep-outs, CG, does it share the host's rigid body?). *Proposal:* an "internal hard point" surface kind; a hosted part keeps its own body and failure, and the host's volume treats it as a keep-out. Rule 7 then reads "inside and not a pressure vessel".

**G3. The same object is a component or a part depending on whether it is dropped.** Stage-1 batteries are components (inside P2); two of the three stage-2 HVBs separate mid-burn, so by rule 7 they are parts; the third never separates. The catalog calls battery packs components. *Proposal:* the housing is the part and the cells its components, used wherever a pack is jettisonable; a non-jettisoned pack inside a bay stays a component. Alternatively a "jettisonable component" that becomes a lump of mass on release (rule 11's mechanism). Owner's call; the second is lighter but makes components into bodies.

**G4. Articulated joints and actuators that span two parts.** The gimbal (P3 on P2) and the fairing hinge (P18 on P12) are moving joints, not bolted surface matches, and rule 12's kinds (rings, hard points, truss nodes, docking, payload adapters) have no pivot. The EMAs anchor on P2 and push P3; rule 14 places a component inside one part. The flex hoses and HV cables also cross a moving surface. *Proposal:* add an articulated surface kind (pivot or hinge: axes, range, rated load, stiffness); the actuator belongs to the moving part and applies its reaction at the surface; ports on an articulated surface require flex-rated components.

**G5. Surfaces that are not full rings.** Each fairing half covers a 180 deg arc of the 1.2 m ring, the halves join each other along a longitudinal seam, and the same stage-2 forward station carries both the fairing base (outboard) and the Kick Stage (inboard). Rule 12 has no arc, seam or concentric-ring notion. *Proposal:* ring surfaces get an arc span and a radius band; add a "seam" (line) kind with a load per metre; two surfaces may share a station if their radius bands do not overlap.

**G6. Release and push are two components, and both need energy.** Rule 9 says one component severs and imparts. Electron's stage separation (pneumatic pusher plus a release, P/U) and fairing (pneumatic latches plus springs plus hinge, P) each use several. The pneumatic ones need a gas supply, so a separation component is in the fluid graph, with its own failure (a pusher that does not fire). *Proposal:* a separation is a set of components on one side of the surface, with an order (release, then push); each declares its energy source.

**G7. Ports to the ground.** The umbilical plate, GSE fill and purge, and the launch-mount hold-downs attach to no part. Rule 13 joins ports only between two surfaces, and GSE is outside the node system (Director finding). *Proposal:* a world-side "ground interface" pseudo-part with standard surfaces, outside the node system, money and mass only; the umbilical's release is a component on the craft side.

**G8. The engine boundary (simulated components vs the tabulated core).** Scope 3.1 lists e-pumps, main valves, gimbal actuators and igniter as components and says the chamber and nozzle are defined geometry with a table; the catalog also lists `injector.element_pattern` as a component; the regen jacket is in neither list. *Proposal:*
- **Simulated components** (in the graphs, with their own failure): prevalves (stage), e-pump motors and pumps, inverters, main valves, igniter (an event with an energy and a failure), EMAs and the gimbal joint, flex hoses, engine controller, sensors, purge valves. Pump spin-up dynamics (about a second) live here, so start and throttle transients come out of the motor model.
- **Tabulated core** (one "combustion device" with fixed design geometry): injector, regen jacket, chamber, throat, nozzle and any nozzle extension. The regen jacket goes inside because its heat flux is the combustion's output and its passage hydraulics are fixed by the design; a separate jacket would need the table's wall heat flux anyway.
- **Table split:** a chamber table (keyed by chamber design: propellants, injector, throat, jacket) and a nozzle table (keyed by expansion ratio and contour). Rutherford SL and Vac share one chamber table and one set of component designs and differ only in the nozzle table, which is exactly the published fact (R 2.5).
- **Inputs, per step:** oxidiser and fuel mass flows at the two inlet ports (the injector LOX manifold; the jacket fuel inlet), inlet temperatures, ambient pressure, ignition state and time since ignition or cutoff command.
- **Outputs:** the hydraulic characteristic, i.e. the inlet pressure each port needs at those flows (Pc plus injector drop, plus jacket drop for fuel), so the fluid network solves the pump operating point against it (this is what makes the table a node in the fluid graph, not a lookup at the end); Pc; thrust magnitude and axis; Isp; exit pressure and a flow-separation flag (a vac nozzle at sea level is invalid and makes side loads); jacket outlet temperature and coolant margin (coking); peak wall temperature margin (burn-through driver); heat into the engine's structure cells; plume descriptor (exit velocity, temperature, composition, half-angle); an envelope flag (MR and Pc range) that triggers a failure model outside it; start and shutdown impulse curves.
- **Calibration:** 311 s and 343 s Isp and 24.9 / 25.8 kN (P) calibrate c* efficiency (chamber) and Cf efficiency (nozzle). Curie uses the same interface, with inlet pressures set by the tanks (pressure-fed). The mass split of the 35 kg (R 5 gap 2) between table hardware and components stays a calibrated unmodelled remainder.

**G9. A ring's load class is not fixed.** P1's thin CFRP wall carries the stack in compression mostly because it is pressurised; its capacity depends on the ullage pressure, an operational parameter. Rule 12 gives a surface a static load class. *Proposal:* a surface's rating is a function of the part's state (pressure, temperature); the class is the rating at the design's rated condition, and the structure model checks the live one.

**G10. Complex components have no internals in the schema.** The e-pump is a motor, a pump, bearings, seals and (maybe) its inverter. Rule 2 says turbopumps are designs; nothing says whether a designed component can hold sub-components. The inverter is swapped and bought separately, so by 5.2's rule of thumb it is its own component. *Proposal:* a designed component's internals are design parameters, not schema components (no nesting); anything swapped or bought separately (the inverter) is a sibling component linked in the power graph. The pump's rotor momentum and reaction torque are parameters (rule 7); with nine engines, twin pumps that run the same direction give a roll torque (direction U).

**G11. "Materials can probably change without a new design" is a loophole.** Rule 1 and 5.2 allow a material change as a production change. Swapping P1 from CFRP to Al-Li keeps the geometry (1.2 m, 8.6 m) but triples the wall and changes mass, allowables, permeation and failure modes. *Proposal:* material changes are free only within a material family (CFRP grades; Al alloys); across families is a redesign with a high similarity discount.

**G12. Published numbers do not close, and two published values conflict.** Stage 1 at 9 x 8.16 kg/s for 155 s burns 11.4 t, leaving 1.6 t of the 13 t lift-off mass for stage-1 dry mass and everything above it, which already includes 2 t of stage-2 propellant. Stage 2 at 7.67 kg/s for 373 s burns 2.9 t against 2.0 t on board (P), and the PUG also says "about five minutes". "Derived > published" cannot arbitrate between two published numbers. Both point to throttling, which nothing published describes. *Proposal:* the engine's operational throttle schedule is a calibration parameter; the calibration log records which published values it gives up.

**G13. Rule 15 says every component type has a node; many should not.** Lines, hoses, harness, baffles, diffusers, fasteners and burst disks are not new in kind and ride F5 levels (the catalog proposes level-only components). Which components get a node is arbitrary. *Proposal:* a component type gets a node only if it is new in kind (5.2 layer 2); otherwise it is gated by its material nodes and field levels.

**G14. World-known vs company-known at the 2030 start.** The e-pump cycle and linerless CFRP tanks exist in the world (Rocket Lab) but are proprietary. 5.2 has the world reach nodes on its own and the player buy designs or parts. Does a new company start with nodes 3, 15, 20? *Proposal:* the world holds them; the player starts with the public subset and can license a design or buy units (5.2's two purchase paths), or research the node at a discount because it is known to exist.

**G15. Undisclosed technology needs a stand-in.** Curie's propellant is unpublished and its 120 N is ambiguous (R 5 gap 4). A node and a CRUCIBLE table need a chemistry. *Proposal:* a named stand-in (a green storable bipropellant chosen to match ~320 s Isp, E) tagged as a stand-in in the calibration log, replaced if Rocket Lab publishes.

**G16. Thruster pods: part, or part of thrusters?** The catalog says each thruster is a part, the bracket is a part, and the pod is a group; the schema findings say pods become parts. Kick Stage pods have three thrusters sharing one exterior. *Proposal:* a pod with a shared exterior is one part with thruster components; a standalone thruster (P11) is a part. Needs the owner's ruling.

**G17. Avionics boxes.** The catalog makes a flight computer, PDU or transponder box a part; rule 7 makes anything inside a part's volume a component, and Electron's sit inside the avionics bay and the thrust structure. *Proposal:* rule 7 wins; boxes are designed or bought components; a box bolted to the outside of a structure on its own surface is a part. The catalog needs that correction.

**G18. Calibration data is published per group, not per part.** The Kick Stage's 40 kg dry mass (P) may or may not include Curie, its tanks and the RCS (separate parts here); stage dry masses would also be per group. Groups carry no physics (3.1). *Proposal:* groups may carry calibration targets (mass, CG) that apportion a mismatch across their parts, and nothing else.

**G19. Missing types.** No catalog type fits a kick-stage bus, an electric pump, a motor inverter, a pressurant diffuser or a feed tunnel; `structure.thrust_structure` must cover both a 9-engine cone and a single-engine skirt. *Proposal:* add `pump.electric_centrifugal`, `power.motor_inverter`, `diffuser.pressurant`; treat the kick stage as an equipment bay with a propulsion class node; confirm the engine pattern is a geometry parameter of one type.

**G20. Linings and coatings.** P18's acoustic foam, like TPS (Director finding), is a field over cells more than a component. *Proposal:* the same mechanism as TPS: a material and thickness field over the host's cells, a production parameter.

**G21. Range safety has no field.** The flight termination system is required, bought, and maps to no field (like GSE). *Proposal:* outside the node system, money and mass only, with a licensing rule per launch site.

**G22. Size regimes and class function are arbitrary at the edges.** Where small ends and medium starts is undefined (Electron 1.2 m, Falcon 1 1.68 m, Falcon 9 3.66 m). Rule 10's class is "cylindrical liquid tank for ground-launched rockets", but Curie's tanks fly only in space and Electron's stage-2 tank is ground-launched but works in vacuum. *Proposal:* regime boundaries at real industrial breaks (tooling, transport: about 1.5 m and 4 m), and "ground-launched" means "must survive ground loads and launch", which includes upper stages and kick stages.

**G23. Live topology changes and port severing order.** The HVB handover switches the power graph and then drops a part with live ports; stage and fairing separations sever ports. Nothing says a port must be de-energised or valved off before its surface separates, or what happens if not (arcing, a spill). *Proposal:* a separation component declares the ports it severs and their safe state; severing a live port is a failure event with stored-energy release (rule 11's language).

Top ten for the owner, in order: G8, G1, G2, G3, G4, G9, G12, G13, G11, G17.
