# Sunscatter

A 2D space game (KSP-like) with 1:1 real-scale orbital mechanics. Build rockets in the editor, launch from Earth, and fly using patched-conic trajectory prediction.

## Workflow Orchestration

### 1. Plan Mode Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately - don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 2. Subagent Strategy to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

### 3. Self-Improvement Loop
- After ANY correction from the user: update 'tasks/lessons.md' with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

### 4. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 5. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: "Knowing everything I know now, implement the elegant solution"
- Skip this for simple, obvious fixes - don't over-engineer
- Challenge your own work before presenting it

### 6. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests -> then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Spec Management
- **Specs update WITH code, not after**: Every code change MUST have its corresponding spec updated in the SAME task, before moving to the next task. Never batch spec updates as a separate step. When you finish implementing a feature or fix, update the relevant spec file immediately — the task is not done until the spec reflects the code.
- **No code-only changes**: If you change behavior, add a feature, or fix a bug, the spec must be updated as part of that same unit of work. Do not commit code without its spec update. Do not defer spec updates to "later" or "cleanup."
- **Merge into existing specs**: When syncing delta specs from an opsx change (design/proposal), merge new requirements into the pre-existing spec files under `openspec/specs/game/`. Only create a new spec file/folder if the change introduces something entirely new that doesn't fit any existing spec.
- **Spec structure**: `openspec/specs/game/<domain>/<capability>/spec.md`. Domains: `editor`, `editor_rendering`, `flight_rendering`, `orbits`, `physics`, `colony`, `vessels`, `tracking_station`, `main_menu`, `pause_system`. See existing files for the pattern.
- **If no spec exists yet**: Create one in the closest matching domain/capability. If the area is entirely new, create the domain folder and spec file.

## Task Management
1. **Plan First**: Write plan to 'tasks/todo.md' with checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add review to 'tasks/todo.md'
6. **Capture Lessons**: Update 'tasks/lessons.md' after corrections

## Core Principles
- **Simplicity First**: Make every change as simple as possible. Impact minimal code.
- **No Laziness**: Find root causes. No temporary fixes. Senior developer standards.
- **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.

## Tech Stack

- **Language**: Rust (edition 2021)
- **Rendering**: wgpu 0.19 (triangle fans, 4x MSAA, custom vertex shaders)
- **Windowing**: winit 0.29
- **UI**: egui 0.27 (immediate mode, via egui-wgpu/egui-winit)
- **Serialization**: serde + ron 0.8
- **No game engine** — custom physics loop, wgpu rendering, egui UI

## Game Modes

Nine modes via `GameMode` enum in `src/game.rs`:
- **TitleScreen**: Initial launch screen
- **MainMenu**: New game / load game / resume
- **Editor**: Build vessels on a grid, save/load blueprints
- **Flight**: Fly vessels with orbital mechanics, staging, maneuver nodes
- **TrackingStation**: View all vessels and colonies, switch vessels
- **Colony**: Manage a specific colony (buildings, resources, production)
- **ColonyOverview**: Summary of all player colonies
- **Management**: Company-level finances and fleet operations
- **TechTree**: Technology research and unlock progression

## Project Structure

