# Entry: ablation, Dragon's lifting entry and the chutes' logic

**Status: design** (RESEARCH, 2026-10-30, at 835f839). Nothing here is built. Decisions: D125 (ablation is one 1-D mechanism per cell from the material's parameters; PICA-X is one material), D126 (the chutes' deploy is a switch), D128 (layer 1 and layer 2), D116 (crew doses), D105 (failures from the simulation), D121 (the thermal network at its own rate), D129. The system: [aero-thermal](../systems/aero-thermal.md); the physics it extends: [aero-thermal.md](aero-thermal.md) (heating per cell, the thermal network), [reentry.md](reentry.md) (the shock layer, blackout). The splashdown: [water-landing.md](water-landing.md). Data: [falcon9.md](../vehicles/falcon9.md) §1.5, §4.5 and the rows tagged *RESEARCH*.

**The picks.**
1. **Ablation:** a charring-ablator column under each ablator cell: 1-D, a fixed number of nodes on a grid that moves with the receding surface. It models three-component Arrhenius pyrolysis, the gas leaving through the char, a surface energy balance with film-theory blowing, and recession from a B′c law. This is the FIAT/CMA model class, with TACOT's open parameters standing in for PICA-X.
2. **Dragon's entry:** lift from the offset centre of mass through the existing aero model, steered by bank angle under a numerical predictor–corrector on the landing guidance's predictor.
3. **Chutes:** the layer-2 deploy logic from navigation and barometric altitude, plus a layer-1 baroswitch backup in the chute part itself, as Apollo's Earth Landing System had.

## 1. Ablation, per cell

### 1.1 What exists, and where the column goes

