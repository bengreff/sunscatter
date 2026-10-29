# Open issues, 2026-09-27 (updated 2026-10-27 after the Falcon 9 night)

Everything known to be open after the realism-1 build, for the next owner interview. Nothing here is being fixed yet (owner, 2026-09-27). Sources: the owner's play, `docs/reviews/2026-09-26-code-review.md`, `docs/design/aero-thermal.md` (limitations), and the agents' reports.

## Performance

- **Medium full screen: 40 fps in the lower atmosphere (Low: 120).** No longer a target: only the Minimal tier (the default) is developed (D073). GPU-bound, scales with pixels. Measured at the pad, 1600×900, High minus one feature (`bench.rs` "High −…" rows): terrain detail layer (ground textures, 3 fbm noises per fragment) ~4.7 ms, atmosphere pass ~4 ms (after the sky light), terrain ~2.8 ms, MSAA 4x ~2.3 ms, bloom ~2.3 ms, terrain error 2 px vs 8 px ~1.9 ms, shadows ~1.3 ms. Sampling only 2 of 5 ground layers saved only 0.15 ms (not the cost).
- **Render scale (D072)** not built; the biggest single lever on Retina.
- Terrain chunk count near the ground ~800 (detail splitting to 2.4 m adds ~25 %).
- Game-side per-frame costs at 1x: `Dominance::of`, `trajectory::vessel_line_end` (line-end rescans, review game 4/6), navball.
- Map view O(n²) and linear lookups (review game 10).
- Sim: coast thermal solves every 600 s lattice point for tumbling vessels (~250 µs each); `ThermalNetwork::step_to` ~15 % of sim time; live tick 74 µs; aero bake 193 ms per craft design at load.
- Demo perf (2026-09-27, perf agent): 294 fps 1x, 307 1000x, 211 1,000,000x with 11 vessels (offscreen, Minimal-ish perf view).

## Owner decisions pending

