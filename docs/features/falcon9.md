# Feature data: Falcon 9 Block 5 with Crew Dragon

**Status:** data sheet, being written (DATA workstream, night of 2026-10-26/27). Sections land in order: §1 the stack and its parts, §2 engines, §3 fluids and tanks, §4 the flight, §5 systems, §6 ratings and failures, §7 materials and fluids (the values in `data/materials.ron` and `data/fluids.ron`). The design this feeds is [ship-model.md](../design/ship-model.md); the decisions are D100–D108; the plan is [overnight-2026-10-27-falcon9.md](../plans/overnight-2026-10-27-falcon9.md).

## How to read this sheet

Every number carries a tag and a source (the source list is at the end, `[S#]`):

- **P, published**: SpaceX (the Falcon User's Guide, its web pages, statements by its officers), NASA, the FAA (environmental assessments, licences), conference papers, patents.
- **E, estimate**: a credible independent analysis (Spaceflight101, Space Launch Report, Espace & Exploration, NASASpaceflight analyses); whose is named.
- **D, derived**: computed here from published numbers or physics; the derivation is shown.
- **U, unknown**: not public. The row says how a generator should derive it.

Where sources disagree the range is given and one value is **picked** with the reason. SpaceX publishes little about the inside of its vehicles; this sheet never fills a gap silently. Where a part's derived mass and a published total disagree, the gap is reported as the part's unmodelled mass (D101), never spread over the parts.

**Frame.** All positions are along the stack axis, **z up from the bottom of the outer engines' nozzle exits** (the first stage's base plane, standing on the pad). Angles are about the axis, 0° at the first stage's +Y axis (the side the strongback and the crew access arm face, LC-39A), counter-clockwise seen from above. Diameters are outer.

---

## 1. The stack and its parts

### 1.1 Overall

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Stages | 2, both LOX/RP-1, Merlin gas-generator engines | P | [S1] §2 |
| Height with the standard fairing | 70.0 m (229.6 ft) | P | [S1] Table 2-1 |
| Height with Crew Dragon | 65.4 m derived (65.7 m in [S7]; 63.3 m in a NASA kit is the Dragon 1 stack) | D | second stage top 57.3 m (§1.4) + Dragon with trunk 8.1 m [S11]. **Pick 65.4 m** (the stack below sums to it). |
| Diameter, both stages and interstage | 3.66 m (12 ft) | P | [S1] Table 2-1 |
| Liftoff mass, fairing configuration, without payload | 549,054 kg | P | SpaceX web page, as quoted by NASA [S12] and Spaceflight101 [S5] |
| Liftoff mass, Crew configuration | **≈ 560,200 kg** | D | §1.7 sum: stage 1 436,800 + stage 2 109,800 + Dragon with trunk 13,563. Check: 549,054 − fairing 1,700 + Dragon 13,563 = 560,917, within 0.2 %. |
| Liftoff thrust | 7,605 kN (9 × 845 kN, 190,000 lbf each) | P | [S1] §2.2 text. The same guide's Table 2-1 says 7,686 kN (9 × 854 kN, a unit slip in the 2021 guide [S2] carried over); 190,000 lbf = 845.2 kN, so 7,605 kN. |
| Liftoff thrust-to-weight, Crew | 1.38 | D | 7,605 kN / (560,200 kg × 9.80665) |
| Structural margin | design factor 1.4 on limit loads (SpaceX: "40 % above the expected flight loads") | E | Spaceflight101 [S5], quoting SpaceX; NASA-STD-5001 asks 1.4 ultimate for crewed vehicles. **Pick safety_factor 1.4** for walls and joints. |

Stack positions (z of the bottom and top of each assembly; the derivation is in §1.2–1.6):

