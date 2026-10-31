# Decision Log: Sunscatter v0.2

This log records decisions that have been made. Each entry is numbered and dated. When a decision is superseded or made redundant, the old entry is **deleted** and its number is not reused; git history keeps the record. If a major reversal needs its reasoning preserved, the new entry says why. Proposals that have not been decided belong in design documents, not here.

Status values: **Decided** · **Direction** (settled in principle, details open)

---

## Project

**D001: v0.1 is archived and v0.2 is written from scratch.** *Decided, 2026-09-25.*
v0.1 now lives in `archive/v0.1/` and still builds and runs there, with its toolchain and lockfile pinned. v0.2 may port v0.1's algorithms and data, but not its code structure, and anything ported is re-tested. The reasons are in [lessons-from-v0.1.md](archive/research/lessons-from-v0.1.md).

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
Patched conics may appear only as tools for a first guess when planning; they never decide motion. Overlapping SOIs, barycenter special cases and "which body is this ship orbiting?" logic must not exist in the physics. The proposed model is in [technical/motion-model.md](technical/motion-model.md).

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
- See [technical/motion-model.md](technical/motion-model.md).

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

**D027: The game starts in 2030, and the player leads an aerospace startup.** *Direction, 2026-09-25. The kind of organization was decided 2026-10-29 (owner; [interview-2026-10-29-next.md](archive/interviews/interview-2026-10-29-next.md)).*
- Every career starts as a startup, with little cash and investors to answer to. "Primarily you need to be able to launch iconic rockets." The startup gets them three ways:
  - it operates rockets bought from their makers, as Arianespace launched Russian-built Soyuz rockets from Kourou;
  - it builds them under license, as Japan built its N-I and H-I from Delta designs;
  - it buys launches, which the provider flies with its own flight software (D117).
- Buying components from contractors is a large part of the game, and generous: "like you can buy a full engine from another company." Each catalog entry (D101) names its maker, price and lead time.
- Real companies keep their real names and products but are abstract. They are suppliers with catalogs, terms and published roadmaps (E), with no simulated management, stock or personalities, and for now they do not compete for contracts.
- Money comes from contracts (governments' and commercial), from selling services and parts, and from investors. Science is a mechanic too, in simplified form. The sandbox stays alongside the career (vision).
- The career is designed after v0.3 (owner, 2026-10-30). Developing and manufacturing your own parts in the career comes later, through D118's components and factories, with D101 giving the parts their physics.

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
- Uncrewed ships are commanded with the real light delay from the nearest mission control (owner, 2026-10-30: "everything new comes from the nearest mission control (initially on earth, eventually on other planets)"): new commands and programs travel over the relay network. A crewed ship nearby does not command probes. A crewed ship (later, one with an onboard superintelligent AI) is controlled at once, "as if the player is right there on the ship".
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

**D043: The ship OS is one flight-computer layer per vessel, and every program on it is a WASM module.** *Decided, 2026-09-25; one layer and one format, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)).*
- The layer F3 built (`sim::systems::flight`) is the ship OS. "Every computer on the ship is summed to one abstracted operating system": its compute budget is the sum of the computers' real processors (or their class, marked E), and a lost computer takes its share.
- Every program is a WASM module with one API (`Knowledge` in, commands out, the budget): the vehicle's own flight software, which ships with the craft (readable, editable, shareable), and the player's programs alike. Native code is only the layer itself (sensors to `Knowledge`, commands to actuators).
- Players can write scripts in any language that compiles to WASM.
- Scripts run sandboxed and deterministically, with an execution budget per tick that models a ship computer's limited power.
- Scripts are plain files on disk, driven by a command-line tool, so AI agents can work with them directly.
- This layer is D128's layer 2. Below it, each part's own logic (layer 1) is native, as part of how the part works.

**D044: The game runs an MCP server for AI agents.** *Decided, 2026-09-25.*
- It exposes game state, planning tools and script deployment.
- The player's own agent (Claude Code or another) connects through it, and acts as the tutorial and assistant (D041).
- It is the only way an agent reaches the running game, for players and for development alike: D122's mission tests use the same tools (owner, 2026-10-30).
- Every agent gets everything the player can see and do, including screenshots, camera placement, run-until, a new flight, and load and save. The look's benchmark and the profiler need a developer flag.
- Game time runs while the agent thinks, as it does for the player. The agent has pause, step and run-until (to an epoch or an event) as tools, so tests stay deterministic.
- An agent flies mostly through decisions, plans and ship-OS programs (pillar 4), since a tool call takes seconds and light delay applies (D069). The player's manual controls are its too (D126).

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