Each skin cell today is one thermal node (its areal mass × specific heat) coupled to the interior node beneath it, heated by `thermal::cell_heat_layer` (Colburn over the cell's boundary layer, Sutton–Graves and Tauber–Sutton at stagnation regions) and stepped by backward Euler with the network (aero-thermal.md). A cell whose skin material is an **ablator** (a material with pyrolysis and surface-chemistry parameters) gets a **column** instead of its single node:
- the column's top is the receding surface, which takes the cell's heating;
- its bottom is the bondline, coupled to the cell's interior node (the carrier structure) as the skin node is today.

Nothing is specific to Dragon: any part with an ablator skin gets columns (D125).

### 1.2 The model (FIAT/CMA class)

From the governing equations as verified for TACOT in COMSOL against FIAT (NTRS 20170005508) and in Ewing et al.'s ITRAC against CMA (NTRS 20140008557):

- **Decomposition** (three components; Goldstein's form): ρ = Γ(ρ_A + ρ_B) + (1 − Γ)ρ_C, with ∂ρ_i/∂t = −A_i·exp(−E_i/RT)·ρ_o,i·((ρ_i − ρ_r,i)/ρ_o,i)^ψ_i above each reaction's onset temperature. For a step at constant temperature this has a **closed form** (x = (ρ_i − ρ_r,i)/ρ_o,i; ẋ = −k x^ψ ⇒ x(t) = (x₀^{1−ψ} + (ψ − 1)k t)^{−1/(ψ−1)}). Integrated that way it is exact, stable at any step and deterministic (`sim::math` for exp and pow). The reaction's time constant at 1,500 K is ~0.2 ms, far below a 20 ms tick, so an explicit step would not do.
- **Energy** in coordinates moving with the surface: ρc_p ∂T/∂t = ∂/∂y(k ∂T/∂y) + ṡρc_p ∂T/∂y + ṁ_g ∂h_g/∂y + (h_g − h̄)∂ρ/∂t, the last term the heat of pyrolysis (the gas's enthalpy against the solid's). The pyrolysis gas flows straight out to the surface (no pore pressure: a Type-1 code in the Ablation Workshop's terms). It is solved implicitly, so it is a tridiagonal solve per column per step.
- **Surface energy balance:** α·q̇_rad + ρ_eu_eC_H(h_r − h_w) + k ∂T/∂y − εσ(T_w⁴ − T_∞⁴) − ρ_eu_eC_H(B′h_w − B′_c h_c − B′_g h_g) = 0.
- **Blowing** reduces the heating: C_H/C_H0 = ln(1 + 2λB′)/(2λB′), with λ 0.5 (CMA's value, used by the Ablation Workshop's test cases), B′ = B′_c + B′_g, B′_g = ṁ_g/(ρ_eu_eC_H).
- **Recession:** ṡ = ρ_eu_eC_M·B′_c/ρ_char, with C_M = C_H (unit Lewis number).
- **Inputs from today's aero-thermal model:** the heat-transfer coefficient ρ_eu_eC_H is the cell's Stanton number × ρ_e u_e (`aero::boundary` computes it), h_r the recovery enthalpy, p_w the cell's pressure, q̇_rad the shock layer's radiation.

**The B′c law.** Full B′ tables (equilibrium surface chemistry, functions of p, T_w and B′_g) are what CMA and FIAT read. They come from a thermochemistry code, ACE or Mutation++. The design takes a **law from the atmosphere's composition** instead, so it works for any gas (D125):
- below ~800 K, none (kinetics, E);
- rising to the **diffusion-limited oxidation plateau** by ~1,500 K. For carbon burning to CO the plateau is B′c = Y_O₂,e·(2·12/32) = 0.232 × 0.75 = **0.174** in air (D, from the edge's oxygen mass fraction). For CO₂ atmospheres it follows from their oxygen likewise;
- a sublimation rise above ~3,300 K (E, from the TACOT B′ charts).

A test checks the law against the workshop's B′ table at 1 atm (case 2.4).

**Grid.** n = 16 nodes per column (E), finest at the surface (geometric, ratio 1.25). They are fixed to the receding surface, so the count never changes and the cost is fixed. The column ends at the material's remaining thickness; when the char front reaches the bondline the column keeps only its char.

**Rate.** The columns step with the thermal network at its own rate (D121), with each tick's heating accumulated between steps. Ablation happens only in air, while live; a coasting craft's columns hold their state (no ablation at radiative equilibrium in vacuum, a test).

**Mass.** The gas and the receded char leave the part. Its mass falls through the one mass truth (D121), and the centre of mass moves with it.

**Failure (D105).** A column that recedes through its thickness, or whose bondline passes the substrate's limit, exposes the structure. The existing overheat rule then destroys the part at its destruction temperature. The bondline limit is a material parameter (U for PICA-X → propose 523 K, the usual adhesive limit for PICA systems, E).

**Cost (estimate).** Dragon's shield is 12.6 m². At ~100 ablator cells × 16 nodes there are 1,600 nodes. A tridiagonal solve (~50 operations per node) plus two Arrhenius terms per node (~20 ns each) comes to ~0.1 ms per thermal step, against the live tick's 74–119 µs today. The ablator's own column rate may be slower than the skin's; measured when built.

### 1.3 The material: PICA-X as parameters

PICA's real properties are proprietary or export-controlled. The Ablation Workshop's **TACOT** (Theoretical Ablative Composite for Open Testing) is an open carbon/phenolic material "comparable to PICA", defined so codes can be compared. The `pica-x` material takes TACOT's numbers, each marked E (TACOT for PICA-X), until better ones are found:

| Parameter | Value | Tag | Source |
|---|---|---|---|
| Virgin, char density | 280, 220 kg/m³ | E (TACOT) | TACOT; NTRS 20170005508 App. Table A1 |
| Composition (by volume) | 10 % carbon fibre, 10 % phenolic, 80 % porous; resin volume fraction Γ = 0.5 of the solid | E (TACOT) | same |
| Resin component A | ρ_o 300, ρ_r 0 kg/m³; A 1.20·10⁴ s⁻¹; E/R 8,556 K (71.1 kJ/mol); ψ 3; onset 333 K | E (TACOT) | Table A1 |
| Resin component B | ρ_o 900, ρ_r 600 kg/m³; A 4.48·10⁹ s⁻¹; E/R 20,440 K (169.9 kJ/mol); ψ 3; onset 556 K | E (TACOT) | Table A1 |
| Fibre (component C) | ρ_o = ρ_r = 1,600 kg/m³, no reaction | E (TACOT) | Table A1 |
| Char yield; char to gas ratio | 78.6 %; 3.66 | D (TACOT) | Table A1's notes |
| Pyrolysis gas, elemental | C 49.6 %, H 13.6 %, O 36.8 % by mass | E (TACOT) | Table A2 |
| Emissivity (= absorptivity) | virgin 0.8, char 0.9 | E (TACOT) | Wang et al. 2019 citing TACOT |
| Conductivity, specific heat | functions of temperature, virgin and char, mixed by the extent of charring. U here → propose virgin k 0.2 → 0.4 W/(m·K) (300 → 1,000 K) and c_p 1,000 → 1,800 J/(kg·K); char k 0.4 → 1.5 W/(m·K) (300 → 3,000 K, mostly radiation through the pores) and c_p 800 → 2,000 J/(kg·K) | U/E | TACOT's tables are in the workshop's spreadsheet (TACOT_3.0.xls, not fetched); the proposals are the PICA class's, to replace with TACOT's tables |
| Blowing coefficient λ | 0.5 | E | CMA; Ablation Test-Case #2 |
| Bondline limit | 523 K | E | class value |

The **heat shield's thickness and layout** are Dragon's data (falcon9.md §1.5): PICA-X/PICA-3 tiles on a carrier, thickness U (the sheet proposes 60 mm; "over 3 inches" is unsourced, E). The heat shield is one part with an ablator skin. Its thickness is a parameter per cell, uniform unless data says otherwise.

### 1.4 Tests that would prove it

1. **Decomposition alone** (TACOT held at 1,000 K): the extents of reaction against the closed form, and against Ewing et al.'s verification case 1.
2. **Ablation Test-Case 2.1** (Lachaud, Martin, van Eekelen & Cozmuta, v2.8, 2012): 5 cm TACOT, adiabatic back, ρ_eu_eC_H 0.3 kg/(m²·s), h_e 1.5 MJ/kg, 1 atm, 60 s heated then 60 s cooling, no recession: surface temperature ~1,644 K.
3. **Test-Case 2.3** (h_e 25 MJ/kg, recession): at 60 s the surface has receded **~15.7 mm**, the char front (98 %) is at **~19.5 mm** and the pyrolysis front (2 %) at **~25.7 mm** from the original surface. CMA and ITRAC agree on these (Ewing et al., NTRS 20140008557, fig. 17, read off the plot to ±0.5 mm). Within 10 %.
4. **Stardust's flight** (PICA, 5.82 cm, peak 1,200 W/cm²): the sim's Stardust entry with TACOT parameters gives recession and char depth of the order of the post-flight 0.65 cm and 9.8 ± 1 mm (within 50 %, since TACOT is not PICA).
5. **An arc-jet-like check:** PICA under an oxy-acetylene torch at 1,000 W/cm² lost 7.1 mm in 30 s (Wang et al. 2019). Order of magnitude only (a torch is not air).
6. **Dragon's LEO return** in the sim: the peak heat-shield surface temperature near the published ~2,200 K (falcon9.md §4.5, within 15 %) and the remaining thickness reported.
7. **Energy and mass:** with heating off the column conserves energy to the network's tolerance. The mass lost equals the gas plus the receded char.
8. **Bits:** the same at any frame chunking (rule 4).

## 2. Dragon's lifting entry

### 2.1 The physics is already universal

A capsule whose centre of mass sits off its axis trims at an angle of attack, and the aero model gives the lift (Apollo's L/D ≈ 0.3 at trim is already one of its tests). Dragon's offset comes from its parts' masses: the tanks and bottles in the service section, plus ballast as a part if the layout does not give it (a mass part, D125).

- **Published:** Dragon (2008 design) flies "a canonical lifting re-entry at a 'low' 12° trim angle of attack with a 0.18 L/D", which "allows a very narrow re-entry corridor and keeps deceleration-loading on the crew below approximately 5 g" (Trevino, SpaceX, IPPW-6, 2008; P for Dragon 1, E for Crew Dragon).
- **A test:** Crew Dragon of parts trims with L/D in 0.15–0.25 (E). The offset needed is reported, not tuned in.

### 2.2 Bank-angle guidance (layer 2, `ship_os::entry`)

Dragon steers its lift vector by rolling about the velocity with the Dracos. Its guidance algorithm is not public (U). Orion flew a numerical predictor–corrector (PredGuid) paired with Apollo's final-phase guidance on Artemis I (AAS 24-174). The design is a **numerical predictor–corrector (FNPEG, Lu 2014)** on the same predictor as the booster ([landing-guidance.md](landing-guidance.md) §3), given the bank law as its attitude input:

- **Bank magnitude**, linear in energy e = v²/2 − μ/r: |σ(e)| = σ₀ + (σ_f − σ₀)(e − e₀)/(e_f − e₀), σ_f fixed (E 70°).
- **Corrector:** each wake (1 Hz, E) predicts to the drogue gate (§3) and adjusts σ₀ by a secant step to null the range-to-go to the splashdown target, less the chutes' expected drift (zero on a calm, windless sea). A fixed number of steps per wake, as for the booster.
- **Sign:** bank reversals when the crossrange error leaves a corridor that narrows with speed (Apollo and Orion's lateral logic; the width ∝ V, E).
- **Limits:** a g-limit (Trevino's ~5 g, and D116's doses) by lifting up when the predicted peak exceeds it. Bank saturation is held at its limit and flagged.
- **Late phase:** below ~Mach 3 (E) the capsule holds its last bank: lift no longer moves the point much, and the chutes come next.
- **Ballistic backup:** if guidance or navigation is lost, the capsule rolls at a constant rate (E 15°/s) to null the mean lift. This is a layer-2 function, like Soyuz's and Apollo's ballistic modes, with higher g (E).

### 2.3 The deorbit and the corridor

- The deorbit burn is a planned burn (the maneuver program, D126) on the Dracos.
- **Since 2025** the trunk separates **after** the deorbit burn, and splashdowns moved to the Pacific off California. The aim is to put the trunk on a known re-entry path uprange of the capsule (SpaceX via SpaceNews, July 2024: P). falcon9.md §4.5's "before the deorbit burn" row is older; the new row is beside it.
- **Timeline** (Crew-9, March 2025, P, NASA): deorbit burn complete 39 min before splashdown; drogues ~4 min before splashdown at ~18,000 ft and ~350 mph; mains less than a minute later at ~6,000 ft and ~119 mph.
- **The corridor** is computed, not assumed. The predictor sweeps the flight-path angle at entry interface (122 km): the steepest at which full lift-up stays under the g-limit, and the shallowest at which full lift-down still reaches the target without overshooting. A test reports it for the Crew Dragon of parts. Trevino's "very narrow" is the expectation.

## 3. The chutes' real logic

The physics exists (`sim::chute`: reefing stages on timers from line stretch, filling, strength, a cluster as one canopy). What is missing is **when to deploy**: today the main opens at a true height (software-layers.md: "the parachutes' deploy heights read the true height").

### 3.1 How real capsules do it

| Vehicle | Triggers | Sequence | Backups | Source |
|---|---|---|---|---|
| Apollo (ELS) | **barometric switches**, two redundant systems (A and B), independent of the guidance computer; time-delay relays below their altitude (low aborts) | forward heat shield off at 24,000 ft + 0.4 s; two drogues mortar-deployed reefed at + 2 s, one stage, disreefed at 10 s; at **11,000 ft** (lower baroswitches) drogues released, three pilot chutes pull the mains; mains disreefed in **two stages at 6 s and 10 s** (full open ~9,000 ft); mains released after splashdown by a crew switch | redundant switch sets; crew switches; descent at 32 ft/s on 3 mains and 36 ft/s on 2 | NASA TN D-7437 (West, 1973), figs. 1–2 (P) |
| Orion (CPAS) | — | forward bay cover off at ~26,500 ft and ~325 mph; two drogues at ~25,000 ft; three mains in two reefed stages | — | NASA (P) |
| Crew Dragon | U (navigation and/or barometric) | drogues ~18,000 ft at ~350 mph; mains ~6,000 ft at ~119 mph, drogues released | lands safely on **3 of 4 mains** (Mark 3 qualification); propulsive landing re-enabled as a backup in 2024 | NASA [S20]; SpaceX tests 2019 (P) |

### 3.2 The design

- **Layer 2, `ship_os::chutes`** (software-layers.md §7 point 4):
  - It arms after peak deceleration once the dynamic pressure is falling (E).
  - It deploys the drogues at the drogue altitude by the navigation solution's height above the sea, confirmed by the static-pressure sensor's barometric altitude (both from `Knowledge`), and only below the drogues' deploy_max_q. Above that q it waits, and deploys anyway after a time limit (E 10 s).
  - It deploys the mains at the main altitude the same way, releasing the drogues.
  - It cuts the mains on water contact (E, from Apollo's practice).
  - The altitudes are parameters of the program's configuration (Dragon: 5.5 km and 1.8 km, falcon9.md §4.5).
- **Layer 1, the chute part's own baroswitch** (D128: "how the part actually works"): an optional pressure threshold per canopy, with its own timer chain (Apollo's ELS was independent of the guidance computer). It fires only when layer 2 has not deployed by then: a dead computer, or the switch set to "not at all" (D126). Its thresholds are the part's parameters (Dragon U → propose the drogue at 50 kPa ≈ 5.5 km and the mains at 81 kPa ≈ 1.8 km, from the standard atmosphere, E).
- **Faults:** a drogue that fails (no deceleration rise within its fill time plus 2 s) does not stop the mains at their altitude. A main lost is physics: three of four give √(4/3) × 7.0 ≈ 8.1 m/s (D). Manual deploy is always possible and runs the same function (D126).
- **Disreef stages** are the canopies' data. Dragon's are U (falcon9.md §4.5 proposes one main stage at 15 % for 6 s). Apollo's real ones (drogue one stage at 10 s; mains two stages at 6 s and 10 s) are the reference class.

**Tests:**
1. Table tests of the deploy rule (pure function): gates, q waits, the time limit, the drogue-fail path.
2. The baroswitch fires when the computer is dead and not when layer 2 already deployed.
3. Dragon of parts from a LEO entry: drogues near 5.5 km at ~156 m/s and mains near 1.8 km at ~53 m/s (within 15 %), splashdown near 7 m/s; on three mains near 8 m/s.

## 4. Prior work: what was taken and what was rejected

| Source | Taken | Rejected |
|---|---|---|
| FIAT (Chen & Milos, NASA Ames, 1999), CMA (Aerotherm) | the model class: charring, B′ surface balance, film-theory blowing | full B′ tables from an equilibrium code (replaced by a law from the gas) |
| PATO (Lachaud & Mansour; OpenFOAM, open source) | the workshop's test cases, Type-1 versus Type-3 | pore pressure and 3-D porous flow (cost; Type-3) |
| Ablation Workshop TACOT and Test-Cases #1–#2 (Lachaud, Martin, van Eekelen, Cozmuta 2011–12) | the parameters and the tests | — |
| Ewing, Laker & Walker, *Numerical modeling of ablation heat transfer*, JTHT 27(4), 2013 (NTRS 20140008557) | the verification numbers | — |
| NTRS 20170005508 (COMSOL against FIAT, TACOT) | the equations and Table A1 | — |
| Trevino, *SpaceX Dragon re-entry vehicle: aerodynamics and aerothermodynamics…*, IPPW-6, 2008 | L/D, trim, the g level | — |
| Lu, *Entry guidance: a unified method*, JGCD 37(3), 2014 (FNPEG); Orion Artemis I entry performance, AAS 24-174 | the predictor–corrector with a linear-in-energy bank | Apollo's reference-trajectory gains (they need a stored trajectory per mission) |
| West, *Apollo experience report: Earth landing system*, NASA TN D-7437, 1973 | the triggers, redundancy, reefing times, the baroswitch as the part's own logic | — |

## 5. Open for the owner

1. **PICA-X's numbers:** TACOT's open values stand in for a proprietary material (E). Should a later pass fit them to PICA's published arc-jet data instead?
2. **Dragon's heat-shield thickness** (U; 60 mm proposed, "over 3 inches" unsourced) and its **centre-of-mass offset** (U): both decide the margin at entry.
3. **The chute part's own baroswitch:** a layer-1 backup is what Apollo flew. Dragon's own backup logic is not public. Keep it as an optional parameter of the chute part?
4. **Columns per cell versus per region:** one column per ablator cell is the D125 reading. If the cost bites, neighbouring cells with the same material and similar heating could share one, which is a coarse-graining for the owner to approve.
