# Decision Log: Sunscatter v0.2

This log records decisions that have been made. Each entry is numbered and dated. When a decision is superseded or made redundant, the old entry is **deleted** and its number is not reused; git history keeps the record. If a major reversal needs its reasoning preserved, the new entry says why. Proposals that have not been decided belong in design documents, not here.

Status values: **Decided** · **Direction** (settled in principle, details open)

---

## Project

**D001: v0.1 is archived and v0.2 is written from scratch.** *Decided, 2026-09-25.*
v0.1 now lives in `archive/v0.1/` and still builds and runs there, with its toolchain and lockfile pinned. v0.2 may port v0.1's algorithms and data, but not its code structure, and anything ported is re-tested. The reasons are in [lessons-from-v0.1.md](lessons-from-v0.1.md).

**D002: The game is about mission design, flight and logistics at real scale, and it is 3D.** *Decided, 2026-09-25.*
It is not only a space game. Designing missions, flying them, and running many of them at once efficiently are the core.

**D003: Performance is a central goal.** *Decided, 2026-09-25.*
Performance is a design constraint from the first day, not a later polishing step. Every system has a budget.

## Scope

**D004: The simulated space is a sphere of radius 100 light-years, containing roughly 10,000–15,000 star systems.** *Decided, 2026-09-25.*
The engine is designed around this number. More of the starfield may later be offered as a data download. Supporting the whole galaxy is not a goal.

**D005: The game covers about 1,000 years of time.** *Decided, 2026-09-25.*
Ephemerides, the precision of the time type, and orbital drift models only need to be valid for this window, plus a margin.

**D007: Development starts with the Earth–Moon system, but every architectural choice must allow interstellar play later.** *Decided, 2026-09-25.*

## Physics and motion

**D008: Ship trajectories use N-body gravity. There are no sphere-of-influence (SOI) transitions.** *Decided, 2026-09-25.*
Patched conics may appear only as tools for a first guess when planning; they never decide motion. Overlapping SOIs, barycenter special cases and "which body is this ship orbiting?" logic must not exist in the physics. The proposed model is in [design/motion-model.md](design/motion-model.md).

**D009: Celestial bodies follow precomputed paths ("rails").** *Decided, 2026-09-25.*
- Most bodies use orbital elements plus drift terms.
- Systems with strong interactions store precomputed N-body results as lookup tables.
- Every body's position is a *pure function of time*: it never depends on when a system was generated or what the player did before.

**D010: Stars move in straight lines.** *Decided, 2026-09-25.*
Over 1,000 years they barely leave the 100 light-year region, so galactic orbits are not modeled.

**D011: Navigation is built on very advanced N-body trajectory-planning tools.** *Direction, 2026-09-25.*
Because trajectories cannot be read as simple conic sections, planning tools are a core feature and not an extra.

**D012: Relativity is part of the physics in 3D.** *Direction, 2026-09-25.*
Special relativity governs ship motion, and ship proper time is tracked separately from the global coordinate time.

## The universe's contents

**D013: The population follows the "actually exists" rule.** *Direction, 2026-09-25.*
- The model covers the bodies that statistically *should* exist, not just the ones we have detected.
- Real catalogs are used where data exists.
- The rest is filled with statistical models and AI-assisted filling from templates.
- This can include black holes or a neutron star within 100 ly, if the statistics call for them.

**D014: Bodies are split into full bodies and procedural bodies.** *Direction, 2026-09-25.*
- **Full bodies** are named and simulated on orbits. The draft rule:
  - planets with radius over 100 km
  - moons with radius over 5 km
  - anything notable for its system
  - only objects bound to a star within a cutoff orbital distance
- **Procedural bodies** are asteroids and rogue planets. They exist only as statistics until the player detects or tracks one, and then they are generated deterministically.
- The size threshold is much larger for rogue planets.

**D015: Every full body is defined in a minimal form.** *Direction, 2026-09-25.*
Everything else about a body, including its deterministic position, is generated when needed.

**D016: Planet surfaces start simple.** *Decided, 2026-09-25.*
The first version uses textured ellipsoids. A later version adds a coarse heightmap with procedural detail. Surfaces are not central to the game.

## Ships and gameplay

**D017: Trade routes are tools for managing missions, not abstract cargo.** *Decided, 2026-09-25.*
- Every ship is a full ship with its own N-body trajectory.
- Trade routes are a system for creating missions efficiently, setting navigation and burn parameters, and managing many simultaneous missions.
- AI agents might be integrated here.
- There is no abstract "cargo capacity" layer.

**D018: The ship model combines a part system with universal non-physical systems layered on top.** *Direction, 2026-09-25.*
- In scope:
  - basic structural mechanics
  - simple aerodynamics, enough for spaceplanes
  - a cell-based heating and radiation model
  - more to come
- Details have not been discussed yet.

**D019: Colonies are abstracted but deeper than in v0.1.** *Direction, 2026-09-25.*
The design is still to be decided, and it has little effect on the rest of the architecture.

**D020: An engine and propulsion simulator is planned as a feature.** *Direction, 2026-09-25.*
- It is a separate solver: a 2D axisymmetric cell-based flow model with combustion, fusion and antimatter, and no turbulence modeling.
- Players can design an engine's shape and receive its performance.
- The game uses the resulting performance maps and never runs the solver in real time.
- Part designs made procedurally in general follow the same pattern.

## More decisions