**D054: Map view is decided per object, by pixel size.** *Decided, 2026-09-26; orbit lines and double-click revised 2026-10-26 (owner). Replaces D049 and D053.*
- An object (body or vessel) is in map view when its sprite is under 1 px and its orbit radius is at least 1 px, with hysteresis. Sizes use one scale: pixels per metre at the camera's distance to its focus, as placed (a surface may pull the camera in from the chosen distance).
- Its icon, hover ring and name, and Ap/Pe markers are shown only while it is in map view. There is no global map mode and no map-view key.
- Its orbit line is shown while it is in map view and its orbit is seen from outside: a radius of at most the focus distance, with hysteresis to 1.5 times. From inside, an orbit wraps around the camera and crosses the sky as a straight line (the owner, 2026-10-26: the planets' and dwarf planets' orbits seen from the pad, "too many appear too early"). The active vessel's own line is exempt: it starts under the camera.
- On hover and on a double-click, the heaviest object under the cursor wins (a planet over its moons).
- Details: [technical/map-view.md](technical/map-view.md).

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
- A save holds no static data: body data and part behaviour come from the game's files. A ship's trajectory and its thermal state stay in the save, whichever is easiest (owner, 2026-10-30: "Don't make things harder than you need to"; [interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)).

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
Simplified is fine, as long as it models, semi-accurately: lift; rocket flight at low and high altitude at all speeds (subsonic, transonic, supersonic, hypersonic, rarefied); and entry from interplanetary velocities (to ~16 km/s). No flow-field solver. The model's limitations are recorded in [technical/aero-thermal.md](technical/aero-thermal.md).

**D072: The 3D view renders at the window's full resolution whenever the frame fits.** *Decided, 2026-09-27 (owner); revised 2026-10-26 (owner: one look, no graphics settings, D073; the policy by measurement).*
No render-scale setting: the 3D scene renders at the window's physical pixels (on Retina, every one) while the frame fits the display's refresh. Where the GPU cannot keep up, the main pass renders at a lower internal resolution, in steps down to half of it (a Retina display's logical resolution), upscaled before tonemapping with a sharp filter, and the full resolution comes back as soon as there is room (`game::render_scale`). The interface is always drawn at full resolution. The look is made cheap enough that this is rare (D073's frame-rate rule).

**D073: Graphics are one look: simple, crisp and functional, with no graphics settings.** *Decided, 2026-09-28 (owner); revised 2026-10-25 and 2026-10-26 (owner). Replaces D048's five tiers.*
- The owner's number-one rule: a 100% accurate spaceflight simulator. Cartoon looks are welcome as long as the physics is realistic.
- Minimal means simple, never blurry: "sharp, crisp, clear surfaces, recognisably water or land or earth, just not necessarily needlessly complex" (2026-10-25).
- Surfaces come from real data (satellite colour, elevation, the sea mask), sharpened by procedural algorithms: coastlines and zone edges stay crisp at every distance. Each zone (ocean, ice, forest, desert…) is drawn as one consistent procedural texture, not as a magnified photograph. Details: [crisp-surfaces.md](technical/crisp-surfaces.md).
- The sky is physical, from the atmosphere and the light: Bevy's scattering model (D052), raymarched from outside an atmosphere so the ground stays clear from orbit. Stars, every star's light, eclipses and planetshine (D055), shadows, a basic sun flare.
- One look for everyone, no graphics settings (2026-10-26: "delete all graphics settings, there should just be one graphics setting … that is so well optimized it doesn't matter"), apart from D109's toggles for heavy, non-vital visuals (exhaust, reentry glow, clouds). The tiers and their code are deleted. All visuals are data-driven for any body. What each part of the look costs is measured (the benchmark turns parts off, never the player).
- The look runs full screen at the display's frame rate on the reference Mac, at full resolution (D072).

**D074: The test craft is a finned rocket, stable nose-first.** *Decided, 2026-09-28 (owner).*
Its shape makes it aerodynamically stable nose-first at every Mach number, on ascent and on entry. It is still a debug craft (D064): flown with debug mode on, so reaching orbit, heating and the parachute's landing speed are not design problems; a real staged rocket comes with the parts model.

**D075: Attitude hold has five modes, and planned burns are flown by the attitude.** *Decided, 2026-09-28 (owner).*
- Modes: **stability** (kill rotation, hold the attitude), **prograde**, **retrograde**, **target**, **maneuver**. Prograde and retrograde follow the navball's speed reference (orbital, surface or target), stated on screen.
- Thrust is always along the craft's actual attitude. A planned burn is flown by the maneuver hold: the craft turns with its own attitude control and the burn's thrust follows the attitude, live and in the predicted plan alike.

**D076: Apsis markers are extrema of distance on the predicted trajectory.** *Decided, 2026-09-28 (owner).*
A marker is placed wherever the predicted trajectory (through planned burns) reaches a local maximum or minimum of distance from the dominant body, kept only when it differs from its neighbouring extremum by a significant fraction of the altitude. Nothing comes from osculating elements, so an escape trajectory has no apoapsis. Every closed orbit shows an Ap and a Pe per revolution: where no extremum is significant (a near-circular orbit), its highest and lowest points, one of each per revolution, shown as plain Ap and Pe (owner, 2026-10-01: no label, because they hold still: no marker moved before the clock reached it in 36,000 frames per warp at 1x to 1000x, and every warp showed what 1x shows at the same time; test `near_circular_markers_hold_still_at_every_warp`). Altitudes are above the reference ellipsoid under the apsis, like the flight panel's.