Defaults taken 2026-09-30 while the owner was away (each reversible, recorded as a decision): engine heat to the nozzle bell (D077), attitude control only from RCS couples and the gimbal (D078). Ap/Pe on every closed orbit (D076) settled by the owner 2026-10-01: plain markers, no near-circular label.
- The test craft's chute was undersized (600 m² Cd·A: 23 m/s at 20 t, measured in the demo). Resized 2026-09-30 to four Apollo-class mains, 1560 m² (NASA TN D-7437: Apollo's three 25.4 m mains, ~5.9 t at ~9 m/s; Soyuz's 1000 m² main, 7.2 m/s, ESA): ~7 m/s at the 5 t landing mass it is sized for, at a full 20 t the reefed mains tear (D079): land light or on the engine. For comparison the Shuttle SRB's three 41 m mains brought 77 t down at 23 m/s (NASA NTRS 19840047186).
- Parachutes (D079, 2026-09-30): drogue, reefed mains and tearing on overload are built. Still simplified: the fill constants n (7 ringsail, 10 ribbon) are from memory; no opening-force overshoot (Knacke's Cx); a cluster shares its load evenly. The test craft's mains are sized for 5 t and tear at a full 20 t (measured in `a_full_craft_tears_its_mains…`).
- RCS propellant is counted only in live flight, not on rails (D078).
- Reentry from LEO nose-first overheats the test craft: skin 1109 K against 1100 K, interior 664 K against 400 K on the chute (demo 2026-09-30, debug mode; 304 K before the deorbit burn). No heat shield or insulation: a craft-design question (heat shield, blunt-end-first entry, or insulation in the thermal model).

Settled 2026-09-28: haze (moot, D073), chute (keep), stability (finned rocket, D074).
Settled 2026-10-01 (owner): a full craft tearing its mains is kept (D079); nose-first reentry overheating is accepted, heat shield later as a part (D064); SAS on a slope kept (D078); near-circular Ap/Pe plain (D076).

## After the Falcon 9 night (2026-10-27, D088–D090, D100–D108)

Each workstream's notes say more: `docs/features/{parts,falcon9,ship-systems,systems-screen,solar-system}.md`, `docs/plans/overnight-2026-10-27-falcon9.md`.

- **Frame rate:** the owner's slow view (185 km, Earth filling the screen, 3024×1964) is ~10.5 ms of GPU (was 18.1) against 8.3 ms for 120 Hz; the render scale should hold the refresh at ~0.9 (not yet measured unpinned on a quiet machine). Over land and close to the ground the terrain's main pass is 13 ms (ascent shots 46–79 fps). The terrain shader is register-bound (lighting + zones together); next levers (GFX2): drop eclipse occluders on the CPU when their shadow cannot reach (~0.9 ms), let the zones' pattern reuse the warp's lattice fetches, the warp's coarse octaves per vertex, the close-up ground pattern's noise from a texture (~2 ms close up). `tools/gpu_trace.py` measures GPU time per pass from a Metal System Trace.
- **Flying the Falcon 9:** RCS translation (`Controls::translate`) has no keys (H/N/I–L are taken). Engines' heat stays D077's static shares (an off engine still heats its nozzle); the engines gimbal together; RCS rotation keeps per-axis authority (translation is allocated); SuperDraco and Draco not flown in tests; Dragon's internal-power switch at T−5 min (the umbilical drops at clamp release instead).
- **Ship systems not built:** slosh pendulums; single-use crush cores (they recover); legs' self-levelling; hard starts; pyroshock (no shock ratings); per-component vibration damage; failure hazards on the rails lattice; blast fragments (in vacuum a blast reaches only a tank's inside); Brinkley landing response (its data "verify"); O₂/N₂ as tanks; suits and leaks; grid-fin heating and their pump's fluid; the nosecone's aero while hinged; thruster couples in the joint loads; legs' strut capacity (legs left to the contact).
- **Data marked U/E** in `falcon9.md`: dome shapes, the first stage's helium bottles, cold-gas thrust, gimbal ranges and rates, NPSP, crush-core stroke, grid-fin deflection, battery capacities, SuperDraco/Draco masses, parachute reefing; the legs derive 173 kg against a published 500 kg (reported as a contradiction); Dragon's CO₂ removal (4.0 kg/day) under four crew's 4.16.
- **Gravity fields (D090):** a craft in low lunar orbit holds warp to ~4×10⁵× (degree 100; budget call open); Earth's rotation model is 0.09° off in longitude in 2030 (no nutation, polar motion or UT1); no tides; the ephemeris generator still uses `body.ron`'s J2/J4/J6.
- **Lines and clock (D088, D089):** Sedna's orbit is never drawn as a line (the zoom limit follows its current distance); `settings_ui` relabelled the trackpad zoom.
- **Planned burns for craft of parts** are refused (`PlanError::PartsUnsupported`).
- **From the night's review (2026-10-27), not fixed:** the booster cannot relight in vacuum (the cold-gas pods' nozzles are all radial, no aft one: data unknown; its burns wait until drag settles the tanks, ~40 km; forcing a start spends a relight into gas ingestion); separating < 1 s after MECO lets the booster pass through the second stage (tail-off; the separation could wait for the engines to stop); bubble members flying together apply their orders only when alone (`bubble.rs:509`); an aborted bubble tick does not restore `ShipState` changes made in `begin_tick`; grid-fin deflections can misalign after a split (`Runtime::partition` copies them whole); `Vessel::landed_at`/`coasting` ignore a configured id's deployed parts; debug mode leaves crew injuries (alarm only) and power shedding on; `craft::engine::ship_thrust` and `ShipState::apply`'s instant Ignite/Deploy duplicate the engine rule (tests only, rule 5); `Vessel::ship_needs_ticks` is `pub(super)`, so the game composes `held_live` from public parts (one owner wanted).

## After the overnight run (2026-10-26, D081–D085)