| Assembly | z bottom (m) | z top (m) | Length (m) | Tag |
|---|---|---|---|---|
| First stage (engines to the interstage joint) | −0.25 (centre nozzle exit) | 39.77 | 40.0 | D (sources: 40.9 [S6], 41.2 [S7], 42.6 [S3]) |
| Interstage (with the first stage's forward skirt) | 39.77 | 46.40 | 6.63 | D (4.5 m in [S3], [S5]: see §1.3) |
| Second stage, above the separation plane | 46.40 | 57.30 | 10.90 | D |
| MVac nozzle, below the separation plane | 41.20 | 46.40 | — | D |
| Crew Dragon trunk | 57.30 | 61.00 | 3.70 | E [S6] |
| Crew Dragon capsule (heat shield to nosecone tip) | 60.90 | 65.40 | 4.50 | E (4.4–5.2 m across sources, §1.5) |

### 1.2 First stage

Construction (all **P**, [S1] §2.2 and Table 2-1): aluminium–lithium skin, aluminium domes; friction-stir welded; **LOX tank on top, monocoque; RP-1 tank below, skin-and-stringer; a common dome between them; a double-wall transfer tube carries LOX through the centre of the RP-1 tank to the engine section**; nine Merlin 1D in a circle of eight around a centre engine, each in its own metal bay; four grid fins near the top; four deployable legs at the base; tank pressurisation by heated helium; nitrogen cold-gas thrusters for coast and recovery attitude control (not used in ascent); pitch, yaw and roll in powered flight by gimballing the engines. The alloy is **Constellium Airware 2195-T8 plate in the barrels and domes** (P, Light Metal Age 2019 [S9], reporting Constellium) → material `al-li-2195`.

Propellant loads (the tanks' design fluids, §3 for temperatures):

| Quantity | Value | Tag | Source |
|---|---|---|---|
| LOX | **287,400 kg** (FAA 2019: 631,300 lb = 286,350 kg) | E/P | Espace & Exploration no. 39 [S3]; Spaceflight101 [S5] computes 287,430 kg from the tank volumes; FAA In-Flight Abort EA [S4] gives 286,350 kg (0.4 % less). **Pick 287,400.** |
| RP-1 | **123,500 kg** (FAA 2019: 257,500 lb = 116,800 kg) | E/P | [S3], [S5] (123,570 kg). The FAA figure is 5 % lower and the same document says the abort-test booster flew with an unusual load ("extra LOX carried as ballast", "less than 10,000 lb of RP-1 at breakup"), so it is not taken as the standard load. **Pick 123,500.** |
| Loaded mixture ratio | 2.33 | D | 287,400 / 123,500; Merlin's own ratio in §2 |
| LOX volume at 66.5 K | 229.4 m³ | D | 287,400 / 1,253 kg/m³ (NIST, §7) |
| RP-1 volume at 266.5 K | 149.7 m³ | D | 123,500 / 825 kg/m³ (§7) |
| Dry mass (Block 5, legs and fins on) | **25,600 kg**, range 22,200–27,200 | E | 22,200 kg [S3] (2017, Full Thrust, likely without recovery hardware); "about 60,000 lb" (27,200 kg) by SpaceX's Hans Koenigsmann, quoted on the NASASpaceflight forum [S10], and 27.2 t "burnout" by Space Launch Report [S6] (with residuals). **Pick 25,600 kg** = 27,200 burnout − ~1,600 kg residuals (0.4 % of the load, typical of large LOX/RP-1 stages). The parts below sum to 22,500 kg; the 3,100 kg gap is the stage's unmodelled mass (brackets, ring frames, fasteners, harness runs, the B5 thermal coatings beyond those listed). |

**Tank geometry** (D; tank inner radius 1.825 m, cross-section 10.46 m²; domes taken as √2:1 ellipsoids, height 1.29 m, 9.0 m³ each, the usual launch-vehicle choice; F9's dome profile is not published (**U**); ullage 2 % of each tank's volume, the usual allowance (**E**)):

| Part | Generator | z (m) | Dimensions | Material | Mass | Notes |
|---|---|---|---|---|---|---|
| Engine section: thrust structure (octaweb) | ThrustStructure | 1.0–4.30 | r 1.83 m; centre engine + ring of 8 at **r 1.29 m** (D), ring starting at 0°, every 45° | octaweb aluminium, alloy U → `al-7075` (machined, bolted in Block 5 [S7]) | ~1,500 kg E | Ring radius: eight 0.93 m nozzles on a ring need r ≥ 0.465/sin 22.5° = 1.22 m to clear each other and r ≤ 1.83 − 0.465 − 0.04 = 1.33 m to stay inside the skirt; middle 1.29 m. Engine gimbal points at z 2.34 m (§2). The centre engine sits 0.25 m lower (Spaceflight101 [S5]: "installed slightly lower with its nozzle protruding"). Each engine in its own metal bay [S1]. |
| Engine heat shield (base closeout) | Revolution (a disc with 9 holes) | 1.0 | 3.66 m disc, nozzles pass through | U: Block 5's "reusable heat shield" [S7]; propose `inconel-718` sheet 1 mm over an insulation blanket | ~500 kg E | Nozzles protrude ~1.0 m below it (photos). |
| Aft skirt/engine-bay shell | Shell | 1.0–4.30 | r 1.83 | `al-li-2195` | in octaweb | |
| RP-1 tank | Tank (compartments: RP-1) | aft dome apex 3.01; barrel 4.30–19.12 (14.82 m); common dome apex 17.83 | volume 152.7 m³ incl. 2 % ullage; downcomer through it | `al-li-2195`, skin-and-stringer | derived (~4,000 kg) | The LOX downcomer (OD 0.45 m E) displaces 0.16 m² of its section. |
| LOX tank | Tank (LOX; common bulkhead with the RP-1 tank) | barrel 19.12–39.77 (20.65 m); forward dome apex 41.06 | volume 234.0 m³ incl. 2 % ullage | `al-li-2195`, monocoque | derived (~4,800 kg) | One Tank part with two compartments and a common bulkhead (`split` at z 19.12) fits the generator. Common dome: **P** (a common dome separates the tanks [S1]); insulation of the common dome U (propose `bulkhead_insulation` 0). |
| LOX downcomer (transfer tube) | Component (line) | 3.0–19.1 | OD ~0.45 m E, double wall (vacuum annulus not stated) | `al-li-2195` or stainless U | ~400 kg E | **P** that it is double-walled and central [S1]. Spaceflight101 [S5]: "fed through 10-centimeter lines until reaching a 9-way manifold". |
| Feed manifolds, prevalves, lines to 9 engines | Component | 2.4–4.3 | 9 × 0.10 m lines [S5] per propellant | stainless/aluminium | ~600 kg E | |
| Helium COPVs (pressurant) | Component Bottle | in the LOX tank, z ~30–38 E | count **U** (several; [S13]: "additional helium COPVs in the (much larger) first stage LOX tank"); derived total volume **≈ 2.6 m³** | carbon overwrap on aluminium liner (COPV 2.0 in Block 5 [S7]) | ~400 kg D | Derivation: at MECO the ullage is 384 m³ at ~3.5 bar and ~250 K → 64,700 mol = 259 kg He; bottles at 5,500 psi (37.9 MPa, E: the usual Falcon value, §3) and 66 K hold ~130 kg/m³ of helium (real gas), ~90 % usable → 2.2 m³; + engine spin-start and leg-deployment helium → ~2.6 m³. Suggest 6 bottles of 0.43 m³ (sphere-cylinders Ø 0.6 m × 1.7 m) mounted on struts inside the LOX tank. |
| Interstage | see §1.3 | | | | | |
| Grid fins | GridFin × 4 (pattern Radial 4 at 45°, 135°, 225°, 315°) | hinge z 44.6 E; stowed tips up to ~46.1 | **1.5 m span (out from the body) × 1.2 m wide** E, lattice depth ~0.2 m E, cell ~0.12 m E, web ~4 mm E | **titanium, single-piece cast and cut** (P, Musk June 2017 [S14]; Block 5 standard [S7]) → `ti-6al-4v` | derived by the generator (~150–250 kg each D; a 70 kg figure online [S15] is too light for a 1.8 m² titanium lattice) | Deflection range U (propose ±20° pitch/yaw/roll as commanded, rate 30 °/s E). **Hydraulic actuators** (Musk Dec 2018 [S16]: "Grid fin hydraulic pump stalled… Pump is single string… we will likely add a backup pump & lines"; a pressure relief valve was added after [S7]); earlier stages used open-loop hydraulics that vent their fluid [S5]. Deployed after stage separation by the same hydraulics (Actuator::Hydraulic). Aligned with the legs (photos). |
| Cold-gas thruster pods (nitrogen) | Component Thrusters × 2 | z ~45.8 E, at 90° and 270° (between fins) E | two pods, each with nozzles for pitch, yaw and roll (count per pod **U**, propose 4) | aluminium housing | ~100 kg each E | Thrust per nozzle **U**: derived from the flip after separation, §2.4. Nitrogen bottles: Component Bottle, in the interstage, U (propose 2 COPVs of 0.2 m³ at 5,500 psi). Spaceflight101 [S5]: "installed on top of the first stage". |
| Landing legs | Leg × 4 (Radial 4 at 45°, 135°, 225°, 315°) | hinge z 1.5 at r 1.9 m E; stowed up to z ~9.7 | **span 18 m deployed** (P, SpaceX via [S9], [S5]); leg length **8.2 m** D; deployed **120° from stowed (30° below horizontal)** E | **carbon fibre and aluminium honeycomb** [S5] → `cfrp` + `al-honeycomb` | **2,000 kg for four** (P: "less than 2,100 kg" [S9], "around 2,100" [S5]) | Length: foot radius 9.0 m − hinge radius 1.9 m = 7.1 m horizontal; at 30° below horizontal L = 7.1/cos 30° = 8.2 m and the feet sit 4.1 m below the hinge, z −2.6 m (photos of landed stages show the bells ~2–3 m above the deck). Deployment by **high-pressure helium pneumatics** (P [S5]); Block 5 legs retract after landing [S7], self-levelling from 2022 [S7]. Telescoping strut with a single-use **crush core** (aluminium honeycomb) — stroke **U**, propose 0.4 m. Deploy time ~ a few seconds (videos), rate U → propose 40 °/s. |
| Raceway (cable tunnel along the tanks) | Shell segment / Component | 4.3–39.8 at 0° | ~0.25 m wide | aluminium, black TPS coat (B5) | ~250 kg E | Black thermal coating on legs, raceway and interstage in Block 5 (P [S7]). |
| Avionics (flight computers, IMU, GPS, engine controllers' hub, telemetry) | Component(s) | in the interstage (z ~43) and engine section | — | — | ~300 kg E | §5. Block 5: three-string, fault-tolerant (P [S1]). |
| Batteries | Component Battery | interstage and engine section | lithium-ion, "multiple redundant" (P [S1]) | — | ~100 kg E | §5. |
| Flight termination system | Component | LOX/RP-1 tank walls (linear charges) + safe-and-arm in interstage | — | — | ~50 kg E | Autonomous (AFSS) with destructors on both stages (P [S4] §2.1.4). |
| Thermal protection coatings (Block 5) | Finish | octaweb, legs, raceway, interstage | — | black ablative/insulating coat U → `black-paint` | in the gap | |

**Hold-down points.** The stage is "held down at the base via **hydraulic clamps**"; "the hydraulic release system is activated at T−0" once the flight computer has checked ignition and full power (P, [S1] §10, [S22] §8.5.5). **Four** clamps (E, from photos of the LC-39A transporter-erector; four is also the count of leg and load hard points). Mount names `hold_#` at z 1.0 m, r 1.83 m, at 45°+90°k (aligned with the legs) E.

### 1.3 Interstage

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Construction | aluminium honeycomb core, carbon-fibre face sheets | P | [S1] §2.4 |
| Length | **6.63 m** from the LOX tank's forward weld (z 39.77) to the separation plane (z 46.40); [S3]/[S5] give 4.5 m, likely measured from the tank's forward skirt | D | The MVac nozzle (5.2 m below the separation plane, §2.2) must clear the first stage's forward dome apex (z 41.06) — sets the separation plane at ≥ 46.4 m. |
| Wall | Sandwich: CFRP faces 1.5 mm each, core 25 mm | E | typical of large launch-vehicle interstages; [S1] does not give thicknesses |
| Mass (shell, rings, separation fittings) | ~900 kg D (shell 52.9 m² × 6.5 kg/m² = 344 kg + rings, latches and pushers) | D/E | |
| Separation system | **three mechanical latches** between the top of the interstage and the base of the second stage's fuel tank, released by a **high-pressure helium circuit through redundant actuators**; the helium also preloads **four pneumatic pushers**, plus a **redundant centre pusher** | P | [S1] §2.4. No pyrotechnics. Pusher stroke and force U: propose a separation Δv of ~0.5 m/s between the stages (E, typical), from 4 × ~35 kN over 0.3 m. |
| Contents | grid fins and their hydraulics, the cold-gas pods and their nitrogen, first-stage avionics | E | Spaceflight101 [S5] |

### 1.4 Second stage

Construction (**P**, [S1] §2.5 and [S8] for the transfer tube): "a shorter version of the first stage tank" with the same materials and tooling; **LOX tank on top (monocoque), RP-1 below (skin-and-stringer; [S5] says monocoque), common dome, a LOX transfer tube from the LOX tank down through the RP-1 tank to the engine** (the CRS-7 investigation describes its annulus [S8]); one MVac with a fixed 165:1 nozzle and dual-redundant TEA-TEB igniters; nitrogen cold-gas attitude control (pitch and yaw with the gimbal, roll by the thrusters, and coasts); heated-helium pressurisation from **three COPVs inside the LOX tank** (P, SpaceX's AMOS-6 findings [S13]).

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| LOX | **76,200 kg** (168,100 lb); [S3] 75,200 | P | FAA In-Flight Abort EA [S4]; the same 168,000 lb appears in the FAA's later Falcon Heavy figures [S17]. **Pick the FAA's.** |
| RP-1 | **29,500 kg** (65,000 lb); [S3] 32,300 | P | [S4] |
| Loaded mixture ratio | 2.58 | D | 76,200/29,500. Higher than stage 1's 2.33: the MVac runs oxidizer-richer or carries LOX margin; U which. |
| Dry mass | **4,000 kg** (3,900–4,500) | E | [S3], [S5]; Space Launch Report 4.5 t at burnout with residuals [S6]. **Pick 4,000.** |
| LOX tank | barrel 4.21 m; volume 62.0 m³ | D | 76,200/1,253 × 1.02; domes as stage 1 |
| RP-1 tank | barrel 3.52 m; volume 36.5 m³ | D | 29,500/825 × 1.02, less the transfer tube (OD 0.35 m E) |
| Length above the separation plane | 10.90 m | D | aft dome apex 0.30 m above the plane (the MVac thrust cone under it, E) + 1.29 + 3.52 + 4.21 + 1.29 + forward skirt 0.29 |
| Overall length with the MVac | 16.1 m | D | 10.90 + 5.2 m of engine below the plane (Space Launch Report 16.0 m "incl. I/S" [S6]; [S7] 13.8 m) |

| Part | Generator | z (m) | Dimensions | Material | Mass | Notes |
|---|---|---|---|---|---|---|
| Aft skirt and MVac thrust cone | Shell | 46.40–47.99 | r 1.83 | `al-li-2195` | ~250 kg E | The separation latch fittings (3) are on its lower ring. |
| RP-1 tank | Tank (RP-1) | aft dome apex 46.70; barrel 47.99–51.51 | 36.5 m³ | `al-li-2195` | derived | |
| LOX tank | Tank (LOX; common bulkhead) | barrel 51.51–55.72; forward dome apex 57.01 | 62.0 m³ | `al-li-2195` | derived | |
| LOX transfer tube | Component (line) | 46.7–51.5 | OD 0.35 m E, double wall (an annulus: P [S8]) | U | ~100 kg E | |
| Helium COPVs | Component Bottle × 3 | in the LOX tank, z ~52–56, on axial struts | **3** (P [S13]); each ~0.17 m³ D | carbon overwrap, aluminium liner (COPV 2.0 since late 2018 [S7]) | ~60 kg each D | Volume: end-of-burn ullage 98.5 m³ at ~3.5 bar, ~250 K → 66 kg He; at 37.9 MPa and 66 K ~130 kg/m³, 90 % usable → 0.56 m³ → 3 × 0.19 m³. Struts: cast stainless rod ends failed on CRS-7 (§6). "Mission extension kits" add COPVs and TEA-TEB for long coasts [S7]; not on Crew flights. |
| MVac | Engine | gimbal at 46.6 | §2.2 | §2.2 | §2.2 | |
| Cold-gas thrusters (nitrogen) | Component Thrusters × 2 pods | aft skirt, z ~47, at 90° and 270° E | count per pod U (propose 4: ±pitch/yaw, ±roll) | | ~30 kg each E | §2.4. Nitrogen bottle(s): Component Bottle in the aft skirt, U (propose 1 × 0.2 m³ at 5,500 psi). |
| TEA-TEB igniter fluid tank | Component Tank | aft skirt | U (propose 0.01 m³) | stainless | ~10 kg E | Dual-redundant igniters (P [S1]); amount sets MVac's ignitions (§2.2). |
| Avionics (the vehicle's main flight computers, IMU, GPS, S-band, C-band transponder) | Component(s) | forward skirt, z ~57 | | | ~150 kg E | The second stage hosts the vehicle's guidance in ascent (Spaceflight101 [S5]); three-string (P [S1]). |
| Batteries | Component Battery | forward skirt | lithium-ion (P [S1]) | | ~60 kg E | |
| FTS | Component | tank walls + forward skirt | | | ~30 kg E | Crew configuration: a pyrovalve for thrust termination on stage 2 (P [S4] §2.1.4). |
| Dragon interface ring (payload adapter for Dragon) | Shell (short ring) | 57.01–57.30 | r 1.83, 0.29 m | aluminium | ~100 kg E | The trunk's base attaches here; separation mechanism U (§4). |

### 1.5 Crew Dragon capsule

Published overall (**P**): height with trunk **8.1 m**, diameter **4.0 m** [S11] (FAA: base width 13 ft = 3.96 m, height 17 ft [S4]); pressurised volume **9.3 m³**, trunk unpressurised volume **37 m³** [S11]; capsule **dry 7,700 kg** (16,976 lb "without cargo", FAA [S4] App.); crew of **4** for NASA (7 seats possible) [S12]; **8 SuperDraco in four pairs** (pods) at the sidewall; **16 Draco**, **4 of them forward-facing under the nosecone** [S7] (the nosecone "protects the docking port and four forward-facing thrusters" — NASA/SpaceX text quoted in [S7]); propellant and helium in **carbon-overwrapped titanium spherical tanks at the capsule's base (the service section)** [S7]; heat shield **PICA-X/PICA-3** [S12], [S7]; **2 drogues and 4 mains** [S4]; **IDSS docking port** under a **hinged nosecone** [S12]; windows (several, see below); welded aluminium pressure vessel, a primary heat-shield support structure and a backshell TPS support structure [S4].

Capsule height (heat shield to nosecone tip): 4.4–5.2 m across sources (4.5 m [S7] via Orbital Velocity; 4.9 m Space Launch Report [S6]; FAA's 17 ft = 5.2 m [S4]). **Pick 4.5 m**, so trunk + capsule = 8.1 m with 0.1 m overlap at the heat-shield rim.

**Profile for the Revolution generator** (E, from the published height and diameter and photos; capsule frame z = 0 at the heat shield's centre, the lowest point): heat shield a spherical cap of radius **4.7 m** (Apollo-like bluntness, E) to the shoulder at r 1.98 m, z 0.45; shoulder radius 0.1 m; **sidewall a cone of 20° half-angle** (E; Dragon 1's was 15°, Dragon 2's looks steeper in photos) from (z 0.55, r 1.98) to (z 3.40, r 0.94); the nosecone hinge ring at z 3.40; the nosecone a dome from r 0.94 to the tip at z 4.50. Points: `[(0.0, 0.0), (0.20, 1.36), (0.45, 1.98), (0.55, 1.98), (3.40, 0.94), (4.00, 0.80), (4.35, 0.45), (4.50, 0.0)]`.

| Part / component | Generator | Where | Dimensions | Material | Mass | Tag, source, notes |
|---|---|---|---|---|---|---|
| Pressure vessel (cabin) | Revolution (inner) | z 0.6–3.4 | volume **9.3 m³** pressurised (P [S11]); cabin 1 atm (14.7 psi) air (§5) | welded aluminium (P [S4]), alloy U → `al-2219` (the usual weldable choice) | derived | Leak tolerance: lands safely with a leak up to a 6.35 mm equivalent orifice (P, [S7] quoting NASA). |
| Heat shield | Revolution (cap) | z 0–0.45 | Ø 3.96 m, cap R 4.7 m; thickness U → propose 60 mm (E: PICA tiles on Dragon 1 ~ 76 mm) | **PICA-X / PICA-3** → `pica-x`, on a carrier structure (U: propose `cfrp` 3 mm) | derived (~800 kg D: 12.6 m² × 0.06 m × 270 kg/m³ = 204 kg TPS + carrier and fittings) | P material [S12]. |
| Backshell TPS (sidewall) | Revolution skin | z 0.55–3.40 | ~22 m² | SpaceX's backshell ablator ("SPAM", a silicone ablator, E) → `silicone-ablator`, ~10 mm E | derived | |
| Nosecone | Revolution, hinged | z 3.40–4.50 | Ø 1.88 m base | composite U → `cfrp` | ~100 kg E | **Hinge** on one side; opens ~**0→~100° in a few minutes** after separation (Demo-2: "nosecone open sequence begins" 46 s after separation [S18]); closed for entry. Rate U → propose 1 °/s. |
| Docking adapter (IDSS) | Component Docking | top of the cabin under the nosecone, z ~3.4 | IDSS, ~1.2 m mating ring (the "cupola" window replacing it is 1.2 m [S7]) | aluminium | ~300 kg E | Standard: P [S12]. |
| SuperDraco pods | Component Engines × 4 (2 engines each) | sidewall, z ~1.2–2.2, at 4 positions 90° apart E | each engine 71 kN | Inconel 718 printed chambers (P, §2.3), each pod in a printed nacelle (P [S7]) | engines ~25 kg each E; pod with housing ~100 kg E | Positions in pairs, canted outward (cant U → propose 15°). |
| Draco thrusters | Component Thrusters | **12 around the lower sidewall** (in 4 clusters of 3, E) + **4 forward-facing under the nosecone** (P) | 400 N each | | ~2 kg each E | §2.3. |
| Propellant tanks (MMH, NTO) | Component Tank × 4 E (2 MMH, 2 NTO) | service section, at the base around the cabin, z ~0.6–1.4 | spheres | **carbon-overwrapped titanium** (P [S7]) → liner `ti-6al-4v` | derived | Load **2,563 kg: NTO 1,588 kg + MMH 975 kg** (P, 3,500 lb + 2,150 lb [S4]); MR 1.63. Tank count U (propose 4 spheres: 2 × NTO 0.56 m³, 2 × MMH 0.58 m³, D from §7 densities + 10 % ullage). |
| Helium pressurant bottles | Component Bottle | service section | U (propose 2 × 0.1 m³ at ~4,500 psi E) | carbon-overwrapped titanium (P) | ~40 kg each E | Since 2019 the helium-to-propellant path uses **burst discs instead of check valves** (P, SpaceX [S19]). |
| Parachutes: 2 drogues, 4 mains | Component Chute | under the backshell near the top, beside the docking ring (cross-section drawing [S7]) | drogues **Ø 19 ft (5.8 m)**, conical ribbon, risers 72 ft (21.9 m); mains **Ø 116 ft (35.4 m)**, Kevlar and nylon, risers 147 ft (44.8 m) | nylon/Kevlar | ~400 kg for all E | P [S4] App. (FAA biological evaluation 2017). Deployment in §4. |
| Seats | Component Seat × 4 (7 possible) | cabin, z ~1.0 | | carbon fibre, Alcantara (P, SpaceX press) | ~25 kg each E | Seats rotate up for launch (Demo-2: "verify ready for seat rotation" [S18]). |
| ECLSS | Component LifeSupport | under the floor and around the cabin | cabin 9.3 m³; O₂ and N₂ in COPVs; CO₂ removal by lithium hydroxide (E) | | ~300 kg E | §5. Cabin temperature 65–80 °F (18–27 °C) (P [S12]). |
| Batteries | Component Battery × 4 | service section | lithium polymer, 28 V and 120 V buses (E [S7]) | | ~200 kg total E | §5. |
| Avionics, displays | Component Computer | cabin, three touch screens above the two centre seats (P [S7]) | triple-redundant flight computers (E) | | ~100 kg E | §5. |
| Windows | Component (Glass) | sidewall | 4 windows (photos; NASA [S12] says "three windows" plus the hatch window) | glass, layers U → `fused-silica` | ~15 kg each E | |
| Side hatch | part of the pressure vessel | sidewall, crew access arm side (0°) | ~1 m E | aluminium | in vessel | |

### 1.6 Crew Dragon trunk

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Height | 3.7 m | E | Space Launch Report [S6] (DM-1: "a 3.7 meter tall trunk") |
| Diameter | 3.7 m at the base (the second stage's 3.66 m) to ~3.9 m at the top | E | photos |
| Unpressurised volume | 37 m³ | P | [S11] |
| Solar array | **body-mounted cells on half the trunk's circumference**; the other half is the **radiator** | P | NASA [S12] ("half of the trunk contains a solar array… the other half contains a radiator") |
| Solar output | 1.5–2 kW | E | [S7] (University of Cincinnati handout); §5 derives it from area and cell efficiency |
| Fins | **4 small fins** at the base for aerodynamic stability in an abort | P (fins), E (count from photos) | [S7] |
| Attachment to the capsule | "the claw" | P | [S7] |
| Structure | composite (U) → `cfrp` sandwich | E | |
| Mass | **2,600 kg** | D | NASA Demo-2: Dragon with trunk ≈ 27,600 lb at undocking [S20]; the capsule ≈ 21,200 lb at entry (crew 2 + 330 lb cargo) [S20]; difference 6,400 lb = 2,900 kg, less ~300 kg propellant spent between undocking and trunk jettison (E) → 2,600 kg. |

### 1.7 Masses and the totals check

| Item | Mass (kg) | Tag |
|---|---|---|
| Stage 1 dry (legs, fins) | 25,600 | E (§1.2) |
| Stage 1 LOX | 287,400 | E/P |
| Stage 1 RP-1 | 123,500 | E |
| Stage 1 helium, nitrogen, TEA-TEB, trapped gases | ~300 | D (He 259 kg + N₂ ~40 kg) |
| **Stage 1 at liftoff** | **436,800** | D |
| Stage 2 dry | 4,000 | E |
| Stage 2 LOX | 76,200 | P |
| Stage 2 RP-1 | 29,500 | P |
| Stage 2 helium, nitrogen, TEA-TEB | ~100 | D |
| **Stage 2 at liftoff** | **109,800** | D |
| Capsule dry | 7,700 | P |
| Dragon propellant (NTO 1,588 + MMH 975) | 2,563 | P |
| Crew of 4 in suits | 400 | E (NASA limits ~100 kg each in suit) |
| Cargo | 300 | E (NASA Crew rotations carry a few hundred kg) |
| Trunk | 2,600 | D |
| **Dragon with trunk** | **13,563** | D (Space Launch Report: ~12.5 t uncrewed DM-1 [S6]) |
| **Liftoff, Crew configuration** | **560,160** | D |

Check against published totals: the fairing configuration without payload is published at 549,054 kg [S12]; this sheet gives 436,800 + 109,800 + fairing 1,700 (E [S5]) = 548,300 kg, **0.14 % low** — consistent. The total propellant load 516,600 kg against the FAA's Block 5 figure of 1,135,925 lb = 515,250 kg (quoted from the FAA/Air Force Falcon EIS [S17]; not read directly) is 0.26 % high — consistent.

---

## 2. Engines

### 2.1 Merlin 1D (Block 5), first stage × 9

Cycle (all **P**, [S1] §1.3, §2.2): open gas generator; one shaft drives the LOX pump, the fuel pump and the turbine; regeneratively cooled chamber and nozzle with a **milled copper-alloy liner**; **pintle injector**; TVC actuators powered by **high-pressure RP-1 from the engine's own pump** (no separate hydraulic system); each engine in its own metal bay; the vehicle shuts down an off-nominal engine before it fails (engine-out capability "through much of first-stage flight"). Ignition by **TEA-TEB** (pyrophoric). The gas generator's exhaust heats the helium for tank pressurisation (E, Spaceflight101 [S5]).

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Thrust, sea level | **845 kN** (190,000 lbf) | P | [S1]; SpaceX May 2018 "8 % more, 190,000 lbf" (Musk, [S7]) |
| Thrust, vacuum | **914 kN** (205,500 lbf) | P | SpaceX, May 2016 ([S5], [S7]) |
| Isp, vacuum | **311 s** | P | SpaceX web page 2013 [S7] (an earlier rating; the later engine is at least as good) |
| Isp, sea level | 282 s published (2013); **287.5 s derived** for Block 5 | P/D | The three Block 5 numbers fix the flow and the exit area: ṁ = 914 kN/(311 s·g₀) = **299.7 kg/s**; Aₑ = (914 − 845) kN / 101,325 Pa = **0.681 m²**; Isp_SL = 845 kN/(ṁ·g₀) = 287.5 s. The 2013 282 s belongs to the 147,000-lbf engine; **pick 287.5 s** so thrust, flow and Isp agree (D101: report the 2 % gap to the 2013 number). Everyday Astronaut gives 285/313 s and 845/934 kN (E, unsourced). |
| Mass flow, full thrust | 299.7 kg/s: LOX 209.7, RP-1 90.0 | D | ṁ above, mixture ratio below |
| Mixture ratio (O/F) | **2.33** (engine total, gas generator included) | D | the stage's loaded ratio (§1.2), since the stage burns to near depletion of both; U published |
| Chamber pressure | 9.7 MPa (1,410 psi) at the 2011 rating (147,000 lbf SL); **~12.5 MPa derived for Block 5** | P/D | 1,410 psi: AvWeek 2011 [S7]. If the throat is unchanged (Dt below), Pc scales with vacuum thrust: 9.7 × 914/690 ≈ 12.9 MPa; from F = Cf·Pc·At with Cf_vac(ε 16, γ 1.22) = 1.78 × 0.97 efficiency: 914 kN/(1.73 × 0.0426 m²) = 12.4 MPa. **Pick 12.5 MPa** (U published). |
| Expansion ratio | **16** | P | SpaceX 2011 goals [S7]; [S3] |
| Exit diameter | **0.93 m** (0.92 m in [S7]) | D | from Aₑ above. A larger 1.03 m follows from the 2013 Isp pair, but the octaweb rules it out: eight 1.03 m bells cannot fit a ring inside 3.66 m (§1.2). |
| Throat diameter | **0.233 m** | D | Aₜ = Aₑ/16 = 0.0426 m² |
| Nozzle length, throat to exit | 1.04 m | D | 80 % bell: 0.8 × (0.93 − 0.233)/2 / tan 15° |
| Powerhead length (gimbal to throat), radius | 1.30 m, 0.45 m | E | photos; Merlin 1C was 2.92 m long overall [S7] |
| Engine length overall | ~2.4 m (gimbal to exit 2.34 m) | D | |
| Dry mass | **470 kg** with its TVC actuators | E | Tom Mueller (SpaceX's founding propulsion CTO), 2015: "The Merlin 1D weighs 1030 pounds, including the hydraulic steering (TVC) actuators … the new full thrust variant weighs the same" [S7]; Spaceflight101 470 kg [S5] |
| Thrust-to-weight (CORE's `thrust_to_weight`, vacuum thrust over sea-level weight) | **198** | D | 914 kN / (470 kg · g₀); Spaceflight101 "198" [S5] |
| Throttle range | **40 %–100 %** (engine); the guide's ascent range is **57 %–100 %** of sea-level thrust | P | Musk, 6 May 2016, of the landing burn: "min is ~40% of 1 Merlin" [S7]; [S1] Table 2-1: 190,000 to 108,300 lbf at sea level (= 60 % in vacuum terms). **Pick `throttle_min` 0.40** (what the engine can do; the guide's 57 % is the ascent profile's), throttle as a fraction of full flow. |
| Thrust against throttle and ambient pressure | F = τ·914 kN − p·0.681 m² (τ the flow fraction, p ambient) | D | ideal nozzle; Isp(τ, p) = F/(τ·ṁ·g₀). At deep throttle a 16:1 nozzle at sea level separates (Summerfield p_e < ~0.35 p_a): p_e at full thrust ≈ 12.5 MPa × 0.0064 = 80 kPa, at 40 % ≈ 32 kPa — separation risk at sea level below ~45 % (D); flown anyway (landing burns are at ~40–70 %); no failure from it unless data says so. |
| Gimbal range | **±5°** | E | spacelaunchlive (low confidence, no primary source); U published. Class: kerolox boosters ±5° to ±8°. |
| Gimbal rate | **20 °/s** | U | class estimate for hydraulic TVC of this size; propose 20 °/s and a 0.1 s actuator time constant. |
| Ignition | TEA-TEB; **on the pad all nine use the ground supply**; three engines carry their own canisters for relights: **E1, E5 and E9** (E9 the centre engine) | E | Everyday Astronaut (SES-22 and other mission pages, 2022) [S21]; FAA: TEA-TEB is "used as a first and second stage ignitor" [S4] (P) |
| Ignitions | pad start: unlimited from the ground (any engine); **in flight: E1, E5, E9 three starts each** (boostback, entry, landing) — the other six none | E | [S21]: "restarts available are 3–4 per mission"; Crew missions to a droneship do entry (3 engines) and landing (1 engine, sometimes 3) |
| Spin start | turbopump spun up by **high-pressure helium** before ignition | E | Spaceflight101 [S5] |
| Start sequence and time | start command at **T−3 s**; to full thrust in **~2 s** (stagger allowed); held down until the computers confirm all nine healthy; clamps released at T−0 | P (T−3 s, hold-down, release) / E (2 s) | [S1] §10 timelines "T−3s Engine start sequence"; [S18] "T−3 s engine controller commands engine ignition sequence"; 2 s: the time from T−3 s to the health check before T−0 (D). **`start_time` 2.0 s.** |
| Shutdown | commanded; tail-off impulse U → propose **0.25 s at full thrust equivalent** (~210 kN·s) | U | class value for gas-generator kerolox engines |
| NPSP required (pump inlets) | LOX **0.17 MPa**, RP-1 **0.10 MPa** | U | not published. Class values for kerolox turbopumps with inducers (NPSH ~15 m LOX, ~13 m RP-1, typical of booster engines). Falcon's densified LOX (vapour pressure 2.7 kPa at 66.5 K, §3) gives large margin, so these values bind only on warm or depleted propellant. |
| Nozzle heat (D077 share) | regeneratively cooled: `nozzle_heat_fraction` 0 | D | no radiatively cooled part on the booster engine |
| Engine-out | the stage tolerates one engine out through much of the ascent; the computers shut a failing engine down | P | [S1] §1.3; CRS-1 (2012) lost one engine at T+79 s and reached orbit (§6) |
| Rated life | Block 5 designed for 10 flights with minor maintenance, up to 100 with refurbishment (stage); one Merlin flew its 22nd mission in Feb 2024 | P | [S7] (SpaceX, Musk 2018); SpaceX 23 Feb 2024 [S7] |

### 2.2 Merlin Vacuum (MVac D), second stage × 1

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Thrust, vacuum | **981 kN** (220,500 lbf) | P | [S1] Table 2-1 |
| Isp, vacuum | **348 s** | P | SpaceX web page [S7] |
| Mass flow | 287.5 kg/s (LOX 207.2 at O/F 2.58, RP-1 80.3) | D | 981 kN/(348 s·g₀); O/F from the stage's loaded ratio (§1.4) |
| Expansion ratio | **165:1**, fixed, non-deploying | P | [S1] |
| Exit diameter | **3.0 m** (3.3 m in [S7] and Everyday Astronaut, unsourced) | D | 165 × Aₜ of the Merlin 1D (0.0426 m², the same chamber) = 7.03 m² → 2.99 m. A 3.3 m exit would need a 0.257 m throat, inconsistent with §2.1. **Pick 3.0 m.** |
| Throat diameter | 0.233 m | D | as §2.1 |
| Nozzle length, throat to exit | **4.13 m** | D | 80 % bell |
| Regeneratively cooled section | throat to ε ≈ 16 (the booster nozzle's exit), then a **radiatively cooled niobium-alloy extension** | P (niobium) / E (split) | the short-nozzle variant uses "75 % less niobium" [S7]; **`Extension.start` 0.25** of the nozzle length (1.04/4.13), material `niobium-c103` with `silicide-coating`, thickness **1.0 mm** (E: gives ~180 kg, see mass) |
| Dry mass | **650 kg** | D | 470 kg (the Merlin 1D, §2.1) + the extension 20 m² × 1.0 mm × 8,860 kg/m³ ≈ 180 kg; Spaceflight101's 490 kg [S5] looks too light for a 3 m niobium bell. |
| Thrust-to-weight | 154 | D | 981 kN / (650 kg · g₀) |
| Throttle range | **64 %–100 %** published (2019–2025 guides); 39 % in the 2015 guide | P | [S1]: 220,500 to 140,679 lbf; [S7] quoting the 2015 guide: 210,000 to 81,000 lbf. **Pick 0.64** (the current published range). |
| Ignition | **dual-redundant TEA-TEB igniters** | P | [S1] |
| Ignitions | **≥ 3 on a standard stage** (SES-1, SES-2, the deorbit burn); more with a mission-extension kit (more TEA-TEB, COPVs) | E | [S7]; the guide's sample GTO timeline has two burns plus disposal [S1] §10 |
| Start | needs settled propellant (§3.5) and chilled-in engine; start time ~**3 s** to full thrust (E: MECO→SES-1 is 7–8 s, sep at +3–4 s) | E | Demo-2: sep +2:36, SES +2:44 [S18] |
| Gimbal | range U → propose ±5°; rate 10 °/s | U | pitch and yaw by gimbal, roll by nitrogen thrusters [S1] |
| Nozzle heat | extension radiatively cooled: `nozzle_heat_fraction` per D077, propose **0.010** of jet power to the extension wall | E | niobium extensions run ~1,400–1,600 K in vacuum (C-103 + R512E limit ~1,640 K, D077) |
| Short-nozzle variant (2023–) | 10 % less thrust (~883 kN), 75 % less niobium | P | [S7] (SpaceX, Transporter-7); not used on Crew flights (E) |

### 2.3 SuperDraco and Draco (Crew Dragon)

| Quantity | SuperDraco (8, in 4 pods of 2) | Draco (16) | Tag | Source |
|---|---|---|---|---|
| Propellants | NTO / MMH, hypergolic, shared tanks | same | P | [S7] (SpaceX, NASA) |
| Thrust | **71 kN** each (16,000 lbf; capable of 16,400 lbf) | **400 N** (90 lbf), vacuum | P | SpaceX 2014 qualification release, FAA DragonFly EA [S7]; Draco: SpaceX [S7] |
| Isp | **235 s** at sea level (E: unsourced) | **300 s** vacuum | E / P | [S7]; Draco: SpaceX 2009 guide [S7] |
| Mass flow | 30.8 kg/s each, 246 kg/s for 8 | 0.136 kg/s | D | F/(Isp·g₀) |
| Chamber pressure | **6.9 MPa** (1,000 psi) | U (propose 1.0 MPa) | P/U | [S7] (NSF and AvWeek, 2014) |
| Throttle | **20 %–100 %** | on/off, pulsed | P | SpaceX 31 July 2014 [S7]: "successfully throttling between 20% and 100%" |
| Start transient | **ignition to full thrust 100 ms** | ~10–20 ms (U, class for 400 N bipropellant thrusters) | P / U | NASA, 1 Feb 2012 [S7] |
| Pulse | — | minimum impulse bit U → propose 20 ms pulses (class: R-4D 490 N ~ 10–20 ms) | U | |
| Cycle | pressure-fed, regeneratively cooled printed Inconel chamber, each pod in a printed nacelle | pressure-fed | P | [S7] (SpaceX 2014) |
| Abort burn | all 8 at full thrust for ~5–6 s on the pad abort; Dragon reached 100 mph in 1.2 s (≈3.8 g mean) | — | P | NASA 2012 (5 s planned), Ars Technica 2016 [S7] |
| Engine mass | U → propose **40 kg** each (T/W ~180) | U → **3 kg** (R-4D class) | U | |
| Ignitions | many (hypergolic); restartable | thousands of pulses | P | [S7] |
| Placement | four pods at the sidewall, 90° apart, each pair canted outward | 4 forward-facing under the nosecone (P); 12 around the lower sidewall, in 4 clusters of 3 (E) | P/E | §1.5 |
| Propulsive landing | enabled again in 2024 as a backup to the parachutes | — | P | [S7] |

### 2.4 Cold-gas thrusters (nitrogen), both stages

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Gas | gaseous nitrogen (GN₂) | P | [S1] Table 2-1 |
| Stage 1 use | coast and recovery attitude control only (flip after separation, entry attitude); **not** used in ascent | P | [S1] Table 2-1 ("recovery only") |
| Stage 2 use | roll during burns; pitch, yaw and roll in coasts; propellant settling before restarts (§3.5) | P (attitude) / E (settling) | [S1] |
| Isp | **70 s** (falls with the bottle's gas temperature as it blows down: Isp ∝ √T) | D | ideal maximum for N₂ at 290 K is 79 s (vₑ = √(2γ/(γ−1)·RT/M) = 776 m/s); real thrusters reach 85–92 % of it |
| Thrust, stage 1 | **U**; propose **2 kN per nozzle**, 4 nozzles per pod, 2 pods | U/D | Order of magnitude from the flip: a booster of ~50 t and ~9×10⁶ kg·m² turned 180° in ~30–40 s (droneship profile, webcasts) needs a couple of 70–90 kN·m, i.e. 2 × ~1.3 kN at a 30 m arm; the faster RTLS flips (~15 s) need several kN per side. Spaceflight101 [S5]: v1.1 brought "a more powerful cold gas reaction control system". |
| Thrust, stage 2 | **U**; propose **200 N per nozzle**, 4 per pod, 2 pods | U | |
| Bottles | Component Bottle, nitrogen at up to **34.5 MPa** (the pad's GN₂ supply is 5,000 psi [S22]): stage 1 ~2 × 0.2 m³, stage 2 ~1 × 0.2 m³ | P (supply pressure) / U (sizes) | 2015 guide's launch-site services [S22] |
| Fairing halves | have their own nitrogen cold-gas thrusters (not on Crew flights) | P | [S4] App. |

## 3. Fluids and tanks

### 3.1 Loading temperatures and densities

| Quantity | Value | Tag | Source |
|---|---|---|---|
| LOX loading temperature | **66.5 K** (−340 °F, −207 °C), about 12 K above the triple point (54.36 K) | P | Musk, 17 Dec 2015: "−340 F in this case … First time anyone has gone this low for O2" [S7]; NASA (Dragon crew loading): LOX "near −340 °F" [S7]; Spaceflight101 −207 °C [S5] |
| LOX density at 66.5 K | **1,253 kg/m³** (1,141 at the normal boiling point 90.19 K: +9.8 %) | D | NIST saturation table (§7): 1,259.7 at 65 K, 1,237.0 at 70 K |
| LOX vapour pressure at 66.5 K | **3.4 kPa** | D | NIST (§7): 2,335 Pa at 65 K, 6,262 Pa at 70 K, log-linear |
| RP-1 loading temperature | **266.5 K** (20 °F, −7 °C) | P | Musk, same tweet: "[RP-1 chilled] from 70F to 20F" [S7]; NASA "near 20 °F" [S7] |
| RP-1 density at 266.5 K | **825 kg/m³** (MIL-DTL-25576: 799–815 at 288.7 K) | D | §7 table (0.72 kg/m³ per K from the specification's midpoint 807) |
| Loading sequence (Crew) | RP-1 from **T−35 min**; stage 1 LOX from **T−35 min**; stage 2 LOX from **T−16 min**; engine chill from **T−7 min**; Dragon on internal power **T−5 min**; tanks pressurised to flight pressure from **T−1 min** | P | NASA Demo-2 timeline [S18] |
| Replenishment | tanks topped (LOX replaced as it warms) until the final minutes; the "load-and-go" order was approved by NASA for crew because the densified LOX warms | P | [S7] (NASA) |

### 3.2 Tank pressures and walls

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Ullage pressure, all four main tanks | **3.5 bar absolute** (~36 psig) in flight | E | "a few tens of psi" (Musk's 20 July 2015 press call, as reported by Space Access Society [S23]); U published. Check: the monocoque LOX tank needs internal pressure to carry the compression above it — at 3.5 bar the pressure lifts 3.5e5 × 10.46 = 3.7 MN, comparable to the 4–5 MN the upper barrel carries near MECO (D). |
| Ullage pressure on the pad before T−1 min | ~1.5 bar (vent-and-relief level) | U | the timeline pressurises "to flight pressure" at T−1 min [S18] |
| Maximum expected operating pressure (MEOP), relief setting | **4.0 bar** | U | propose relief at 115 % of flight pressure |
| Burst pressure (design) | **1.4 × MEOP = 5.6 bar** from the walls (the generator's `safety_factor` 1.4) | D | §1.1 |
| Minimum gauge | **3.0 mm** (machined Al-Li sheet/plate) | E | class value; friction-stir welded panels |
| Axial design load, stage 1 RP-1 barrel (skin-stringer) | **6.0 MN** compression (limit) | D | everything above it at liftoff: LOX 287 t + LOX tank ~5 t + interstage + stage 2 110 t + Dragon 13.6 t = 418 t × 1.39 g₀ (T/W at liftoff, thrust-limited) = 5.7 MN, + bending at max-Q ≈ 6 MN |
| Axial design load, stage 1 LOX barrel (monocoque) | **5.5 MN** compression (limit), relieved by pressure | D | near MECO: stage 2 + Dragon + interstage ≈ 124 t at up to ~4.5 g₀ (Crew flights throttle to stay below ~4 g, E) |
| Axial design load, stage 2 tanks | **0.6 MN** (Dragon 13.6 t at 4.5 g₀) | D | |

### 3.3 Pressurisation scheme

- **Pressurant**: helium stored at **5,500 psi (37.9 MPa)** in COPVs **submerged in the LOX tanks** of both stages (colder helium is denser) (E: Musk's 2015 press call via [S23]; Block 5's COPV 2.0 [S7]; P that the bottles are in the LOX tank [S8], [S13]). The pad supplies helium at 5,700 psi at SLC-40 and 6,000 psi at Vandenberg (P, 2015 guide [S22]), an upper bound on the bottles' MEOP.
- **Heating**: the helium passes through heat exchangers on the engines (the gas generator's exhaust, E [S5]) before entering the tanks: "heated helium" (P [S1]). Propose an inlet temperature to the ullage of **~250 K** for the LOX tank and **~300 K** for the RP-1 tank (U).
- **Regulation**: each tank's ullage held at its setpoint while the bottles can (U how: SpaceX's "pressurization controllers" are mentioned [S1] §2.6, so active valves, not fixed regulators — propose a bang-bang valve per tank with ±0.1 bar deadband).
- **Helium budget**: stage 1 ~260 kg delivered (384 m³ of ullage at 3.5 bar, 250 K at MECO), stage 2 ~66 kg (98.5 m³) (D, §1.2, §1.4); plus spin-start helium for the Merlins (U) and the separation and leg-deployment pneumatics (P that these are helium [S1], [S5]).
- **Bottles**: COPV burst ≥ 2.0 × MEOP for vessels outside DOT rules (P: the factor SpaceX requires of payload vessels, [S1] Table 6-4; their own vessels' factor is U → propose 1.5 for flight COPVs, the AIAA S-081 minimum for crewed vehicles is 1.5).
- **Block 5 and AMOS-6**: after AMOS-6 SpaceX changed the helium loading (warmer, slower) and redesigned the COPVs (COPV 2.0) so oxygen cannot pool and freeze in liner buckles (P [S13], [S7]); Block 5 also allowed "faster, later, denser, chilled kerosene fuel loading" (E, Space Launch Report [S6]).

### 3.4 Lines and engine inlets

| Quantity | Value | Tag | Source |
|---|---|---|---|
| Stage 1 LOX downcomer | through the RP-1 tank's centre, double-walled | P | [S1] |
| Engine feed lines | **10 cm** lines to each engine from a 9-way manifold | E | Spaceflight101 [S5] |
| Line lengths, stage 1 | LOX: tank outlet (z 17.8) → manifold (z ~3) ≈ 15 m downcomer + 2 m branch; RP-1: tank outlet (z 3.0) → engine ≈ 1.5 m | D | from §1.2 positions |
| Line loss | propose ΔP = 0.05 MPa at full flow per line (k such that ΔP = k·ṁ²) | U | |
| Inlet pressure | p_ullage + ρ·a·h − ΔP_line; at liftoff, stage 1 LOX: 3.5 bar + 1,253 × 13.6 m/s² × ~37 m (full) = 6.6 bar + 3.5 = 10.1 bar; near MECO (4 g, 1 m head) ≈ 4 bar | D | §5.2 of the design |

### 3.5 Settling (ullage) before MVac ignition

- After MECO the stack coasts ~3 s, separates (the pushers give stage 2 a forward push) and **MVac starts ~8 s after MECO** (Demo-2: MECO 2:33, sep 2:36, SES 2:44 [S18]). The pushers' impulse and the second stage's nitrogen thrusters firing aft settle the propellant (E: the published architecture [S1] has no ullage motors; the nitrogen thrusters are the only means).
- For restarts after a coast (SES-2, the deorbit burn) the stage settles with aft-firing nitrogen thrusters before ignition (E).
- Settling criterion for the sim (D, design §5.2): Bond number Bo = ρ·a·R²/σ ≫ 1 (LOX: σ = 0.0193 N/m at 66.5 K, NIST): with R = 1.83 m, a = 1×10⁻³ m/s² gives Bo ≈ 220 (propose settled above Bo = 100); then a settling time t ≈ √(2L/a) for the liquid to cross the tank (L the tank length): with 200 N × 8 thrusters ≈ 1.6 kN on a 110 t stage, a = 0.015 m/s² → ~30 s for a 6 m tank. Faster with the pushers.

### 3.6 Boil-off and warming

- The main tanks are **uninsulated** aluminium (frost forms on the LOX tanks on the pad, photos: E).
- On the pad the densified LOX warms from the wall: propose a wall heat flux of **2 kW/m²** (U; frosted bare aluminium in humid air, class value 1–5 kW/m²): stage 2 LOX tank (~62 m², 76 t) warms ~**0.6 K per 10 min** without replenishment (D: 124 kW / (76,200 kg × 1,680 J/(kg·K)) = 0.97 mK/s). This is why loading is late and continuous.
- At 3.5 bar the LOX saturates at ~104 K (NIST, §7), so in flight it does not boil; during the coast to the deorbit burn (~50 min) the stage 2 LOX warms in sunlight (U → the thermal network does it).
- Dragon's MMH/NTO are storable: no boil-off; NTO freezes at 261.9 K and MMH at 220.8 K (§7), so their tanks are heated (U → propose heaters keeping 288–300 K).

## 4. The flight

### 4.1 Launch site: LC-39A

| Quantity | Value | Tag | Source |
|---|---|---|---|
| Position | **28°36′30″ N, 80°36′16″ W** (28.6083°, −80.6044°) | P | [S7] (Kennedy Space Center Launch Complex 39A, from NASA) |
| Pad deck elevation | ~14.6 m (48 ft) above sea level, the Apollo-era hardstand | E | U published for today's deck; propose 15 m |
| Launch azimuth to the ISS (51.6°) | **45°** (north-east) | D | sin A = cos 51.6° / cos 28.61° = 0.707 |
| Integration | horizontal, in the Horizontal Integration Facility; rolled out and raised by the **transporter-erector** (TE), whose strongback holds the umbilicals and retracts "just prior to launch" | P | [S1] §10; [S7] |
| Hold-down | **hydraulic clamps** at the base, released at T−0 after the full-thrust health check (§1.2) | P | [S1], [S22] |
| Crew access | the **crew access arm** on the fixed service structure (a new level built in 2018) at the capsule's side hatch; retracts at **T−42 min** | P | [S7]; [S18] |
| Flame trench and deluge | Apollo/Shuttle flame trench; water deluge system (tested 2017) | P | [S7] |
| Other things on the pad | the Starship tower and mount (in work) — out of scope (D104: nothing else on the ground) | — | |

### 4.2 Countdown (Crew; from the Demo-2 timeline [S18])

| Time | Event |
|---|---|
| T−2:35 h | crew ingress |
| T−1:55 h | hatch closed |
| T−45 min | go for propellant load |
| T−42 min | crew access arm retracts |
| T−37 min | **Dragon's launch escape system armed** |
| T−35 min | RP-1 loading begins; stage 1 LOX loading begins |
| T−16 min | stage 2 LOX loading begins |
| T−7 min | Falcon 9 begins **engine chill** |
| T−5 min | Dragon to internal power |
| T−1 min | flight computer's final checks; **tanks pressurised to flight pressure** |
| T−45 s | launch director's go |
| **T−3 s** | **engine controller commands the ignition sequence** |
| **T−0** | **liftoff** (clamps released after the health check) |

The countdown itself is abstracted as a time and funds sink (D104); the sim starts on the pad at about T−1 min (fuelled, pressurising) per the plan's acceptance.

### 4.3 Ascent (Crew to the ISS; P from NASA unless marked)

| Event | Demo-2 (2020) [S18] | Crew-2 (2021) [S12] | Crew-10 (2025, RTLS) [S24] | Notes |
|---|---|---|---|---|
| Liftoff | 0:00 | 0:00 | 0:00 | T/W 1.38 (D) |
| Throttle-down for max-Q | ~0:45–1:15 | | | **E**: to ~75–80 % (webcasts; U published). The guide: "Both the first and second stage engines may be throttled to help maintain … steady state acceleration limits" [S1]. |
| Max-Q | **0:58** | 1:02 | ~1:02 | ~30–35 kPa (E, typical for Falcon 9) |
| MECO | **2:33** | 2:36 | ~2:26 | stage 1 burns ~150 s; it leaves 6–10 % of its load for recovery (D: 152 s at full flow empties the stage, §2.1) |
| Stage separation | **2:36** | 2:39 | ~2:30 | MECO + 3 s; pneumatic latches and pushers (§1.3) |
| Second engine start (SES-1) | **2:44** | ~2:47 | ~2:36 | sep + ~8 s; settling first (§3.5) |
| (no fairing on Crew flights) | | | | |
| Stage 1 entry burn | **7:15** (start) | | 6:18–6:29 | 3 engines (E1, E5, E9), ~15–25 s (E); Crew-10's RTLS 11 s |
| SECO-1 | **8:47** | 8:47 | ~8:38 | stage 2 burns ~363 s; MVac throttles down near the end to keep the crew's acceleration (E: at full thrust a ~20 t stage-plus-Dragon would reach 5 g) |
| Stage 1 landing burn | **8:52** (start) | | 7:22–7:39 | 1 engine (E9), sometimes 3 at first (E); Demo-2 ~30 s; the NASA timeline prints "entry burn" twice, the second is the landing burn |
| Stage 1 landing | **9:22** | | 7:39 | droneship, or LZ for RTLS |
| Dragon separation | **12:00** | 11:58 | 9:58 | |
| Nosecone open | **12:46** | | 10:46 | |
| Insertion orbit | ~190 × 205 km, 51.6° | | | Space Launch Report [S6] (Demo-2) |

### 4.4 The booster's return (Crew missions: droneship)

| Quantity | Value | Tag | Source, or derivation |
|---|---|---|---|
| Profile | no boostback on droneship missions; flip engines-first with the nitrogen thrusters, fins deployed, **entry burn** with 3 engines, grid-fin guided descent, **landing burn** with 1 engine (or 3 then 1) | P/E | Demo-2 timeline [S18]; Spaceflight101 [S5] for the sequence |
| Separation state | ~65–80 km altitude, ~2.0–2.4 km/s (Mach 6–8) | E | webcast telemetry (Spaceflight101: "about 80 km … Mach 10" for the heavier GTO profile [S5]) |
| Apogee | ~120–140 km | E | [S5] |
| Entry burn | starts at ~70 km (E [S5]), 3 engines, ~20 s, slowing the stage from ~1,300 to ~250 m/s on GTO profiles ([S5]: "from 1,300 m/s to about 250 m/s" with drag) | E | [S5] |
| Landing burn | single engine (E9) from ~2–3 km, ~25–30 s; touchdown at ~2 m/s ([S5]); legs deploy ~10 s into the burn ([S5]) | E | [S5]; [S18] (8:52 → 9:22) |
| Hoverslam | a single Merlin at its 40 % minimum (338 kN at sea level) exceeds the empty stage's weight (~27–30 t × g₀ ≈ 270–290 kN): **the stage cannot hover** | D | §2.1 throttle and §1.2 dry mass |
| Droneship position, Crew missions | **~510–550 km downrange** along the 45° azimuth | E | FAA 2017: "a drone ship located approximately 500 miles downrange" (generic, all missions) [S4] App.; Crew-mission positions from launch coverage (E). **Pick 520 km** at azimuth 45° from LC-39A. |
| Droneship | ~91 m × 52 m deck ("Marmac 300"-class barge), dynamically positioned to ~3 m, deck water deluge; Octagrabber robot secures the stage | E | Spaceflight101 [S5]; [S7] |
| RTLS alternative | boostback with 3 engines from ~T+2:40, entry and landing at LZ-1/LZ-2 or LZ-40 (Crew-10) | P | [S24] |

### 4.5 Dragon: entry and splashdown (for the later Crew mission; P from NASA [S20] unless marked)

| Quantity | Value | Tag | Source |
|---|---|---|---|
| Trunk jettison | before the deorbit burn | P | [S20] |
| Entry speed | ~7.8 km/s (17,500 mph) | P | [S20] |
| Peak heat-shield temperature | ~2,200 K (3,500 °F) | P | [S20] |
| Communications blackout | ~6 min | P | [S20] |
| Drogue deployment | **~5.5 km (18,000 ft) at ~156 m/s (350 mph)** in a nominal entry | P | [S20]. In the in-flight abort test the drogues opened higher, at ~5.5–6 miles (8.9–9.7 km), the mains at ~1.1 miles (1.8 km) [S4] Table 2-1. |
| Main deployment | **~1.8 km (6,000 ft) at ~53 m/s (119 mph)**; drogues released | P | [S20] |
| Splashdown speed | ~**7 m/s** (15 mph) D; FAA says ~13 mph (5.8 m/s) [S4] App. | D/P | D: four 35.4 m mains at Cd₀ 0.8 (ringsail class, E) give Cd·A ≈ 3,150 m²; a 9,600 kg capsule then falls at √(2mg/(ρ·Cd·A)) = 7.0 m/s at sea level. |
| Parachute model (for `Canopy`) | drogues: 2 × D₀ 5.8 m, Cd·A **29 m²** for both (Cd₀ 0.55 conical ribbon, E), fill constant n ≈ 10 (ribbon class, E: Knacke), deploy_max_q **12 kPa**, max_load **500 kN** for both (D: q 8.5 kPa at deployment × Cd·A × opening factor 2); mains: 4 × D₀ 35.4 m, Cd·A **3,150 m²** for four, n ≈ 7 (ringsail class, E: Knacke), one reefed stage at **15 % for 6 s** (U, propose), deploy_max_q 2.5 kPa, max_load **1.0 MN** for four (D: q 1.4 kPa × reefed Cd·A 470 m² × 1.5) | D/E/U | sizes P [S4] App.; the rest as marked |
| Main canopy material | Kevlar and nylon; risers 147 ft (44.8 m); drogues risers 72 ft (21.9 m) | P | [S4] App. |
| Recovery | ships *Go Searcher*, *Go Navigator* (later *Megan*, *Shannon*); 45–60 min to crew egress | P | [S20] |

## 5. Systems

### 5.1 Avionics and software

| Item | Value | Tag | Source |
|---|---|---|---|
| Falcon 9 architecture | **three-string, single-fault-tolerant**, "designed to human-rating requirements"; fault tolerance by isolated compartments inside boxes or triplicated units | P | [S1] §1.3, §2.6 |
| Falcon 9 contents | flight computers, GPS receivers, inertial measurement units, SpaceX controllers for propulsion, valves, pressurisation, separation and payload interfaces, a network backbone, S-band transmitters (telemetry and video from both stages, even after separation), a C-band transponder for range tracking | P | [S1] §2.6 |
| Where | stage 2 hosts the vehicle's guidance in ascent; stage 1 has its own triple-redundant flight control and GPS/IMU navigation, the only controller in its descent | E | Spaceflight101 [S5] |
| Engine controllers | each Merlin has its own controller with **three processors** cross-checking | E | [S5] |
| Software | Linux, C++; radiation-tolerant commercial parts rather than radiation-hardened | E | [S5] |
| Flight termination | **autonomous** (AFSS), destructors on both stages; for Crew a pyrovalve for stage 2 thrust termination | P | [S1] §2.6, [S4] §2.1.4 |
| Dragon | triple-redundant flight computers (each with cross-checking processors, E), three touchscreen displays plus physical buttons for critical commands (abort, fire suppression, deorbit) (E), fully autonomous docking | E | [S7] |

**For the sim (D106 sensors and computers as components):** per stage 3 flight computers (`Computer { channels: 1 }` × 3 or one with 3 channels), 3 IMUs, 2 GPS receivers, 1 S-band transmitter set, 1 C-band transponder; Dragon 3 flight computers, 2 IMUs, 2 GPS, 2 star trackers (E), a docking sensor suite (lidar and cameras under the nosecone, E from the press kit's "guidance and navigation control sensors" under the nosecone [S12]).

### 5.2 Sensors

| Sensor | Where | Measures | Accuracy, rate | Tag |
|---|---|---|---|---|
| IMU (ring-laser or fibre-optic gyros + accelerometers) | each stage, Dragon | attitude, rates, specific force | gyro bias ~0.01 °/h, accel ~50 µg (navigation-grade class); 100–400 Hz | P (present [S1]) / E (class) |
| GPS receiver | each stage, Dragon | position, velocity | ~5 m, ~0.1 m/s (single-frequency class); 10 Hz; no lock above ~GPS altitudes or in plasma blackout | P (present) / E |
| Star tracker | Dragon (and likely stage 2 for long coasts, U) | attitude | ~5–10 arcsec; 10 Hz | E |
| Radar altimeter | **none known** on Falcon 9 (navigation by GPS/IMU; the landing uses "an onboard predictive simulation" with a wind profile from balloons) | U | FAA [S4] App. |
| Tank pressure transducers | every tank and bottle (CRS-7 and AMOS-6 investigations use them) | pressure | ~0.1 % FS, 100 Hz+ | P (present: "a sense line for a pressure sensor", 2024 [S25]) / E (class) |
| Tank level sensors (point sensors and capacitance probes) | main tanks | propellant level | E | E |
| Thermocouples / RTDs | engines, tanks, avionics, TPS | temperature | ±1 K | E |
| Engine controller readouts | each engine | chamber pressure, turbopump speed, inlet pressures, valve positions | | E |
| Strain gauges | structure (development and early flights) | strain | | E |
| Dragon docking sensors | under the nosecone | range, bearing (lidar, thermal cameras) | | P [S12] (sensors present) / E |
| Dragon cabin sensors | cabin | total pressure, ppO₂, ppCO₂, humidity, temperature, smoke | | E |

### 5.3 Power

| Item | Value | Tag | Source, or derivation |
|---|---|---|---|
| Falcon 9 batteries | **lithium-ion, "multiple redundant" on each stage** | P | [S1] §2.6 |
| Falcon 9 bus | 28 V DC class | E | |
| Capacity, stage 1 | U → propose **2 × 2.5 kWh** | U/D | loads: avionics ~1 kW for ~10 min, the grid-fin hydraulic pump ~20 kW for ~5 min (D: 4 fins × ~10 kN·m hinge moment × 0.5 rad/s), valves and pumps → ~2.5 kWh needed; doubled for redundancy |
| Capacity, stage 2 | U → propose **2 × 2 kWh** | U/D | avionics ~1.5 kW for ~1 h until the deorbit burn |
| Ground power | through the T−0 umbilical until internal power (stage: ~T−3 min, U; Dragon: **T−5 min** P [S18]) | P/U | |
| Dragon batteries | **4 lithium-polymer**; buses **28 V and 120 V DC** | E | [S7] |
| Dragon battery capacity | U → propose **4 × 2.5 kWh** (entry and splashdown on batteries after trunk jettison: ~1.5 kW for ~2 h) | U/D | |
| Dragon solar | cells on the trunk's solar half; **1.5–2 kW** orbit average (E [S7]); peak ~**3.6 kW** with the solar side to the Sun (D: projected area 3.7 m × 3.0 m × 80 % cell coverage × 30 % triple-junction efficiency × 1,361 W/m²) | E/D | §1.6 |

### 5.4 Communications

| Link | Band | Tag | Source |
|---|---|---|---|
| Falcon 9 telemetry and video, both stages | **S-band**, 2,200–2,400 MHz (the guide reserves 2,206–2,216, 2,227.5–2,237.5, 2,242.5–2,260.5, 2,267.5–2,277.5, 2,365.5–2,375.5, 2,377.5–2,387.5 MHz for the vehicle's own emitters) | P | [S1] §2.6, §5.6 |
| Range tracking | **C-band transponder** | P | [S1] |
| Command destruct | none (autonomous FTS) | P | [S1] |
| Dragon | S-band via TDRS and to ground stations (E); IDSS docking-link hardline after capture | E | |

### 5.5 Crew Dragon life support (cut first if the night runs short, by the plan)

| Item | Value | Tag | Source |
|---|---|---|---|
| Cabin | **9.3 m³** pressurised; sea-level air (101.3 kPa, 21 % O₂) | P (volume [S11]) / E (pressure) | |
| Cabin temperature | 18–27 °C (65–80 °F), crew-selectable | P | [S12] |
| Leak tolerance | lands safely with a 6.35 mm equivalent orifice | P | [S7] |
| O₂ and N₂ storage | COPVs in the service section | E | |
| CO₂ removal | lithium hydroxide canisters (E); keep ppCO₂ below NASA's limits | E | |
| Crew consumption | O₂ **0.82 kg** per person-day; CO₂ produced **1.04 kg**; water vapour ~**1.9 kg** (respiration and perspiration) | E | NASA BVAD (Baseline Values and Assumptions Document, NASA/TP-2015-218570): nominal values |
| Suits | SpaceX IVA pressure suits, umbilicals for air, power and data at the seats; protect the crew in a cabin depressurisation | P | [S7] |
| Free flight | up to ~1 week autonomously (U exact); **docked up to 210 days** | P (210 d) | [S7] (NASA) |

## 6. Ratings and failures

### 6.1 The record (Falcon 9 Block 5 to 23 October 2026)

From Wikipedia's maintained statistics [S7]: **642 Block 5 Falcon 9 launches, 1 failure** (Starlink Group 9-3, July 2024: second-stage engine), 0 partial; **652 Block 5 landing attempts, 646 successes, 6 failures** (flights 65, 81, 83, 108, 367, 443); a booster has flown **37** times; one Merlin had flown **22** missions by Feb 2024.

### 6.2 The real failures and their mechanisms (each a failure mode the sim can reproduce)

| Event | Mechanism | Sim mode (D105) | Tag, source |
|---|---|---|---|
| **CRS-1** (Oct 2012, Falcon 9 v1.0, Merlin 1C) | Engine 1 lost pressure at T+79 s near max-Q (a breach of the chamber jacket, attributed to a material flaw); the computers shut it down, its fairing panels blew off; the other eight burned longer and Dragon reached orbit; the secondary payload did not (stage 2 restart forbidden by ISS rules) | **benign engine shutdown** (statistical, turbomachinery/chamber) + engine-out recovery by the vehicle | P (SpaceX statement Oct 2012, [S7]) |
| **CRS-7** (June 2015, v1.1) | A helium COPV in the stage 2 LOX tank broke free when a **cast stainless steel rod end** of its axial strut failed at **~2,000 lbf at 3.2 g** (rated 10,000 lbf, normal peak 3,000–3,500 lbf); the buoyant bottle shot up, struck the LOX tank's forward dome and ruptured the helium plumbing → stage 2 overpressure and breakup at T+139 s. NASA's IRT: a **design error** (an industrial-grade casting in a critical cryogenic load path, without screening) | a **joint snap** (the strut's capacity is a statistical draw on a flawed casting: a "fate" on the strut's ultimate load, D105) → a **liberated COPV** hitting the dome → **burst** of the LOX tank | P [S8]; Musk 2015 [S23] |
| **AMOS-6** (Sept 2016, on the pad during loading for a static fire) | A stage 2 helium COPV failed: **liquid oxygen pooled in buckles between the liner and the overwrap and solidified** (the helium had been loaded colder than LOX's freezing point), ignited by friction or broken fibres → COPV rupture → stage 2 LOX tank breach → fire and explosion of the vehicle and payload | **physical trigger**: bottle temperature below 54.4 K with LOX contact → COPV burst (Block 5's COPV 2.0 and warmer helium loading remove it: debug the condition, not a rate) | P [S13] |
| **Starlink v1.0-L5** (Mar 2020, B5) | **Isopropyl alcohol** (cleaning fluid) trapped in a sensor dead-leg ignited; the engine was shut down at T+2:22; payload delivered; booster landing failed | **benign engine shutdown** (statistical) | P (SpaceX via Spaceflight Now, NASA ASAP) [S7] |
| **Starlink** (Feb 2021, B5, flight 108) | Hot exhaust passed through a **damaged heat-shield cover ("boot")** around an engine; the engine shut down; booster lost | **benign engine shutdown**; history-dependent (a reused part's damaged cover) | P [S7] |
| **GPS III SV04 abort** (Oct 2020) | Two engines started early at T−2 s: a **gas generator relief port blocked by masking lacquer**; abort on the pad; two Crew-1 engines replaced | **ignition failure → pad abort** (safe; the hold-down catches it) | P (Ars Technica, SpaceX/NASA) [S7] |
| **CRS-16** (Dec 2018, B5) | The **grid-fin hydraulic pump stalled** (single string); the stage spun, landed in the sea | **component lost** (pump) → fins frozen | P [S16] |
| **Crew Dragon C204** (20 Apr 2019, static fire at LZ-1) | **NTO leaked through a check valve into the helium pressurisation line** during ground processing; when the abort system pressurised (~100 ms before SuperDraco ignition) the NTO slug was driven at high speed into the **titanium check valve**, which ignited in high-pressure NTO → explosion and loss of the capsule. Fix: **burst discs** replace the check valves; flaps seal the SuperDracos before splashdown | **leak** (statistical, valve) → **physical** (oxidiser in the pressurant line at pressurisation → explosion) | P [S19] |
| **Starlink Group 9-3** (July 2024, B5 stage 2) | A **sense line for a pressure sensor on the stage 2 LOX system cracked from fatigue** (high loading from engine vibration and a loose clamp); the LOX leak over-cooled engine components, including those delivering the igniter fluid; at the second burn the MVac **hard-started**, damaging the engine; the satellites were left in a low orbit | **vibration damage (Miner) → leak** of a line → over-cooling → **hard start** at the next ignition (physical, D105 §6.2) | P (SpaceX update, July 2024) [S25] |

### 6.3 Statistical rates from the vehicle's own record (D105: the part's own data first, at the low end)

Method (D): with k failures in N exposures, the point rate is k/N and the **low end** is the one-sided 10 % Poisson bound χ²₀.₁₀(2k)/(2N) (0.105/N for k = 1, 0.532/N for k = 2); with **k = 0 the low end is zero, so the mode is not statistical** ("if the simulator is not sure, don't trigger the failure"). Exposures: 642 Block 5 boosters × 9 engines = **5,778 engine-ascents** of ~150 s (241 engine-hours); ~656 MVac flights with ~1,300 starts; 652 landings.

| Part, mode | k / N | Point | **Low end (use)** | Hazard for `FailureMode` | Outcome |
|---|---|---|---|---|---|
| Merlin 1D booster, in-flight shutdown (benign) | 2 / 241 engine-hours | 8.3×10⁻³ /h | **2.2×10⁻³ /h** | `PerHour(2.2e-3)` | `BenignShutdown` |
| Merlin 1D, uncontained in flight | 0 / 5,778 | 0 (95 % upper 5.2×10⁻⁴ per flight) | **0** | none | — |
| Merlin 1D, ignition failure on the pad | ≥ 1 known (2020) / ~700 pad starts × 9 | ~1.6×10⁻⁴ per start | **1.7×10⁻⁵ per start** | `PerStart(1.7e-5)` | `IgnitionFailure` (pad abort) |
| MVac, failure at a restart | 1 / ~1,300 starts (and its cause was a vibration-fatigue leak, modelled physically) | 7.7×10⁻⁴ | **8×10⁻⁵ per start** if not modelled physically; **0** if the leak path is modelled | `PerStart(8e-5)` | `Uncontained` |
| Grid-fin hydraulic pump stall | 1 / 652 landings (before the fix) | 1.5×10⁻³ | **1.6×10⁻⁴ per landing** | `PerCycle(1.6e-4)` | `Lost` |
| Landing failure, all causes | 6 / 652 | 9×10⁻³ | not a mode: the sim's physics lands or crashes the stage | — | — |
| COPV, strut rod end (CRS-7 type) | 1 / ~2 vehicles' worth of flawed castings, then fixed | — | **0** after the fix (Block 5) | none | — |

Class-level rates where the vehicle's record is silent (valves, batteries, computers, sensors): the vehicle's record of **zero** mission failures from avionics, batteries or valves in 642 flights gives a low end of **zero** — so **no statistical mode for them on Falcon 9 and Dragon** (D105). For other parts of the catalog the class values to use, low ends: liquid-engine catastrophic failure ~1×10⁻³ per engine-flight historically (Aerospace Corp., Chang 2001, the guide's citation [S1]); propulsion causes about two-thirds of launch failures (same); valves, batteries and electronics from NPRD/MIL-HDBK-217 class data (U here; to be sourced when such parts enter the catalog).

### 6.4 Vibration and shock environments and qualification

| Quantity | Value | Tag | Source |
|---|---|---|---|
| Random vibration at the payload interface (all flight events, P95/50) | 20–100 Hz 0.0044 g²/Hz; 300–700 Hz 0.01; 800–925 Hz 0.03; 2,000 Hz 0.00644; **5.13 g rms** | P | [S1] Table 5-10 (mid band from aeroacoustics, 600–2,000 Hz from **MVac structure-borne vibration**) |
| Acoustic (inside the fairing, 60 % fill) | OASPL **131.4 dB** with blankets, **137.9 dB** without | P | [S1] Tables 5-5, 5-6 |
| Sine | 5–100 Hz, 0.5–1.0 g (axial and lateral, by payload class) | P | [S1] Table 5-4 |
| Quasi-static limit load factors (payload > 1,800 kg) | axial +6.0 / −2.0 g, lateral ±2.0 g (with axial 3.5 g) | P | [S1] Table 5-3 |
| Shock at the separation plane | LV-induced: 30 g at 100 Hz to 300 g at 500–10,000 Hz (standard fairing); separation systems: 1,000 g from 1–10 kHz | P | [S1] Tables 5-8, 5-9 |
| Shock sources | hold-down release, stage separation, fairing and spacecraft separation; all pneumatic on Falcon ("benign shock") | P | [S1] |
| Qualification margins (general) | qualification = MPE **+6 dB**, duration ~3–4 × the flight exposure (min. 2 min/axis); protoflight +3 dB; acceptance at MPE | P | GSFC-STD-7000B (GEVS); SMC-S-016; NASA-HDBK-7005 |
| GEVS component random vibration | qualification **14.1 g rms**, acceptance 10.0 g rms (20–2,000 Hz), 2 min/axis (qual), 1 min/axis (acceptance) | P | GEVS (GSFC-STD-7000B) Table 2.4-3 |
| Pressure vessels (payloads on Falcon) | design: yield 1.5 × MEOP, ultimate 2.0 × MEOP (lines ≥ 1.5 in: 2.5; < 1.5 in: 4.0; valves and regulators 2.5); proof 1.5 × MEOP | P | [S1] Table 6-4 |

**Proposed environments for the sim (§5.9 of the design)** (E/D): engine-mounted components (powerhead) **20 g rms**, qualification 26 g rms (class: engine-mounted equipment); thrust structure and engine bay 10 g rms; tank-mounted and interstage 6 g rms; stage 2 forward skirt and Dragon interface 5.13 g rms (P, the guide's envelope) with qualification **10.3 g rms** (+6 dB) for 3 × the flight exposure. Transmissibility across a bolted joint ~0.5–0.7 (−3 to −6 dB) (E). Miner exponent b ≈ 4 (E, typical for random-vibration fatigue of metallic structure).

### 6.5 Explosions

| Quantity | Value | Tag | Source |
|---|---|---|---|
| LOX/RP-1 yield factor (lb TNT per lb of propellant involved), by impact speed | **0 at rest → 0.215 at ≥ 52 m/s (170 ft/s)**, linear between | P (upper-bound screening model from Project PYRO) | FAA AC 450.137-1 (2024), Table 1 [S26] |
| Hypergols (NTO/MMH) yield factor | 0.020 at rest; 0.085 at 30 m/s; 0.135 at 61 m/s; 0.180 at 91 m/s; 0.220 at 122 m/s; 0.245 at 152 m/s; 0.260 at 183 m/s; 0.275 at 213 m/s; 0.280 at 244 m/s | P | [S26] Table 1 |
| LOX/LH₂ (for later) | 0.16 to 26 m/s, 1.50 at ≥ 227 m/s | P | [S26] |
| Static LOX/RP-1 (pad, siting) | 10 % (test stands) to 20 % (launch pads, up to 500,000 lb) TNT | P | DoD 6055.9-STD, as summarised in [S27] |
| D105's low end for in-flight tank ruptures (no impact) | the at-rest values: LOX/RP-1 **0** blast (a deflagration and fireball, not a detonation), hypergols **2 %** | D | [S26] |
| Gas-bottle burst energy | E = p·V/(γ−1)·[1 − (p_a/p)^((γ−1)/γ)] (isentropic expansion): a stage 2 helium COPV, 0.19 m³ at 37.9 MPa → **9.8 MJ = 2.3 kg TNT**; a 0.43 m³ stage 1 bottle → 22 MJ = 5.3 kg TNT; a 0.2 m³ nitrogen bottle at 34.5 MPa → 14 MJ = 3.3 kg TNT | D | standard (Brode-type) energy of a burst vessel; TNT 4.184 MJ/kg |
| Blast | Kingery–Bulmash overpressure against scaled distance Z = R/W^⅓ | P | design §6.5 |

### 6.6 Crew limits (NASA-STD-3001 Vol. 2 Rev. F, 2025 [S28])

Sustained (> 0.5 s) translational acceleration, **seated**, as (duration s, limit m/s²) points (log-linear between):

| Axis | Non-deconditioned crew (launch) | Deconditioned crew (entry after long flight) | Emergency (abort) |
|---|---|---|---|
| +Ax (chest-to-back, "eyeballs in") | (0.5, 186), (5, 157), (300, 73.5) | (0.5, 137), (10, 98.1), (30, 78.5), (50, 61.8), (90, 49.0), (120, 42.2), (150, 39.2), (10000, 39.2) | (0.5, 373), (120, 86.3), (300, 73.5), (1200, 49.0) |
| −Ax | (0.5, −216), (5, −147), (120, −58.8), (400, −39.2) | (0.5, −132), (10, −78.5), (30, −58.8), (50, −46.1), (90, −39.7), (100, −39.2), (10000, −39.2) | (0.5, −284), (120, −75.5), (300, −60.8), (1200, −42.2) |
| ±Ay | (0.5, 29.4), (1000, 19.6) | (0.5, 19.6), (100, 9.81), (10000, 9.81) | (0.5, 49.0), (1000, 19.6) |
| +Az (head-to-foot... "eyeballs down") | (0.5, 81.4), (5, 62.8), (1200, 39.2) | (0.5, 19.6), (15, 12.3), (30, 9.81), (36, 9.12), (50, 8.53), (80, 7.55), (120, 6.28), (170, 4.90), (1000, 4.90) | (0.5, 167), (120, 58.8), (1200, 37.3) |
| −Az | (0.5, −58.8), (5, −37.3), (60, −21.6), (1200, −9.81) | 0 until the total reaches 9.81 m/s², then −1.96 | (0.5, −63.7), (120, −22.6), (1200, −15.7) |

Transient (≤ 0.5 s, landing and abort): the **Brinkley dynamic response** criterion, β ≤ 1.0 at the "low" risk limits (NASA/TM-2013-217380, required by [S28] §6.5.4). Model per axis: a damped spring, DR = ωₙ²·δ/g; parameters and "low" limits as commonly tabulated from Brinkley (1985): x ωₙ 62.8 rad/s, ζ 0.20, DR limits +35 / −28; y ωₙ 58.0, ζ 0.09, ±14; z ωₙ 52.9, ζ 0.224, +15.2 / −13.4 (E: from secondary tabulations; **verify against NASA/TM-2013-217380** before use). Injury-risk targets: [S28] Table 6.5-9 (e.g. < 0.1 % severe for > 95 % of dynamic cases).

## 7. Materials and fluids

*(in progress)*

---

## Sources

- **[S1]** SpaceX, *Falcon Payload User's Guide*, version 8, May 2025 (falcon-users-guide-2025-05-09.pdf): §2 (vehicle, Table 2-1), §5 (environments: Tables 5-3 to 5-10), §6.4 (pressure vessel factors, Table 6-4).
- **[S2]** SpaceX, *Falcon User's Guide*, September 2021.
- **[S3]** *Espace & Exploration* no. 39 (May 2017), "Fiche technique: Falcon 9", pp. 36–37 (as tabulated in Wikipedia's Falcon 9 Full Thrust and Block 5 articles).
- **[S4]** FAA, *Final Environmental Assessment and FONSI for Issuing SpaceX a Launch License for an In-Flight Dragon Abort Test*, June 2019 (§2.1.2–2.1.7), with its appendix *FAA Biological Evaluation for SpaceX Landing and Recovery Operations*, August 2017 (Dragon parachutes, Dragon 2 mass).
- **[S5]** Spaceflight101, "Falcon 9 FT" vehicle page (updated 2017-01-03 "with data provided by SpaceX and the FAA"), via the Internet Archive.
- **[S6]** Ed Kyle, *Space Launch Report: SpaceX Falcon 9 v1.2 Data Sheet* (2019–2021 versions), via the Internet Archive.
- **[S7]** Wikipedia: *Falcon 9 Block 5*, *Falcon 9 Full Thrust*, *SpaceX Dragon 2*, *SpaceX Merlin*, *SpaceX Draco* (October 2026), used only for statements with their own primary citations, named where used.
- **[S8]** NASA Launch Services Program Independent Review Team, *Public Summary: SpaceX CRS-7 Accident Investigation Report*, March 2018.
- **[S9]** *Light Metal Age*, "How Light Metals Help SpaceX Land Falcon 9 Rockets with Astonishing Accuracy", 26 April 2019 (Constellium Airware 2195-T8; legs: 18 m span, < 2,100 kg).
- **[S10]** NASASpaceflight forum, "Falcon 9 Performance Calculations" thread (topic 40566), quoting SpaceX's Hans Koenigsmann on the first stage's dry weight (about 60,000 lb).
- **[S11]** SpaceX, Dragon vehicle page (height 8.1 m with trunk, diameter 4 m, 9.3 m³ pressurised, 37 m³ trunk; 16 Draco, 8 SuperDraco), as quoted in [S7].
- **[S12]** NASA, *Commercial Crew Program Press Kit* (web), sections Falcon 9, Crew Dragon, Launch Complex 39A, mission timeline (Crew-2 reference).
- **[S13]** SpaceX AMOS-6 investigation statements (23 Sept 2016, 2 Jan 2017), as reported by NASASpaceflight.com and AmericaSpace: "one of the three COPVs inside the second stage LOX tank"; first-stage COPVs in the LOX tank.
- **[S14]** Elon Musk, Twitter, 25 June 2017: "Flying with larger & significantly upgraded hypersonic grid fins. Single piece cast & cut titanium."
- **[S15]** spacelaunchlive.com, "Falcon 9 Grid Fins" (1.5 m × 1.2 m; 70 kg — the mass not taken).
- **[S16]** Elon Musk, Twitter, 5 December 2018 (CRS-16 landing): "Grid fin hydraulic pump stalled… Pump is single string… we will likely add a backup pump & lines."
- **[S17]** FAA / Department of the Air Force, *EIS for Authorizing Changes to the Falcon Launch Program at Vandenberg SFB* (2025): Falcon 9 Block 5 total propellant 1,135,925 lbm; Falcon Heavy 1,898,000 lb LOX and 807,000 lb RP-1 (first stage), 168,000 lb LOX and 64,950 lb RP-1 (second stage). Seen only through a search engine's summary (the document refused automated download): cross-check only.
- **[S18]** NASA, *Mission Timeline for Launch Saturday, May 30* (Demo-2), 2020.
- **[S19]** SpaceX, statement on the 20 April 2019 Crew Dragon static-fire anomaly (15 July 2019), as reported (Wikipedia SpaceX Dragon 2, CBS News, SpaceNews).
- **[S20]** NASA, "Top 10 Things to Know for NASA's SpaceX Demo-2 Return", 23 July 2020 (Dragon with trunk ~27,600 lb at undocking; capsule ~21,200 lb at entry).
- **[S21]** Everyday Astronaut (Tim Dodd's team), mission pages, e.g. "SES-22 | Falcon 9 Block 5" (2022): TEA-TEB from the ground for the pad start; canisters on E1, E5, E9 for relights; Merlin 845/934 kN, 285/313 s (estimates).
- **[S22]** SpaceX, *Falcon 9 Launch Vehicle Payload User's Guide*, Rev 2, 21 Oct 2015: throttle ranges, launch-site services (helium 5,700 psi SLC-40 / 6,000 psi VAFB, GN₂ 5,000 psi), hold-down by hydraulic clamps released at T−0, sample timelines.
- **[S23]** Space Access Society, *Space Access Update #145*, 20 July 2015 (summary of Musk's CRS-7 press call: helium bottles at 5,500 psi in the LOX tank; tanks pressurised to "a few tens of psi"; the strut: steel, ~2 ft long, ~1 in diameter, rated 10,000 lbf, normal peak 3,000–3,500 lbf, failed near 2,000 lbf at 3.2 g).
- **[S24]** NASA, "A Look at Tonight's/Today's Key Launch Milestones" (Commercial Crew blog), Crew-10 (March 2025): entry burn 6:18–6:29, landing burn 7:22–7:39, landing 7:39, Dragon separation 9:58, nosecone 10:46.
- **[S25]** SpaceX, Starlink Group 9-3 update, 25 July 2024, as reported by Space.com and SpaceNews: "a crack in a sense line for a pressure sensor attached to the vehicle's oxygen system … due to fatigue caused by high loading from engine vibration and looseness in the clamp"; the engine hard-started at its second burn.
- **[S26]** FAA, Advisory Circular AC 450.137-1, *De Minimis Far-Field Overpressure Blast Effects Analysis*, 10 Sept 2024: §4.7 and §7.3, Table 1 (liquid-propellant yield factors from Project PYRO by impact speed).
- **[S27]** DoD 6055.9-STD explosive equivalents for liquid propellants (LO2/RP-1 10 % test stands, 20 % launch pads up to 500,000 lb), as summarised by DTIC report AD1104981 *Explosive Equivalence of Hydrocarbon Propellants* and NASA NTRS 20150002957.
- **[S28]** NASA, *NASA-STD-3001 Volume 2, Revision F* (2025), §6.5.1 Tables 6.5-1 to 6.5-3 (sustained acceleration limits, seated), §6.5.3 Table 6.5-9, §6.5.4 (Brinkley criterion, NASA/TM-2013-217380).