**D077: Engine heat leaves with the exhaust and by radiation from the nozzle; the interior warms only through the mount.** *Default taken 2026-09-30 (owner unavailable; reversible).*
The chamber's wall heat returns to the flow (regenerative cooling) and is not a heat source. The radiatively cooled nozzle extension absorbs a share of the jet power sized so it runs near 1400 K (coated niobium), radiates it away, and conducts to the craft only through its mount (~1 W/K). Each skin material has its own temperature limit. Numbers and sources in [technical/aero-thermal.md](technical/aero-thermal.md). Replaces the earlier 2e-4 of the jet power into the mount's node, which drove the interior to 1100–1500 K on long burns.

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
- Menus are hierarchical: a short summary on top, sections that expand to everything there is to know. Details: [flight-menus.md](technical/flight-menus.md).
- The detailed ship-systems view (an abstracted schematic of the ship, no planets) is still a separate screen, built once ships have systems; Utilities is its brief summary.
- GUI style: clean sci-fi, function first.

**D081: Scale: 50 craft of ~300 parts to 1,000,000×; live physics for a few, "limbo" for many.** *Decided, 2026-10-26 (owner).*
- The target: 50 craft of ~300 parts each in one game, at every warp up to 1,000,000×.
- **Live physics** (fixed ticks, contact, full aero) for every craft that needs it, at low warp: no cap on the number (D121, which also sets the scale target: a ~5,000-part craft live as well as this fleet).
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
Close the D070 gaps listed in `docs/technical/aero-thermal.md`. Model laminar–turbulent transition and its effect on friction and heating over the whole body. Stay fast enough for 1–2 large craft live, or many small pieces after a breakup.

**D086: Committed map data may grow to about 150 MB for the Solar System.** *Default taken 2026-10-26 (owner away; reversible). Revises D046's ~60 MB total.*
- D046's rule stands: public-domain sources, downsampled, each file under ~50 MB, close to final. The total grows to ~150 MB so the planets and moons get real maps like Earth's (D073): heights where a global DEM exists, colour, zones, roughness.
- Bodies without global data get procedural surfaces from their measured statistics (albedo, colour, crater density), stated as such.

**D087: Exposure follows the starlight at the camera.** *Default taken 2026-10-26 (owner away; reversible). Refines D055's fixed exposure.*
- The camera's exposure is scaled by the stellar flux at the camera's position (a photographer setting exposure for the light where they are), never by the scene's luminance. Near Earth nothing changes; at Saturn (1 % of Earth's sunlight) a day side reads like Earth's, and night sides stay as dark relative to day sides as D055 requires. Always on (no settings, D073).