Each workstream's feature notes say more: `docs/features/{limbo,parts,proximity,scale-performance,solar-system}.md`, `docs/design/aero-thermal.md`, `docs/plans/scale-baseline-2026-10-26.md`.

- **1,000,000× with 50 craft is not reached:** ~309,000× of real time on a quiet machine after the last thermal work (93d50b4; ~183,000× measured under load after COAST's DOP853 switch) (honestly shown as "limited: compute"); every frame stays under 16.7 ms. The bound is now the 300-part craft's coast thermal lattice (~18 ms per craft per 10⁶× frame; its budget units underestimate the work, 1–3.3 µs per unit), not integration (~0.75 ms). Options: a reduced-order coast thermal model for big craft (PERF rejected per-part aggregates: tens of kelvin of error), the fleet running continuously on worker threads instead of 8 ms per frame, interpolating body positions within a step (~15–20 %).
- **The coast thermal skip never engages for limbo craft:** the thin-air input moves 1–4 % between lattice points (osculating-orbit averages jittered by J2's short-period terms) against a 1 % allowance; and where it engages, it can err up to 1.3 K on a held 300-part craft (error control to check). Smoothing the air input like the sunlight would let it engage (`scale-performance.md` §7).
- **DOP853's longer steps:** the dense output between stored samples is ~1 m off at mid-step in LEO (25 µm before; still under the propagation error), and a dip into the live boundary shorter than one step (now ~6× longer) can be missed.
- **1,000 years (D005) for the moons is not affordable as tables** (~330 MB); the Solar System covers the shipped span (see `solar-system.md` for the options). Moons drift in phase (Oberon up to ~280 km/yr before the refit; the refit to Horizons was in progress at the end of the night). Moons over 5 km not added (the fastest would cost ~12 MB each as tables). Gas giants have no surface or atmosphere model (vessels pass through them).
- **Parts are a skeleton:** placeholder properties only; the game draws pieces with the test craft's mesh and no game craft has parts, so nothing breaks up in play yet; the load trigger is a pressure ceiling, not a structural solver; overheating judged only in live ticks; built craft cached without eviction; a large piece's design still costs seconds (small pieces 15–20 ms).
- **Proximity:** coasting craft pass through each other when nothing live is near (a conjunction search on stored trajectories would fix it); landed craft are not bubble members; every member flies full live ticks (keeping members on rails until contact is possible would be cheaper and exactly invariant, but changes the reading of D082: owner's call); no docking or merging; a leader going live inside a frame for a reason other than its engine takes its neighbours one frame late (~1e-9 m, not bit-identical across frame rates).
- **Limbo:** no solar cycle in thin-air density; no albedo or infrared pressure; a low-thrust burn whose craft isn't settled at ignition snaps its attitude to the burn; the D062 rails floor still blocks rails warp for the active ship coasting below 150 km (owner's call: the live boundary now decides by q).
- **Aero/heating (D085):** turbulent transition rise ×4.7–5.6 against STS-1's ×3.2; laminar real-gas heating 10–15 % low at 7 km/s; no pressure history along the layer (hemisphere shoulder low); no corner heating on flat faces; Huygens heating 22 % low (Sutton–Graves in N₂–CH₄); Venus CO₂ and Titan CN radiative heating not modelled; CO₂ γ constant; the area rule not built (too costly per bake); no gusts (no winds yet). Tauber–Sutton Earth constants confirmed only through independent implementations.
- **The test craft under its drogues falls nose-down** (demo 2026-10-26, `58_parachute`): its drogues mount at the nose, and with body–fin interference its fins now lift ~3.6× more, enough to beat the chute's flipping moment. It still lands upright. A craft-design question (drogues at the tail for a nose-first craft, or a tail-first entry), for the owner with craft creation.
- **Map:** with 44 bodies, planets' orbit lines cross the lunar-transfer view at 1.2e9 m (correct, but busy).

## Surfaces (D073, 2026-10-25)

- **Earth from low orbit is still low-contrast under blue haze**, though coasts and zones are now crisp (demo `orbit_down`, 300 km, looking down ~70°, a winter afternoon with the Sun ~60° from the zenith). Measured: sea linear (0.044, 0.080, 0.165), forest land barely brighter; the in-scatter is ~1.5–2× a single-scattering Rayleigh estimate (multiple scattering accounts for part). Not tuned by eye. To decide with the owner: physical haze (default 1) or a lower default; or verify Bevy's in-scatter at this geometry against an independent calculation first.
- **Zone albedos** (`visual.ron`, cited per zone) were chosen inside published ranges; the Gao 2005 and Tsvetsinskaya 2002 tables were paywalled and quoted from memory: verify them.
- **The Moon close up** (4 km, low Sun) looks like grey clouds with straight seams between terrain chunks, in every tier (older than the zones). Likely causes (read from code, not verified): geomorphing moves positions but not normals; the mesh undersamples the procedural relief; the detail uv wraps every 6.4 km on the Moon. Real lunar terrain needs craters at every scale: a sim-side terrain feature (D047, D059).
- The coast at the pad is only as good as the 4–5 km maps: the Atlantic is ~0.6 km east of LC-39A and is not seen from the pad (a launch-site patch was declined 2026-09-28).
- Zone edges show small "beads" where a third zone has a single texel; coastlines seen from under ~100 m may show 2–5 m steps (f32 texture coordinates).
- Frame cost of the zones not measured yet (`bench` row "Minimal −zones").

## Physics and model gaps

- Aero (D070 table in `docs/design/aero-thermal.md`): no base drag vs Mach, no plume effects, no body–fin interference, crossflow Cd constant in Mach, adiabatic smooth-wall friction, no supersonic area rule, radiative heating spread like convection, no ablation.
- Aero constants checked against sources on 2026-09-30: Wilmoth bridge exact (NTRS 20040105522); Sutton–Graves k for Earth and Mars (TFAWS 2012); Tauber–Sutton Mars C, a, b (West & Brandis 2020, NTRS 20200002354); the equilibrium-air shock density ratio was 12–30 % low and is now NASA SP-3093's table (Cp,max moved ~1 %). Still unchecked: Tauber–Sutton's Earth constants and both f(V) tables (the 1991 paper is not on NTRS; they matter only above 9 km/s, lunar return). Mars has no atmosphere data.
- No air above `top` (150 km): orbital decay above 150 km not modelled.
- Coast skin temperatures are orbit averages (eclipse swing only in live flight); radiation to 2.725 K, no planet infrared; landed/crashed vessels keep their temperatures.
- Every vessel carries the same antenna constant (`game::comms::VESSEL_ANTENNA`), not from craft data.
- Planned burns: toggling debug mode mid-burn drops that burn; non-active vessels hold stability only; replanning the next burn re-integrates the current coast from its start (slow for years-long coasts).
- Review sim items open: 5 (rails kinematics), 6 (impact on the Hermite chord, non-solid bodies), 7–8 (save format, validation), 9 (Kepler edge cases), 12, 13.

## Game and UI

- Landing prediction: the attitude is assumed from the controls (holds other than surface prograde/retrograde keep the current attitude).
- The navball does not show when the maneuver hold has taken over for a planned burn.
- Review game items open: 5 (lighting occluders solid bodies only), 6, 11, 14, 15, 17.
- `SUNSCATTER_DEMO_STOP_AFTER=Perf` does not stop (the step is `Perf(n)`).
- Colour detail matching the sub-sample terrain detail (realism-1 1b) not done.

## Visual

- The Moon's sub-sample detail is noise-like (no craters).
- Terrain close up limited by the ~5 km colour map (a launch-site patch is the fix).
- Ultra pad view showed no ship shadow in the last demo (not investigated).

## Process

- The Windows check was skipped (backhouse unreachable 2026-09-27); last Windows run predates realism-1.
- Nothing from realism-1 has been played by the owner except the lower-atmosphere frame rate.
