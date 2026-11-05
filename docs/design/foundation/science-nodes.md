# Science nodes: how the science layer meshes with the part layer (draft)

Status: Director draft for the science design session, 2026-11-05. Not canon. Binding inputs: [scope](../../plans/scope-2026-11.md) 5.1 (15 fields) and 5.2 (four layers), [part schema](part-schema.md) rules 1-38 (especially 15, 20, 32, 33, 36), [present-day catalog](research/type-catalog.md) (90 part / 181 component types) and [future catalog](research/type-catalog-future.md) (84 / 110).

## 1. Node kinds and naming

A node is a discrete piece of knowledge: something new in kind (5.2 layer 2). It never is a part or a design. It makes a *type* buildable, or widens what a type can be, or adds a step or a feature. Field levels (section 4) do everything continuous. The test for "node or level?": if a better value of something already known is enough, it is a level; if the physics, the geometry or the process is different in kind, it is a node.

**Six kinds.** The id prefix is the kind.

| Kind | Prefix | What it is | What it unlocks | Example |
|---|---|---|---|---|
| Class / size regime | `cls.` | one part class (rule 10) at one size regime (rules 15, 33) | the part type at that regime: its geometry range is capped at the regime's upper bound | `cls.tank.cylindrical.m` (2-5.5 m) |
| Alternative design | `alt.` | a different way to build the same function (rule 15): an engine cycle, a common dome, a monolithic heatshield | a design option inside a type: a choice in the design program, its own parameter set and sometimes extra component slots | `alt.engine.cycle.electric_pump` |
| Material | `mat.` | a structural, thermal, optical, electrochemical or nuclear material, or a propellant | the material in the production-parameter menu of every type that accepts its family; its known properties are revealed on unlock (5.2) | `mat.al_li_2195`, `mat.prop.lch4` |
| Component | `cmp.` | a component type new in kind (rule 20) | the component type in slots that accept it; for complex components (rule 2) the right to run a component design program or buy one | `cmp.turbopump` |
| Process | `prc.` | a manufacturing or processing method | a recipe step (5 industry): without it, recipes that need the step can't be built at that site | `prc.friction_stir_weld` |
| Capability | `cap.` | a method for the planner, a sensor mode, an operational technique | a code feature: a planner method, a guidance mode, a data product, an operation (docking, transfer) | `cap.terrain_relative_nav` |

**Naming.** `kind.family.name[.regime]`, lower case, the family taken from the catalog id where one exists (`cls.engine.liquid_biprop.l` for `engine.liquid_bipropellant` at the large regime). Alternatives name the axis they vary: `alt.engine.cycle.*`, `alt.engine.cooling.*`, `alt.tank.*`. In the tables below, `{s,m,l}` expands to one node per regime and is counted as such.

**Size regimes** (rule 33, from manufacturing thresholds). One shared ladder per kind of hardware, so a player learns three or four words, not a ladder per class:

