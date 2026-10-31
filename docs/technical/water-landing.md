# Water landing on a calm sea: slamming, added mass and buoyancy

**Status: design** (RESEARCH, 2026-10-30, at 835f839). Nothing here is built. Decisions: D123 ("a calm, flat sea first. Each contact point gets slamming pressure, added mass and buoyancy (E until checked against water-impact data). Waves come later."), D066 (rigid-body contact), D125 (one mechanism for every craft), D105 (failures from the simulation), D116 (crew doses), rule 4. The system: [motion](../systems/motion.md); the contact it extends: `sim::contact` (D066). The descent before it: [entry-ablation.md](entry-ablation.md) §3.

**The pick.** The sea is a flat surface at the body's sea level, wherever the terrain lies below it. A craft touching it gets three forces from its own surface cells:
1. **hydrostatic pressure** on every submerged cell (buoyancy and the righting moment, from the geometry);
2. **the slam** by von Kármán's momentum method on the instantaneous wetted footprint: the force is the rate of change of the added mass's momentum along the water's normal, the added mass treated implicitly;
3. **drag** of the wetted cells in water for the motion along the surface and the settling.

Checked against Apollo's water-impact data, this gives 20.4 g where the rigid theory gives ~22 g, and a footprint pressure of 490 kPa where 414 kPa was measured.

## 1. What is there to fit

