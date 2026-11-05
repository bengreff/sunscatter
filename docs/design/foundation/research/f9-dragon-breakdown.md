# Research: Falcon 9 Block 5 + Crew Dragon, part/component breakdown

Status: complete (research spike, not a design-session decision). Written section by section; see §4 for the points this doc leaves for session 1 to settle.

Canon used (read first, not reopened): `docs/plans/scope-2026-11.md` §3.1 (owner, 2026-11-03). A **part** is the smallest placeable physical object: one shape, one rigid body, one exterior ("some parts are tiny, a radial RCS thruster; some are huge, a big tank"). A **component** is a parameter inside a part's shape: mass plus internal logic, never placed as its own object (example given: "a tank's components are its pressurant bottle, feed line, valves and sensors"). Groups (a stage, "Dragon") carry no physics. Graphs (power, fluid, data, thermal, load) connect components inside and across parts, crossing part boundaries only through ports on mount faces.

Source for every number below: `docs/vehicles/falcon9.md` (DATA sheet), cited by section (§). That sheet's own tags carry through: **P** published, **E** estimate, **D** derived, **U** unknown/proposed. I do not re-derive anything it already derived; where I add a tag of my own (a reclassification judgement, not a number), it is marked **(research, this doc)**.

Read-only inputs: `data/parts/{falcon9,dragon,engines,equipment}/*.ron` (today's catalog, 51 files) and `data/craft/falcon9-crew/craft.ron` (today's one assembled craft, which already splits a `parts:` list, mounted at ports, from a `components:` list, placed `in:` a part at a local position — this is closer to the owner's split than the catalog's flat file layout suggests; see §4).

---

## 1. Part list