**D022: The highest time warp is 1,000,000x.** *Decided, 2026-09-25.*
The game is about realistic progression across many simultaneous missions, not one mission at a time. Continuous warp has to cover interstellar cruises: 43 years to Alpha Centauri at 0.1c takes about 23 minutes. Skipping ahead by jumping to events is not the main mechanism. A sandbox mode exists alongside the progression game.

**D023: Spheres of influence are replaced by the motion model.** *Decided, 2026-09-25.*
- The physics has no reference bodies. Anchors exist only for precision, and a CI test checks that trajectories are the same under different anchor choices.
- The body tree clusters distant subtrees into point masses.
- **Gravity sources whose tidal acceleration (relative to the anchor) is below a threshold are not simulated in full:** they pull the vessel exactly as they pull the anchor, so they cancel. The set is re-checked every step and only grows within a segment.
- Rails-or-table is decided by measuring how well a fit matches.
- See [design/motion-model.md](design/motion-model.md).

**D024: A ship's precomputed trajectory is its actual path.** *Decided, 2026-09-25.*
- Coast segments are computed once and stored.
- Time warp only changes how fast the stored segment is played back, so a trajectory is bit-identical at every warp level.
- Computing a segment in chunks must give exactly the same result as computing it in one go.
- Rendering never integrates anything.
- The planning horizon is bounded, but it effectively covers the 1,000-year window.
- *Reason:* v0.1's worst bug class was behavior that changed with time warp: flashing trajectories, and sometimes different physics.

**D025: Ships do not attract each other. The ship physics model is much richer than KSP's.** *Decided, 2026-09-25.*
It includes radiation pressure, drag in very thin upper atmosphere, rotation that persists through coasts, and similar effects.

**D026: Determinism means exact agreement, not accuracy to reality.** *Decided, 2026-09-25.*
- Celestial body positions must agree to better than 1 m across machines, saves, generation order, visit history and warp settings.
- They do not need to match real life that closely.
- Simplified sets of perturbing bodies are allowed as long as the assumptions are applied consistently and recorded.

**D027: The game starts in 2030, and the player leads an organization.** *Direction, 2026-09-25.*
- Which kind of organization is still open: a NASA-like agency, a SpaceX-like company, or an international body.
- Funding and science are both mechanics, in simplified form.

**D028: Platforms and performance target.** *Decided, 2026-09-25.*
- **macOS and Windows** are required. There is **no web build.**
- **Performance target:** 60 fps on the reference Mac, an Apple M2 Pro MacBook Pro (Mac14,9) with 16 GB of memory, with **10 or more ships in flight**, at the lowest graphics settings.

**D029: Warp while thrusting.** *Decided, 2026-09-25.*
- **Physics warp** runs full rigid-body physics (structure, aerodynamics, thrust) at up to about 4x. The exact limit is whatever proves stable.
- **Burns during on-rails warp** are allowed at any warp level if the burn meets certain criteria. The player sets them up in a dedicated burn interface, inspired by the KSP *Persistent Thrust* mod.

**D030: Multiplayer is not planned before v1.** *Direction, 2026-09-25.*
- The long-term idea is separate exploration timelines that don't affect each other's cause and effect, where space stations built by other players might be visitable somehow.
- The architecture keeps this possible, mainly through determinism (D026).

**D031: The language is Rust, the simulation crate does not depend on any engine, and a Bevy prototype comes before choosing the engine.** *Decided, 2026-09-25.*
- The prototype tests whether Bevy can meet D028.
- Godot is ruled out, because its double-precision builds are unofficial and have bugs.
- A custom wgpu renderer is the fallback.

**D032: Vessels are rigid bodies whose parts can break.** *Decided, 2026-09-25.*
- Structural reinforcement can be added.
- Materials are unlocked through tech.
- No stacks of parts held together by struts.

**D033: Ship systems are layered on the part model, with moderate realism.** *Decided, 2026-09-25.*
- **Systems:** power, structure, consumables (including fuel), avionics, life support, comms and crew.
- **The realism rule:** nothing contradicts reality, but complex engineering is abstracted generously.
- **Power and fuel follow semi-physical paths through the ship.** How the ship is laid out has consequences.
- **Radiation** is modeled from both outside sources and sources on the ship.

**D034: Control depends on the speed of light.** *Decided, 2026-09-25.*
- The player can control any actual crew member.
- Uncrewed ships are commanded with the real light delay to the nearest human.
- A later technology (AGI or full autonomy, possibly brain uploads) allows direct control of probes.

**D035: Every ship has a simple operating system and supports scripting.** *Decided, 2026-09-25.*
- Arbitrary scripting is required, because distant probes have to be controlled by scripts.
- Scripts must integrate with a development environment that AI agents can use.

**D036: Ships are built in a constrained, programmatic way.** *Decided, 2026-09-25.*
- Specifying coordinates is a valid way to build a ship.
- A free-form editor is in scope but comes later. v0.2 uses blocks with made-up thrust at first, then predefined ships.
- Parts can be procedural or designed, and designing and sharing parts is a feature.

**D037: Earth uses real elevation data at low resolution. Oceans are solid for now.** *Decided, 2026-09-25.*
Oceans get proper behavior when aerodynamics is implemented.

**D038: Landings in milestone 1 use parachutes.** *Decided, 2026-09-25.*