**Apollo command module**, Stubbs, NASA TN D-3980 (1967): a ¼-scale dynamic model with a scaled-stiffness heat shield, all values full scale. Mass 3,900 kg, diameter 3.85 m, spherical heat shield of radius ~4.69 m (E: Apollo's aft shield).

| At ~9.1 m/s (30 ft/s) vertical | Value |
|---|---|
| Peak normal acceleration | from ~4 g at −39° pitch (the shoulder first) to **38 g at −12°**, with "pronounced oscillations due to heat-shield vibration" |
| Rigid-vehicle theory (the report's ref. 1) | **~22 g** at 0° |
| Peak mean pressure on a 0.2 m² panel | **1,475 kPa** (214 psi) at −18° |
| Mean pressure at peak acceleration | **414 kPa** (60 psi) over ~1.9 m² |
| Longitudinal, angular | 7.5 g; 180 rad/s² |
| Horizontal velocity 0–15 m/s | little effect on peak normal acceleration |
| Flotation | stable upright and near inverted; 0.6 m × 11 m waves did not upset either |

**Apollo's ELS** (TN D-7437): descent at 32 ft/s (9.8 m/s) on three mains, 36 ft/s on two.

**Orion** (SPLASH and the Ground Test Article drops at NASA Langley's Hydro Impact Basin, e.g. the nominal 25 ft/s vertical drops): instrumented for pressures and accelerations, used to check LS-DYNA. LS-DYNA "provided reasonable predictions for peak accelerations" but did not always follow the whole impact (NASA/CR-2018-219801 and its companions). The peak values were not pulled here (open point 3).

**Crew Dragon:** splashdown ~7 m/s (D, falcon9.md §4.5; FAA 5.8 m/s), ~9.6 t. No published impact loads (U).

## 2. The model

### 2.1 The sea

- The surface is the body's reference surface at sea level (height 0), flat and still, where the solid surface lies below it. On Earth that is the ocean. Lakes and rivers come with terrain detail later.
- Seawater density **1,025 kg/m³** (E), its temperature for the thermal contact later. The body's data names its liquid, so Titan's methane seas get their own density (D125).
- **Calm (D123):** no waves, swell, currents or wind. They come later, under the same forces, by moving the surface.

### 2.2 Hydrostatics (buoyancy and the righting moment)

Each surface cell below the sea surface takes the pressure p = ρ_w·g·d at its centroid's depth d, as a force −p·A·n̂ at its centroid. A cell cut by the surface takes its submerged fraction from its vertices' depths.
- Summed over a closed hull, that is Archimedes' buoyancy at the centre of buoyancy. The righting moment comes out of the geometry, including Apollo's two stable floating attitudes.
- Nothing is per-vehicle (D125). Flooding through hatches or windows, and uprighting bags, are later.

### 2.3 The slam (von Kármán's momentum method on the footprint)

- **Footprint:** the cells' intersection with the water plane gives the wetted area A_wp, its centroid c_wp and the normal velocity w of the craft at c_wp (into the water).
- **Added mass:** a one-sided disc of the same area, of equivalent radius a = √(A_wp/π): **m_a = (4/3)·ρ_w·a³** along the water's normal. This is half of a disc's added mass in an unbounded fluid.
- **Force:** F = −d(m_a·w)/dt along the normal, at c_wp. Split:
  - −(dm_a/dt)·w from the footprint's growth over the substep: explicit;
  - −m_a·dw/dt: the added mass moves with the craft. It can exceed the craft's own mass (Apollo's at full immersion of the shield is ~2.4× the craft), so an explicit form is unstable. It enters **implicitly** as a rank-one addition to the rigid body's 6×6 mass matrix at c_wp along the normal, solved by Sherman–Morrison in closed form, which is deterministic.
- **Wagner's correction:** Wagner's wetting factor (the water piles up, a² = 3Rh instead of the geometric 2Rh for a sphere) raises the peak by ~50 %. It is an option (k = 2 or 3 below), off by default: von Kármán's form matches the rigid-body reference (§3), and the flexible-shield peak (38 g) lies beyond both rigid models anyway.
- **Pressure distribution:** the slam force is spread over the cells wetted during the substep (where the spray root passes), by area. That feeds the joint loads (D103) and the cells' pressure for damage. The local peak pressure at the spray root is estimated by Wagner's p ≈ ½ρ_w·ȧ² (E), checked against Stubbs's 1,475 kPa panel value.
- **Closed form for tests:** for a sphere of radius R entering vertically at V₀ with c² = kRh, the peak deceleration is
  a_max = (512/729)·(2kρ_w R c*/m)·V₀², reached when m_a = m/8, at c* = (3m/(32ρ_w))^{1/3}
  (D, derived here; k = 2 von Kármán, 3 Wagner; valid while c* is inside the shield's rim).

### 2.4 Drag and settling

Wetted cells moving through the water take a pressure drag ½ρ_w·C_D·|u_n|·u_n·A on their normal velocity component, C_D 1.0 (E). This carries the horizontal velocity's loss and damps the bobbing. Wave radiation damping, which really ends the bobbing, is absent on a calm sea with no waves; the drag stands in for it (E).

### 2.5 Integration and rest

- The sea is a ground of `sim::contact`: it uses the same substeps of the live tick, and the same rule that a point near the surface makes the vessel step its contact.
- **Floating at rest:** a craft whose kinetic energy relative to the sea stays under the rest threshold for the rest ticks freezes into a **floating** state. It then moves with the body's rotation, as `Landed` does, at its hydrostatic equilibrium draft and attitude. It wakes on input or on a separation (D121).
- **Chutes** are cut on water contact by their program (entry-ablation.md §3).

### 2.6 Loads, damage and the crew

- **Structure (D105):** the slam's cell pressures load the parts like any external pressure. A heat shield or hull over its rated pressure fails by the existing rules (Dragon's rating U).
- **Crew (D116):** the acceleration history goes to the crew's landing-load limits. Brinkley's dynamic response is "not done" in life-support.md, and it is the right consumer of these pulses.

## 3. The check against Apollo (done here, by hand)

The momentum model for a rigid sphere cap, R 4.69 m, 3,900 kg, 9.1 m/s, fresh water (Langley's basin), 0° pitch:

| Model | Peak deceleration | Where |
|---|---|---|
| von Kármán (k = 2) | **20.4 g** | 5.5 cm deep, footprint radius 0.72 m |
| Wagner (k = 3) | 30.6 g | 3.6 cm deep |
| TN D-3980's rigid theory | ~22 g | — |
| TN D-3980, flexible shield, measured | 38 g at −12°, 4 g at −39° | — |

Mean footprint pressure at the von Kármán peak: 3,900 kg × 200 m/s² / (π·0.72²) ≈ **490 kPa**, against the **414 kPa** measured over ~1.9 m² at peak acceleration. Agreement within ~10 % on the force and ~20 % on the mean pressure is the test's tolerance below.

**Dragon** by the same model (R 4.7 m, 9.6 t, seawater): **6.6 g at 7.0 m/s** (10 g with Wagner) and 4.6 g at 5.8 m/s, flat on the shield. A pitched entry (the shoulder first, as capsules hang under their mains) is much lower, as Apollo's −39° shows.

## 4. Tests that would prove it

1. **Archimedes:** a sphere, a cylinder and a box float at the draft that displaces their mass (within 1 %, at the cell count used). A box's righting moment matches its metacentric height.
2. **Apollo, rigid, 0°:** the closed form above reproduced by the sim's footprint version within 5 %; and **20 g ± 10 %** against the rigid theory's ~22 g (TN D-3980). Mean pressure at the peak 410–490 kPa.
3. **Pitch trend:** at −39° the peak is far below the 0° case (TN D-3980 measured 4 g against 38 g): below 10 g, and decreasing monotonically with the pitch's magnitude.
4. **Flotation:** the Apollo-like shape floats stably upright and near inverted (TN D-3980), and comes to rest in both.
5. **Dragon under four mains** (~7 m/s): peak 5–10 g (D, model); under three (~8.1 m/s) correspondingly higher. The capsule floats upright or reports its equilibrium attitude (Dragon's uprighting is U).
6. **Bits:** the same at any frame chunking; a craft at rest on water stays at rest over a long warp.

## 5. Prior work: what was taken and what was rejected

| Source | Taken | Rejected |
|---|---|---|
| von Kármán, *The impact on seaplane floats during landing*, NACA TN 321, 1929 | the momentum method | — |
| Wagner, *Über Stoß- und Gleitvorgänge an der Oberfläche von Flüssigkeiten*, ZAMM 12, 1932 | the wetting factor (an option), the spray-root pressure | the full Wagner problem (needs the free surface) |
| Stubbs, NASA TN D-3980, 1967 | the data and the rigid-theory reference | — |
| Orion SPLASH, LVIT and Ground Test Article drops (NASA Langley, 2011–2016) | that LS-DYNA predicts peaks; a source of later checks | LS-DYNA's ALE or SPH water (cost) |
| West, NASA TN D-7437, 1973 | splashdown speeds on 3 and 2 mains | — |

## 6. Open for the owner

1. **Where the sea is:** terrain below sea level counts as ocean. That is wrong for the few dry depressions (the Dead Sea's shore, Death Valley) until the body's data carries a water mask for physics as well as for visuals. Acceptable for now?
2. **Wagner or von Kármán** by default: von Kármán matches the rigid reference for Apollo. Wagner is ~50 % higher, nearer the flexible shield's measured peaks. Recommended: von Kármán, with the flexible structure's ringing left out (D103: rigid until it breaks).
3. **Orion's measured peaks** at the 25 ft/s drops were not pulled from the reports. They would make a third check.
4. **Dragon's floating attitude and uprighting:** not public (U). The hydrostatics will say where it floats.