**D088: The time shown is the origin's clock; the physics runs on TDB.** *Decided, 2026-10-26 (owner: "the game clock should be wherever the origin location (launch site) is"); the details are defaults (reversible). Refines D012 and D063.*
- The coordinate time stays TDB, the ephemeris' time scale. Every clock's proper time is integrated against it: dδ/dt = (L_B − (U + v²/2)/c²)/(1 − L_B), with L_B = 1.550519768e-8 (IAU 2006 Resolution B3). The rate was against TCB, so every clock lost 1.34 ms a day against the time shown (the owner's "ticking a bit fast").
- The time shown is the proper time of the origin, the first launch site in `comms.ron` (Kennedy): a site clock by the same rule as a landed ship, on the same lattice, from the ephemeris' first epoch, where it is set to TT (USNO Circular 179 eq. 2.6). So it reads TT, the geoid's time, within microseconds (6.4 µs over a year against the IAU series, which is good to ~10 µs, the site's own ±1.8 µs diurnal term included), and is labelled TT with the site's name. Its reading is a pure function of time.
- Ship clocks are shown against it, and a new vessel's clock starts synchronised with it. Tested: an ISS-like orbit loses 24.7 µs a day against the site, a GPS orbit gains 38.6 µs a day over the equator, and the site follows TT − TDB's 1.66 ms annual term.

**D089: The orbit line follows the navball's ORBIT/SURFACE/TARGET switch, which switches where rails warp stops.** *Decided, 2026-10-26 (owner); the TARGET line and the boundary are defaults (reversible). Refines D075 and D080; replaces the plotting-frame option (Tab).*
- The active vessel's line is one physical path drawn in the switch's frame: ORBIT about the reference body without rotation; SURFACE over its ground, each point in the body-fixed frame at its own time, drawn where that ground is now (the path over the ground, with the planet's and the air's rotation); TARGET about the target body. A vessel target has no frame of its own (its future is a prediction too), so the line stays about the reference body, with both lines and their closest approach drawn. Other vessels' lines are drawn about their dominant bodies.
- The switch changes by itself only when the vessel crosses the reference body's rails floor (D062; Earth 150 km), the altitude that restricts rails warp: SURFACE below, ORBIT above, back up only 1 % above the floor. The q-based live boundary (D081) depends on speed and density and could chatter. A mode picked by hand holds until the next crossing; LOCK keeps it. This replaces the 36 km rule.
- Where a vessel flies live in air, or its coast enters air that needs live flight, the line is predicted with the vessel's own force model (`sim::landing::air_path`: its aerodynamics against the air turning with the planet, at the attitude its controls will hold), until the air is clear for coasting again; then the coast. The owner's kinked line (2026-10-26) was the coast's thin-air drag stopping the craft against the air within seconds, drawn without the ground's rotation.

**D090: Vessels feel the bodies' measured gravity fields, as spherical harmonics truncated by significance.** *Decided, 2026-10-26 (owner: "accurate gravity fields"). Refines D023 and motion-model §4.*
- A body's field beyond its point mass is its published spherical-harmonic model (fully normalized C̄nm, S̄nm, with the model's reference radius and GM) in its own rotating frame, through its rotation model (`sim::gravity`, Pines' singularity-free recursion). A body without one keeps its J2 as the degree-2 zonal case of the same model. Every vessel force goes through it: coasts, limbo, live ticks, planned burns, the landing predictor, the bubble.
- **Truncation, in the spirit of the cutoff:** a vessel includes a body's degrees up to where the bounds of all higher degrees together stay under a hundredth of the cutoff (10⁻¹⁴ m/s²) at the vessel. The field acts on the vessel only (the anchor's acceleration is its ephemeris'), so leaving a degree out errs by that degree at the vessel whatever the anchor: the rule depends on the vessel's distance from the body alone (rule 1). The degree only grows within a segment and is stored with it (chunked = single pass, rule 4). A far vessel pays for nothing.
- **Steps resolve the field:** an integration step spans at most three half-wavelengths of the highest degree that can matter over a step, at the vessel's speed over the body's turning surface. Longer steps alias the high degrees, which error control does not see (degree 60 in LEO erred by 700 m a day at the coasts' tolerance, against its 8 m truncation error).
- **Data** in `data/bodies/<body>/gravity.ron` (D051), generated by `ephem-tool gravity` from the public models and truncated where a day in the body's lowest common orbit converges to tens of metres at a cost that keeps D081 in reach: each doubling of the degree costs ~8× (evaluations go as N², steps as N). Sources, degrees, frames and the measurements in [solar-system.md](technical/solar-system.md).
- The proper-time potential includes the field to the same degree (J2 is ~3e-13 of the rate in LEO).

## Ships: parts, systems, failures (2026-10-26 evening interview)

The owner's interview: [interview-2026-10-26-parts.md](archive/interviews/interview-2026-10-26-parts.md). The design: [technical/ship-model.md](technical/ship-model.md). Numbers D091–D099 are left to the evening's other workstreams.

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
- There the player defines **stages** and saves **functions**, and pins functions to a **mini systems panel** shown in flight, where they are activated by a click (D111: the action-group bar above the navball, no craft-specific keys). Space activates the next stage. Flight keeps D080's essentials.

**D108: The craft is flown by hand until scripting; scripting reads the information model.** *Decided, 2026-10-26 (owner).*
The player flies everything by hand for now (including a booster landing). Scripting comes soon, integrated with the data model of D106, and commands go through one command type shared by the screens, the MCP server and scripts.

**D109: Visuals come from physics or the simulation, never for their own sake.** *Decided, 2026-10-27 (owner). Revises D073.*
- "All visuals are derived from either a truth about how physics works, simplified, or an element of the simulation."
- The exterior camera is "a magical visualization", not a simulated camera: it does "only what is clearest and most efficient". A later on-board-cameras mode would simulate optics. The sun flare stays, for every light-producing body.
- The graphics options are toggles for what is compute-heavy and not vital: **engine exhaust** (off: today's flame cone), **reentry glow** and **volumetric clouds** (off by default). Exhaust is a fast render model from the exhaust's state at the nozzle exit and the local air; the exit state comes from published engines for now, later from the owner's engine testbench (CRUCIBLE, `~/inquiry-project`). Reentry glow comes from the aero model's shock layer, and the model is improved where it cannot show it. Clouds come from a general per-body atmosphere model plus, for Earth, D113's weather.
- Parachutes become meshes that collide with the craft (open).

**D110: A craft's light is the craft's, never the camera's.** *Decided, 2026-10-27 (owner). Refines D055.*
- Each star's light is taken at the craft and applied to the whole craft at once: bodies' eclipses ("orbital sunset would happen on the whole craft instantly") and the air along the line to the star. The craft also gets the sky light of the air around it, and planetshine "derived from the area reflecting light, intensity of light, and angle of the craft relative the reflecting surface" (no special cases).
- Shadows come from bodies and craft: a craft's own parts, other craft nearby, the ground under it. They do not change when the camera moves or turns.

**D111: The flight interface is the navball, a staging bar, part cards and flat task menus.** *Decided, 2026-10-27 (owner). Refines D080 and D107.*
- **Navball:**
  - The g-load replaces V/S, which moves to Landing. The g-load is the proper acceleration and its direction, against the limit in that direction (the crew's tolerance or the weakest structure, named).
  - An altitude switch (sea level, or the lowest surface elevation where there is no sea, a reference level on giants; or terrain) that every altitude above the surface follows.
  - An RCS button opens the RCS menu.
  - The action-group bar sits above the ball: functions pinned in the systems screen, abort among them, with no craft-specific keys.
- **Staging bar:** thin, at the right edge, next stage at the bottom.
  - Each stage where it applies shows a bar per propellant (kg/kg, from the tanks it draws on), the burnout time of the first to run out, and Δv.
  - The active stage also shows thrust, TWR, mass now and at burnout, and mean Isp.
  - A switch under the bar chooses between the current air and vacuum.
  - Estimates assume no further input.
  - Each stage lists what it acts on as part-type icons. Stages can be reordered, deleted and their contents moved in flight.
- **Part cards:** one per part, with the live essentials of that part on this ship now. A value that is not measured is a dash, whose tooltip after 2 s names the sensor that would measure it.
  - Clicking the part on the ship opens the card near the part, draggable. Clicking its icon opens it stacked left of the staging bar.
  - A line joins the card to the part or icon.
- **Menus:** a button per menu along the top, keys 1–0.
  - Each menu is one flat page for one task, with links into the systems screen for more. RCS opens only from the navball.
  - Menus fit the screen and are never cut off.
  - Notifications go to a scrollable corner window with links to their vessel or screen.
- **Keys:** no function keys (they need fn on a Mac). The systems screen is on Tab. `,` at 1× pauses.
- **Time warp** is limited once thrust is produced (engines, RCS, settling), not by where the throttle sits.

**D112: A vessel is what is joined by structure or by data, and keeps its own controls.** *Decided, 2026-10-27 (owner).*
- "A vessel is everything connected structurally, plus information being sent EITHER along a wire or wirelessly." Its RCS menu lists every thruster of the vessel, each selectable, all on by default.
- Each vessel keeps its own controls (throttle, SAS, holds), and a script may fly a vessel that is not active. `[` and `]` step through the vessels, nearest first.

**D113: Weather is deterministic from the seed, smooth and cheap.** *Decided, 2026-10-27 (owner).*
- Earth's clouds move smoothly and follow large-scale weather trends. Any time and place can be queried cheaply from the save's seed, and the detail is fractal. No gameplay effect yet.
- The atmosphere and weather model stays open to exotic conditions: the seasons of an eccentric orbit, or an atmosphere that freezes out.

**D114: One revert, from a launch save; a closed throttle shuts engines down; stages run again.** *Decided, 2026-10-28 (owner).*
- "Reverting flight to launch/to the pad should be ONE function, and it should RESET EVERYTHING to the last time the current active vessel was launched from the pad." When a vessel is placed on the pad the game writes a launch save to disk; every piece that splits from it keeps the link. Revert loads that file directly: the whole game as it was, fuelled, throttle closed. A save records its active vessel and the screen showing.
- "Throttling down to 0% should automatically shut off any engine currently running": closing the throttle (from open to 0) shuts every starting or running engine down (its tail-off). An engine lit with the throttle already closed runs at its minimum until the throttle is opened and closed again.
- "It should be possible to run stages multiple times, activating the functions that can be activated and doing nothing otherwise": a stage already run can be run again; its commands that can act do (an engine with an ignition left starts), the rest do nothing. Opening the throttle again does not relight an engine: a restart is a stage run again (owner, 2026-10-29). Every stage already run has AGAIN on the staging bar: the active one, and below it the stages run before, one line each (the owner could not relight the Merlins once Liftoff was the active stage, 2026-10-29). The systems screen's stage editor locks the stages already run.
- A pad abort (the clamps never let go) ends that launch: "Reset the ship fully, after all it needs to refuel. New launch." Once its engines are shut down, a fresh vessel of the same kind goes on the pad as loaded (full tanks, new engines, nothing staged), with a new launch save. The clock runs on: this is not a revert (owner, 2026-10-29).

**D115: The ship model follows the real vehicle: its sensors only, published figures over derived ones, adaptive flight software, and guidance only where the real vehicle flies itself (D117).** *Decided, 2026-10-29 (owner; [interview-2026-10-29.md](archive/interviews/interview-2026-10-29.md)).*
- Sensors: "ONLY add things the real vehicle has." What nothing real measures shows as a dash outside debug mode.
- Published wins: where a derived value disagrees with a sourced one, the model's inputs change until it reproduces the source (recorded). Inconsistencies with the real Falcon 9 are bugs.
- Readings carry real noise, drift and outages only where they matter; elsewhere a value shown to its sensor's significant figures stands for its uncertainty. "I really would not like flat out wrong measurements in most cases." Estimates where appropriate.
- Built-in flight software is adaptive functions reacting to the vehicle's state (engine-out re-trim, launch-commit checks, propellant utilisation, the navigation filter), not guidance: the player flies, except where the real vehicle flies itself (D117: the booster's return, Dragon's autonomous docking). All automation is a switch (D126).
- Fidelity next: powered components heat their parts (an engine off heats nothing), Dragon's coolant loop and radiators; per-engine gimbals with the engine-out response and hard starts; O₂ / N₂ tanks through regulators and leaks, suits abstract.

**D116: Crew limits are doses: integrated over time, lethal past a threshold drawn for each crew member, no injuries.** *Decided, 2026-10-29 (owner); the numbers are the lead's logical defaults.*
- "Rough physiology+NASA data is fine, just get something consistent (no instant deaths, statistical and time integrated for radiation and g's. No injuries, the crew members just die past a threshold."
- **One rule for every hazard:** each crew member carries a dose per hazard. It grows by `dt / T(x)`, where `T(x)` is how long level `x` can be survived, and it does not shrink. They die when it passes their threshold. Below that they are well: no injuries and no impairment, only the alarms and the menus' readings as it rises.
- **Thresholds are statistical.** Each crew member draws one per hazard when the craft is placed on the pad: lognormal around 1, σ 0.3 (E), seeded like failures (D105), so a flight replays the same. No instant deaths: every `T` is finite, so even a spike takes time to kill.
- **Acceleration:** `T(g)` by axis is NASA-STD-3001 Vol. 2's limit duration for that level, times 3 (E: the standard is the no-injury limit, and the lethal one is rougher). Sideways is still judged against the more tolerant axis.
- **Radiation:** the absorbed dose (Gy), integrated, against LD50/60 ≈ 4.5 Gy (E, untreated acute radiation syndrome). It lands when radiation enters the ship model (open question 1).
- **Air:** the same rule for low O₂ partial pressure and high CO₂ (E: minutes at the red limits of the Crew menu, which are rough physiology).

**D117: Foundations first, then Crew Dragon and the booster's return flown end to end; what flies itself in reality flies itself in the game.** *Decided, 2026-10-29 (owner; [interview-2026-10-29-next.md](archive/interviews/interview-2026-10-29-next.md)). Refines D115.*
- Order: the fidelity review's foundations come before D115's fidelity work ([fidelity-review-2026-10-29.md](archive/research/fidelity-review-2026-10-29.md) §4). They are, in order: the vessel's state split and merged by construction, one mass truth, and flight software that reads only `Knowledge`.
- Then two flights end to end, in parallel: Crew Dragon to the ISS and back, and the Falcon 9 booster's return (a landing zone first, the droneship later).
- After separation the booster is flown by its own flight software (boostback, entry and landing burns), as the real one is, reading only what its sensors know. Later it becomes a ship-OS program the player can read and edit (pillar 4).
- Dragon's rendezvous and docking have three levels the player picks. "2 can be activated if wanted, or you can switch on 1 and only have auto burn completion, or you can go fully 3":
  - autonomous, as the real Dragon (the crew can take over);
  - the player plans the burns, the burns run themselves, and the player hand-flies the final approach;
  - fully manual.

**D118: Things are built from a few thousand abstract components, materials and factories, the same way on Earth and in colonies.** *Direction, 2026-10-29 (owner; [interview-2026-10-29-next.md](archive/interviews/interview-2026-10-29-next.md)). A later roadmap goal: nothing is built now.*
- "We define a few thousand basic components, new ones for various tech levels, and they are relatively abstracted (not a single bolt, but rather a certain type of turbopump, not a specific brand of solar cell but rather a standard solar cell)."
- "Then we have entries for things like materials, time to construct, multi-purpose factories (for colonies), etc, and everything is built to work off a similar system on earth and on a colony except in a colony you need far more manufacturing from scratch." Building it yourself will eventually be the only way to build anything in a colony.
- How it fits D101's generators (a part's physics) and D019's colonies is still to be worked out.

**D119: On a coast, internal systems run coarse-grained models, solved analytically where possible, and the trajectory reads the same law.** *Decided, 2026-10-30 (owner). Refines D081.*
- "On coasts, just do a coarse grained model for things like that. You should still be simulating boil off on coast. Solve analytically when you can."
- Boil-off, venting and similar slow changes keep running through a coast. Where a closed form exists (a tank's heat leak, its pressure rising to relief, then venting at the heat leak over the latent heat), it is evaluated at any time, not stepped. Elsewhere a coarse-grained law stands in for it.
- The coast segment's mass for its perturbations (drag, radiation pressure) and the ship state come from the same law, so there is one mass truth (F2). It stays deterministic, warp-invariant and chunked = single pass (rule 4).

**D120: The staging bar's Δv is in vacuum by default; an abort is offered while an escape system can pull the crew or the vehicle away.** *Decided, 2026-10-30 (owner); the escape check simplified the same night ([interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)).*
- "Delta v should be vacuum by default. NO patchwork fixes." The bar's current-air figure stays a choice and is not the default. It is not patched stage by stage.
- "Launch abort should only show when there is a valid abort procedure that would actually save the rocket or the crew." For the Falcon 9, that means while Crew Dragon, with its launch escape system (eight SuperDracos), is still attached to a stack it can escape from. Never on the booster after separation, on Dragon alone, or on a craft with no escape system (Cargo Dragon has no SuperDracos). The rule is derived from what the piece holds (crew, escape engines that can start, the joint to separate), not from the function's name.
- "This does not need to be a complex system. Literally just if a LES system exists for this purpose, the player can fire it at any time, and it always runs the same." No prediction of the escape. When the craft is flown automatically, its layer-2 software (D128) decides whether to abort, for example on a thrust-to-weight margin of the escaping piece (E).

**D121: The ship model scales to a 5,000-part craft flown live and 50 craft of 300 parts at 10⁶×, through one kind of craft, one state and one advance law per subsystem.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)). Refines D081 and D119.*
- **Scale:** a ~5,000-part craft flown live at 1× and 60 fps on the M2 Pro, and D081's fleet of 50 craft of ~300 parts at every warp up to 10⁶×. Live physics has no cap on the number of craft (owner: "No cap"). The game's side counts too, but by judgement: "I don't want to steer you towards chasing some target of 5000 cubes arbitrarily rendered … we don't even know what those crafts will look like" (owner, 2026-10-30, [interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)).
- **One kind of craft.** The test craft and the made-up path (one placeholder part with a fuel number) are deleted: every craft is built from parts (D100), and benchmarks and tests use craft of parts. D074 goes with the test craft's code, and D064 keeps only debug mode.
- **One state per vessel.** `ShipState` and `Runtime` merge into one, grouped by subsystem, split and merged by construction (F1). Ids are strings only at the edges (files, saves, the interface, MCP) and typed handles inside.
- **One advance law per subsystem.** Each subsystem advances over any interval, exact or coarse-grained, chunked = single pass. Live ticks, the systems lattice and coasts only choose the interval (this closes the review's F6).
- **Live costs:**
  - The aero cells are evaluated once per tick, and their forces are held through the integrator's stages and the contact substeps.
  - The thermal network steps at its own rate, set by its fastest time constant, with the ticks' heat accumulated between steps.
  - On a coast, each node relaxes analytically toward its orbit-averaged equilibrium (D119), with breaks at eclipse entry and exit.
- **Separations.** At every separation each new piece is checked: "is this thing controlable (radio/crew/onboard computer, power)".
  - If it is, it becomes a full craft, simulated while inactive.
  - If not, it becomes a ballistic lump: its mass, its inertia, a drag area and heating from its bounding shape, and one heat node. A lump is deleted when it burns up or hits the ground; lumps in orbit stay.
  - Designs: the pieces the stage sequence can make get their designs when the craft is built. An unplanned controllable piece gets a design composed from its parts' bakes.

**D122: One testing framework: unit tests, and mission tests flown headless through MCP by a development agent on the real flight software.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)).*
- "We just need the headless mission test with screenshots for large integrations of new features, plus maybe specific cases if needed. The game can run 100% autonomously after all with an AI agent driving. Tests should be real flight software plus an AI agent making decisions, not a scripted demo. Clean, ONE testing framework. Ensure you have unit tests too."
- **How a mission test runs:**
  - The game runs headless with its MCP server.
  - Screenshots, the look's benchmark and the performance numbers are MCP tools.
  - A development agent flies the mission through MCP, deciding as a player would, and checks the results and the shots.
- **CI** runs the unit tests and specific deterministic sim cases.
- The scripted demo and the body tour are deleted, and the `SUNSCATTER_*` variables shrink to a few (the home dir, offscreen).

**D123: The first flights: convex landing guidance, a calm sea, a placeholder station and fail-safe engines; visual work frozen.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)). Refines D117.*
- **The booster's landing burn** uses convex powered-descent guidance: lossless convexification (Açıkmeşe & Ploen, 2007), as SpaceX's (Blackmore, 2016). It is re-solved several times a second by a solver with a fixed number of iterations, so it stays deterministic. The boostback and entry burns are targeted by a predictor–corrector on the real trajectory propagator.
- **Water:** a calm, flat sea first. Each contact point gets slamming pressure, added mass and buoyancy (E until checked against water-impact data). Waves come later.
- **The docking target** is one rigid body, a placeholder. The ISS "will be deorbited soon anyways, it will likely not even be in the game."
- **Fail-safe engines:** an engine whose flight computers are all lost, or whose own controller is lost, shuts down (E until sourced).
- **Debug mode** shows the truth, but a dead or unpowered computer is still dead.
- **Aborts (D120):** an uncrewed Crew Dragon is offered the abort, since it saves the vehicle. Whether its escape thrust is enough is decided by the automatic flight's software, not by the abort's offer (D120, as simplified the same night).
- **Random failures (D105) are off** until both flights fly end to end cleanly, then on.
- **Visuals are frozen** until both flights fly end to end; only bugs that mislead the player are fixed. "I don't know if we really need the last plume things."
  - The interface is not frozen: access to every piece of information and every control, and the systems screen's thermal mesh and part isolation (owner, 2026-10-31: "making it easy to access all the information and controls, plus adding a feature for the vessel view: thermal actually shows the grid level thermal mesh, plus clicking on a part isolates it and zooms in"). The look stays frozen.