**D039: The workflow uses design documents, feature documents and tests. OpenSpec is removed.** *Decided, 2026-09-25.*
- Studying KSP mods and other prior work is a standard strategy for hard problems. See [process.md](process.md).

**D040: v0.2 is milestone 1: one ship flying in the Earth–Moon system, plus the foundations.** *Decided, 2026-09-25.*
- Graphics are close to final: atmospheric scattering and lighting, heavily optimized.
- The loop is launch, fly and land.
- How the game progresses beyond that will be defined over time.

**D041: What kind of game this is.** *Decided, 2026-09-25.*
- It is not a casual game: it is a challenging, vast environment for spaceflight and exploration.
- Nothing in the game contradicts science.
- The player's own AI agent, connected to the game, acts as the tutorial and assistant.
- See [vision.md](vision.md).

**D042: Accuracy tolerances for fits.** *Decided, 2026-09-25.*
- Planets: 1 km over 1,000 years.
- Moons: 100 m.
- Earth and the Moon near the start date: 10 m.
- These measure accuracy against our reference integration. Determinism (D026) is exact regardless.

**D043: Ship scripts are WASM modules.** *Decided, 2026-09-25.*
- Players can write scripts in any language that compiles to WASM.
- Scripts run sandboxed and deterministically, with an execution budget per tick that models a ship computer's limited power.
- Scripts are plain files on disk, driven by a command-line tool, so AI agents can work with them directly.

**D044: The game runs an MCP server for AI agents.** *Decided, 2026-09-25.*
- It exposes game state, planning tools and script deployment.
- The player's own agent (Claude Code or another) connects through it, and acts as the tutorial and assistant (D041).

**D045: The engine is Bevy, with the version pinned per milestone.** *Decided, 2026-09-26.*
The prototype showed no blocking issues and plenty of performance headroom: 11 vessels at 1,000,000x warp ran at ~265 fps on the reference Mac. This resolves D031's engine question.

**D046: Terrain and texture data are committed directly at reduced resolution.** *Decided, 2026-09-26.*
- Earth and Moon heightmaps and colour maps come from public-domain sources: ETOPO 2022, LRO LOLA, NASA Blue Marble and LROC.
- They are downsampled to sizes that can live in git: each file under ~50 MB, about 60 MB in total.
- They should be close to final, not placeholders.

**D047: Terrain is physical.** *Decided, 2026-09-26.*
- Altitude, landing and collision use the same heightmap that is rendered.
- The sim samples it deterministically.
- Oceans are solid at sea level (D037).

**D050: The tracking station is a separate screen that shows the map.** *Decided, 2026-09-26.*
It lists tracked vessels (and later asteroids), lets you focus and switch to a vessel, and draws on ideas from KSP and v0.1's tracking station.

**D051: Each body is a data directory with separate sim and game files.** *Decided, 2026-09-26 (autonomous session; for the owner's review).*
- `data/bodies/<body>/` holds `body.ron` (physical data, read by `sim`), `visual.ron` (looks, read by `game`), and the committed maps (`height.png`, `color.jpg`).
- The heightmap and sea level are physical data; the renderer gets heights from `sim`, so there is one owner of the surface.
- Adding a body means adding its directory and an ephemeris entry, with no code change.

**D052: Rendering uses physical light units and Bevy's built-in atmosphere.** *Decided, 2026-09-26 (autonomous session; for the owner's review).*
- Physical units: sunlight is about 126,600 lux at 1 AU, from the Sun's luminosity in data. How light reaches each object and how exposure works: D055.
- Atmospheres use Bevy's Hillaire 2020 implementation (its lookup tables inside an atmosphere, raymarched from outside it), with scattering coefficients and scale heights as data. We work around its runtime-toggling bugs rather than maintain our own scattering, unless it proves insufficient.

**D054: Map view is decided per object, by pixel size.** *Decided, 2026-09-26. Replaces D049 and D053.*
- An object (body or vessel) is in map view when its sprite is under 1 px and its orbit radius is at least 1 px, with hysteresis. Sizes use one scale: pixels per metre at the camera's distance to its focus.
- Its icon, orbit line, hover ring and name, and Ap/Pe markers are shown only while it is in map view. There is no global map mode and no map-view key.
- On hover, the heaviest object under the cursor wins (a planet over its moons).
- Details: [map-view-lighting-controls.md](features/map-view-lighting-controls.md).