Each row is one placeable object under the owner's definition: one shape, one rigid body, one exterior. "New?" marks a part this doc proposes that is not yet a separate catalog/craft entry today (it exists today only as a lumped mass or an embedded component — see §3). Frame and z-values are falcon9.md §1.1's (z up from the first stage engines' nozzle exits).

### 1.1 Stage 1 (booster)

| Part | Shape | Dims | Material | Mass | Tag/source |
|---|---|---|---|---|---|
| Octaweb + aft skirt/engine-bay shell | Thrust structure + cylindrical shell, ring of 8 engine mounts + 1 centre | r 1.83 m, z 1.0–4.30 (3.3 m); engine ring r 1.29 m | `al-7075` machined, bolted | ~1,500 kg (shell folded in) | E, falcon9.md §1.2 (one welded/bolted assembly: shell listed "in octaweb") |
| Engine heat shield (base closeout) | Disc, 9 holes | Ø 3.66 m | U → `inconel-718` sheet 1 mm over insulation | ~500 kg | U/E, §1.2. **New** (research, this doc): today a `mass-item` inside the octaweb (§3, §4.1); proposed a part of its own because it ablates/heats independently of the structural shell |
| Stage 1 tank (RP-1 + LOX, common dome) | Two cylindrical barrels + 3 domes (√2:1 ellipsoid), one shell | r 1.83 m, z 4.30–39.77 (35.47 m); RP-1 149.7 m³, LOX 234.0 m³ | `al-li-2195` | RP-1 tank ~4,000 kg + LOX tank ~4,800 kg derived, 22,500 kg across the stage's listed parts, 3,100 kg unmodelled gap | D/E, §1.2. One part: monocoque LOX + skin-stringer RP-1 share the common bulkhead as one welded shell |
| Interstage | Cylindrical shell, sandwich wall | r 1.83 m, z 39.77–46.40 (6.63 m) | CFRP face/Al-honeycomb core | ~900 kg | D/E, §1.3 |
| Grid fin ×4 | Lattice fin, hinged + deploying | span 1.5 m × width 1.2 m, depth 0.2 m | `ti-6al-4v`, single-piece cast | ~150–250 kg each | D/E, §1.2, pattern Radial(4) at 45°/135°/225°/315° |
| Landing leg ×4 | Telescoping strut + foot, hinged + deploying | length 8.2 m, span 18 m deployed | CFRP + Al-honeycomb | 2,000 kg for four (P) | P/D/E, §1.2 |
| Merlin 1D ×9 | Engine (chamber/nozzle geometry + its own components) | exit Ø 0.93 m, length ~2.4 m | copper-alloy chamber, regen nozzle | 470 kg each | E, §2.1. Already one part each — matches the owner's "each Rutherford one part" example exactly |
| Nitrogen cold-gas pod ×2 | Thruster housing, 4 nozzles | Box ~0.3×0.3×0.4 m | aluminium housing | ~100 kg each | U/E, §1.2, §2.4. **Reclassified** (research, this doc) from today's Component to a part — see §4.2 |

Stage 1 total listed parts' mass ≈ 22,500 kg; published dry mass 25,600 kg (§1.2); the 3,100 kg gap is stage 1's unmodelled mass (brackets, ring frames, fasteners, harness, B5 coatings) — D101, not spread over parts.

### 1.2 Stage 2

| Part | Shape | Dims | Material | Mass | Tag/source |
|---|---|---|---|---|---|
| Stage 2 tank (RP-1 + LOX, common dome) | Two barrels + domes, one shell | r 1.83 m, z 46.70–57.01 | `al-li-2195` | derived, ~4,000 kg dry across stage 2's listed parts | D, §1.4 |
| Aft skirt + MVac thrust cone | Cylindrical shell | r 1.83 m, z 46.40–47.99 (1.59 m) | `al-li-2195` | ~250 kg | E, §1.4 |
| MVac (Merlin Vacuum D) ×1 | Engine, fixed 165:1 nozzle with radiatively-cooled niobium extension as one shape | exit Ø 3.0 m, overall ~5.2 m below the separation plane | copper-alloy + niobium-C103 extension | 650 kg | D, §2.2. Extension is a parameter of this one part (same rigid body continuing to a longer bell), not a second part — see §4.3 |
| Dragon interface ring | Short cylindrical shell | r 1.83 m, length 0.29 m | aluminium | ~100 kg | E, §1.4 |
| Nitrogen cold-gas pod ×2 | Thruster housing | — | aluminium | ~30 kg each | U/E, §1.4, §2.4. **Reclassified** (research, this doc), see §4.2 |

### 1.3 Crew Dragon capsule

| Part | Shape | Dims | Material | Mass | Tag/source |
|---|---|---|---|---|---|
| Pressure vessel + backshell | Revolution (profile §1.5) | z 0.6–3.4 m (frame), 9.3 m³ pressurised | welded `al-2219` | derived | P/E, §1.5 |
| Heat shield | Revolution, spherical cap | Ø 3.96 m, cap R 4.7 m, 60 mm PICA-X | PICA-X on CFRP carrier | ~800 kg (derived: 204 kg TPS + carrier) | D/E, §1.5 |
| Nosecone (hinged) | Revolution, dome | Ø 1.88 m base | `cfrp` sandwich | ~100 kg | E, §1.5 |
| Trunk | Cylindrical/conical shell | Ø 3.66→3.9 m, 3.7 m long, 37 m³ unpressurised | CFRP sandwich | 2,600 kg | D, §1.6 |
| Trunk fin ×4 | Fin | small, base-mounted | CFRP sandwich | in trunk's 2,600 kg | P (count), E (size), §1.6 |
| SuperDraco pod ×4 | Printed nacelle housing 2 engines as one casting | Box ~0.4×0.6×0.9 m | Inconel 718 printed | ~100 kg each incl. engines | P/E, §1.5, §2.3. **Reclassified** (research, this doc) from Component to a part: falcon9.md itself calls each pod "a printed nacelle" — one casting, one rigid body, one exterior — see §4.2 |
| Draco cluster (lower sidewall) ×4 | Thruster bracket/housing, 3 nozzles | Box ~0.3×0.3×0.3 m | — | ~2 kg × 3 each | E, §1.5, §2.3. **Reclassified**, see §4.2 |
| Draco forward (under nosecone) ×1 | Thruster bracket/housing, 4 nozzles | Box ~0.9×0.9×0.2 m | — | ~2 kg × 4 | P (placement), §1.5, §2.3. **Reclassified**, see §4.2 |

Capsule check: published dry 7,700 kg (P, §1.7) against the listed parts plus components in §2 below; the sheet does not spread a gap over named items (D101).

**Part count, this doc's recast:** 23 in stage 1 (1 octaweb, 1 heat shield, 1 tank, 1 interstage, 4 grid fins, 4 legs, 9 Merlins, 2 cold-gas pods) + 6 in stage 2 (tank, skirt, MVac, ring, 2 cold-gas pods) + 17 in Dragon (capsule, heat shield, nosecone, trunk, 4 fins, 4 SuperDraco pods, 4 Draco clusters, 1 forward cluster) = **46 parts**, plus the ground launch mount (not a vehicle part; §4.6). Today's `craft.ron` instantiates 32 vehicle-parts + 1 ground mount, because it still carries the reclassified thrusters as components (§4.2) and the heat shield as a mass-item (§4.1); the difference (46 − 32 = 14) is exactly those reclassifications.

---

## 2. Components, by part

Graph membership: **F** fluid, **P** power, **D** data, **T** thermal, **L** structural load. A component's parameters are what the owner's "types + parameters" model scales it by (§3.1).

### 2.1 Octaweb + heat shield (stage 1)

| Component | Parameters | Graphs | Tag/source |
|---|---|---|---|
| Feed manifolds, prevalves, 9×2 engine lines | 9 × Ø 0.10 m per propellant, ΔP 0.05 MPa/line (U) | F | E, §1.2, §3.4 |
| Flight computers ×3 channels, 3 IMUs, 2 GPS | triplex, three-string fault tolerant | D, P | P/E, §5.1 |
| Batteries ×2 | lithium-ion, 28 V, ~2.5 kWh each (U) | P | U/D, §5.3 |
| Flight termination (linear charges in the tank walls + safe-and-arm) | — | D | P, §5.1 |
| Hold-down interface | 4 clamp points, z 1.0, r 1.83, at 45°+90°k | L | E, §1.2 |

### 2.2 Stage 1 tank

| Component | Parameters | Graphs | Tag/source |
|---|---|---|---|
| LOX downcomer (double-wall transfer tube) | OD ~0.45 m, through the RP-1 tank | F, T | P/E, §1.2, §3.4 |
| Helium COPVs | **U count** (proposed 6 × 0.43 m³), 37.9 MPa, cold (66.5 K) | F, L, T | D, §1.2, §3.3 |
| Raceway (cable/pipe run along the tank) | ~0.25 m wide, ~250 kg | D, F, P | E, §1.2. Flagged in §4.5: this duplicates what the owner's auto-router (§3.1 "routing") should compute, not a fixed mass |
| Level sensors, pressure transducers ×2 each (LOX, RP-1), thermocouple, COPV pressure transducer | class accuracies, §5.2 table | D | E, §5.2 |
| LOX/RP-1 prevalves ×2 | Ø 0.3 m, isolation | F, D | E (equipment/prevalve.ron) |

### 2.3 Grid fins, legs, Merlins: self-contained

Grid fins and legs carry their own actuator (`Hydraulic` / `Pneumatic`) and deployment rate as part parameters, not separate components (§1.1's `generator: GridFin`/`Leg` already encode this). Each Merlin's components (D101/§3.1's "pumps, valves, lines, gimbals, igniters" example almost verbatim): turbopump (shared shaft, LOX+fuel pumps+turbine), gas generator, pintle injector, TVC actuator (hydraulic, powered by the engine's own RP-1 pump — no separate hydraulic component), TEA-TEB igniter canister (only E1/E5/E9 carry one). Graphs: F (propellant in, hot gas internally), L (gimbal load into the octaweb), D (engine controller, 3 cross-checking processors), T (regen cooling, no `nozzle_heat_fraction`). Source: §2.1, all P for the cycle description.

### 2.4 Interstage / stage 2 skirt region

| Component | Parameters | Graphs | Tag/source |
|---|---|---|---|
| Nitrogen bottles (stage 1, in the interstage) | **U**, proposed 2 × 0.2 m³ at 34.5 MPa | F | U, §1.2, §3.3 |
| Nitrogen bottle (stage 2, in the aft skirt) | **U**, proposed 1 × 0.2 m³ | F | U, §1.4 |
| TEA-TEB igniter fluid tank (stage 2) | **U**, proposed 0.01 m³ | F | U, §2.2 |
| LOX transfer tube (stage 2) | OD 0.35 m, annulus | F, T | P (annulus)/E (size), §1.4 |
| Helium COPVs ×3 (stage 2, in the LOX tank) | each ~0.19 m³, 37.9 MPa | F, L, T | P (count)/D (volume), §1.4, §3.3 |
| Avionics, batteries, FTS pyrovalve (stage 2) | triplex computer, 2 battery packs ~2 kWh (U) | D, P | E, §5.1, §5.3 |

### 2.5 Dragon capsule

| Component | Parameters | Graphs | Tag/source |
|---|---|---|---|
| Docking adapter (IDSS) | Ø 1.2 m mating ring, ~300 kg | D, L (capture load) | P standard/E mass, §1.5. Boundary case: the adapter is a component, but once docked it is also the **port** the owner's graphs cross through (§4.4) |
| Propellant tanks (NTO ×2, MMH ×2) | spheres, 0.56–0.58 m³ each, carbon-overwrapped titanium, 16.5 MPa regulated feed / 19.0 MPa MEOP (U) | F, L | P (load, 2,563 kg)/D (sizing), §1.5, §3.2 |
| Helium pressurant bottles ×2(+) | **U**, 0.1 m³ at ~31 MPa proposed; §3.2's RESEARCH row says this undersizes an abort burn — needs ~4 × 0.5 m³ at 41.4 MPa | F | U/D, §1.5, §3.2 (two numbers in tension, flagged there already) |
| Seats ×4 | carbon fibre/Alcantara, 25 kg each, rotate for launch | L | E, §1.5 |
| ECLSS | cabin 9.3 m³, O₂/N₂ COPVs, LiOH scrubber (η 0.9, Q 15 L/s, 60 kg sorbent) | F, T, D | E/D, §5.5 |
| Batteries ×4 | lithium-polymer, 28 V/120 V, ~2.5 kWh each (U) | P | E, §5.3 |
| Flight computers (triplex), 2 IMU, 2 GPS, 2 star trackers | — | D | E, §5.1 |
| Windows ×4 | fused silica, ~15 kg each | L | P (count)/E (mass), §1.5. Gap: no dedicated window/viewport component type exists in the catalog today (§3) |
| Side hatch | ~1 m, crew-access side | L | folded into the pressure vessel, falcon9.md's own table does this (§1.5) |
| Parachutes: 2 drogues + 4 mains | Ø 5.8 m / 35.4 m, nylon/Kevlar, ~400 kg | L, D | P (sizes)/E (mass), §1.5, §4.5 |

### 2.6 Dragon trunk

| Component | Parameters | Graphs | Tag/source |
|---|---|---|---|
| Solar array (body-mounted cells, half the circumference) | 8.9 m² effective, 30 % efficiency, 1.5–3.6 kW | P | P (placement)/D (output), §1.6, §5.3 |
| Radiator (the other half) | **not in the catalog at all** — a gap, see §3, §5 | T | P (existence only), §1.6 |

---

## 3. Mapping table: today's catalog → new part/component

"OK" means the file already lands where this doc's recast puts it. "Move" means the owner's definitions put it somewhere else.

| File | Today's generator | New home | Note |
|---|---|---|---|
| `falcon9/f9-octaweb.ron` | ThrustStructure | Part | OK |
| `falcon9/f9-s1-tank.ron` | Tank | Part | OK — one part despite two compartments; §4.5's "big part" question |
| `falcon9/f9-interstage.ron` | Shell | Part | OK |
| `falcon9/f9-grid-fin.ron` | GridFin | Part | OK |
| `falcon9/f9-landing-leg.ron` | Leg | Part | OK |
| `falcon9/f9-dragon-ring.ron` | Shell | Part | OK |
| `falcon9/f9-s2-aft-skirt.ron` | ThrustStructure | Part | OK |
| `falcon9/f9-s2-tank.ron` | Tank | Part | OK |
| `falcon9/f9-cold-gas-pod.ron` | **Component** | **Part** | Move — §4.2 |
| `falcon9/f9-s2-cold-gas.ron` | **Component** | **Part** | Move — §4.2 |
| `falcon9/lc39a-hold-down.ron` | Pad | Ground object, not a vehicle part | §4.6 |
| `dragon/crew-dragon-capsule.ron` | Revolution | Part | OK |
| `dragon/crew-dragon-heat-shield.ron` | Revolution | Part | OK |
| `dragon/crew-dragon-nosecone.ron` | Revolution | Part | OK |
| `dragon/crew-dragon-trunk.ron` | Shell | Part | OK |
| `dragon/crew-dragon-trunk-fin.ron` | Fin | Part | OK |
| `engines/merlin-1d.ron`, `merlin-1d-vac.ron` | Engine | Part | OK — matches "each Rutherford one part" exactly |
| `engines/superdraco-pod.ron` | **Component** | **Part** | Move — §4.2, falcon9.md's own words ("a printed nacelle") say one rigid body |
| `engines/draco-cluster.ron` | **Component** | **Part** | Move — §4.2 |
| `engines/draco-forward.ron` | **Component** | **Part** | Move — §4.2 |
| `dragon/dragon-helium-copv.ron`, `dragon-tank-mmh.ron`, `dragon-tank-nto.ron` | Component | Component | OK — matches the owner's own tank-component example verbatim |
| `equipment/f9-helium-copv.ron`, `f9-s2-helium-copv.ron`, `f9-nitrogen-copv.ron`, `f9-s2-nitrogen-copv.ron` | Component | Component | OK |
| `equipment/f9-lox-downcomer.ron`, `f9-s2-transfer-tube.ron` | Component | Component | OK |
| `equipment/prevalve.ron` | Component | Component | OK |
| `dragon/dragon-battery.ron`, `equipment/f9-battery.ron` | Component | Component | OK |
| `dragon/dragon-flight-computer.ron`, `equipment/f9-flight-computer.ron` | Component | Component | OK |
| `equipment/imu.ron`, `gps-receiver.ron`, `star-tracker.ron`, `thermocouple.ron`, `pressure-transducer.ron`, `high-pressure-transducer.ron`, `copv-pressure-transducer.ron`, `level-sensor.ron`, `barometric-altimeter.ron`, `cabin-air-monitor.ron` | Component (Sensor) | Component | OK, but generic: none names a real part/model number, all are class values (§5.2's "E"). Fine as placeholders |
| `dragon/dragon-eclss.ron` | Component | Component | OK — but is really several components (O₂/N₂ bottles, the LiOH bed, the fan) folded into one catalog entry; the new model's per-type parameters (§3.1) should probably split these so a bottle failure and a scrubber failure are different events |
| `dragon/dragon-seat.ron` | Component | Component | OK |
| `dragon/dragon-docking-adapter.ron` | Component | Component (+ the physical half of a port) | §4.4 |
| `dragon/dragon-parachutes.ron` | Component | Component | OK |
| `dragon/dragon-trunk-solar.ron` | Component | Component | OK — body-mounted cells, no separate shape (§4.4) |
| `equipment/mass-item.ron` | Component (`Other`) | Placeholder, not a real type | Used 9 times in `craft.ron` for the engine heat shield, raceway, avionics boxes, windows, separation hardware, crew+cargo — each is really a different real part or component hiding behind one generic "estimated mass" type (§4.1, §4.5) |

**Not in the catalog at all** (gap, not a mismatch): the trunk's radiator (half its circumference, §1.6); a named window/viewport component (§2.5); per-part load-path/margin data for anything but the four main tanks (§3.2's axial loads only cover the barrels); the stage 1 raceway's actual routed contents (today a fixed mass, §2.2).

---

## 4. Boundary cases specific to these two vehicles

### 4.1 The octaweb and its heat shield

The octaweb is the clearest "one part" case in the whole stack: nine engines in their own metal bays, bolted to one machined ring, one rigid body, one exterior (§1.2, P). Today's catalog already treats it this way (`f9-octaweb.ron`, generator `ThrustStructure`), with the engine heat shield and the aft-skirt shell folded into its mass budget because falcon9.md's own table marks the shell "in octaweb" (§1.2). The heat shield is different in kind, not just mass: it is the one surface that sees direct plume recirculation and heats independently of the structural ring underneath it (its material is U, proposed ablative/insulating, not the ring's `al-7075`). Under §3.1's failure model ("heating: cells ablate or melt one by one… the part fails once a ring of cells can no longer carry its load"), a thermal failure of the heat shield should not take the whole octaweb's load path with it. This doc proposes splitting it out as its own small part (§1.1); the session should confirm whether the bolted base closeout is a separate rigid body in reality (photos suggest yes — it is removed and refurbished separately between flights) or stays folded in for simplicity's sake.

### 4.2 Thruster pods: Component today, Part by the letter of the canon

This is the biggest reclassification in this doc. The owner's own example of a tiny part is "a radial RCS thruster" (§3.1). Five catalog entries model exactly that kind of object as a `Component` instead: `f9-cold-gas-pod`, `f9-s2-cold-gas` (nitrogen RCS), `superdraco-pod`, `draco-cluster`, `draco-forward`. Falcon9.md's own language supports treating them as parts, not parameters: the cold-gas thrusters are "two pods" with their own aluminium housing (§2.4, P); the SuperDraco pods are explicitly "each pod in a printed nacelle" (§1.5, §2.3, P) — a single 3D-printed casting is as clean a "one shape, one rigid body, one exterior" as the Merlin is. The Draco clusters are grouped "in 4 clusters of 3" from photographic evidence (§1.5, E), implying a shared mounting bracket per cluster, i.e. one rigid body each.

Today's catalog instead nests all five inside the parent part (the capsule, the interstage, the aft skirt) as a `Component` with its own `envelope` box and a local `pos:` — the same pattern used for a battery or a sensor, i.e. "a parameter inside the parent's shape," contrary to how a part this size, with its own mount, canted nozzles and its own gimbal-free rigid attachment, behaves physically. Reclassifying them as parts also matters for the sim: a part fails as a whole rigid body and can be inspected/isolated in the part view (§4 of scope-2026-11.md); a component cannot be separately selected the same way. Recommend: move all five to parts, each mounted via a port on its parent's exterior (the capsule sidewall/nosecone, the interstage, the aft skirt), with the individual engines (SuperDraco ×2, Draco ×3 or ×4) as the part's own internal nozzle/chamber parameters — mirroring exactly how the MVac's nozzle extension is already one part's parameter (§4.3).

### 4.3 The Merlin/MVac nozzle extension: one part, not two

The MVac is the Merlin 1D's chamber with a longer, two-section nozzle: regeneratively cooled to the booster engine's expansion ratio (ε 16), then a radiatively cooled niobium-alloy extension out to ε 165 (§2.2, P for the niobium, E for the split fraction). Both sections are the same rigid body (one continuous bell, no joint, no separate mount), so this is correctly one part today (`merlin-1d-vac.ron`, one `Engine` generator with an `extension:` field) — not a second "nozzle extension" part. This is the right precedent for any future engine with a deploying or two-material nozzle (electric-pump, dual-expander, etc.): split by material/cooling as a parameter, not by part, as long as nothing hinges or separates.

### 4.4 Draco clusters, docking adapter, and what a port is for

Two related points. First, the Draco "cluster of 3" (§1.5, E — grouped from photos, not confirmed hardware) is exactly the kind of judgement call §3.1 leaves to the generator: if the real hardware turns out to be one bracket per 3 thrusters, it is one part per cluster (this doc's pick); if each Draco individually bolts to the sidewall, it is 12 tiny parts plus the forward 4, for 16 total RCS parts on the capsule alone, each with its own 400 N thruster as the entire part (no separate "component" engine inside it at all, since there is nothing else in the shape). Either reading satisfies the owner's definitions; which one matches the real hardware is a sourcing gap (§5).

Second, the IDSS docking adapter (`dragon-docking-adapter.ron`) sits at the boundary between a component and a port. It is physically a mechanism with mass and moving latches — a component, correctly. But once two craft dock, the graphs that then connect across the join (data, power, maybe fluid for a future tanker) cross through exactly where this component sits, on the capsule's "top" mount face (§3.1: "graphs cross part boundaries only through ports on mount faces… joining parts joins their ports"). So the docking adapter is a component that also hosts a port, and the two concepts (the physical ring with its capture mechanics, and the zero-mass graph junction two craft's systems join through once mated) should probably be modelled separately even though they coincide physically on Dragon. Nothing in today's catalog represents the port half of this at all; it is out of scope for a single-craft catalog entry and belongs with the docking/joint design instead.

### 4.5 The common-dome tank as one very big part

Falcon 9's first-stage tank is the largest single part in the whole craft by a wide margin: one welded shell carrying 287,400 kg of LOX and 123,500 kg of RP-1 (§1.2) over a 35.47 m barrel. It is unambiguously one part under the owner's definition (one shape, one rigid body, two compartments sharing a common bulkhead — §1.2, P that the bulkhead exists), and it is already modelled that way (`f9-s1-tank.ron`, one `Tank` generator with a `split` parameter). It is also the test case for scope-2026-11.md §3.1's open question 6 ("the maximum size per part type before a part must be split or gain cell-level failure"): at 35+ m with two different wall constructions (monocoque LOX, skin-stringer RP-1) and CRS-7's history (a COPV breaking the LOX tank's forward dome, §6.2) showing exactly the "pressurised vessel fails whole" mode §3.1 describes, this tank is a direct, sourced example for that design-session question rather than a hypothetical one.

### 4.6 The launch mount: a "part" that is not a vehicle part

`lc39a-hold-down.ron` uses the same `Generator` enum and the same catalog mechanics as every vehicle part (a `Pad` shape, a material, a mass), and `craft.ron` lists it in the vehicle's own `parts:` array with a `HoldDown` joint, because that is the simplest way to model "stays welded to the ground until the clamps release" (§1.2, P). It is not a vehicle part under the owner's definition — it never flies, and it is shared across every craft that launches from LC-39A, not owned by this one. This doc counts it separately from the 46 (§1) for that reason; the craft/ground split deserves its own line in session 1, since right now the catalog does not distinguish "a part belonging to this craft" from "ground equipment modelled with the same machinery for convenience."

---

## 5. Validation data available, and the gaps

### 5.1 What falcon9.md already gives, usable as-is

- **Mass closure.** Liftoff mass check: stage sums give 548,300 kg against SpaceX's published 549,054 kg fairing-config total, 0.14 % low (§1.7, P/D). Propellant load check: 516,600 kg against the FAA's 515,250 kg, 0.26 % high (§1.7). Both are real external checks on the part/component split's mass, not just self-consistency.
- **Flight timelines, four real missions.** Demo-2 (2020), Crew-2 (2021), Crew-10 (2025, RTLS) and Crew-13 (2026, RTLS) give MECO, separation, SES-1, entry burn, landing burn and landing times to the second (§4.3, §4.4, P/E from NASA and SpaceX timelines). These are ready-made warp-invariance and mission-sequencing test vectors: the same craft, flown at 1x, should reproduce these event times from the modelled thrust/Isp/mass, independent of the part/component split chosen.
- **Entry and splashdown.** Drogue/main deploy altitudes and speeds, splashdown speed on 4 and on 3 mains, entry g-limits and the Brinkley transient criterion (§4.5, §6.6, mostly P). Good acceptance tests for the parachute and ECLSS components once built.
- **Failure history with named mechanisms.** Seven real failures (§6.2) each already mapped to a `FailureMode`/`Outcome` pair and, where the record supports it, a statistical rate with its low-end bound (§6.3). This is unusually strong validation data for a game: CRS-7 and AMOS-6 in particular validate the COPV-as-component model directly (a component failure propagating to a part-level burst).
- **Vibration/acoustic/shock envelopes** from the Payload User's Guide (§6.4, P) — these are qualification envelopes, not flight-measured data, but they are a real, sourced ceiling to check any derived structural/vibration model against.

### 5.2 Gaps the existing research does not close

1. **No independent moment-of-inertia or centre-of-gravity data.** Every mass number is scalar; nothing in falcon9.md gives a measured or published CG/MOI trajectory through the burn, which is exactly what the part/component split needs to validate (each part's placement should reproduce the real CG travel). This has to come from the generated geometry, with no outside check.
2. **No flight-measured vibration or acoustic spectrum**, only the qualification envelope (§6.4) — the actual spectrum a flown Falcon 9 part experiences is not public, so the ratings in the catalog (e.g. `vibration_grms` on `merlin-1d.ron`) are qualification-level proxies, not flight-validated.
3. **Draco cluster hardware is not confirmed.** The "4 clusters of 3" grouping (§1.5) comes from photo interpretation, not a SpaceX document; whether it is one bracket or three independent mounts changes the part count by up to 12 (§4.4). The sourced ICES-2020-333 ECLSS paper (§5.5's RESEARCH row) could not be read in full (repository refused the download, §source S35) — it likely has the doghouse/bracket detail and is worth a renewed fetch attempt.
4. **No public thrust/chamber-pressure transient**, only steady-state values for every engine (Merlin, MVac, SuperDraco, Draco) — start/shutdown transients (§2.1, §2.2, §2.3) are all U or class estimates, so no real transient exists yet to validate a warp-invariant startup model against.
5. **The trunk's radiator has no numbers at all** — not area, not rejected power, not even a rough estimate — only that it exists on "the other half" of the trunk (§1.6, P). Thermal validation of Dragon in orbit cannot proceed past "there is a radiator" until this is estimated.
6. **Helium budget for the abort burn is internally unresolved.** §3.2 already flags this in falcon9.md itself: the proposed 2 × 0.1 m³ bottles (§1.5) cannot supply a 5 s abort burn at the published 16.5 MPa feed pressure; a RESEARCH row in the same section proposes 4 × 0.5 m³ instead, but the craft file (`dragon-helium-copv.ron`) still uses the smaller, inconsistent figure. This is a correctness bug waiting in the data, not just a gap.
7. **No published GTO/droneship Crew mission for cross-checking** — all four cited missions are ISS profiles; the droneship RTLS alternative (§4.4) mixes a GTO-profile entry-burn figure ("1,300 m/s to 250 m/s") with Crew mission timelines, because no Crew droneship mission timeline this detailed is public yet.

---

## Sources

Every number in this doc traces to `docs/vehicles/falcon9.md` by section (§), whose own 36 primary sources are listed there (its "Sources" section, [S1]–[S36]). This doc adds no new primary sources; its only additions are the part/component classification calls themselves, marked **(research, this doc)** where they reclassify something against today's catalog.