**D124: The docs are organised by system, finished work is archived, and there is no file-size cap.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)).*
- The layout:
  - a reading guide in the README;
  - `docs/vision.md` and `docs/decisions.md`;
  - an owners table, and `process.md`;
  - `docs/systems/`, one general doc per main system, open to later ones ("colonies, economics, construction, fusion, various interstellar techs, etc, just leave that open");
  - `docs/technical/` for specific details;
  - `docs/vehicles/` for research dossiers such as the Falcon 9's;
  - `docs/archive/` for finished plans, briefs, interviews and logs;
  - `docs/plans/` for live work and the hand-off only.
- "I never made this cap, you can remove it": the 700-line check goes from CI and the hooks.

**D125: Physics is universal: no ship-specific models, and every part is fully described by its parameters.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)).*
- "We CANNOT have ship specific physics." A behaviour is modelled once, for every craft that has the parameters for it, or it is not modelled.
- "EVERY part must be FULLY described by a set of parameters." Ablation is one mechanism for every heat shield: 1-D, per cell, from the material's parameters (PICA-X is one material).
- Supersonic retropropulsion is not modelled as such: thrust and the clean body's drag stay independent ("clean independence, as this is an edge case anyways").

**D126: Automation is a switch; manual control is always possible.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)). Refines D115 and D117.*
- Every automatic sequence can run three ways: by itself, when the player presses for it, or not at all. Examples are the parachutes' deploy on barometric altitude, a planned burn, and the booster's guidance. A manual "deploy" runs the same deploy function as the automatic sequence. The autopilot is a switch with a config menu.
- **Planned burns** run closed-loop to their target state, with real engine starts, ullage settling and tail-off. They are flight-layer programs. Planned burns for craft of parts come right after ONEKIND.
  - They are set as nodes (Δv prograde, normal and radial at a time), and close the loop on the orbit the node gives. Orbit targets and rendezvous come with the flights (owner, 2026-10-30, [interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)).
  - An automatic burn settles its propellant with the craft's own thrusters before ignition, as the Falcon 9's second stage does; the player can always do all of it by hand.
  - They fly live, at physics warp, until D029's warp while thrusting integrates them into the warp systems (D129).