**D055: Lighting is physical and per body, at a fixed exposure.** *Decided, 2026-09-26; exposure revised the same day.*
- Stars have a luminosity in data. Each lit object receives flux L/(4πd²) from every nearby star at *its own* position, plus light reflected by nearby bodies (albedo, phase angle) and eclipses.
- A single global light evaluated at the camera is not allowed (it was KSP's interstellar lighting bug, and ours for Earthshine).
- Exposure is fixed (no eye adaptation): night sides are realistically dark. Eye adaptation was built and removed on the owner's review, because it lit night sides up and made day sides blinding.

**D056: Orbit lines end after one revolution about the dominant body.** *Decided, 2026-09-26.*
- A line runs until it has swept 360° about its dominant body, with that body dominant throughout; after a change of dominant body, counting restarts. There are caps on length, and the length is a setting.
- "Dominant" means tidal/relative dominance (Laplace-sphere style), not raw force, so the Moon's dominant body is Earth.
- This is display logic only; physics never uses it (rule 1).

---

**D058: Visuals are made minimal and stable before anything else; what does not work is simplified, not debugged.** *Decided, 2026-09-26.*
- If a visual feature misbehaves, replace it with something simpler that works instead of spending sessions debugging it.
- The next chunk starts by fixing the visual bugs the owner still sees (haze, blur, flashing, dark horizon, navball flicker), under this rule.
- Haze gets physical tuning (no setting: D073).

**D059: Terrain detail below the heightmap's resolution is procedural and lives in `sim`.** *Decided, 2026-09-26.*
Deterministic fractal detail shaped by the local slope and roughness of the real data, so physics, landing and rendering see the same surface (D047); matching detail in the colour.

**D060: The next theme is ultra realism, with ships as abstracted blobs.** *Decided, 2026-09-26.*
- Every feature: the data model first, then the user interface.
- Before parts (D100), a **test craft** stood in, and it stays as the debug craft: one placeholder part for the whole ship with one state, fuel, thrust and Isp (temperatures per cell, D065); shaped like a ship (a 3D model), fixed landing gear allowed as geometry; crewed, minimal simple resources, no life support, no staging (D064).
- Order: visual fixes (D058), a foundation pass (the high-severity code-review items), the test craft's data model, relativity and light, aero/heating/hitbox (D061), then the UI (burn planner, powered Moon landing, rendezvous), then the MCP server (D063).

**D061: Aerodynamics, heating and collision act on cells of the ship's 3D model.** *Direction, 2026-09-26.*
- Medium to high fidelity, still efficient, somewhat more complex than KSP. Nothing per part: in flight a ship is a 3D model, and aero forces, heating and the hitbox are computed on cells of that model.
- Structure comes later and will split the model into parts; heating's effects on heat-dependent subsystems, and heat transfer between parts and along heat-transport systems, come with it.

**D062: Rails warp is disabled below a per-body altitude.** *Decided, 2026-09-26.*
The altitude is data per body, chosen by rule: the top of the atmosphere, or on airless bodies the highest terrain plus a margin.

**D063: The MCP server comes before ship scripting; light delay starts with telemetry.** *Decided, 2026-09-26.*
- The MCP server (state, planning tools, commands) comes first, on a shared command API that WASM scripting (D043) later builds on.
- In the first realism pass, light-time delay applies to telemetry and ground commands only; the crew flies without delay. Ships get proper time (D012).
- Saves need not stay compatible across versions yet.

**D064: The test craft is a debug craft with a realistic engine, and debug mode is one switch.** *Decided, 2026-09-26.*
- The engine is realistic but abstracted: thrust, Isp (vacuum and sea level) and mass flow as numbers, no engine simulation and no elaborate plume graphics.
- **Debug mode** turns on, together: infinite propellant, no overheating, infinite impact tolerance, unbreakable parachutes (D079). Otherwise the craft is destroyed at its limits.
- No heat shield. Owner, 2026-10-01: a nose-first reentry from LEO overheating the craft is the accepted result (surviving it is piloting); a heat shield comes later as a real part once parts exist (D060); limits are never loosened to make a craft survive.

**D065: Heat is a network of surface cells and interior volume nodes, independent of parts.** *Decided, 2026-09-26; revised the same evening (owner).*
- **Skin cells:** the surface split into cells as small as they can be without significant performance drops, adaptive to the geometry. They take heat from the flow and sunlight, radiate, and conduct to neighbours.
- **Volume nodes:** a coarse 3D grid over the craft's interior (≈1 m for the test craft, adaptive for big craft), each with the heat capacity of the mass inside it, conducting to neighbours and to the skin cells above it. This replaces the single internal temperature.
- **Heat sources go into the nodes that contain them:** engine losses into the chamber and mount nodes, reactor waste heat (thermal minus electrical) into the reactor's nodes, electronics and crew their power draw.
- **Heat transport systems are explicit links:** coolant loops carry ṁ·c_p·ΔT between the nodes they pass (only while pumped, costing power); radiators are high-emissivity skin cells on a loop; heat pipes are high-conductance links within their working range.
- **Parts, later, own the nodes and cells inside their shape** (a big part spans many nodes), with limits per node; joints are links with contact resistance; heat-dependent subsystems read their node's temperature.
- Exceeding a skin or node limit destroys the part cleanly (no ablation yet); debug mode disables it (D064).

**D066: Ground contact is rigid-body contact.** *Decided, 2026-09-26.*
Contact points on the gear and hull touch the physical terrain with spring-damper contact and friction; a craft can bounce or tip over, and impacts are judged per contact point.

**D067: The player and their agent act from control locations.** *Decided, 2026-09-26.*
- A control location is a crewed vessel or a mission control (on Earth now; later colonies with one, in any star system). Being at a location means being there: what you see arrives with light delay from everywhere else, and your commands to other places travel at light speed.
- The tracking station is the view from mission control; flying a crewed vessel means being its crew.
- Each location's technology is defined by a knowledge file for its computer. Switching between locations is allowed at will, but knowledge never moves with the player; each location is assumed to make its own decisions.

**D068: Communication goes through a simulated relay network.** *Decided, 2026-09-26.*
- Ground stations (the DSN complexes) are data; any colony or vessel with a sufficient antenna relays. Links need line of sight (occlusion by bodies) and have a data rate from a link budget.
- Delay is the light time along the path.

**D069: The MCP server uses local HTTP, and the agent is at a control location like the player.** *Decided, 2026-09-26.*
Streamable HTTP on localhost, off by default. Agent commands have the origin of the agent's control location (D067), so light delay applies to them exactly as to the player's.

**D070: The aerodynamics model's required reach.** *Decided, 2026-09-26 (owner).*
Simplified is fine, as long as it models, semi-accurately: lift; rocket flight at low and high altitude at all speeds (subsonic, transonic, supersonic, hypersonic, rarefied); and entry from interplanetary velocities (to ~16 km/s). No flow-field solver. The model's limitations are recorded in [design/aero-thermal.md](design/aero-thermal.md).

**D072: The 3D view renders at the window's full resolution.** *Decided, 2026-09-27 (owner); revised 2026-10-26 (owner: one look, no graphics settings, D073).*
No render-scale setting: the 3D scene renders at the window's physical pixels (on Retina, every one), so it stays crisp, and the look is made cheap enough for that (D073's frame-rate rule).