```
src/
├── main.rs              # Entry point, event loop, render orchestration
│                        # Input handling, flight/editor mouse+keyboard dispatch
├── lib.rs               # Module re-exports
├── game.rs              # Game struct (central state), GameMode (9 modes), FlightState
│                        # launch_from_editor(), save/load coordination
├── bodies.rs            # CelestialBody, Orbit, SolarSystem (21 hardcoded bodies)
│                        # Kepler equation solver, orbital position/velocity from time
├── save.rs              # Save/load game state serialization (RON format)
│
├── ship/                # Ship physics (the point-mass that flies)
│   ├── mod.rs           # Ship struct, Velocity Verlet integration, thrust/rotation
│   │                    # ShipState enum (Flying/Landed), AutopilotTarget
│   │                    # VesselPhysicsData (bridges FlightVessel -> Ship physics)
│   ├── orbit.rs         # State vectors <-> orbital elements conversion
│   ├── patched_conics.rs # Trajectory prediction across SOI boundaries
│   ├── transfer.rs      # Lambert solver, porkchop plots
│   └── soi.rs           # SOI transition detection, frame conversion, on-rails mode
│
├── parts/               # Part definition and vessel systems
│   ├── definition.rs    # PartDefinition, PartCategory (9), PartSize (5)
│   │                    # Propellant enum (8 families), PartDefinitions registry
│   ├── blueprint.rs     # VesselBlueprint, PlacedPart (serializable vessel design)
│   │                    # Mirror symmetry, blueprint_to_parts conversion
│   ├── registry.rs      # BlueprintRegistry (save/load from data/blueprints/)
│   └── vessel.rs        # FlightVessel, FlightPart (runtime vessel with physics)
│                        # Fuel zones, staging activation, delta-v calculation
│
├── editor/              # Vehicle editor
│   ├── state.rs         # EditorState (parts HashMap, stages, camera, ghost preview)
│   │                    # Part placement, dragging, deletion, stats calculation
│   ├── ui.rs            # egui UI: toolbar, stats bar, parts palette, staging panel
│   │                    # Part info panel, save/load dialogs, drag-and-drop staging
│   └── render.rs        # Vertex generation for editor scene
│                        # Grid, placed parts, ghost preview, procedural details
│
├── colony/              # Colony simulation
│   ├── mod.rs           # Colony struct, building placement, resource storage
│   ├── buildings.rs     # Building definitions, construction, upgrades
│   ├── contracts.rs     # Contract generation, tracking, rewards
│   ├── economy.rs       # Company finances, pricing, market simulation
│   ├── notification.rs  # Colony event notifications
│   ├── resources.rs     # ResourceType enum (26 types), extraction rates
│   ├── simulation.rs    # Colony tick: production, consumption, maintenance
│   ├── tech.rs          # Tech tree nodes, unlock requirements, research
│   ├── trade.rs         # Trade route management, cargo scheduling
│   └── transfer.rs      # Resource transfer logic
│
├── galaxy/              # Procedural galaxy generation
│   ├── mod.rs           # GalaxyState, ProceduralStar, StarType enum
│   ├── density.rs       # Stellar density: exponential disk, bulge, spiral arms
│   ├── generation.rs    # Per-sector star generation, spectral types, orbits
│   ├── prng.rs          # Deterministic PRNG for reproducible galaxies
│   └── star_color.rs    # Blackbody temperature -> RGB color mapping
│
└── render/              # Rendering and UI
    ├── state.rs         # RenderState struct, wgpu/egui setup, resize
    ├── camera.rs        # Camera struct (position, zoom, body tracking)
    ├── flight.rs        # Flight HUD (egui): velocity, orbit info, staging, autopilot
    ├── scene.rs         # Geometry generation: bodies, orbits, ships, atmosphere glow
    ├── interaction.rs   # Hover/click detection, body/vessel selection
    ├── maneuver.rs      # Maneuver node management (create/delete/drag/burn)
    ├── menus.rs         # Title screen, main menu, tracking station, pause
    ├── colony_ui.rs     # Colony management screen
    ├── colony_overview_ui.rs # All-colonies summary screen
    ├── management_ui.rs # Company management screen
    ├── tech_tree_ui.rs  # Tech tree visualization and interaction
    ├── trade_ui.rs      # Trade route UI
    ├── editor_render.rs # Editor scene rendering
    ├── formatting.rs    # Number/distance/duration formatting
    ├── geometry.rs      # Circle, ring, triangle primitives
    ├── sprites.rs       # Sprite atlas loading
    ├── textures.rs      # Texture management
    └── types.rs         # Render data structures, vertex format
```

```
data/
├── parts/               # 22 RON files defining 148 parts across 9 categories
├── sprites/             # Engine and plume sprite atlas
├── blueprints/          # User-saved vessel designs (RON files)
├── saves/               # Save games (auto-save + quicksave)
└── bodies/              # Body definitions (stale/unused — bodies hardcoded in bodies.rs)
```

## Key Architecture Details

### Solar System
- Bodies are **hardcoded** in `src/bodies.rs`. The `data/bodies/` RON files are stale/unused.
- Body indices are **dynamic** — looked up by name at init. Use `solar_system.earth_index`, `solar_system.sun_index`, `solar_system.moon_index`.
- Real-world values: Earth radius 6,371 km, orbital velocity 7.8 km/s, Moon at 384,400 km.
- Gravitational constant `G = 6.67430e-11`.

### Coordinate Systems
- All positions use **f64** to prevent precision loss at large distances.
- **Grid square** = 0.5m x 0.5m (`GRID_SQUARE_SIZE` in definition.rs).
- Part sizes: Tiny=1, Small=3, Medium=5, Large=9 grid squares wide.
- Editor uses camera-relative coordinates for rendering; world coordinates for placement logic.
- Flight rendering scales by `SCALE * BODY_SCALE` constants in main.rs.

### Ship vs Vessel
- **Ship** (`src/ship/mod.rs`): The physics point-mass. Handles position, velocity, rotation, gravity, thrust integration, on-rails mode, autopilot. Always exists.
- **FlightVessel** (`src/parts/vessel.rs`): The parts-based vessel. Optional (`game.flight.vessel: Option<FlightVessel>`). Contains parts, fuel, staging. Created when launching from editor.
- Ship and FlightVessel are synced in `game.update()`: vessel position/rotation mirrors ship.