- **Warp to a point** ramps the warp up and down. It never jumps through time (D022).
- **Manual control** is always available, with today's simplified aids (automatic engine vectoring), "even if it leads to catastrophic failure (part of the fun of driving your own spacecraft)."
- **Taking over:** "It should be a full switch ONLY when input disagrees/when it would interfere." Input that agrees with the running sequence, or touches what it does not command, leaves it running; input that conflicts with it switches that sequence fully to manual (owner, 2026-10-30).

**D127: Vessels advance on parallel workers at one fixed live rate, bit-identical across machines.** *Decided, 2026-10-30 (owner; [interview-2026-10-30.md](archive/interviews/interview-2026-10-30.md)).*
- Each vessel, and each coast, advances on its own worker, and the results are gathered in a fixed order. Craft in contact in the bubble advance together on one worker.
- Every live craft ticks at one fixed rate. The costly parts step at their own rates inside it (D121).
- Every simulation result is the same bits on every machine. CI flies the same cases on macOS and Windows and compares hashes.
- Performance is measured by hand (the scale benchmarks); there is no gate in CI.

**D128: Ship software has three layers: each part's own logic, the ship computer's programs, and the agent outside the game.** *Decided, 2026-10-30 (owner; [interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)). Refines D043, D115 and D126.*
- **Layer 1, the part's own logic.** "Each part should have built in, fundamental logic … synonomous with how the part actually works", such as an engine controller's start sequence and its fail-safe shutdown (D123), or a valve's relief. In the abstraction it runs on the ship's computer, but it belongs to the part: "I don't want "ship computers" mingling until assembling a ship means fighting an integration of procedures."
  - The bare minimum is well defined: with layer 1 alone a ship "should be enough to coast for months if everything is set up", and can in theory be flown by hand, but hardly ("dig into menus and regulate atmosphere flow and similar").