**D073: Graphics are one look: simple, crisp and functional, with no graphics settings.** *Decided, 2026-09-28 (owner); revised 2026-10-25 and 2026-10-26 (owner). Replaces D048's five tiers.*
- The owner's number-one rule: a 100% accurate spaceflight simulator. Cartoon looks are welcome as long as the physics is realistic.
- Minimal means simple, never blurry: "sharp, crisp, clear surfaces, recognisably water or land or earth, just not necessarily needlessly complex" (2026-10-25).
- Surfaces come from real data (satellite colour, elevation, the sea mask), sharpened by procedural algorithms: coastlines and zone edges stay crisp at every distance. Each zone (ocean, ice, forest, desert…) is drawn as one consistent procedural texture, not as a magnified photograph. Details: [crisp-surfaces.md](features/crisp-surfaces.md).
- The sky is physical, from the atmosphere and the light: Bevy's scattering model (D052), raymarched from outside an atmosphere so the ground stays clear from orbit. Stars, every star's light, eclipses and planetshine (D055), shadows, a basic sun flare.
- One look for everyone, no graphics settings (2026-10-26: "delete all graphics settings, there should just be one graphics setting … that is so well optimized it doesn't matter"). The tiers and their code are deleted. All visuals are data-driven for any body. What each part of the look costs is measured (the benchmark turns parts off, never the player).
- The look runs full screen at the display's frame rate on the reference Mac, at full resolution (D072).

**D074: The test craft is a finned rocket, stable nose-first.** *Decided, 2026-09-28 (owner).*
Its shape makes it aerodynamically stable nose-first at every Mach number, on ascent and on entry. It is still a debug craft (D064): flown with debug mode on, so reaching orbit, heating and the parachute's landing speed are not design problems; a real staged rocket comes with the parts model.

**D075: Attitude hold has five modes, and planned burns are flown by the attitude.** *Decided, 2026-09-28 (owner).*
- Modes: **stability** (kill rotation, hold the attitude), **prograde**, **retrograde**, **target**, **maneuver**. Prograde and retrograde follow the navball's speed reference (orbital, surface or target), stated on screen.
- Thrust is always along the craft's actual attitude. A planned burn is flown by the maneuver hold: the craft turns with its own attitude control and the burn's thrust follows the attitude, live and in the predicted plan alike.

**D076: Apsis markers are extrema of distance on the predicted trajectory.** *Decided, 2026-09-28 (owner).*
A marker is placed wherever the predicted trajectory (through planned burns) reaches a local maximum or minimum of distance from the dominant body, kept only when it differs from its neighbouring extremum by a significant fraction of the altitude. Nothing comes from osculating elements, so an escape trajectory has no apoapsis. Every closed orbit shows an Ap and a Pe per revolution: where no extremum is significant (a near-circular orbit), its highest and lowest points, one of each per revolution, shown as plain Ap and Pe (owner, 2026-10-01: no label, because they hold still: no marker moved before the clock reached it in 36,000 frames per warp at 1x to 1000x, and every warp showed what 1x shows at the same time; test `near_circular_markers_hold_still_at_every_warp`). Altitudes are above the reference ellipsoid under the apsis, like the flight panel's.

**D077: Engine heat leaves with the exhaust and by radiation from the nozzle; the interior warms only through the mount.** *Default taken 2026-09-30 (owner unavailable; reversible).*
The chamber's wall heat returns to the flow (regenerative cooling) and is not a heat source. The radiatively cooled nozzle extension absorbs a share of the jet power sized so it runs near 1400 K (coated niobium), radiates it away, and conducts to the craft only through its mount (~1 W/K). Each skin material has its own temperature limit. Numbers and sources in [design/aero-thermal.md](design/aero-thermal.md). Replaces the earlier 2e-4 of the jet power into the mount's node, which drove the interior to 1100–1500 K on long burns.

**D078: Attitude control is only what real actuators give: RCS couples and the engine's gimbal.** *Default taken 2026-09-30 (owner unavailable; reversible).*
A craft declares its reaction-control thrusters (thrust, Isp, fore and aft ring stations, quad radius); SAS and the pilot get the pure couples they make (pitch and yaw `2·F·Δz`, roll `8·F·r`) plus the gimbal while the engine runs, and nothing else: no abstract torque, no reaction wheels (none big enough fly on craft this size), no special case on the ground. The thrusters burn the main propellant in live flight (their share of full command times their flow); with none left (outside debug mode) only the gimbal turns the craft. The test craft carries Apollo SM-class 445 N thrusters: 8.0 kN·m pitch and yaw, 3.7 kN·m roll (was 40/40/20 kN·m). On rails (coasts, planned burns) the torque limit holds but RCS propellant is not yet counted. Owner, 2026-10-01: SAS balancing the craft on two feet on a 20° lunar slope (3 kN·m of the 8 available) is kept: real actuators, real result; the pilot can turn SAS off.