| Ladder | Regimes | Threshold that sets it |
|---|---|---|
| Vehicle diameter (tanks, shells, solids, heatshields) | `s` < 2 m; `m` 2-5.5 m; `l` 5.5-10 m; `xl` > 10 m (orbital or on-site build, near era) | air freight and small tooling; road transport ~4-5 m plus fairing envelope; barge ~10 m; beyond that the part is built where it flies |
| Engine thrust | `xs` < 1 kN; `s` 1-100 kN; `m` 0.1-1 MN; `l` 1-3 MN; `xl` > 3 MN | test-stand classes; combustion-stability scaling (F-1's instability problem); turbomachinery size |
| Spacecraft panel and dish | `s` < 1 m (CubeSat and small sat); `m` 1-5 m (fits one fairing flat); `l` > 5 m (folded, multi-hinge or roll-out) | fairing envelope; number of deploy hinges |
| Electric power level | `s` < 2 kW; `m` 2-15 kW; `l` 15-100 kW; `xl` > 100 kW | PPU and thermal limits; vacuum-chamber facility size for testing |

**Which classes get regimes.** Rule 15 says every class gets a node per size regime. Applied literally that is ~250 class nodes for 90 present-day classes, most of them meaningless (a sun sensor has no barge threshold). Proposal, adopted in section 2: regimes only where a rule-33 threshold changes how the thing is built or tested; every other class gets one `cls.` node and its size rides the field levels. See mesh finding M1.

**What a node carries.** id; kind; fields (the field levels that gate it and that its R&D project draws on, principle 2 of 5.1); prerequisites (nodes, and field-level floors where they matter); how it is reached (5.2's four routes: **W** the world gets there, **R** a lab project, **RF** a lab project plus a flight measurement or observed part behaviour, **O** instantly on an observation); properties revealed on unlock (materials only); and what it enables (type ids, parameter ranges, recipe steps, features). Readiness (TRL) is a property of the *technology a node enables*, raised by test campaigns; a world-known node arrives with the world's TRL for it (KRUSTY gives fission surface power TRL 5 in 2030, not 9).

**World knowledge at 2030** (rule 32), one of:
- **Y**: published or widely practised. Licensable cheaply, or researched quickly (rule 36: world-known is cheap novelty).
- **P (company)**: demonstrated only by one real company, kept proprietary. The world knows it exists and roughly works (that is all the player knows). Reached only by R&D at full price, never licensed. Real companies appear by name here as design notes; whether the game names them is open question Q6.
- **N**: not reached by anyone by 2030 (lab ideas count as N unless a test article exists).


## 2. The present-day node set (the world in 2030)

Every node a present-day catalog type needs. Columns: id (prefix = kind), fields, prerequisites, world-known at 2030, TRL at 2030 of what it enables (9 = flown operationally, 6 = ground or flight demo, 4-5 = lab or breadboard), enables. Prerequisites list nodes only; a field-level floor is written as `F5>x` where it matters. Components with no node (rule 20) are listed per family at the end of the section as **field levels only**. GSE sits outside (2.21).

### 2.1 Shared materials

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| mat.al_2219 | F5 | prc.vppa_tig_weld | Y | 9 | all metallic tank, shell and structure types; Centaur, Shuttle ET heritage |
| mat.al_li_2195 | F5, F6 | mat.al_2219, prc.friction_stir_weld | Y | 9 | same set, lighter; F9, SLS tanks |
| mat.al_7xxx_6xxx | F5 | | Y | 9 | machined structure, rings, fittings, `structure.ring_frame`, `structure.truss_strut` |
| mat.steel_stainless_cryo | F5 | | Y | 9 | tanks and shells (Atlas balloon tanks, 301/304L) |
| mat.steel_30x | F5, F6 | mat.steel_stainless_cryo | P (SpaceX) | 8 | lighter stainless at the l regime; Starship |
| mat.ti_6al4v | F5 | | Y | 9 | spherical tanks, COPV liners, fittings, engine parts |
| mat.ni_superalloy | F5 | | Y | 9 | turbines, manifolds, injectors (Inconel 718/625), ox-rich hardware with coatings |
| mat.cu_alloy_grcop | F5, F6 | prc.lpbf_metal_am | Y | 9 | regen chamber liners (NASA GRCop-42) |
| mat.niobium_c103 | F5 | | Y | 9 | radiation-cooled nozzles and extensions, small thrusters |
| mat.refractory_re_ir | F5 | | Y | 9 | high-temperature bipropellant thruster chambers (Re/Ir), catalyst-bed housings |
| mat.cfrp_im | F5, F6 | prc.afp_composite | Y | 9 | composite shells, struts, panels, booms, dishes, overwraps |
| mat.cfrp_cryo | F5, F3 | mat.cfrp_im | Y | 8 | composite cryotank walls (Electron), `alt.tank.linerless_composite` |
| mat.aramid_fabric | F5 | | Y | 9 | overwraps, MMOD layers, chute risers, restraints |
| mat.vectran_fabric | F5 | | Y | 9 | airbags, inflatable modules (BEAM), HIAD straps |
| mat.honeycomb_core | F5 | | Y | 9 | `structure.panel_sandwich`, crush cores, ablator carriers (Al and Nomex cores) |
| mat.polyimide_film | F5, F3 | | Y | 9 | MLI layers, sails, flexible array substrates (Kapton) |
| mat.beryllium | F5, F8 | | Y | 9 | light, stiff mirrors (JWST), scan mirrors |
| mat.optical_glass_lowcte | F8, F5 | prc.optics_figure_polish | Y | 9 | ULE/Zerodur mirrors and benches, windows (fused silica) |
| mat.sic_optical | F8, F5 | | Y | 9 | SiC mirrors and benches (Herschel, Gaia) |
| mat.carbon_carbon | F5, F2 | prc.cvi_densify | Y | 9 | RCC leading edges, SRM nozzle exits, hot structure |
| mat.cmc_c_sic | F5, F2 | prc.cvi_densify | Y | 7 | reusable hot structure, nozzle extensions |
| mat.pica | F5, F2 | | Y | 9 | ablative heatshields up to ~1,500 W/cm² (Stardust) |
| mat.pica_x | F5, F2 | mat.pica | P (SpaceX) | 9 | cheaper reusable-tolerant PICA (Dragon) |
| mat.avcoat | F5, F2 | prc.ablator_honeycomb_fill | Y | 9 | Apollo/Orion shields |
| mat.sla_561v | F5, F2 | | Y | 9 | low-flux Mars aeroshells, backshells |
| mat.carbon_phenolic | F5, F2 | | Y | 9 | very high flux entry (Galileo probe); heritage precursor rayon lapsed, so an R&D project to requalify |
| mat.heeet | F5, F2 | prc.3d_weave | Y | 6 | woven high-flux ablator (NASA), not flown |
| mat.silica_tile | F5, F2 | prc.ceramic_tile_fab | Y | 9 | reusable ceramic tiles (LI-900), Starship hex tiles |
| mat.flexible_tps | F5, F2 | | Y | 7 | flexible TPS blankets (HIAD, LOFTID) |
| mat.shield_hydrogenous | F10, F14 | | Y | 9 | polyethylene and water shielding (`radiation_shield_mass`) |
| mat.shield_neutron_gamma | F10 | | Y | 7 | LiH, B4C and tungsten shielding for reactors and RTGs |

### 2.2 Propellants and working fluids

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| mat.prop.lox | F4, F3 | | Y | 9 | oxidiser in every LOX engine, cryo tanks |
| mat.prop.rp1 | F4 | | Y | 9 | kerolox engines (Merlin, Rutherford) |
| mat.prop.lh2 | F4, F3 | mat.prop.lox | Y | 9 | hydrolox engines, NTR propellant (near) |
| mat.prop.lch4 | F4, F3 | mat.prop.lox | Y | 9 | methalox (Raptor, BE-4, Vulcan) |
| mat.prop.storable_biprop | F4 | | Y | 9 | NTO/MMH/UDMH (Draco, OMS, apogee engines) |
| mat.prop.hydrazine | F4 | | Y | 9 | monoprop thrusters and engines |
| mat.prop.green_monoprop | F4 | mat.prop.hydrazine | Y | 8 | AF-M315E/ASCENT, LMP-103S (needs a hotter catalyst bed) |
| mat.prop.htp | F4 | | Y | 9 | hydrogen peroxide monoprop and oxidiser |
| mat.prop.n2o | F4 | | Y | 9 | hybrid oxidiser, self-pressurising |
| mat.prop.solid_apcp | F4, F6 | prc.solid_grain_cast | Y | 9 | `engine.solid_rocket_motor` |
| mat.prop.hybrid_fuel | F4 | | Y | 8 | HTPB/paraffin grains for `engine.hybrid_rocket` |
| mat.prop.ep_gas | F9, F4 | | Y | 9 | xenon, krypton, argon for electric thrusters (Starlink runs Kr and Ar) |
| mat.prop.iodine | F9, F4 | mat.prop.ep_gas | Y | 7 | solid-stored EP propellant (ThrustMe, 2020) |
| mat.prop.cold_gas | F2 | | Y | 9 | N2/He cold-gas RCS, pressurants |
| mat.prop.pyrophoric_tea_teb | F4 | | Y | 9 | hypergolic-slug ignition and relights |
| mat.prop.curie_standin | F4 | | P (Rocket Lab) | 9 | labelled stand-in (rule 38) |
| mat.coolant_loop | F3 | | Y | 9 | ammonia, propylene glycol, NaK for heat pipes and pumped loops |

### 2.3 Shared processes (recipe steps)

| id | fields | prereqs | 2030 | TRL | enables (recipe step for) |
|---|---|---|---|---|---|
| prc.vppa_tig_weld | F6 | | Y | 9 | welded metallic tanks and shells |
| prc.friction_stir_weld | F6 | | Y | 9 | Al-Li tanks, large welded domes |
| prc.spin_form | F6 | | Y | 9 | one-piece domes and shells |
| prc.isogrid_machining | F6 | | Y | 9 | `alt.structure.isogrid` |
| prc.afp_composite | F6 | | Y | 9 | composite layups (shells, tanks, panels) |
| prc.filament_wind | F6 | prc.afp_composite | Y | 9 | COPVs, SRM cases |
| prc.autoclave_cure | F6 | | Y | 9 | aerospace-grade composite cure; out-of-autoclave is a field-level gain |
| prc.honeycomb_bond | F6 | | Y | 9 | sandwich panels, fairings |
| prc.lpbf_metal_am | F6, F5 | | Y | 9 | printed injectors, chambers, valve bodies (SuperDraco, Rutherford) |
| prc.ded_large_am | F6 | prc.lpbf_metal_am | Y | 7 | large printed nozzles and manifolds |
| prc.vacuum_braze | F6 | | Y | 9 | regen jackets, heat exchangers |
| prc.solid_grain_cast | F6, F4 | | Y | 9 | solid motor grains |
| prc.cvi_densify | F6 | | Y | 9 | C/C and CMC parts |
| prc.ablator_honeycomb_fill | F6 | | Y | 9 | AVCOAT gunning into honeycomb |
| prc.ceramic_tile_fab | F6 | | Y | 9 | reusable tiles |
| prc.3d_weave | F6 | | Y | 6 | HEEET and woven TPS |
| prc.iii_v_epitaxy | F6, F7 | | Y | 9 | multi-junction cells, detectors |
| prc.radhard_fab | F6, F7 | | Y | 9 | rad-hard processors and memory (few foundries) |
| prc.li_cell_fab | F6, F4 | | Y | 9 | Li-ion cells |
| prc.optics_figure_polish | F6, F8 | | Y | 9 | mirrors, lenses, gratings |
| prc.textile_canopy | F6, F2 | | Y | 9 | parachutes, airbags, inflatables |
| prc.pyro_device_mfg | F6, F4 | | Y | 9 | every pyrotechnic component (export-controlled in practice) |
| prc.pu238_production | F10, F6 | | Y | 9 | Pu-238 heat sources (US DOE only; supply is the limit, ~1.5 kg/yr) |
| prc.leu_haleu_fuel_fab | F10, F6 | | Y | 7 | reactor fuel (HALEU, UMo, TRISO) |

### 2.4 Structure

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.structure.interstage.{s,m,l} | F5, F1 | a shell material | Y | 9 | `structure.interstage` |
| cls.structure.fairing.{s,m,l} | F5, F2 | mat.cfrp_im or Al; prc.honeycomb_bond | Y | 9 | `structure.fairing_half` |
| alt.fairing.recoverable | F2, F1 | cls.structure.fairing.m, alt.chute.parafoil | P (SpaceX) | 9 | fairing half with a parafoil and cold-gas pod |
| cls.structure.payload_adapter.{s,m} | F5 | | Y | 9 | `structure.payload_adapter` (ESPA is `s` with radial ports) |
| cls.structure.thrust_structure.{s,m,l} | F5, F1 | | Y | 9 | `structure.thrust_structure` |
| cls.structure.equipment_bay | F5, F7 | | Y | 9 | `structure.equipment_bay` |
| cls.structure.truss_strut | F5 | | Y | 9 | `structure.truss_strut`; size rides F5 |
| cls.structure.panel_sandwich | F5, F6 | mat.honeycomb_core | Y | 9 | `structure.panel_sandwich` |
| alt.structure.isogrid | F5 | prc.isogrid_machining | Y | 9 | isogrid/orthogrid walls on any metallic shell or tank |
| alt.structure.composite_sandwich_shell | F5 | prc.honeycomb_bond, mat.cfrp_im | Y | 9 | sandwich shells for interstages, fairings, adapters |
| (no node) | F5 | | | | `structure.ring_frame`: a trivial part (M7) |

### 2.5 Tankage

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.tank.cylindrical.{s,m,l} | F5, F2, F3 | a tank material; a weld or layup process | Y | 9 | `tank.cylindrical` |
| cls.tank.spherical.{s,m} | F5 | mat.ti_6al4v or Al | Y | 9 | `tank.spherical` |
| cls.tank.toroidal | F5, F2 | cls.tank.cylindrical.s | Y | 6 | `tank.toroidal` (flown on Soviet stages and sat buses; rare) |
| cls.tank.conformal | F5, F6 | prc.lpbf_metal_am or prc.afp_composite | Y | 6 | `tank.conformal` |
| cls.tank.cryo_vacuum_jacketed.{s,m} | F3, F5 | cls.tank.cylindrical.s | Y | 9 | `tank.cryo_vacuum_jacketed` (Shuttle ET was foam, not jacketed) |
| alt.tank.common_bulkhead | F5, F3 | cls.tank.cylindrical.s | Y | 9 | one part, two volumes (rule 17): Centaur, S-II, Electron |
| alt.tank.balloon | F5, F2 | mat.steel_stainless_cryo | Y | 9 | pressure-stabilised thin wall (Atlas, Centaur) |
| alt.tank.linerless_composite | F5, F3 | mat.cfrp_cryo | Y | 8 | linerless CFRP tank (Electron; NASA CCTD test) |
| cmp.copv | F5, F6 | prc.filament_wind, mat.cfrp_im | Y | 9 | `vessel.copv`, `inflation_gas_bottle` |
| cmp.diaphragm_bladder | F5, F2 | | Y | 9 | `diaphragm.bladder` |
| cmp.pmd | F2, F3 | | Y | 9 | `screen.capillary_pmd` (low-g propellant positioning) |

### 2.6 Primary propulsion

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.engine.liquid_biprop.{xs,s,m,l,xl} | F4, F5, F2, F3 | one cycle node, one cooling node, propellant nodes | Y | 9 | `engine.liquid_bipropellant` (xl Y from F-1/RD-170 heritage) |
| alt.engine.cycle.pressure_fed | F2, F4 | | Y | 9 | pressure-fed cycle |
| alt.engine.cycle.gas_generator | F2, F4 | cmp.turbopump | Y | 9 | GG cycle (Merlin) |
| alt.engine.cycle.expander | F3, F2 | cmp.turbopump, mat.prop.lh2 or lch4 | Y | 9 | closed and open expander (RL10, Vinci) |
| alt.engine.cycle.tap_off | F2 | cmp.turbopump | Y | 8 | tap-off (BE-3) |
| alt.engine.cycle.staged_fuel_rich | F2, F4, F5 | alt.engine.cycle.gas_generator | Y | 9 | fuel-rich staged combustion (RS-25) |
| alt.engine.cycle.staged_ox_rich | F2, F4, F5 | alt.engine.cycle.gas_generator, ox-rich-compatible coatings | Y | 9 | ox-rich staged combustion (RD-180, BE-4) |
| alt.engine.cycle.full_flow | F2, F4, F5 | both staged-combustion nodes | P (SpaceX) | 9 | full-flow staged combustion (Raptor) |
| alt.engine.cycle.electric_pump | F2, F7, F4 | cmp.e_pump, cmp.battery_pack | Y | 9 | electric-pump cycle (Rutherford; the cycle is public, the design isn't) |
| alt.engine.cooling.regen | F3, F5 | prc.vacuum_braze or prc.lpbf_metal_am | Y | 9 | regeneratively cooled chamber |
| alt.engine.cooling.ablative | F5 | | Y | 9 | ablative chamber/nozzle |
| alt.engine.cooling.radiative | F5, F3 | mat.niobium_c103 or Re/Ir | Y | 9 | radiation-cooled chambers and extensions |
| alt.engine.deep_throttle | F2, F1 | | Y | 9 | throttle range beyond ~3:1 (LMDE, landing engines) |
| alt.engine.reusable_rated | F5, F6 | | Y | 9 | life and refurbishment parameters for multiple flights |
| cmp.turbopump | F2, F5, F3 | mat.ni_superalloy | Y | 9 | `pump.turbopump_assembly` |
| cmp.e_pump | F2, F7 | cmp.motor_inverter_hv | Y | 9 | `pump.electric_centrifugal` |
| cmp.motor_inverter_hv | F7 | | Y | 9 | `power.motor_inverter` |
| cmp.injector | F2, F4 | | Y | 9 | `injector.element_pattern` |
| cmp.igniter | F4 | prc.pyro_device_mfg for the pyro kind | Y | 9 | `igniter.pyrotechnic`, `igniter.torch`, `igniter.hypergolic_slug` (one node, three kinds, M6) |
| cmp.catalyst_bed | F4, F5 | | Y | 9 | `catalyst_bed` |
| cmp.hydraulic_tvc | F2, F5 | | Y | 9 | `hydraulic_actuator`, `accumulator.hydraulic` |
| cls.engine.monoprop.{xs,s} | F4, F5 | cmp.catalyst_bed, a monoprop | Y | 9 | `engine.liquid_monopropellant` |
| cls.engine.solid.{s,m,l} | F4, F5 | mat.prop.solid_apcp | Y | 9 | `engine.solid_rocket_motor` (by case diameter) |
| alt.solid.segmented | F5 | cls.engine.solid.m | Y | 9 | field-joint segmented cases (SRB) |
| alt.solid.flex_seal_tvc | F5 | | Y | 9 | flexible-bearing nozzle TVC |
| cmp.nozzle_throat_insert | F5 | mat.carbon_carbon | Y | 9 | throat inserts (slot named in catalog, no row: M2) |
| cls.engine.hybrid.{s,m} | F4, F5 | mat.prop.hybrid_fuel, an oxidiser | Y | 8 | `engine.hybrid_rocket` |
| cls.engine.hall.{s,m,l} | F9, F7 | cmp.hollow_cathode, cmp.ppu | Y (l: N flight) | 9 / 9 / 5 | `engine.electric_hall_thruster` |
| alt.hall.magnetic_shielding | F9, F5 | cls.engine.hall.s | Y | 8 | long-life Hall channels (AEPS) |
| cls.engine.gridded_ion.{s,m} | F9, F7, F11 | cmp.ion_optics, cmp.hollow_cathode, cmp.ppu | Y | 9 | `engine.electric_gridded_ion_thruster` |
| alt.ion.rf_ionisation | F9, F8 | cls.engine.gridded_ion.s | Y | 9 | RF ion thrusters (RIT) |
| cls.engine.mpd_vasimr.l | F9, F7, F3 | cmp.hts_coil, cmp.ppu, cls.engine.hall.l | Y | 5 | `engine.electric_mpd_vasimr` (ground only) |
| cmp.hollow_cathode | F9, F5 | | Y | 9 | hollow cathodes (slot, no row: M2) |
| cmp.ion_optics | F9, F5 | | Y | 9 | ion grids (slot, no row: M2) |
| cmp.ppu | F7, F9 | cmp.power_conditioning | Y | 9 | `power_processing_unit` |

### 2.7 RCS

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.thruster.cold_gas | F2 | mat.prop.cold_gas | Y | 9 | `thruster.rcs_cold_gas` |
| cls.thruster.rcs_mono.{xs,s} | F4, F5 | cmp.catalyst_bed | Y | 9 | `thruster.rcs_monopropellant` |
| cls.thruster.rcs_biprop.{xs,s} | F4, F5 | mat.prop.storable_biprop | Y | 9 | `thruster.rcs_bipropellant` (Draco xs, SuperDraco s) |
| alt.rcs.pod_housing | F5 | any RCS class | Y | 9 | a pod part hosting thrusters as components (catalog section 3, no type: M4) |
| cmp.valve.fast_solenoid | F2, F7 | | Y | 9 | `valve.solenoid`, `valve.poppet` |

### 2.8 Fluid-system components

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cmp.valve.general | F2, F5 | | Y | 9 | `valve.ball`, `valve.check`, `valve.relief_pressure`, `valve.burst_disk`, `purge_valve`, and the catalog's unrowed `main_valve`, `vent_valve` |
| cmp.valve.cryogenic | F3, F2 | cmp.valve.general | Y | 9 | `valve.cryogenic_shutoff` (slot `cryo_valve`) |
| cmp.pressure_regulator | F2 | | Y | 9 | `regulator.pressure` |
| cmp.qd_fluid | F2, F5 | | Y | 9 | `coupling.quick_disconnect_fluid`, `umbilical_connector` fluid side |
| cmp.fluid_pump | F2, F7 | | Y | 9 | `pump.centrifugal`, `coolant_pump.reactor_loop` (radiation-tolerant at F10 level) |
| cmp.heat_exchanger_compact | F3 | prc.vacuum_braze | Y | 9 | `heat_exchanger.plate`, `heat_exchanger.shell_tube`, `humidity_condenser` |

### 2.9 Power

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.power.solar_array_rigid.{s,m,l} | F7, F5, F1 | a PV material, cmp.pv_laminate | Y | 9 | `power.solar_array_rigid` (l: ISS-class multi-panel wings) |
| cls.power.solar_array_flexible.{m,l} | F7, F5, F6 | cls.mechanism.boom.m, mat.polyimide_film | Y | 9 | `power.solar_array_flexible` (ROSA, iROSA) |
| mat.pv.silicon | F7 | | Y | 9 | Si cells (cheap, ~22%) |
| mat.pv.iii_v_multijunction | F7 | prc.iii_v_epitaxy | Y | 9 | triple-junction GaAs/Ge cells; moves the efficiency limit from S-Q to the multi-junction limit |
| mat.pv.imm_thin | F7, F5 | mat.pv.iii_v_multijunction | Y | 7 | inverted metamorphic thin cells (high W/kg) |
| cmp.pv_laminate | F7, F6 | a PV material | Y | 9 | `solar_cell.photovoltaic_multijunction` (cell laminated with coverglass) |
| cls.power.battery_housing | F4, F3 | | Y | 9 | `power.battery_module_housing` |
| cmp.battery_pack | F4, F7 | a cell chemistry, prc.li_cell_fab | Y | 9 | `battery.cell_pack` |
| mat.cell.li_ion | F4 | | Y | 9 | Li-ion (NMC/NCA) cells; high-power variants for e-pumps |
| mat.cell.lfp | F4 | | Y | 9 | LFP cells (long life, lower Wh/kg) |
| cmp.bms | F7, F15 | | Y | 9 | `battery.management_unit`, `charge_controller_mppt` |
| cls.power.fuel_cell_module | F4, F3 | cmp.pem_stack, mat.prop.lh2 | Y | 9 | `power.fuel_cell_module` |
| cmp.pem_stack | F4 | | Y | 9 | `fuel_cell.pem_stack` (alkaline is Y historical, same node) |
| cls.power.rtg | F10, F3, F7 | cmp.gphs, cmp.thermoelectric | Y | 9 | `power.rtg` |
| cmp.gphs | F10 | prc.pu238_production | Y | 9 | `radioisotope_heat_source`, `radioisotope_heat_unit` |
| cmp.thermoelectric | F7, F3 | | Y | 9 | `thermoelectric_module` |
| alt.rtg.stirling | F3, F10 | cmp.stirling_convertor | Y | 6 | ASRG-type generator (4x the efficiency, moving parts) |
| cmp.stirling_convertor | F3, F5 | | Y | 6 | `generator.stirling_convertor` |
| cls.power.fission_surface.{s,m} | F10, F3, F5 | cmp.reactor_core, cmp.reactor_control, one converter | Y | 5 / 4 | `power.fission_reactor_surface` (s: 1-10 kWe, KRUSTY; m: 40 kWe FSP class) |
| cmp.reactor_core | F10, F5 | prc.leu_haleu_fuel_fab | Y | 5 | `reactor_core.fission` |
| cmp.reactor_control | F10 | | Y | 5 | `control_rod_drive` (rods or drums) |
| cmp.brayton_tac | F3, F2 | cmp.heat_exchanger_compact | Y | 5 | `generator.brayton_turboalternator` |
| cmp.power_conditioning | F7 | | Y | 9 | `dc_dc_converter`, `power_conditioning_unit`, `bus_regulator_shunt`, `avionics.power_distribution_unit` |
| cmp.slip_ring | F7, F5 | | Y | 9 | `slip_ring.rotary` |
| cmp.electromagnet_coil | F8, F7 | | Y | 9 | `magnet_coil.electromagnet` (slot `coil_winding_core`) |
| cmp.hts_coil | F7, F3 | F7>REBCO tape; a cryocooler | Y | 4 | `magnet_coil.hts_superconducting` (lab magnets; never flown) |

### 2.10 Thermal

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.thermal.radiator.{s,m,l} | F3, F5 | cmp.heat_pipe or a pumped loop | Y | 9 | `thermal.radiator_panel` |
| cls.thermal.deployable_radiator | F3, F5 | cls.thermal.radiator.m, cmp.lhp | Y | 9 | `thermal.deployable_radiator` |
| cls.thermal.droplet_radiator | F3, F2, F9 | | N | 3 | `thermal.droplet_radiator`, unrowed `droplet_generator`/`droplet_collector` |
| cls.thermal.coldbox | F3, F7 | a cryocooler | Y | 9 | `thermal.coldbox_cryo` |
| cls.thermal.heatshield_ablative.{s,m,l} | F2, F5, F3 | an ablator material | Y (l: N) | 9 / 9 / 4 | `thermal.heatshield_ablative` (s: sample capsules; m: Dragon, Orion; l: > 5.5 m) |
| alt.heatshield.tiled_blocks | F5, F6 | cls.thermal.heatshield_ablative.m | Y | 9 | block or tile ablator layup (Orion block AVCOAT, PICA tiles) instead of monolithic |
| cls.thermal.reusable_tps.{m,l} | F2, F5 | mat.silica_tile or mat.carbon_carbon | Y | 9 | `thermal.heatshield_reusable_tps` (Shuttle; Starship tiles are P SpaceX designs on a Y node) |
| cmp.heat_pipe | F3 | mat.coolant_loop | Y | 9 | `heat_pipe.fixed_conductance` |
| cmp.lhp | F3 | cmp.heat_pipe | Y | 9 | `heat_pipe.variable_conductance_lhp` |
| cmp.cryocooler.stirling | F3, F5 | | Y | 9 | `cryocooler.stirling`, `focal_plane_cooler` |
| cmp.cryocooler.pulse_tube | F3 | | Y | 9 | `cryocooler.pulse_tube` (JWST MIRI uses a hybrid) |
| cmp.louver | F3, F5 | | Y | 9 | `louver.thermal` |

### 2.11 Comms

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.comms.transponder | F8, F15 | cmp.rf_electronics | Y | 9 | `comms.transponder_unit` (should be a component: M3) |
| cls.comms.dish.{s,m,l} | F8, F5 | mat.cfrp_im | Y | 9 | `comms.antenna_dish_parabolic` |
| cls.comms.omni_patch | F8 | | Y | 9 | `comms.antenna_omni_patch` (slot `rf_feed_horn` rides it) |
| cls.comms.laser_terminal | F8, F15, F7 | cmp.laser_transmitter, cmp.fine_steering_mirror | Y | 9 | `comms.laser_terminal_optical` (LEO crosslinks 9; deep space DSOC 7) |
| cls.comms.relay_crosslink | F8, F15 | cmp.sdr | Y | 9 | `comms.relay_crosslink_unit` (should be a component: M3) |
| cmp.rf_electronics | F8, F7 | | Y | 9 | `rf_frontend_card`, unrowed `power_amplifier_rf`, `rf_transceiver_card` |
| cmp.sdr | F8, F15 | | Y | 9 | `software_defined_radio_card` |
| cmp.laser_transmitter | F8, F7 | | Y | 9 | unrowed `laser_diode_module` |
| cmp.fine_steering_mirror | F8, F1 | | Y | 9 | unrowed `fine_steering_mirror` |

### 2.12 GNC sensors and actuators

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.gnc.star_tracker | F1, F8, F13 | cmp.detector_array | Y | 9 | `gnc.star_tracker` (slot `baffle_stray_light` is trivial) |
| cls.gnc.sun_sensor | F1, F7 | | Y | 9 | `gnc.sun_sensor` |
| cls.gnc.horizon_sensor | F1, F8 | cmp.detector_array | Y | 9 | `gnc.horizon_sensor` |
| cls.gnc.imu | F1, F7 | one cmp.gyro | Y | 9 | `gnc.imu_inertial` (should be a component: M3) |
| cmp.gyro.{mems,fog,rlg,hrg} | F1, F7, F8 | | Y | 9 | `gyroscope_element` by type: bias stability bands differ in kind |
| cls.gnc.gnss_receiver | F1, F8, F15 | cmp.rf_electronics | Y | 9 | `gnc.gnss_receiver` (should be a component: M3) |
| cls.gnc.ranging | F1, F8 | cmp.rf_electronics or cmp.laser_transmitter | Y | 9 | `gnc.radar_altimeter_lidar` |
| alt.ranging.flash_lidar | F8, F1 | cls.gnc.ranging | Y | 8 | 3D flash lidar for hazard detection and rendezvous |
| cls.gnc.magnetic_torquer | F1, F8 | cmp.electromagnet_coil | Y | 9 | `gnc.magnetic_torquer` |
| cmp.reaction_wheel | F1, F5 | | Y | 9 | `gnc.reaction_wheel` |
| cmp.cmg | F1, F5 | cmp.reaction_wheel | Y | 9 | control moment gyros (ISS, WorldView): no catalog row (M4) |

### 2.13 Mechanisms and separation

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.mechanism.boom.{s,m} | F5, F6 | cmp.mechanism_drive | Y | 9 | `mechanism.deployable_boom` (m: 10-30 m, ROSA/ACS3 class) |
| alt.boom.coilable | F5 | cls.mechanism.boom.s | Y | 9 | STEM, Bi-STEM, composite tape-spring booms |
| cls.mechanism.robotic_arm.{s,m,l} | F1, F5, F15 | cmp.mechanism_drive | Y | 9 | `mechanism.robotic_arm` (s: sampling arms; l: Canadarm2) |
| cmp.mechanism_drive | F5, F7 | | Y | 9 | `drive_motor.mechanism`, `electromechanical_actuator`, `joint_actuator.robotic`, `bolt_actuator`, `motor_wheel_hub`, `tether_winch`, `motor_winding` (internal) |
| cmp.end_effector | F5, F15 | cmp.mechanism_drive | Y | 9 | `end_effector_tool` |
| cmp.pyro_release | F4, F5 | prc.pyro_device_mfg | Y | 9 | `separation_bolt.pyrotechnic`, `frangible_nut`, `pyro_cutter.cable`, `bolt_cutter.pyro`, `valve.pyro_normally_closed`, `valve.pyro_normally_open`, `harpoon_pyro`, `gas_generator_inflator`, unrowed `mortar_pyro`, `pyro_charge` |
| cmp.clamp_band | F5 | | Y | 9 | `clamp_band.marman` |
| cmp.pneumatic_release | F2, F5 | | Y | 9 | pneumatic latches and pushers (settled 22; F9 stage separation): no catalog row (M4) |

### 2.14 Recovery and entry

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.recovery.parachute.{s,m,l} | F2, F5 | prc.textile_canopy | Y | 9 | `recovery.parachute_canister` (s: drogues; m: mains to ~35 m; l: larger, Ares-class) |
| alt.chute.parafoil | F2, F1 | cls.recovery.parachute.s | Y | 9 | ram-air guided canopy on `recovery.parachute_canister` (X-38, fairing recovery; no catalog type, M4) |
| cls.recovery.drogue_mortar | F5, F4 | cmp.pyro_release | Y | 9 | `recovery.drogue_mortar` (duplicates the parachute's `mortar_pyro` slot: M5) |
| cls.recovery.airbag | F2, F5 | mat.vectran_fabric, cmp.pyro_release | Y | 9 | `recovery.airbag_landing_system` |
| cls.recovery.landing_legs.{s,m,l} | F5, F1 | cap.propulsive_landing | Y | 9 | `recovery.retro_landing_legs` |
| cls.recovery.hiad.{m,l} | F2, F5 | mat.flexible_tps, cmp.hiad_torus | Y | 7 / 4 | `recovery.inflatable_decelerator_hiad` (m: LOFTID 6 m) |
| cmp.hiad_torus | F5, F2 | mat.vectran_fabric | Y | 7 | `torus_bladder`, `tps_flexible_blanket` |
| cls.recovery.ballute | F2, F5 | prc.textile_canopy | Y | 4 | `recovery.ballute` |

### 2.15 Crew and habitat

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.crew.capsule_shell | F5, F14, F3 | | Y | 9 | `crew.pressurized_module_shell` as a capsule |
| cls.crew.module_shell.{m,l} | F5, F14 | | Y | 9 | the same type as a station module (m: ISS 4.2 m; l: Skylab 6.6 m) |
| alt.crew.inflatable_module | F5, F14 | mat.vectran_fabric | Y | 7 | soft-goods modules (BEAM): no catalog type of its own (M4) |
| cls.crew.life_support_rack | F4, F14, F3 | | Y | 9 | `crew.life_support_rack` (a rack inside a module: should be a component, M3) |
| cmp.co2_removal.{lioh,amine_swing,zeolite} | F4, F14 | | Y | 9 | `co2_scrubber` by method |
| cmp.o2_electrolysis | F4 | | Y | 9 | `o2_generator` |
| cmp.water_recovery | F4, F14 | | Y | 9 | `water_recovery_unit` (98% closure at ISS, 2023) |
| cls.crew.seat | F14, F5 | | Y | 9 | `crew.seat_couch` |
| cls.crew.airlock | F14, F5 | cmp.hatch | Y | 9 | `crew.airlock_module` |
| cls.crew.eva_suit | F14, F5, F3 | cmp.plss | Y | 9 | `crew.eva_suit` |
| cmp.plss | F14, F4, F3 | | Y | 9 | `portable_life_support_pack` |
| cmp.hatch | F5 | | Y | 9 | `hatch_mechanism` |
| cmp.pressure_window | F5, F8 | mat.optical_glass_lowcte | Y | 9 | `window_pane.pressure`, unrowed `visor_optical` |
| cls.crew.radiation_shelter | F14, F10, F5 | mat.shield_hydrogenous | Y | 8 | `crew.radiation_shelter` |
| cls.crew.centrifuge_habitat | F14, F5, F1 | cmp.slip_ring, continuous rotary mount (future G3) | N | 2 | `crew.centrifuge_habitat` |

### 2.16 Science instruments

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.instrument.optical.{s,m,l} | F8, F13, F12 | cmp.detector_array; a mirror material | Y | 9 | `instrument.imaging_camera` by aperture (s < 0.3 m; m to 2.4 m; l segmented) |
| alt.optics.segmented_deployable | F8, F5, F1 | cls.instrument.optical.m, cmp.mechanism_drive | Y | 9 | segmented, deployed primaries (JWST) |
| cmp.detector_array.{visible,ir,bolometer} | F7, F8 | prc.iii_v_epitaxy for IR | Y | 9 | `optical_detector_array` by band; unrowed `photodiode_array` rides `visible` |
| cls.instrument.spectrometer | F8, F12, F4 | cmp.detector_array | Y | 9 | `instrument.spectrometer` |
| cls.instrument.mass_spectrometer | F4, F6, F11 | cmp.mass_spec_core | Y | 9 | `instrument.mass_spectrometer` |
| cmp.mass_spec_core | F11, F4 | | Y | 9 | `ion_source`, `mass_analyzer`, `electron_multiplier_detector` |
| cls.instrument.magnetometer | F8, F12, F13 | cmp.fluxgate | Y | 9 | `instrument.magnetometer` |
| cmp.fluxgate | F8 | | Y | 9 | `fluxgate_sensor_element` |
| cls.instrument.particle_detector | F11, F10, F14 | cmp.particle_sensing | Y | 9 | `instrument.particle_detector` |
| cmp.particle_sensing | F11, F7 | | Y | 9 | `solid_state_detector`, `scintillator_element`, `photomultiplier_tube` |
| cls.instrument.radar_sounder | F8, F12 | cmp.rf_electronics, cls.mechanism.boom.s | Y | 9 | `instrument.radar_sounder`, `antenna_dipole_deployable` |
| cls.instrument.sample_arm | F5, F12, F15 | cmp.end_effector | Y | 9 | `instrument.sample_collection_arm` |
| cmp.sample_seal | F5, F12 | | Y | 9 | `sample_canister_seal` |
| cls.instrument.atmospheric_suite | F12, F2 | | Y | 9 | `instrument.atmospheric_sensor_suite` |

### 2.17 Docking and landing

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.docking.androgynous | F1, F5 | cmp.capture_latch, cap.auto_rendezvous_docking | Y | 9 | `docking.port_androgynous` (IDSS and APAS are standards on it: open question in M10) |
| cls.docking.probe_drogue | F1, F5 | cmp.capture_latch | Y | 9 | `docking.port_probe_drogue` |
| cls.docking.berthing | F1, F5, F15 | cmp.mechanism_drive, cls.mechanism.robotic_arm.l | Y | 9 | `docking.berthing_mechanism` |
| cmp.capture_latch | F5, F1 | | Y | 9 | `capture_latch` |
| cls.landing.leg_fixed | F5, F1 | | Y | 9 | `landing.leg_fixed` |
| cls.landing.skid | F5 | | Y | 9 | `landing.skid_runner` |
| cls.landing.wheel_gear | F5 | | Y | 9 | `landing.wheel_gear` |
| cls.landing.rover_wheel | F5, F12, F6 | | Y | 9 | `landing.rover_wheel` (soil interaction: granular gap, M8) |
| cls.landing.rocker_bogie | F5, F12 | | Y | 9 | `landing.suspension_rocker_bogie` |
| cls.landing.anchor | F5, F12 | cmp.pyro_release | Y | 7 | `landing.anchoring_harpoon_foot` (Philae: flown, failed) |
| cls.landing.descent_stage.{s,m} | F5, F1 | alt.engine.deep_throttle | Y | 9 | `landing.terminal_descent_mount` |

### 2.18 Avionics and data

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cmp.flight_computer | F15, F7 | | Y | 9 | `avionics.flight_computer_unit`, `flight_computer_card`, `watchdog_timer`, `avionics.subsystem_controller` |
| alt.computer.radhard | F7, F15 | cmp.flight_computer, prc.radhard_fab | Y | 9 | rad-hard parts (RAD750, HPSC from ~2027) |
| alt.computer.cots_redundant | F15, F7 | cmp.flight_computer | Y | 9 | voting COTS processors (Falcon, Dragon) |
| cmp.memory_radhard | F7 | prc.radhard_fab | Y | 9 | `memory_module_radhard`, `solid_state_recorder` |
| cmp.data_handling | F15, F7 | | Y | 9 | `avionics.data_handling_unit`, `fpga_card.processing`, `processing_card.instrument`, `avionics.sensor_interface_unit` |

### 2.19 Capabilities (planner, guidance, sensing, operations)

These enable code features, not types (finding M9). World-known ones are on by default for anyone who has licensed or researched them.

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cap.gravity_assist_planning | F1 | | Y | 9 | multi-flyby planner |
| cap.low_thrust_optimisation | F1, F15 | | Y | 9 | low-thrust trajectory solver |
| cap.aerobraking | F1, F2 | | Y | 9 | aerobraking corridors in the planner |
| cap.aerocapture | F1, F2 | cap.aerobraking | N | 5 | single-pass capture (never flown) |
| cap.skip_entry | F1, F2 | | Y | 9 | lifting/skip entry guidance (Apollo, Orion) |
| cap.auto_rendezvous_docking | F1, F15, F8 | cls.gnc.ranging | Y | 9 | autonomous RPO and docking |
| cap.formation_flying | F1, F15 | cap.auto_rendezvous_docking | Y | 8 | precision formation (PRISMA, GRACE-FO) |
| cap.terrain_relative_nav | F1, F15, F12 | cmp.detector_array | Y | 9 | map-relative landing (Mars 2020, SLIM) |
| cap.hazard_avoidance | F1, F15 | alt.ranging.flash_lidar | Y | 9 | onboard hazard detection and divert |
| cap.propulsive_landing | F1, F2, F15 | alt.engine.deep_throttle | Y | 9 | booster return and landing guidance |
| cap.supersonic_retropropulsion | F2, F1 | cap.propulsive_landing | Y | 9 | retro-burns in supersonic flow (SpaceX data shared with NASA) |
| cap.optical_navigation | F1, F15 | cmp.detector_array | Y | 9 | onboard optical nav (AutoNav, DART) |
| cap.storable_transfer | F2, F1 | cap.auto_rendezvous_docking, cmp.qd_fluid | Y | 9 | storable propellant transfer (Progress to ISS) |
| cap.cryo_transfer | F3, F2, F1 | cap.storable_transfer, cmp.pmd | P (SpaceX) | 6 | in-space cryogenic transfer (Starship demo; assumes it has flown by 2030) |
| cap.tower_catch | F1, F15 | cap.propulsive_landing | P (SpaceX) | 9 | booster catch by tower (needs the near catalog's `structure.catch_ring_interface`) |
| cap.fdir | F15 | | Y | 9 | onboard fault detection, isolation, recovery (layer-2 programs) |
| cap.ldpc_coding | F15, F8 | | Y | 9 | near-Shannon channel coding (CCSDS) |
| cap.dtn | F15 | | Y | 7 | delay-tolerant store-and-forward networking |
| cap.delta_dor | F1, F8 | | Y | 9 | precise ground tracking (only with ground stations) |
| cap.optical_deep_space_link | F8, F15 | cls.comms.laser_terminal | Y | 7 | deep-space optical comms (DSOC, 2023) |

### 2.20 Present-day placeholders in the present catalog

| id | fields | prereqs | 2030 | TRL | enables |
|---|---|---|---|---|---|
| cls.engine.ntr | F10, F3, F5 | cmp.ntr_fuel_element, cmp.turbopump, mat.prop.lh2 | Y (historical) | 5 | `engine.nuclear_thermal_rocket` (NERVA ground tests; DRACO ended 2025) |
| cmp.ntr_fuel_element | F10, F5 | prc.leu_haleu_fuel_fab, F5 > 2,500 K service | Y | 5 | NTR cores |
| cls.power.fusion_core | F9, F10, F11 | see 3.2 | N | 2 | `power.fusion_reactor_core` and `fuel_pellet_injector`, `first_wall_panel` |
| cls.sail.photon.{s,m} | F8, F5, F1 | cls.mechanism.boom.m, mat.polyimide_film | Y | 7 | `power.sail_photon_solar`, `membrane_reflective_film`, `tensioning_cable` (IKAROS 200 m², ACS3 80 m²) |
| cls.sail.magnetic | F9, F7 | cmp.hts_coil | N | 2 | `power.sail_magnetic_plasma` |
| cls.propulsion.sep_cluster_mount | F9, F7, F1 | cls.engine.hall.m | Y | 7 | `propulsion.sep_thruster_cluster_mount` (Gateway PPE) |
| (via mat.shield_neutron_gamma) | F10 | | | | `neutron_shield` |

### 2.21 Outside the node system: GSE

The `gse.*` family (`gse.umbilical_connector_plate`, `gse.fill_drain_panel`, `gse.ground_power_interface`, `gse.transporter_erector_interface`, `gse.access_arm_interface`) and their own components `hold_down_release`, `release_actuator`, `quick_disconnect_electrical` have no nodes. They are bought or built for money and mass only, always available, never gated (catalog section 4). The vehicle side still uses gated components where they are shared: a fluid QD on a fill-drain panel uses `cmp.qd_fluid` because the same QD is used on docking ports. The pad and tower (rule 24) have no node either; their size follows the vehicle's diameter regime.

### 2.22 Field levels only (trivial components, rule 20)

No node. Ranges follow the named fields. Grouped by graph:

- **Fluid (F2, F5):** `line.tube_rigid`, `line.flex_hose`, `line.bellows_compensator`, `conduit.feed_tunnel`, `orifice.flow_restrictor`, `filter.inline_mesh`, `diffuser.pressurant`, `baffle.antislosh`, `pressure_transducer`, `level_sensor_cryo` (F3), `pressure_relief_valve.habitat`, `depress_valve`.
- **Thermal (F3, F5):** `mli_blanket.multilayer`, `insulation.foam_rigid`, `insulation.aerogel`, `heater.resistive_patch`, `thermostat.bimetallic_or_solid_state`, `radiator_coating.optical_surface`, `thermal_strap.flexible_copper`, `phase_change_material_pack`, `coldplate.component_mount`, `vacuum_getter_pump`, `strain_isolation_pad`, `gap_filler.tps`. `ablator_tile.charring` and `tps_tile.ceramic_reusable` are not components at all but material zones on the heatshield's cells, gated by the `mat.` ablator and tile nodes (M5).
- **Power (F7):** `bypass_diode`, `blocking_diode`, `fuse.power`, `relay.power_switch`, `current_sensor`, `wiring_harness.power_bundle`.
- **Data (F15, F7):** `data_bus_node`, `adc_card`, `signal_conditioning_card`, `encoder_position`, `tachometer`, `dosimeter` (F14).
- **Structural (F5):** `fastener.bolted_joint`, `bracket.mounting`, `damper.vibration_isolator`, `spring.preload_or_release`, `torsion_spring`, `latch_mechanism`, `position_sensor`, `joint.hinge_pin`, `bearing.rotary_assembly`, `accelerometer_element` (F7), `crush_core.honeycomb`, `load_limiter_strut`, `contact_sensor`, `shock_strut`, `brake_unit`, `tire_pneumatic`, `suspension_strut`, `pivot_bearing`, `differential_linkage`, `load_cell`, `gimbal_bearing_ring`, `insert.potted`, `doubler.reinforcement`.
- **Instrument (F8):** `filter_wheel`, `diffraction_grating`, `calibration_lamp`, `anemometer_element`.
- **Crew and docking (F14, F5):** `restraint_harness`, `suit_umbilical_port`, `alignment_guide`, `seal_ring`, `umbilical_connector`, `radiation_shield_mass` (its material is the node).
- **Unrowed slot names that are trivial:** `acoustic_blanket`, `baffle_stray_light`, `strain_gauge`, `thermocouple_sensor`, `pressure_switch`, `pilot_chute`, `riser_line`, `piston`, `joint_bearing`, `rotary_bearing_large`.

### 2.23 Counts (present day)

339 nodes (284 table rows, regimes expanded): **133 class/regime**, 33 alternative design, 53 material (17 of them propellants and fluids), 76 component, 24 process, 20 capability. World-known at 2030: 327 Y, 7 P (SpaceX 6, Rocket Lab 1), 5 N (droplet radiator, centrifuge habitat, fusion core, magnetic sail, aerocapture). They cover all 90 part types (85 by node, 5 GSE outside) and all 184 component rows (the catalog says 181; it has 184, M2): 96 by a node, 88 by field levels only.



## 3. Near, mid and far nodes

Route column: **W** world, **R** lab project, **RF** lab project plus an in-flight measurement or observed part behaviour, **O** unlocked instantly by an observation. Every RF and O node names the data it needs; those are the exploration bias of 5.1 principle 4.

### 3.0 Near-catalog types that are already real by 2030

Thirteen "near" types in the future catalog (9 parts, 4 components) are flying today. They belong in the 2030 set (section 2) as world-known or proprietary nodes, not as near-era unlocks: `aero.grid_fin_control_surface` (Y, Falcon 9 since 2014), `structure.catch_ring_interface` + `catch_pin_latch` (P SpaceX, with `cap.tower_catch`), `structure.reusable_booster_base_heat_shield` (P SpaceX), `cryo_densification_unit` (P SpaceX, subcooled loading since 2015), `engine_relight_pyrophoric_slug` (Y, = `mat.prop.pyrophoric_tea_teb`), `whipple_shield_standoff_bumper` and `thermal.cryo_tank_micrometeoroid_shield` (Y, ISS), `landing.sample_return_capsule_ballistic` (Y, = `cls.thermal.heatshield_ablative.s`), `landing.comet_low_gravity_lander_bus` (Y, Philae), `isru.atmospheric_co2_acquisition_unit` (Y TRL 6, MOXIE), `comms.antenna_mesh_deployable_large` with `rib_actuator` and `mesh_tensioning_cable` at 12-22 m (Y, AstroMesh on Thuraya and SkyTerra) and `aerostat.venus_balloon_platform` (Y historical, Vega 1985). Proposed new 2030 nodes: `cls.aero.grid_fin`, `cls.structure.catch_ring`, `cmp.cryo_densification`, `cls.shield.whipple`, `cls.comms.mesh_reflector.l`, `cls.aerostat.superpressure`. With these the 2030 set is ~345.

### 3.1 Near era (2035-2060)

| id | fields | prereqs | route (data needed) | enables |
|---|---|---|---|---|
| cap.zbo_storage | F3 | cmp.cryocooler.turbo_brayton, cmp.boiloff_recondenser | RF (a year of LH2 in orbit below 1% loss) | zero-boil-off operating mode; depot tank ratings |
| cmp.cryocooler.turbo_brayton | F3, F2 | cmp.cryocooler.pulse_tube | R | `cryocooler.reverse_turbo_brayton` (20 K, 100s of W) |
| cmp.boiloff_recondenser | F3 | cmp.heat_exchanger_compact | R | `boiloff_recondenser` |
| cmp.rf_mass_gauge | F8, F2 | | RF (gauging in microgravity) | `propellant_mass_gauging_sensor` |
| cmp.cryo_transfer_coupling | F3, F2 | cmp.qd_fluid, cap.cryo_transfer | R | `coupling.cryogenic_transfer` |
| cls.tank.depot.xl | F3, F5 | cls.tank.cryo_vacuum_jacketed.m, cap.zbo_storage, cls.mfg.robotic_assembly | R | `tank.depot_cryo_storage` (built in orbit) |
| cls.docking.prop_transfer_port | F1, F3, F5 | cls.docking.androgynous, cmp.cryo_transfer_coupling | R | `docking.propellant_transfer_port`, `docking.propellant_transfer_boom_assembly` |
| cls.structure.depot_bus | F7, F15 | cls.structure.equipment_bay | W | `structure.depot_station_core_bus` |
| cls.thermal.sunshield | F3, F5 | mat.polyimide_film, deformable family (rule 8) | R | `thermal.sunshield_deployable`, `layer_separator_standoff` |
| cls.structure.deployable_truss.l | F5, F6 | cls.mechanism.boom.m | R | `structure.deployable_truss_mast`, `structure.deployable_platform_truss_hub` |
| cls.structure.inflatable_rigidizable | F5, F6 | alt.crew.inflatable_module | RF (cure behaviour in vacuum) | `structure.inflatable_rigidizable_structure`, `rigidization_cure_system` |
| cls.mfg.robotic_assembly | F5, F15, F1 | cls.mechanism.robotic_arm.l, cmp.metrology | R | `manufacturing.robotic_assembly_platform` |
| cmp.metrology | F8, F15 | | W | `metrology_scanner`, `heat_shield_tile_inspection_sensor` |
| prc.in_space_am | F6, F5 | prc.ded_large_am | RF (melt-pool behaviour in microgravity) | in-orbit print recipe step |
| cls.mfg.printer | F6, F15 | prc.in_space_am | R | `manufacturing.in_space_additive_printer`, `print_head_extruder`, `metal_wire_feeder`, `powder_bed_recoater`, `build_plate_positioner` |
| cls.mfg.feedstock_store | F6 | | W | `manufacturing.feedstock_storage_module` (`feedstock_spool`, `hopper_gate` trivial) |
| cmp.regolith_handling | F5, F12 | | RF (per-body geotechnical data) | `regolith_hopper_feed`, `regolith_conveyor_belt`, `regolith_sieve_screen` |
| cls.isru.excavator | F5, F12, F6 | cls.landing.rover_wheel, cmp.regolith_handling | R | `isru.regolith_excavator_rover` |
| prc.molten_regolith_electrolysis | F4, F6, F3 | cmp.electrolysis_stack, cmp.induction_heater | RF (in-situ regolith run) | O2 and metal recipe steps on a body |
| cmp.electrolysis_stack | F4 | cmp.o2_electrolysis | R | `electrolysis_cell_stack`, `water_electrolyzer_isru` |
| cmp.induction_heater | F7, F3 | | W | `induction_heater_element` |
| cls.isru.mre_plant | F4, F6 | prc.molten_regolith_electrolysis | R | `isru.molten_regolith_electrolysis_plant` |
| prc.sabatier_isru | F4, F6 | cmp.methanation_bed, cmp.co2_freeze_capture | RF (first methane from Martian CO2) | CH4 + H2O recipe step on Mars |
| cmp.methanation_bed | F4 | | W | `methanation_reactor_bed`, `propellant_synthesis_reactor` |
| cmp.co2_freeze_capture | F3, F4 | | R | `co2_freeze_capture_unit` |
| cls.isru.sabatier_plant | F4, F6 | prc.sabatier_isru | R | `isru.sabatier_reactor_plant`, `mining.propellant_production_plant_asteroid` |
| cls.isru.ice_mining | F4, F12, F6 | cmp.drill | O (lunar or Martian ice measured in situ) then R | `isru.water_ice_mining_rig`, `landing.lunar_polar_ice_lander` |
| cmp.drill | F5, F12 | cmp.mechanism_drive | W | `extraction_auger_bit`, `drill_bit_rotary_percussive` |
| prc.regolith_sintering | F6, F5 | | RF (in-situ sintering) | `isru.lunar_regolith_sintering_paver`, `regolith_sintering_microwave_emitter` |
| cls.aero.lifting_surface | F2, F5 | | W | `aero.wing_fixed`, `aero.control_surface_moving`, `aero.fuselage_pod_streamlined` |
| cls.aero.rotor | F2, F5 | continuous rotary mount (schema G3) | W (Dragonfly at Titan, 2034) | `aero.propeller_rotor_blade_assembly`, `blade_pitch_actuator`, `rotor_swashplate_control`, `aircraft.titan_rotorcraft` |
| cls.aerostat.superpressure | F2, F12, F5 | mat.ptfe_envelope, buoyancy domain (G5) | W (Vega heritage) | `aerostat.venus_balloon_platform`, `balloon_envelope_ptfe`, `altitude_control_valve`, `balloon_gondola_suspension_rig` |
| mat.ptfe_envelope | F5, F4 | | W | acid-proof envelopes |
| cls.probe.cryobot | F3, F12 | cmp.melt_tip, cmp.tether_reel | O (ice shell thickness measured from orbit) then R | `probe.cryobot_ice_penetrator`, `ice_shell_depth_sensor` |
| cmp.melt_tip | F3, F10 | cmp.gphs | R | `melting_tip_heater_rtg` |
| cmp.tether_reel | F5, F15 | cmp.mechanism_drive | R | `tether_reel_communication`, `tether_deployment_reel_large` (later) |
| cls.probe.hydrobot | F12, F14, F2 | cls.probe.cryobot | O (subsurface ocean sampled) | `probe.hydrobot_underwater_explorer`, `acoustic_modem` |
| cls.probe.giant_entry | F2, F12, F5 | mat.carbon_phenolic (requalified), cls.recovery.parachute.m | RF (target's atmosphere profile at 10% tier) | `landing.gas_giant_atmospheric_entry_probe`, `pressure_crush_sensor` |
| alt.lander.radiation_hard | F7, F10 | alt.computer.radhard, F7 > 1 Mrad | RF (Jovian dose measured at the target) | `landing.icy_moon_static_lander_bus` |
| alt.lander.thermal_cycling | F5, F3 | | R | `landing.mercury_thermal_lander` |
| cls.mining.anchor_platform | F5, F12 | cls.landing.anchor | RF (surface strength of the body measured) | `mining.asteroid_anchor_platform`, `mining.regolith_bagging_containment`, `net_closure_winch` |
| cls.mining.volatile_drill | F4, F12 | cmp.drill | R | `mining.volatile_extraction_drill_rig`, `volatile_capture_bag` |
| prc.asteroid_refining | F4, F6 | cmp.electrolysis_stack | RF (microgravity melt and separation) | `mining.ore_refining_plant`, `ore_crusher_mill`, `ore_assay_sensor` |
| cls.habitat.spin_ring | F14, F5, F1 | cls.crew.module_shell.l, cap.partial_g_tolerance, rotary mount (G3) | R | `habitat.rotating_ring_module`, `habitat.spin_hub_nonrotating`, `despin_drive_motor` |
| cap.partial_g_tolerance | F14 | | RF (crew or animal data at 0.16-0.5 g) | artificial-gravity requirement curves |
| cls.habitat.tethered_spin | F14, F1, F5 | cap.tether_dynamics | R | `habitat.counter_rotating_tether_mass`, `tether_climber_grapple` |
| cap.tether_dynamics | F1, F5 | | RF (a flown tether's dynamics) | tether planner and solver |
| cls.power.solar_array_rigid.xl | F7, F5 | cls.mfg.robotic_assembly | R | `power.sbsp_collector_module`, `power.sbsp_reflector_mirror_element` |
| cls.power.beam_transmitter | F8, F7 | cmp.rf_power_source, cmp.phased_array | RF (first beamed-power test in orbit) | `power.microwave_power_transmitter_array`, `beam_pointing_control_unit` |
| cmp.rf_power_source | F8 | | W | `klystron_magnetron_module` |
| cmp.phased_array | F8, F15 | | W | `phase_control_rf_element`, `optical_cross_connect_switch` |
| cap.inspace_reactor_ops | F10, F3 | cls.power.fission_surface.s | RF (first reactor critical in space) | start, run and restart a reactor in flight |
| cls.power.fission_space.{l,xl} | F10, F3, F5 | cap.inspace_reactor_ops, cls.power.fission_surface.m, cmp.brayton_tac, cls.thermal.radiator.xl | R | `power.fission_reactor_space_nep` |
| cls.thermal.radiator.xl | F3, F5 | cls.thermal.deployable_radiator | R | `thermal.radiator_panel_megawatt_class` |
| cls.docking.berthing.l | F1, F5 | cls.docking.berthing | W | `docking.cargo_berthing_standard_large` |
| alt.engine.rotating_detonation | F2, F4 | | R | RDE alternative for `engine.liquid_bipropellant` (enables no type: orphan, M11) |
| mat.cell.solid_state | F4 | mat.cell.li_ion | W | battery Wh/kg limit raised (moves the S-curve's ceiling) |
| mat.pv.perovskite_tandem | F7 | mat.pv.silicon | RF (10-year radiation stability in orbit) | cheap high-efficiency cells |
| mat.cfrp_cryo.l | F5, F3 | mat.cfrp_cryo | RF (first large composite cryotank flown) | composite cryotanks at the l regime |
| cap.autonomous_cruise | F15, F1 | cap.fdir, cap.optical_navigation | RF (a fault recovered 20 light-minutes out) | layer-2 programs with weeks without ground |

### 3.2 Mid and far eras: skeleton chains

Each chain names its trunk nodes; branches reuse section 2 and 3.1 nodes. "R+F" = a lab project that needs a flight measurement; "O" = observation unlock.

| id | era | fields | prereqs (the chain) | route | enables |
|---|---|---|---|---|---|
| mat.rebco_hts_large | mid | F7, F3 | cmp.hts_coil | W | `magnet_coil.hts_large_toroid`, `cryostat_structural`, `quench_protection_system` |
| cap.plasma_confinement | mid | F9 | cls.engine.mpd_vasimr.l, F9 level floor | R | steady confined plasma (shared by MCF and drives) |
| mat.first_wall | mid | F5, F10 | mat.refractory_re_ir | R | `first_wall_panel`, `divertor_heat_extraction_panel` |
| cmp.tritium_breeding | mid | F10, F4 | | R | `tritium_breeding_blanket` |
| cls.power.fusion_mcf | mid | F9, F10, F5, F3 | mat.rebco_hts_large, cap.plasma_confinement, mat.first_wall, cmp.tritium_breeding | R | `power.fusion_reactor_mcf`, `neutron_flux_monitor`, `plasma_confinement_sensor` |
| cmp.icf_driver | mid | F8, F11 | cmp.laser_transmitter, F8 laser-efficiency floor | R | `driver_laser_or_beam_module`, `fuel_pellet_injector` |
| cls.power.fusion_icf | mid | F9, F10, F11 | cmp.icf_driver, mat.first_wall | R+F (ICF pulse characteristics observed in orbit) | `power.fusion_reactor_icf_pulsed` |
| cmp.magnetic_nozzle | mid | F9 | mat.rebco_hts_large | R+F (a magnetic nozzle measured in vacuum) | `magnetic_nozzle_coil_assembly` |
| cls.engine.fusion_drive | mid | F9, F10, F3 | one fusion core node, cmp.magnetic_nozzle, cls.thermal.radiator.xl | R | `engine.fusion_pulsed_propulsion_drive` |
| mat.he3_resource | mid | F12, F10 | | O (He-3 measured in lunar regolith or a giant's atmosphere) | `facility.helium3_fuel_processing_plant`, `helium3_isotope_separator`, D-He3 fuel |
| cls.thermal.lm_mhd_radiator | mid | F3, F9 | cls.thermal.droplet_radiator, cmp.lm_pump | R+F (sheet flow in vacuum) | `thermal.liquid_metal_mhd_radiator`, `liquid_metal_pump_mhd` |
| cls.shield.active_magnetic | mid | F11, F9, F7 | mat.rebco_hts_large, shared-field graph (G10) | R+F (GCR flux measured inside a test field) | `shield.active_magnetic_radiation_shield`, `field_uniformity_sensor` |
| cls.launcher.mass_driver | mid | F7, F8, F5 | lunar base, cmp.power_conditioning at MW, guideway mount (G11) | R | `launcher.mass_driver_track_segment`, `launcher.payload_bucket_sled` and their 4 components |
| mat.ultra_fiber | mid | F5 | mat.aramid_fabric, F5 specific-strength floor | R | tether cables, `tether_cable_segment_splice` |
| cls.tether.rotovator | mid | F1, F5 | mat.ultra_fiber, cap.tether_dynamics | R+F (tip capture at km/s) | `tether.rotovator_facility`, `tether.momentum_exchange_tether_segment`, `tip_grapple_capture_mechanism`, `ballast_mass_block`, `ring.stabilization_thruster_pod` |
| mat.cnt_cable | mid | F5, F6 | mat.ultra_fiber | R | orbital-ring cable (moves F5's specific-strength ceiling) |
| cls.ring.orbital | mid | F5, F7, F8 | mat.cnt_cable, cls.launcher.mass_driver | R | `ring.orbital_ring_segment` and its 4 `ring.*` components |
| cls.power.laser_station | mid | F8, F7 | cls.power.beam_transmitter, cmp.icf_driver | R | `power.laser_beam_station_array`, `laser_amplifier_module`, `phased_optic_element`, `adaptive_optics_corrector`, `beam_safety_interlock_sensor` |
| cls.sail.beam_driven | mid | F8, F5 | cls.sail.photon.m, cls.power.laser_station | R+F (sail under beam) | `sail.beam_driven_sail`, `sail_thermal_management_coating` |
| cls.sail.magnetic.l | mid | F9, F7 | cls.sail.magnetic, mat.rebco_hts_large | R+F (solar-wind coupling measured) | `sail.magnetic_plasma_sail_large_interplanetary` |
| prc.antiproton_production | mid | F11, F10 | world physics (CERN) | W, then R for yield | `facility.antimatter_production_complex`, `antiproton_source`, `antiproton_decelerator_ring`, `positron_accumulator` |
| cmp.penning_trap_portable | mid | F11, F7 | prc.antiproton_production | R+F (antimatter stored for a year in flight) | `vessel.antimatter_storage_pod`, `penning_trap_coil`, `containment_breach_detector`, `antimatter_pod_radiation_monitor` |
| cap.bioregenerative_closure | mid | F14, F4 | cmp.water_recovery | R+F (a closed biology run in interplanetary space) | `crew.bioregenerative_life_support_module`, `habitat.agricultural_deck_module` and their 5 components |
| cls.structure.spine.xl | mid | F5, F1 | cls.mfg.robotic_assembly | R | `structure.ship_spine_truss_megastructure`, `docking.cargo_module_standard_interplanetary` |
| cls.habitat.generation | mid | F14, F5 | cls.habitat.spin_ring, cap.bioregenerative_closure | R+F (5 years of crew off Earth) | `habitat.generation_ship_segment` |
| cls.engine.antimatter_beam_core | far | F11, F9, F10 | cmp.penning_trap_portable, cmp.magnetic_nozzle | R | `engine.antimatter_beam_core_drive`, `annihilation_chamber`, `antimatter_injector`, `pion_gamma_shield_panel` |
| alt.fusion.antimatter_catalysed | far | F11, F10, F9 | cls.engine.fusion_drive, cmp.penning_trap_portable | R | `engine.antimatter_catalyzed_fusion_drive`, `antimatter_trigger_injector` |
| cap.ism_map | far | F13 | | O (ISM density and dust measured beyond the heliopause) | ISM erosion planner input; gates the shields |
| cls.shield.ism | far | F11, F9, F13, F5 | cap.ism_map, cls.shield.active_magnetic | R | `shield.relativistic_ism_deflector`, `shield.whipple_sacrificial_erosion_shield` and their 3 components |
| cls.probe.starchip | far | F8, F5, F15 | cls.sail.beam_driven, cmp.chip_scale_stack | R | `probe.starchip_laser_sail`, `sail_metamaterial_film` |
| cmp.chip_scale_stack | far | F15, F7 | alt.computer.radhard | W | `chip_scale_electronics_stack` |
| cap.pulsar_navigation | far | F1, F13 | | W (SEXTANT heritage), R for accuracy | `interstellar_navigation_beacon_receiver` |
| cls.hull.interstellar | far | F5, F1, F11 | cls.structure.spine.xl, cls.shield.ism | R | `structure.interstellar_cruise_hull_segment`, `cruise_radiation_shelter_core`, `long_duration_component_health_monitor` |
| cls.swarm.element | far | F7, F8, F1 | cls.power.solar_array_rigid.xl, population representation (G6) | R | `power.dyson_swarm_collector_element` and its 3 components |

The NTR chain, as data: `cls.engine.ntr` (world-known from NERVA, TRL 5) needs `cmp.ntr_fuel_element` (which needs `prc.leu_haleu_fuel_fab` and an F5 floor of 2,500 K service temperature) + `cmp.turbopump` + `mat.prop.lh2`. Flying one for a long mission additionally needs `cap.zbo_storage` (cryocooler at 20 K plus a year of LH2 held in orbit) and `cap.inspace_reactor_ops` (a reactor made critical in space). Flight rating itself is not a node: it is the TRL that NTR's test campaign raises (5.2 layer 3). The same holds for HTS coils in space and the first aerocapture: those are flight tests of `cmp.hts_coil` and `cap.aerocapture` (route RF from N), not new nodes. The far-era comms relay rows (`comms.deep_space_optical_array_megastructure`, `probe.interstellar_relay_buoy`, `probe.interstellar_flyby_bus_nuclear_pulse`, `shield.magnetic_sail_deceleration_brake`) are size regimes or compositions of nodes above and need no node of their own.

**Counts, future.** Near 61 nodes (60 rows, `fission_space` expands to 2) plus the 6 moved to 2030 in 3.0; mid 25; far 9. Nodes that need an in-flight measurement or an observation (RF or O): 22 of 61 near, 12 of 34 mid and far. Every near-catalog type is named above; mid and far components are covered as "and their N components" of the node that enables their host part (skeleton level, as asked).

## 4. Field levels

Each field's level moves a few continuous bounds, each on an S-curve that saturates at a physical limit (5.2, answer 79). A node can raise a limit (multi-junction cells raise PV from the single-junction to the multi-junction ceiling); levels never do. Values are 2030 starting points; "limit" is the curve's ceiling under present-day nodes, with the node that would move it in brackets. Sources are the trend series to fit; every number is a fitting target to verify, not canon.

| Field | Quantity | 2030 | Limit | Trend source |
|---|---|---|---|---|
| F1 Astrodynamics and control | lunar landing miss distance<br>autonomous deep-space position knowledge<br>pointing stability<br>real-time LEO orbit knowledge | ~50 m (SLIM, 2024)<br>~1-10 km (AutoNav, DART)<br>~1 mas (JWST)<br>~1 m (GNSS) | ~1 m (sensor and actuation)<br>~10 m [`cap.pulsar_navigation`, optical clocks]<br>~1 µas (drag-free noise floor)<br>~1 cm (signal-in-space) | JAXA SLIM; Mars 2020 EDL; DS1 AutoNav papers; JWST commissioning; GRACE-FO |
| F2 Fluids and aerothermodynamics | entry heating uncertainty (design margin)<br>hypersonic lift-to-drag<br>supersonic decelerator Cd prediction error<br>inflatable decelerator diameter flown | ±30% convective, ±50% radiative<br>0.3 capsule, ~1.5 winged<br>~5-10%<br>6 m (LOFTID) | ~±5% (turbulence and chemistry floor)<br>~4-5 (Küchemann's L/D barrier)<br>~1%<br>set by F5 fabric strength (~30 m) | NASA TPS margin policy; Küchemann 1978; LDSD and Mars 2020 chute data; LOFTID |
| F3 Thermodynamics and cryogenics | cryocooler input per W lifted at 20 K<br>passive LH2 boil-off in orbit<br>MLI heat leak, 300 K to 20 K<br>power conversion, fraction of Carnot<br>radiator areal mass | ~50-100 W/W<br>~2%/day (Centaur)<br>~0.5-1 W/m²<br>~45% (Stirling), ~40% (Brayton)<br>~5-10 kg/m² | 14 W/W (Carnot)<br>~0.1%/day passive [`cap.zbo_storage`: zero]<br>~0.1 W/m²<br>100%<br>~1 kg/m² [droplet radiators]; σT⁴ per area | Radebaugh (NIST) cryocooler reviews; NASA CFM roadmap; KSC MLI test data; NASA GRC ASRG; Juhasz (NASA) |
| F4 Chemistry and electrochemistry | Li-ion cell specific energy<br>c* (combustion) efficiency<br>LOX/LH2 vacuum Isp<br>water electrolysis efficiency (HHV)<br>PEM fuel cell efficiency | ~350 Wh/kg<br>0.97-0.99<br>465 s (RL10B-2)<br>~70%<br>~55% | ~500 Wh/kg [`mat.cell.solid_state`, Li-S: higher]<br>1.0<br>~470 s at practical expansion (CEA)<br>~100% (thermoneutral)<br>83% (ΔG/ΔH at 25 °C) | Ziegler and Trancik 2021; BNEF battery survey; Sutton; NASA CEA; IRENA 2020 |
| F5 Materials and solid mechanics | carbon fibre tensile strength<br>Al alloy yield (tank grade)<br>refractory service temperature<br>reusable tile multi-use temperature<br>composite cryotank mass vs Al-Li | ~7 GPa (T1100)<br>~590 MPa (2195)<br>~2,800 K (C/C, inert)<br>~1,600 K (silica)<br>~-25% | ~10-15 GPa practical [`mat.cnt_cable`: 50+]<br>~700 MPa<br>~4,200 K (Ta-Hf-C melting)<br>~2,000 K [CMC]<br>~-40% | Toray datasheets; Huang 2009; Cedillos-Barraza 2016; NASA CCTD |
| F6 Process science | metal AM deposition rate per machine<br>first-pass recipe yield<br>regolith oxygen yield by mass (space)<br>energy per kg O2 from Mars CO2 (space) | ~5-10 kg/h (DED wire)<br>~85-95%<br>0 (lab only)<br>~25-50 kWh/kg (MOXIE) | melt-energy bound (~200 kg/h per 100 kW)<br>100%<br>~43% (regolith O content)<br>4.9 kWh/kg (ΔH of CO2 splitting) | Wohlers Report; NASA ISRU studies; Schreiner 2016 (MRE); MOXIE results 2023 |
| F7 Condensed matter and electronics | space PV cell efficiency (AM0, BOL)<br>array specific power<br>thermoelectric efficiency<br>power electronics efficiency<br>rad-hard total dose<br>HTS field at 20 K | ~32%<br>~150 W/kg (ROSA)<br>~6-8% (MMRTG, eMMRTG)<br>~96%<br>~0.1-1 Mrad<br>~20 T (CFS, 2021) | ~45% multi-junction practical (68% infinite-junction) [`mat.pv.*`]<br>~3,000 W/kg (F5 areal mass)<br>Carnot [higher ZT]<br>~99.5%<br>~10 Mrad [SiC/GaN]<br>~45 T (stress-limited) | NREL Best Research-Cell Efficiency chart; Shockley and Queisser 1961; NASA RPS; NHMFL 45.5 T (2019) |
| F8 Electromagnetism and photonics | deep-space optical downlink<br>detector quantum efficiency<br>laser wall-plug efficiency<br>optical terminal pointing | 267 Mb/s at 0.2 AU (DSOC)<br>~90%<br>~50-60% (diode)<br>~1 µrad | photon-efficiency (Holevo) bound for given aperture and power<br>100%<br>~80% (quantum defect)<br>~0.1 µrad (diffraction and jitter) | NASA JPL DSOC reports; Crump 2013 (diode lasers); LLCD/TBIRD |
| F9 Plasma physics | Hall thruster efficiency<br>gridded ion efficiency<br>demonstrated thruster life<br>flight power per thruster | ~60%<br>~70%<br>~50,000 h (NEXT, ground)<br>~13 kW (AEPS) | ~85% (ionisation cost)<br>~90%<br>~100,000 h (cathode-limited) [magnetic shielding]<br>~100 kW Hall [MPD, VASIMR: MW] | Goebel and Katz; NEXT life test; X3 at 100 kW (2017) |
| F10 Nuclear physics | RTG specific power<br>NTR propellant exit temperature<br>space reactor specific power<br>Pu-238 supply | 2.8-5.1 We/kg<br>~2,550 K (NERVA)<br>~2.5 We/kg (KRUSTY)<br>~1.5 kg/yr | ~20 We/kg (Pu-238 at 0.57 W/g, 30% conversion)<br>~3,200 K (carbide fuel; Isp ~950 s)<br>~50-100 We/kg at MWe (radiator-bound)<br>policy, not physics | NASA RPS programme; Rover/NERVA reports; Mason (NASA FSP); DOE Pu-238 supply project |
| F11 Fundamental and particle physics | antiproton production energy efficiency<br>longest antimatter storage<br>clock fractional instability (space) | ~1e-9 (CERN)<br>405 days (BASE, 2016)<br>~3e-15 (DSAC, 2021) | ~1e-4 (dedicated factory, Forward)<br>none<br>~1e-18 (lab optical clocks; quantum projection floor) | Forward 1985; CERN AD/ELENA; Burt 2021; Bothwell 2022 |
| F12 Planetary science (per body) | gravity field degree<br>best surface image resolution<br>weather forecast skill horizon<br>resource map resolution | Moon 1,500 (GRAIL); Mars ~120; Titan ~5<br>Mars 0.25 m; Moon 0.5 m<br>Earth ~10 days; Mars ~1-3 sols<br>lunar ice ~km scale | ~R/h of the lowest orbit flown<br>~cm (diffraction from orbit)<br>~2 weeks Earth-like (chaos)<br>~m | GRAIL; MRO HiRISE; LRO; Zhang et al. 2019 (predictability limit) |
| F13 Astronomy and astrophysics | solar particle event warning lead time<br>exoplanet atmospheres characterised<br>astrometric precision | ~10-60 min<br>~100 (JWST)<br>~20 µas (Gaia) | ~1-3 days (CME transit)<br>the target population<br>~1 µas (photon noise for aperture) | NOAA SWPC; JWST programmes; Gaia DR3 |
| F14 Life sciences | ECLSS water recovery<br>O2 loop closure<br>longest continuous zero-g data<br>radiation risk model uncertainty | 98% (ISS, 2023)<br>~50%<br>437 days (Polyakov)<br>~3x (95% interval) | 100%<br>100% [`cap.bioregenerative_closure`]<br>none: extended only by flown data<br>~1.2x (needs crew data) | NASA ISS ECLSS; NASA-STD-3001; NCRP and Cucinotta risk models |
| F15 Computing and information | flight compute per watt (rad-hard)<br>channel coding gap to Shannon<br>lossless science compression | ~2-10 GOPS/W (HPSC)<br>~0.5-1 dB (CCSDS LDPC)<br>~2-3:1 | ~commercial-node parity (~1,000 GOPS/W), Landauer far above<br>0 dB<br>data entropy | Koomey 2011; NASA HPSC; CCSDS 131.0-B, 123.0-B |

Two rules fall out. First, F12 is per body, not global, and so are parts of F13 (per star system) and F14 (per gravity level): their "level" is a vector (M12). Second, the space-only rows (F6's regolith and CO2 rows, F12 almost entirely, F14's zero-g and dose rows, F2's flight-validated margins) have no world pace: they move only on the player's data (5.2, answer 78).

## 5. Mesh checks

**Coverage (checked by script against both catalogs).** All 90 present-day part types map: 85 to a class node, 5 GSE outside. All 184 present-day component rows map: 96 to a node, 88 to field levels only (2.22). Every near-catalog type is named in section 3; mid and far components are covered as groups. Exceptions: none unmapped. The findings, most important first:

**M1. Rule 15 read literally makes ~250 class nodes; the selective reading still makes 133.** 133 class/regime nodes for 85 gated classes is 39% of all 2030 nodes. Proposal: regimes only on the four ladders in section 1, and only on classes where a rule-33 threshold changes tooling, transport or test (tanks, shells, engines, solids, arrays, dishes, chutes, heatshields, arms, legs). Thirty-odd classes (sensors, small instruments, seats, hatches) get one node. Owner call: Q2.

**M2. The catalog's own counts and slots don't close.** It has 184 component rows, not 181. About 28 slot names in the part table have no component row: `main_valve`, `vent_valve`, `cryo_valve`, `hollow_cathode`, `ion_optics_grid`, `nozzle_throat_insert`, `laser_diode_module`, `fine_steering_mirror`, `rf_feed_horn`, `power_amplifier_rf`, `rf_transceiver_card`, `photodiode_array`, `coil_winding_core`, `droplet_generator`, `droplet_collector`, `mortar_pyro`, `pyro_charge`, `pilot_chute`, `riser_line`, `piston`, `pressure_switch`, `strain_gauge`, `thermocouple_sensor`, `acoustic_blanket`, `baffle_stray_light`, `visor_optical`, `joint_bearing`, `rotary_bearing_large`, and the generic `gimbal_actuator`. This draft gives each a node or a trivial home (sections 2.6-2.22). The catalog should add rows for the non-trivial ones (cathode, ion optics, throat insert, laser module, steering mirror, RF amplifier, droplet generator and collector).

**M3. Rule 21 was applied to five avionics boxes but not to their siblings.** `comms.transponder_unit`, `comms.relay_crosslink_unit`, `gnc.imu_inertial`, `gnc.gnss_receiver` and `crew.life_support_rack` are boxes or racks inside another part's volume. By rule 7 they are components (the GNSS *antenna* and the dish stay parts). Node-side nothing changes (their `cls.` nodes become `cmp.` nodes), but the catalog should move them.

**M4. Present-day hardware missing from the present catalog.** Grid fins, catch ring, Whipple shields, mesh reflectors, the superpressure balloon, the sample-return capsule (13 near-catalog types, 3.0), plus five with no row anywhere: control moment gyros, pneumatic separation pushers and latches (rule 22 names them), the ram-air parafoil, the inflatable habitat module (BEAM, flown 2016) and the shared-housing RCS pod (catalog section 3 rules it a part but gives it no type). All are Y or P at 2030.

**M5. TPS tiles and one duplicate.** `ablator_tile.charring` and `tps_tile.ceramic_reusable` are listed as components, but catalog section 3 recommends a material field over the heatshield's cells. Nodes agree with the recommendation: tiles are `mat.` nodes applied as zones, so the two component rows should go. Separately, `recovery.drogue_mortar` is both a part and the parachute's `mortar_pyro` slot: pick one (proposal: the part, since it has its own exterior and fires its own body).

**M6. One node for many types, by design.** Several nodes cover a cluster: `cmp.pyro_release` (11 types), `cmp.mechanism_drive` (7), `cmp.valve.general` (7), `cmp.power_conditioning` (4), `cmp.data_handling` (4), `cmp.igniter` (3). These types differ in parameters, not in kind; a player who knows pyrotechnics knows bolt cutters. Proposal: allowed when (a) every type in the cluster is world-known at the same time and (b) no future node splits them. `cmp.igniter` fails (b) if a future relightable-igniter node arrives (the near catalog's reuse-rated slug), so it may split later; that's fine.

**M7. Trivial parts.** Rule 20 exempts trivial *components*. Some parts are just as trivial: `structure.ring_frame` (and arguably `structure.truss_strut`, `landing.skid_runner`). Proposal: extend rule 20 to parts; a trivial part has no node and its ranges follow F5.

**M8. Domains with no field, and where they go.** None of the gaps needs a 16th field. Granular mechanics (rover wheels, excavation, cratering): F5 for the mechanics, with soil data per body from F12. Hypervelocity impact and MMOD: F5. Vibro-acoustics: F2 for the environment, F5 for the response. Spacecraft charging: F9. GSE engineering: no field at all (M13). Recommendation: add one line to each field's description in 5.1 so nodes have a home.

**M9. Capability nodes enable code, not types.** All 20 `cap.` nodes are orphans against the part catalog by construction; each is a feature switch in the planner, guidance, comms or operations code. They need their own registry (what code path each one turns on). Without it, a `cap.` node is unfalsifiable.

**M10. Standards: node or parameter?** Docking standards (IDSS, APAS, probe-drogue), mount ring standards and bus protocols (1553, SpaceWire, CAN) are compatibility rules, not knowledge. Proposal: a standard is a *parameter value* of a class, available with the class node; no standard gets a node unless it needs new physics (none do in 2030).

**M11. True orphans.** Nodes that enable no type at all: `alt.engine.rotating_detonation` (no type distinguishes it), `mat.cell.solid_state` and `mat.pv.perovskite_tandem` (they only move limits, which is legitimate: rule 79's "a limit moves only when a node changes the physics"), `mat.prop.curie_standin` (a calibration device, not knowledge; rule 38). And all 24 `prc.` nodes enable recipe steps that don't exist yet, because the industry layer is undesigned. Proposal: keep limit-raising nodes and say so; process nodes wait for the recipe design and are checked then.

**M12. Some field levels are vectors.** F12 is per body (Titan's gravity degree has nothing to do with the Moon's). Parts of F13 (per star system), F14 (per gravity level, per dose regime) and F2 (per atmosphere: entry margins for Venus aren't Mars's) are too. 5.2 says "a continuous global level per field". Proposal: each field has a global level plus optional per-body sub-levels; data fields (answer 77) feed the sub-level, the sub-level gates body-specific nodes (`cls.probe.giant_entry` needs the target's profile, not Jupiter's).

**M13. GSE and the ground.** GSE types and their components sit outside nodes (2.21). The one leak: shared components (a fluid QD on a fill panel is also on a docking port) keep their node. Pad and tower (rule 24) size with the vehicle's diameter regime; they need no node but inherit the regime as a build cost.

**M14. Proprietary nodes need their own rule.** 7 nodes are P at 2030 (full-flow cycle, 30X steel, PICA-X, recoverable fairing, cryo transfer, tower catch, Curie's propellant). The design distinction matters: the *node* is knowledge (Rocket Lab's electric-pump cycle is public, so Y), the *design* is always proprietary. Proposal: P nodes can't be licensed, cost full novelty in R&D (rule 36), and become Y when the world catches up (a dated event per node, from the world pace) or when another actor publishes.

**M15. Propellants and fluids are materials.** Rule 15 names materials but not propellants. 17 `mat.prop.*` and fluid nodes are needed; they gate engines (a methalox engine needs `mat.prop.lch4`) and have revealed properties (density, boiling point) exactly like structural materials. Proposal: confirm propellants as material nodes.

**M16. "No designs, buy components" implies a 2030 supplier catalog.** A company with zero designs and every Y node licensed still can't fly until it has designs for every *designed* component (rule 2) on its first vehicle. So the world needs a supplier catalog at 2030: real-class component designs for sale, as finished units or as licensed designs (5.2). Proposal, by what real startups actually buy:
- **Offered** (~50 designs): flight computers (rad-hard and COTS-redundant), SDRs and transponders, GNSS receivers, IMUs (MEMS, FOG), star trackers and sun sensors (finished parts), reaction wheels (3 sizes), magnetorquers, COPVs (5 sizes), cryo and general valves, regulators, every pyro and separation device, clamp bands, Li-ion cells and packs, PV laminates, heat pipes, cryocoolers, monoprop and cold-gas thrusters (finished parts), small Hall thrusters with PPUs, docking ports (IDSS), parachutes, seats and suits (crewed licence only).
- **Not offered** (the player designs them, as real launch startups do): turbopumps, e-pumps, injectors, main engines, tanks, structures, heatshields. This is what makes the first engine the first real design program.
- **Prices** follow the world's Wright's-law position (5.2 manufacturing); publishing results lowers them (answer 73).
Parametric components (rule 2: lines, valves, sensors, battery packs, heaters) need only their node, not a supplier design.

**What a first small launcher touches.** An Electron-like vehicle needs about 40 nodes: tank `s`, common bulkhead, linerless composite with `mat.cfrp_im`/`mat.cfrp_cryo`/`prc.afp_composite`/`prc.autoclave_cure`, engine `s` with the electric-pump cycle, regen cooling, `cmp.e_pump`, `cmp.motor_inverter_hv`, `cmp.injector`, `cmp.igniter`, `prc.lpbf_metal_am`, LOX and RP-1, battery pack and Li-ion, interstage, fairing, adapter, thrust structure and equipment bay (`s`), flight computer (COTS), pyro release, COPV, two valve nodes, regulator, mechanism drive, IMU and a gyro, GNSS, transponder, omni, RF and SDR, cold-gas RCS with its gas. Every one is Y. That count is the pacing question (Q1).

## 6. Open questions for the owner

**Q1. How many nodes does a player meet in the first hours?** A first launcher needs ~40 world-known nodes; a first satellite ~25 more. Shown one by one, that is a wall of licence clicks before anything flies. *Recommendation:* group world-known nodes into licence **bundles** by goal ("small kerolox launcher", "LEO smallsat bus", "crewed capsule"), each a single decision with one price and one wait. The player meets ~5-8 bundle decisions in the first hours and their first *individual* node choices (common bulkhead or not, which cycle, CFRP or Al-Li) inside the design program, where they are design choices, not purchases. Individual nodes appear as the frontier: the first P or N node the player wants is the first real research project, ideally within the first 1-2 hours.

**Q2. Size regimes: visible unlocks or invisible caps?** *Recommendation:* visible, but only on the four ladders of section 1, and bundled: licensing "medium tankage" grants every `m` diameter-ladder class node at once (tank, interstage, fairing, thrust structure). That cuts the player-facing class nodes from 133 to about 50 without breaking rule 15, which still holds in the data.

**Q3. Licence vs research for world-known nodes (rule 32).** *Recommendation:* licensing costs money and a short wait (weeks of game time), research costs lab time and nothing else; both give the same node. Licensing is the default early, research when cash is short. A licensed node doesn't let you publish results about it (answer 73's prestige goes to the original holder).

**Q4. Pacing of world drift.** At real trend rates the world's levels move noticeably each game year (PV +0.3 points, compute per watt ~x1.4, battery Wh/kg ~+5%). *Recommendation:* show it as a yearly "state of the art" summary, and make world drift slower than a focused player programme in any one field, so the player can lead a field within a decade but never lead all 15.

**Q5. Do proprietary (P) nodes exist?** They add a competitor texture (SpaceX's full-flow cycle is real knowledge you can't buy). *Recommendation:* yes, few (7 at 2030), each with a dated world catch-up event, so no P node blocks a playstyle for more than ~5-10 game years.

**Q6. Name real companies?** *Recommendation:* in design data, yes (traceability); in the game, through a fictionalised competitor list whose capabilities match the real ones. Trademark and tone both favour this.

**Q7. RF and O nodes: how loud?** About a third of near-era nodes need a flight measurement or an observation. *Recommendation:* show every RF node's missing data as a mission objective in the planner ("needs: a year of LH2 in orbit"), and every O node as an unknown with a hint ("something is waiting to be measured at Europa"). That is how the exploration bias becomes something the player can see and plan for.

**Q8. Who designs the first engine?** M16's supplier catalog sells no main engines. *Recommendation:* keep it so in career mode, but allow buying *finished* small engines and kick stages from fictional suppliers at a steep price, so a player who wants to start with satellites can.
