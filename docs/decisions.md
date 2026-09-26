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
- **Gravity sources below a threshold acceleration are not simulated at all.**
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

**D048: Graphics have five tiers: Minimal, Low, Medium, High, Ultra.** *Decided, 2026-09-26.*
- Minimal has near-zero cost. Ultra looks like KSP1 with visual mods. Every tier is highly optimized.
- Each feature can also be toggled on its own, and tiers are presets of those toggles.
- All visuals are data-driven for any body.
- Every feature's cost is measured.

**D050: The tracking station is a separate screen that shows the map.** *Decided, 2026-09-26.*
It lists tracked vessels (and later asteroids), lets you focus and switch to a vessel, and draws on ideas from KSP and v0.1's tracking station.

**D051: Each body is a data directory with separate sim and game files.** *Decided, 2026-09-26 (autonomous session; for the owner's review).*
- `data/bodies/<body>/` holds `body.ron` (physical data, read by `sim`), `visual.ron` (looks, read by `game`), and the committed maps (`height.png`, `color.jpg`).
- The heightmap and sea level are physical data; the renderer gets heights from `sim`, so there is one owner of the surface.
- Adding a body means adding its directory and an ephemeris entry, with no code change.

**D052: Rendering uses physical light units and Bevy's built-in atmosphere.** *Decided, 2026-09-26 (autonomous session; for the owner's review).*
- Physical units: sunlight is about 126,600 lux at 1 AU, from the Sun's luminosity in data. How light reaches each object and how exposure works: D055.
- Atmospheres use Bevy's Hillaire 2020 implementation (raymarched from Medium up), with scattering coefficients and scale heights as data. We work around its runtime-toggling bugs rather than maintain our own scattering, unless it proves insufficient.

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

**D057: The flight view holds only flight controls; ship systems get their own view.** *Direction, 2026-09-26.*
- The flight view shows vital flight information: time and warp, the navball, trajectory data, and the flight state.
- Ship systems (power, propellant, thermal, crew and so on) go in a separate ship-management view: an abstracted schematic of the ship with no planets drawn. It will be built once ships have systems.
- GUI style: clean sci-fi, function first; dense menus are fine.

**D058: Visuals are made minimal and stable before anything else; what does not work is simplified, not debugged.** *Decided, 2026-09-26.*
- If a visual feature misbehaves, replace it with something simpler that works instead of spending sessions debugging it.
- The next chunk starts by fixing the visual bugs the owner still sees (haze, blur, flashing, dark horizon, navball flicker), under this rule.
- Haze gets physical tuning plus a player setting for its strength.

**D059: Terrain detail below the heightmap's resolution is procedural and lives in `sim`.** *Decided, 2026-09-26.*
Deterministic fractal detail shaped by the local slope and roughness of the real data, so physics, landing and rendering see the same surface (D047); matching detail in the colour.

**D060: The next theme is ultra realism, with ships as abstracted blobs.** *Decided, 2026-09-26.*
- Every feature: the data model first, then the user interface.
- No ship internals or part definitions yet (they will be very complex under the realism standard). A **test craft** stands in: one placeholder part for the whole ship with one state, fuel, thrust and Isp (temperatures per cell, D065); shaped like a ship (a 3D model), fixed landing gear allowed as geometry; crewed, minimal simple resources, no life support, no staging (D064).
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
- **Debug mode** turns on, together: infinite propellant, no overheating, infinite impact tolerance. Otherwise the craft is destroyed at its limits.
- No heat shield.

**D065: Heating is modeled per cell; a part has a maximum skin and a maximum internal temperature.** *Decided, 2026-09-26.*
- Each surface cell has its own skin temperature, so localized heating is captured; the part has one internal temperature.
- Cells are as small as they can be without significant performance drops, with adaptive resolution fitted to the geometry.
- Exceeding either limit destroys the part cleanly. Ablation may come later.

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

## Open questions

1. What kind of organization the player leads (D027).
2. Details of the ship model: structural solver, the cell model for aero, heat and radiation (D018, D032, D033, D061).
3. The crew model.
4. The colony design (D019).
5. Which model the player's AI agent uses and what it costs the player (D044).
6. Motion-model tunables: the cutoff threshold, opening angle, anchor hysteresis, and choice of coast integrator.