**D079: Parachutes open in a sourced sequence and an overload tears them; nothing refuses the command.** *Default taken 2026-09-30 (Director, owner unavailable; reversible).*
- **Sequence** (`sim::chute`): **P** arms it. The drogue opens at once; the main opens below `main_height` above the ground once the drogue is out of its reefing, and the drogue is cut away. This is the Apollo, Orion and Soyuz order (Apollo: drogues at 24,000 ft, mains at ~11,000 ft; NASA TN D-7437).
- **Reefing and filling:** each canopy opens through timed reefed stages (Apollo's mains 6.8 % of full Cd·A for 6 s, then 25.7 % to 10 s; its drogues 57 % for 10 s), and its mouth grows at V/n (Knacke's t_f = n·D/V). The fill constants n (7 ringsail, 10 ribbon) are from memory, not checked.
- **Overload fails physically:** a canopy whose drag exceeds its breaking load tears and is lost. A command above `deploy_max_q` is not refused, because no real system refuses one: the pilot fires the mortar, and the canopy holds or not. Real canopies hold somewhat above their design load (Apollo's mains passed ultimate tests at 1.39× the design limit, TN D-7437; a CPAS main took 40,000 lb above its limit undamaged, NTRS 20110011562; CPAS is certified at 110 % q for 3 s, NTRS 20190026520) and fail beyond it (an Apollo gore split at 20,000 to 23,000 lb while disreefing). So the breaking load is the design limit × 1.39, and `deploy_max_q` is the design opening q, shown on the HUD.
- **Simplified, known:** the load is q·Cd·A as the canopy fills, without the added-mass overshoot (Knacke's Cx). A cluster shares it evenly (Apollo's lead canopy took ~1.9× the mean).
- The test craft carries two Apollo drogues (5.5 kPa design q) and four Apollo mains (3.27 kPa), sized for its 5 t landing mass. At a full 20 t the mains tear. Owner, 2026-10-01: keep it, it is what the physics gives (like not reaching orbit without debug mode). A propellant dump is a fine real feature for later.

**D080: The flight view is the navball plus task menus in fixed docks.** *Decided, 2026-10-25 (owner). Replaces D057.*
- "Designated, elegant locations for menus; completely cut unnecessary information." Always on screen: time and warp, the navball cluster, and a menu bar. Nothing else unless the player opens it.
- **Navball cluster:** the essentials only: speed with the orbit/surface/target switch, altitude, Ap and Pe, SAS and its hold modes, throttle, the flight state.
- **Task menus**, each opened from the menu bar or its key, each in a fixed dock: **Orbital** (advanced orbital data, the target, later maneuvers), **Landing/Reentry** (impact, braking, entry and parachute; the impact marker is drawn only while it is open), **Utilities** (a brief status of propellant, thermal, power, life support, comms), **Staging**. New tasks add a menu (docking gets a Docking menu).
- Menus are hierarchical: a short summary on top, sections that expand to everything there is to know. Details: [flight-menus.md](features/flight-menus.md).
- The detailed ship-systems view (an abstracted schematic of the ship, no planets) is still a separate screen, built once ships have systems; Utilities is its brief summary.
- GUI style: clean sci-fi, function first.

**D081: Scale: 50 craft of ~300 parts to 1,000,000×; live physics for a few, "limbo" for many.** *Decided, 2026-10-26 (owner).*
- The target: 50 craft of ~300 parts each in one game, at every warp up to 1,000,000×.
- **Live physics** (fixed ticks, contact, full aero) is needed for 1–3 craft at a time, at low warp.
- **Limbo:** 10 or more craft at once on rails under continuous forces: low thrust, upper-atmosphere drag, radiation pressure. Their internal systems (thermal, propellant, power, consumables) evolve on fixed time lattices. Nothing a limbo craft does may change with the warp rate: chunked equals single pass (rule 4), tested.
- Craft creation stays with the owner. A synthetic stress-craft generator, with placeholder part properties, exists only for benchmarks and tests.
- Live craft cap the warp: rails warp is refused while any craft is in live physics, with the reason shown.

**D082: Craft within 10 km share live physics and collide.** *Decided, 2026-10-26 (owner).*
Craft within 10 km of a controllable craft in live physics fly live with it in a shared local frame, and craft collide with each other (rigid-body contact). Docking ports come later.

**D083: Debris is a full vessel until it is dead, uncontrollable and 10 km from every controllable craft.** *Decided, 2026-10-26 (owner).*
A piece that breaks off becomes a vessel with its own trajectory, aerodynamics and heating. It is deleted only if it is totally uncontrollable and dead **and** 10 km or more from every controllable craft; until then it is fully simulated.

**D084: The Solar System follows the D014 rule, largest bodies first.** *Decided, 2026-10-26 (owner).*
First every planet, dwarf planet and moon over 100 km radius, through the motion-model pipeline (rails or tables by measured residual). Each gets its physical data: GM, radii, rotation, J2, and an atmosphere where real (Venus, Mars, Titan). Moons over 5 km follow.

**D085: Aerodynamics: full-body cells with boundary-layer turbulence, fast enough for breakups.** *Decided, 2026-10-26 (owner).*
Close the D070 gaps listed in `docs/design/aero-thermal.md`. Model laminar–turbulent transition and its effect on friction and heating over the whole body. Stay fast enough for 1–2 large craft live, or many small pieces after a breakup.

**D086: Committed map data may grow to about 150 MB for the Solar System.** *Default taken 2026-10-26 (owner away; reversible). Revises D046's ~60 MB total.*
- D046's rule stands: public-domain sources, downsampled, each file under ~50 MB, close to final. The total grows to ~150 MB so the planets and moons get real maps like Earth's (D073): heights where a global DEM exists, colour, zones, roughness.
- Bodies without global data get procedural surfaces from their measured statistics (albedo, colour, crater density), stated as such.

**D087: Exposure follows the starlight at the camera.** *Default taken 2026-10-26 (owner away; reversible). Refines D055's fixed exposure.*
- The camera's exposure is scaled by the stellar flux at the camera's position (a photographer setting exposure for the light where they are), never by the scene's luminance. Near Earth nothing changes; at Saturn (1 % of Earth's sunlight) a day side reads like Earth's, and night sides stay as dark relative to day sides as D055 requires. Always on (no settings, D073).

