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

**Hold-down points.** The stage stands on the pad on **four hold-down clamps** at the base of the engine section (E from photos of the LC-39A transporter-erector; four is also the count of leg and load hard points). Mount names `hold_#` at z 1.0 m, r 1.83 m, at 45°+90°k (aligned with the legs) E.

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

*(in progress)*

## 3. Fluids and tanks

*(in progress)*

## 4. The flight

*(in progress)*

## 5. Systems

*(in progress)*

## 6. Ratings and failures

*(in progress)*

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