- **Layer 2, the ship computer** (D043's ship OS). It "makes pretty much the same inputs the player makes, but with simple hard coded logic, plus some elements that are more automated".
  - A ship flown by hand usually has parts of layer 2 running in the background. The player can trigger its actions and functions, which run sequences of actions (D126's switch).
  - Some of it is isolated functions shipped with the craft, such as the booster's landing loop or the maneuver function. Some is a central script that handles the edge cases and calls those functions.
  - Done well, it flies whole missions start to finish and handles errors as well as the effort put into it. A mission from Earth to Mars without people is possible, but needs reliable parts and still risks failure.
  - The game ships the baseline software that comes with a craft or a part. Players share layer-2 software as they share craft.
- **Layer 3, the AI agent outside the game** (D044): it reaches the ship computer directly, as the player can in a "computer" tab, calls functions, takes screenshots and uses the controls.
- **Stopping warp.** A ship with only layer 1 has a built-in line: a failure that puts the crew or the vessel at risk (the crew's limits will be breached, or the vessel will be lost) stops warp at its own epoch. Anything else is logged. A layer-2 program can replace that line entirely with its own.
- **Notifications** can be switched off per craft, for fleets on trade routes: then nothing on it stops warp, "if something fails, it fails, the autopilot scripts do the best they can."

**D129: The course of events is the same at every warp; only the player's input changes it.** *Decided, 2026-10-30 (owner; [interview-2026-10-30-roadmap.md](archive/interviews/interview-2026-10-30-roadmap.md)). Refines D029, D081, D119 and D121.*
- "EXACTLY the same behavior at 100,000 warp as you would get at 1x warp. That is part of determinism." "Deterministic course of events is only broken when the player inputs something."
- The systems of a craft not being flown are "deterministic, coarse grained or analytic", and "don't change behavior across warp levels". This goal defines how warp is restricted and how systems are simulated under warp:
  - a craft's model is chosen by its state (thrust, air, contact, a live craft near it), never by the warp;
  - where a model cannot give the same events at high warp, the warp is restricted, and the model is not changed.
- Planned burns fly live for now. "Warp while thrusting" (D029) comes later, as a special case integrated into the warp systems under the same law.
- Failures do not stop warp unless they are critical (D128), and "in general not much should fail during cruise when a ship is not being actively flown". "Generally repeat missions will rarely have a critical failure."

## Open questions

1. Radiation in the ship model (dose, single-event upsets), and how the career's knowledge of a design's reliability grows (D105).
2. The crew model beyond its limits (D116): skills, tasks, who flies.
3. The colony design (D019, D118).
4. Which model the player's AI agent uses and what it costs the player (D044).
5. Motion-model tunables: the cutoff threshold, opening angle, anchor hysteresis, and choice of coast integrator.
