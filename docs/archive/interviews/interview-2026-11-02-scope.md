# Interview 2026-11-02/03: scope reassessment (owner + Director)

68 questions in 17 rounds, asked by the Director. Answers are the owner's; quotes are verbatim. Synthesis and proposals: [scope-2026-11.md](../../plans/scope-2026-11.md).

Context (Ben): "The biggest concern I have right now is that we are overengineering meaningless specifics rather than building a versatile foundation for a generalized simulator, so high fidelity that it can virtually be used for professional aerospace mission initial prototyping except for exact hardware/assembly specifics." Interested in adopting existing physics simulators (r/simulated; weather for ANY planet). Hasn't seen complete visualizations of ship components (click a part: zoom to it, show all its subsystems; wiring and plumbing routed around blockers within a part). "Maybe a scope reassessment and high level design session would be effective rather than patching on features."

## R1 purpose
1. Pro credibility: ALL of trajectory/dv design, vehicle sizing/trades, subsystem behaviour over a mission, ascent/entry/landing flight dynamics.
2. Game vs tool: EQUAL weight. "game" = "game for crazed space nerds, NOT a game for a general audience".
3. Generality: "1 [any vehicle from generic components] and 2 [any world from generic models] are the eventual aspiration, but starting with a single concrete example is fine to calibrate before we can meaningfully extrapolate to procedurally generated systems. Also, foundation means it supports the later speculative technologies that we implement. 3 [one network for all domains] is not entirely required but within a domain it must be integrated. One data/operating system, one thermal/aero model, but exceptions include abstracted engines (given the results of inquiry)."
4. Reset size: decide after the design doc.
## R2 overengineering
5. Meaningless specifics: interface minutiae; edge-case rules. Vehicle-specific numbers "somewhat fine ... when the same thing could eventually be a derived field or an inherent characteristic of a part (scalable parameter)". The two flights' scripting "kind of needed ... that software will be needed to ship with a craft to make it flyable. It should be minimalistic scripting though, still relying mostly on the pilot (although you can make a more comprehensive autopilot for debugging)."
6. Fidelity rule: fidelity by consequence.
7. Calibration: "a single 'solar panel' model that can generalize to dozens of variants, and the dragon panels are one type". Some parts are craft-specific (Dragon) but still composable: "put the dragon on top of some other rocket with an adapter or change the number of engines or lengthen a tank easily". "We should not be building a full part catalog now (tens of thousands of components) but rather breaking the F9 and dragon into components as a kind of initial trial run for our system, while simultaneously designing generality." "a smaller calibration is needed. falcon 1, electron, and/or whatever else is well documented and simple. We do F9 right after." Game ships real crafts = defined parts in a specific configuration, with scripts and some hand-set calibration values. "We need to define classes and types for parts, components, the scope of what data defines a part/component, and so much more."
8. v0.2 milestone: REDEFINE around the foundation.
## R3 data model
9. Engines: "the ONLY thing abstracted about the engine is its performance, saved to a lookup table/function. An engine still has components with independent behavior ... as soon as the fuel passes into the combustion chamber the calculation gets handed off to inquiry's data/real life data and the nozzle heat, thrust, plume characteristics etc come out." "Engines should STILL have gimbals etc as components, those are simulated. The nozzle and combustion chamber are defined, but whatever happens inside is abstracted."
10. Part definition: type + scalable parameters (+ calibration overrides).
11. Data trust: DERIVED > published > estimate (reverses D115's published-over-derived).
12. First pass covers: structure+tanks+feed; engines (abstracted); power+avionics+data; crew+thermal+entry.
## R4 hierarchy, part view, routing
13. Hierarchy: recursive assembly tree + system graphs (power/fluid/data/thermal) crossing the tree.
14. Part view: "The part isolates and zooms in, maybe shows some internals, and the side panel displaying whole part data shows up. Then you can either click on a component within the isolated view and isolate on THAT component, click on a component linked in the side panel, pull up graphs, change viewing layers at the top, etc. For example, you should be able to see the cell by cell thermals of the part, maybe switching between physical exterior view and internal component view as a toggle."
15. Routing: auto-routed, physically meaningful (route length/bends set mass, pressure drop, resistance, heat leak; player can pin waypoints).
16. Routing crosses parts only through defined interfaces/ports.
## R5 adopt vs build
17. License: open source, GPL fine.
18. Runtime: Rust core, C/C++ via FFI OK (deterministic); Python/Fortran offline.
19. Adopt: fluid properties (CoolProp), atmosphere+climate (offline GCM), spacecraft dynamics+GNC (Basilisk/42 as reference), rigid body+contact (e.g. Rapier).
20. Oracles: GMAT/Orekit, Basilisk/42, JSBSim/OpenRocket/POST, CEA/CRUCIBLE, "with some big caveats ... with how many regimes this tool bridges, we can't really validate integrated systems against anything, only smaller systems ... Just don't restrict yourself, remember the equal weighting of game and tool." CEA is part of inquiry's validation, "but we go beyond it in simulation fidelity and breadth, bridging into fusion and plasma and the effects of nozzle shape." Wants ~50 more questions across ALL topics.
## R6 worlds
21. Weather: offline GCM climatology + deterministic runtime perturbations; "leave it open for the climate to change over years or centuries, such as for planets with odd orbits. The time scope for the game is at least thousands of years, let's say 10,000".
22. Weather affects: ascent/entry loads+dispersion; launch scrubs+ops; surface ops; long-term climate.
23. Star systems: "fully 'simulate' (the bodies are in a catalog with deterministic behavior stored+computed on demand) every star and system for a given scope, probably 100-300ly around Sol." Real catalog where verified, unknowns filled, the rest filled statistically by a generator "consistent with all our observations". Showcase: Ben's own system (three-body-islands / calder?) as "an imaginary system placed in another galaxy or across the milky way where you direct an alien space program". Some systems within 100 ly get hand-built elements: "An LLM should create a consistent set of assumptions on planet formation, life, etc, and define interesting and realistic key systems. Really the only procedurally generated bodies are the boring ones ... the major ones are hand built." MVP: 100 ly of stars around Sol, expand later.
24. Terrain: what exists now is fine early; final = functional for gameplay, not excessive.
## R7 sim core
25. Inactive things: hierarchy exact-to-statistical, each level validated against the one above.
26. Warp: max needed 100,000-500,000x; "Significant compute can be dedicated to progress decent eras of time. However, we should still be incredibly efficient and leverage determinism." Leaning to physics-identical + seeded stochastic, "but I would need precise examples with trade-offs in terms of reproducibility. It cannot be anything with a broad effect ... What cannot happen is a craft's temp is stable at 100x warp, then at 1000x warp it overheats. Unacceptable in any scenario. Do research on this for novel solutions."
27. Stepping: multi-rate co-simulation under one deterministic scheduler.
28. Domain solver: lumped node/link network per domain.
## R8 future tech
29. Must be expressible later: NTR/NEP/SEP; fusion + antimatter; beamed + sails. Exotic: "Kugelblitzes would be out. Antimatter torch, liquid/plasma radiators, relativistic particle shields, and anything else that does not depend on unknown physics is in. All that is very long term. Realistic and continuous tech progression is required ... (very sophisticated 'tech tree'). And of course v1 is just present day stuff."
30. Engine interface: families with their own interfaces.
31. CRUCIBLE: tables now, live in-game designer later.
32. Tech intro: engineering readiness (TRL, test campaigns) + physics-gated tree; start present day; sandbox mode exists.
## R9 game layer
33. Core loop: design>simulate>iterate and flying live first; org and discovery "still SUPER important. Infrastructure, interplanetary trade, management strategy (like a strategy game but based entirely in physics), plus true discovery, before you send a probe to a planet you literally only see what your telescopes have seen. Don't entirely neglect the 'factorio' elements ... more like a solar system scale factory." AI: "There is not just a user UI, there is an AI interface alongside ... you can test nearly anything about the game that does not need my vision or aesthetic judgement, and eventually a person's AI agent can be playing the game as much as that person decides."
34. Economy: market + contracts, physically grounded.
35. Actors: DESCOPED. "The player always plays one company, the ISS has deorbited and all stations are built by the player, all other organizations are scripted with governments mostly requesting contracts as well as full missions, and this company (if well managed) eventually acquires most of the space market and gets a monopoly on orbital manufacturing."
36. Crew: minimal (mass, consumables, dose limits).
## R10 ship software and AI
37. Scripting: sandboxed WASM, any language, instruction-metered.
38. Ship computer limits simulated as resources (compute, power, radiation upsets).
39. AI interface: strict parity with the player's info model; separate, marked developer interface for ground truth.
40. Org-scale automation: "We CANNOT have abstracted missions with an outcome distribution. They are scripted missions run with an advanced layer 2 script, sometimes delegated to AI agents, later an option to record difficult procedures or to extract procedures from a past flight." (Supersedes the 'behavioural approximations' wording in R7/goals.md.)
## R11 background craft, visuals, editor, UI
41. Background craft: same physics, cheaper exact-enough solvers, no statistical shortcuts; "plus support to compute one mission independently if it doesn't interact with other vessels, and efficiency in our timewarp solvers. We have three tiers of fidelity, and the last two are nearly the same: physics warp (atmosphere, near another craft), coast warp, and inactive craft coast warp."
42. Visual style: engineering-tool clean for part view/systems, "plus a single exterior view for each part that is realistic-lite for drawing in flight and a single layer in ship view."
43. Editor: drag-and-place + drag-handle parameters + readable craft text file, constraints checked live.
44. Existing UI work: FREEZE it, redesign later.
## R12 structure, failures, performance, aero
45. Structure: rigid dynamics + computed load paths and margins per joint/part; flex modes later only where consequential.
46. Failures: physics-caused, wear/fatigue, manufacturing/quality. "Always a cause. Never failure for the sake of failure ... if we don't simulate what causes a failure and instead just claim this should fail because the merlin engine has this operational history, that should not be a failure."
47. Performance: ~10 craft for live play + hundreds of debris pieces (deleted quickly; "an in-orbit collision is a failure case from statistics/generalized debris models, not colliding with a piece of debris created years earlier"). Other ships computed offline/all at once "but ONLY when their route only depends on celestial bodies ... you cannot do this through an interaction with a refueling station". "for heavy warp you MUST be able to sustain 100,000x effective warp with those 10 crafts. For the active craft, benchmark remains 5k parts ... you can launch an offline run of a 5k part craft and THAT run can hit 100,000x warp, even with all the exceptionally complex interacting systems on that vessel."
48. Aero/thermal: runtime cell/panel model + boundary layer, calibrated by offline CFD tables (e.g. SU2) per shape class; validated against published entry data.
## R13 proof and process
49. v0.2 proof: Electron/Falcon 1 from generic parts; F9 + Dragon rebuilt on it; warp + scale benchmarks; "Also 5000 part craft online, active, burning up in atmosphere, hundreds of pieces of debris being created, obviously FPS will drop during this but even in a radically extreme scenario no less than 10fps. Also we should have at least one probe, maybe new horizons or DART."
50. Design phase: SEVERAL LIVE DESIGN SESSIONS (one topic each).
51. Research: measured spikes on the risky items (warp invariance, multi-rate stepping, adoption builds); literature for the rest.
52. Cadence: "A director session with sunscatter will be more interactive. Directed look regularly and constant feedback."
## R14 discovery, science, comms, modding
53. Discovery: only what instruments measured; ground truth separate from the player's knowledge; Solar System starts at present-day real knowledge with real uncertainty.
54. Science: physical instruments produce data (downlinked over real links). Converting data into advancement is ABSTRACTED: thousands of defined data fields and concrete milestones, each with a fixed, logical reward (contracts unlocked, grants, prestige, contract completion, science points in a category). 5-20 science categories (Director to propose); R&D generates points in the chosen category; one-time bonuses from data or part-use milestones (e.g. restart a chemical engine N times -> propulsion points). Some categories advance on their own (the world), advanced aerospace ones need R&D or off-world discoveries. Examples: first chip made in space; Titan density curve + composition; biology experiment in interplanetary space; a human back after 5 years in zero-g; a GEO launch; extrasolar life (one milestone per data field); ICF drive pulse characteristics in orbit. "each colony and controllable ship has a record of what technologies they know of and what discovery milestones they have reached ... For anything taking place aboard that colony/ship we pretend they only have the tech that is stored locally. Results can be transmitted, but always travel at the speed of light." (e.g. a physics breakthrough near a black hole in another system; Earth may not know for centuries.)
55. Comms: link budget + ground network + relays; bandwidth and scheduling as resources.
56. Modding: data-first open formats. "make sure there is a clear distinction somewhere when a game instance becomes modded ... Within the original 'in universe' ranges that you can edit things, there is complete freedom, but stepping outside that means making a mod with documented changes that can be saved by itself ... anything the player can do in game (intended) has an interface in game."
## R15 industry, colonies, logistics, time
57. Factories: "almost entirely" process chains with real inputs, but "no physical supply chains, everything gets added to one inventory" and recipes kept short: "Instead of tank wall, a fuel tank has all its components, plus something along the lines of 'structural metal sheet' and 'insulation' and 'pipe'. Just make smart decisions to avoid scope explosion. None of these decisions except the first one are final or definitive."
58. Colonies/stations: same component model as ships "but no structure, all geometry and arrangement/placement abstracted away."
59. Logistics: only real flights move mass, PLUS in-space infrastructure (depots, tethers, mass drivers, beamed power) as physical assets.
60. Over 10,000 years the player is the company, persisting, which becomes the backbone of humanity's expansion; "Politics very abstract".
## R16 inventory, interstellar, keep, engine
61. Inventory: "One inventory on EACH PLANET/BODY"; moving goods between bodies takes a real flight.
62. Interstellar physics carried from day one: special relativity for craft; stellar motions over millennia; interstellar medium; galactic tide / perturbations.
63. Keep as-is: ephemeris + N-body + frames; determinism + warp tests; MCP + headless mission tests; knowledge/sensor model (D106).
64. Engine: keep Bevy; `sim` stays engine-free.
## R17 process
65. First design session: the craft data model.
66. The ~50 owner questions of 30-31 Oct: DROPPED (defaults stay frozen in code; re-ask only what the new design needs).
67. Code during the design phase: land stash@{0}, then only research spikes.
68. The owner's calls: vision + pillars + scope; look and feel; fun / gameplay balance. Architecture is the Director's to make and report.