**D090: Vessels feel the bodies' measured gravity fields, as spherical harmonics truncated by significance.** *Decided, 2026-10-26 (owner: "accurate gravity fields"). Refines D023 and motion-model §4.*
- A body's field beyond its point mass is its published spherical-harmonic model (fully normalized C̄nm, S̄nm, with the model's reference radius and GM) in its own rotating frame, through its rotation model (`sim::gravity`, Pines' singularity-free recursion). A body without one keeps its J2 as the degree-2 zonal case of the same model. Every vessel force goes through it: coasts, limbo, live ticks, planned burns, the landing predictor, the bubble.
- **Truncation, in the spirit of the cutoff:** a vessel includes a body's degrees up to where the bounds of all higher degrees together stay under a hundredth of the cutoff (10⁻¹⁴ m/s²): at the vessel for its anchor's field, and for another body's field their change between the anchor and the vessel, as the cut rule does for pulls. The degree only grows within a segment and is stored with it (chunked = single pass, rule 4); it depends on the vessel's position, so a far vessel pays for nothing.
- **Data** in `data/bodies/<body>/gravity.ron` (D051), generated from the public models and truncated to the degree the game's lowest orbits need (measured; sources, degrees and frames in [solar-system.md](features/solar-system.md)).
- The proper-time potential includes the field to the same degree (J2 is ~3e-13 of the rate in LEO).

## Ships: parts, systems, failures (2026-10-26 evening interview)

The owner's interview: [interview-2026-10-26-parts.md](plans/interview-2026-10-26-parts.md). The design: [design/ship-model.md](design/ship-model.md). Numbers D091–D099 are left to the evening's other workstreams.

**D100: Ships are parts, components and layers; the first vehicle is Falcon 9 Block 5 with Crew Dragon.** *Decided, 2026-10-26 (owner). Details D018, D033.*
- A **part** is a physical object with a shape, a place and mounting points (a tank, an engine, a grid fin, the capsule). A **component** is equipment inside a part's volume, with mass, volume, power and heat but no outer shape (a helium bottle, a battery, a flight computer, a sensor, a valve, a seat). A **layer** is a network, not placed as a part: structure (joints), plumbing, electrical, data, thermal, mechanisms.
- The first vehicle is the Falcon 9 Block 5 with Crew Dragon, every part and component adjustable and interchangeable. The goal is a whole Crew mission (launch, booster landing, ISS docking, entry, splashdown); the first step is the complete Falcon 9 in the part system, its controls and systems at the proper realism, flyable by the owner.
- The test craft stays as the debug craft (a craft of one part).

**D101: Every part comes from a generator; real hardware is a catalog entry; properties are derived and published numbers override them.** *Decided, 2026-10-26 (owner).*
- A generator turns parameters into a part or component (shape, mounts, mass, tanks, ports, functions, ratings, failure modes). A catalog entry is a generator, its parameters, its published numbers (each with a source), ratings, era and cost. The catalog holds a large variety of real systems and later goes beyond them; players make their own parts (their own parameter sets now, their own generators later).
- Parameters, including technology progression, are tuned so procedural parts are realistic; "truly published numbers overrule procedural numbers, for example thrust or battery capacity", so overrides are minimal and most things are derived: "build to minimize contradiction". A gap between a published number and what the generator can reach is reported, not hidden.
- In the career a player's part takes its properties from the generators' physics; free numbers are for sandbox only, marked unverified.

**D102: Craft are built in code for now: craft files with mounting points, shapes from a modelling tool, a visual editor later.** *Decided, 2026-10-26 (owner). Refines D036.*
- A craft file lists parts with their parameters, attached through **defined mounting points** ("rather than just anything touching is attached"), with patterns for symmetric copies; components placed in parts; layer connections; joints; the stage sequence and saved functions. Coordinates stay valid for placement. MCP tools let the player's agent build and check craft.
- A part's shape can come from a simple 3D modelling tool: Blender's glTF (`.glb`), mounting points as named empties, part data as custom properties; the physics uses the same shape. A visual editor comes later.

**D103: Craft are rigid until they break; mechanisms move; nothing wobbles.** *Decided, 2026-10-26 (owner). Refines D032.*
- "No bending … the crafts should be rigid until they break, with potentially LIMITED give implemented as a specified allowance in specific scenarios. Don't even touch wobbly rockets, instead just find where they break and model the effects of the minimal structural vibrations." Every live tick each joint's load is computed exactly for the rigid body (the subtree beyond it accelerated by it, against its external forces) and checked against its capacity at temperature.
- Cables bend, intentionally dynamic parts move (legs, fins, hinges, actuators), springs bounce, liquids slosh. Structural vibration is an environment that damages parts (D105), never a deformation.

**D104: Internal systems run as behaviour, abstracted but not too far.** *Decided, 2026-10-26 (owner).*
- **Plumbing:** a lumped network: tank and pressurant pressures, propellant temperature and density, boil-off, line losses, pump inlet pressure (cavitation), settled or floating propellant (ullage), slosh.
- **Engines from published data before the engine designer** (D020): start sequence with limited ignitions and a transient, the steady map (thrust, Isp, mixture ratio against throttle, inlet conditions and ambient pressure), throttle limits, shutdown, gimbal, heat. No flow inside the engine at runtime: what happens there fails statistically.
- **RCS** thrusters are placed components whose torques come from the geometry (for craft of parts; the test craft keeps D078's couples).
- **Power** flows in watts over buses (voltage derived): batteries with charge, temperature and internal resistance, solar cells per skin cell, loads with priorities and shedding.
- **Crew and life support:** the cabin's air, consumables, suits, the crew's acceleration and landing-load limits; radiation dose later.
- **Ground:** the launch pad and its hold-down clamps are in scope; the countdown later, abstracted as a time and funds sink. Nothing else on the ground ("no cars driving around the launch site").
- A controllable craft stays fully simulated until it is destroyed (the booster after separation, D083).

**D105: Failures come from the simulation where it knows, from statistics only where appropriate, and never when unsure.** *Decided, 2026-10-26 (owner).*
- "Anything that would cause a failure in real life that we have the data for and that is within simulated ranges should actually cause the failure. If the simulator is not sure, don't trigger the failure."
- **Physical triggers** fire deterministically: a joint over its ultimate load snaps, a tank over its burst pressure bursts, a part at its destruction temperature is destroyed, between its rated and destruction temperatures heat accumulates as damage over time, an engine on floating propellant ingests gas, below its suction pressure it cavitates. **Accumulated damage:** vibration against the qualification level (Miner's rule), time at temperature, cycles.
- **Statistical** modes only for what is not simulated inside a part (turbomachinery, valves, electronics), from the part's data or else its class's historical rate, at the low end of the rate's range.
- **Fates:** each statistical mode draws its threshold once from the save's seed, the launch time and the part; it fails when its accumulated hazard (which depends on the conditions it saw) reaches it. The same ascent from the same launch time gives the same outcome; a different ascent can differ; reloading never re-rolls; warp never matters (rule 4).
- **Ratings** shown for comparison: a reliability score (1–10), rated life to a 50 % chance of a defect, rated and destruction temperatures, time at temperature, rated and ultimate loads, burst pressure, vibration qualification, rated burn time. Quality is a parameter: a better part costs more and rates higher.
- **Modes have outcomes:** something snapping is not something melting. Each mode of each kind has its outcome (a split, a destroyed part, an explosion, a benign shutdown, a stuck valve…). Explosions release the energy of what was released and damage their neighbours, so failures cascade; no fire.
- **History:** every part keeps its history across flights, and it moves its hazards. Debug mode (D064) turns all failures off.

**D106: Information comes from sensors; the interface never shows something wrong.** *Decided, 2026-10-26 (owner).*
- What the ship cannot measure is shown as a dash ("If you do not have a sensor in your fuel tank, you don't know how much fuel is in there"; sensors are simple components). Values are shown to the precision the sensors give ("You can't get your velocity to 50 decimal places"); the errors themselves are simplified.
- The mechanisms by which the data reaches the player are simulated: sensors, power, data paths, light delay, occlusion, relays and signal strength (D068).
- Scripting (D035, D043) reads the same knowledge. Debug mode gives perfect information instantly.

**D107: The systems screen is a separate screen; flight keeps the essentials and a mini systems panel.** *Decided, 2026-10-26 (owner). Refines D080.*
- "To truly control every bit of the craft you need a whole separate screen." Its main view is the craft, switchable by layer (physical, structural, thermal, electrical, plumbing, data, health); every part and component can be inspected and commanded there.
- There the player defines **stages** and saves **functions**, and pins functions to a **mini systems panel** shown in flight, where they are activated by a click or a key. Space activates the next stage. Flight keeps D080's essentials.

**D108: The craft is flown by hand until scripting; scripting reads the information model.** *Decided, 2026-10-26 (owner).*
The player flies everything by hand for now (including a booster landing). Scripting comes soon, integrated with the data model of D106, and commands go through one command type shared by the screens, the MCP server and scripts.

## Open questions

1. What kind of organization the player leads (D027).
2. Radiation in the ship model (dose, single-event upsets), and how the career's knowledge of a design's reliability grows (D105).
3. The crew model.
4. The colony design (D019).
5. Which model the player's AI agent uses and what it costs the player (D044).
6. Motion-model tunables: the cutoff threshold, opening angle, anchor hysteresis, and choice of coast integrator.