### Part System
- **PartDefinition**: Loaded from RON files. Has mass (tonnes), dimensions, optional EngineData/TankData/PodData/DecouplerData.
- **5 part sizes**: Tiny=1, Small=3, Medium=5, Large=9, XL=13 grid squares wide.
- **9 categories**: Pods, Propulsion, Fuel Tanks, Structural, Aerodynamic, Utility, Electricity, Interstellar, Cargo.
- **PlacedPart**: Editor representation with position, fuel_type, tank_filled state.
- **FlightPart**: Runtime part with resources HashMap, engine state, decoupled flag.
- **Three hitbox types**: Build/flight hitbox (collision), welding hitbox (5% larger, for connections), visual sprite (what's drawn).
- Grid alignment: odd dimensions snap to square centers, even to grid lines.

### Staging
- Editor: `Vec<Vec<PlacedPartId>>` — stages contain part IDs.
- Flight: `Vec<Vec<usize>>` — stages contain part indices.
- Stage 0 fires first. `activate_next_stage()` enables engines and fires decouplers.
- Decouplers mark themselves + all parts below (by Y position) as `decoupled`.
- Decoupled parts are excluded from mass, thrust, rendering, and fuel consumption.
- UI uses egui drag-and-drop (`dnd_drag_source`/`dnd_drop_zone`) for reordering.

### Fuel System
- Fuel is **shared** across all non-decoupled tanks (no crossfeed control).
- Eight propellant families: Kerolox (RP-1), Methalox (CH4), Hydrolox (LH2), Hydrogen (NTR), Xenon (Electric), Fusion Fuel, Antimatter, Nuclear Pulse.
- Each tank stores LOX + one fuel type. Capacity scales linearly with grid area.
- Engines consume fuel proportionally from all available tanks.
- Tank dry mass: 35 kg per grid square.

### Rendering Pipeline
- wgpu with custom vertex shader (`shader.wgsl`), 4x MSAA.
- `Vertex { position: [f32; 2], color: [f32; 4] }` — everything is colored triangles.
- Editor: `generate_part_vertices()` builds vertex arrays each frame.
- Flight: parts rendered via `generate_part_shape_vertices()`, then fairings in a second pass.
- Procedural details: engines get chambers/nozzles, pods get windows, decouplers get rings + adapter trapezoids with frustum-projection detail lines.

### Delta-V Calculation
- Per-stage: Tsiolkovsky equation `Δv = Isp * g0 * ln(wet/dry)`.
- Simulates staging sequentially: fire decouplers, enable engines, burn all fuel, next stage.
- Isp is thrust-weighted average of active engines (vacuum).
- Displayed per-stage (green text) in staging panel and as total in stats bar.

### Colony System
- Colony state per-body in `game.colonies: HashMap<usize, Colony>`.
- Buildings produce/consume resources each tick via `colony/simulation.rs`.
- 26 resource types (9 raw, 5 processed, 11 fuels, 1 consumable) in `colony/resources.rs`.
- Tech tree gates building unlocks and part availability.
- Trade routes transfer cargo between colonies on a schedule.
- Contracts provide objectives and rewards (money + science).

### Galaxy System
- Procedurally generated stars per-sector from deterministic seed.
- `GalaxyState` caches generated sectors in a `HashMap<SectorCoord, Vec<ProceduralStar>>`.
- Density model: exponential disk (8,500 ly scale) + Gaussian bulge (σ=2,000 ly) + 2-arm spiral (12.6° pitch).
- Each star has: spectral type, temperature, luminosity, mass, galactic orbital elements.
- Stars orbit Sgr A\* on elliptical paths using enclosed galactic mass model.
- Galaxy view activates at ~400 ly camera span.

### Save System
- `src/save.rs` handles full game state serialization (RON format).
- Auto-save every 5 minutes, quicksave slots.
- Saves: ship state, vessel, all colony data, tech progress, economy, time.

## Terminology

- **Ship view**: Zoomed in close enough that the ship's triangle icon is invisible.
- **Map view**: Zoomed out enough that the ship's triangle icon is visible.
- **On-rails**: High time warp mode using Keplerian propagation instead of numerical integration.
- **SOI**: Sphere of Influence — boundary where a body's gravity dominates.
- **Patched conics**: Trajectory prediction that stitches Keplerian orbits across SOI boundaries.

## Common Patterns

- `main.rs` orchestrates: it calls `render_flight_frame()` or `render_editor_frame()` based on game mode, handles input dispatch, and bridges data between Game/RenderState.
- `render/` is split across modules: `state.rs` (struct + setup), `flight.rs` (HUD), `scene.rs` (geometry), `menus.rs` (title/tracking station), `interaction.rs` (hover/click), `editor_render.rs` (editor). All are `impl RenderState` blocks.
- Editor UI is in `editor/ui.rs`. Editor vertex generation is in `editor/render.rs`.
- Data flows: EditorState -> Blueprint -> FlightVessel (on launch). FlightVessel -> ShipRenderData -> RenderState (each frame for display).
- `RenderState` has request fields (e.g., `staging_reorder`, `engine_toggle_request`) that main.rs reads and applies to the vessel each frame.
