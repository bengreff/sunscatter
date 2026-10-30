# The world: bodies, ephemeris, gravity, air, weather, terrain

The places craft fly through and land on: the Sun, planets, moons and dwarf planets on their real orbits, each with its shape, rotation, gravity field, atmosphere and surface. Each body is a data directory, `data/bodies/<body>/`, with `body.ron` for the sim and `visual.ron` for the game (D051).

**Status:** motion and physical data are *playable* for the whole Solar System (D084), tested against JPL. Gravity fields beyond the point mass are *playable*. Earth's and the Moon's surfaces use real elevation and colour data; the other solid bodies have maps, zones and physical terrain, measured where a global DEM exists and procedural otherwise. Weather is decided (D113) and not built: visual work is frozen until the first two flights fly end to end (D123). Oceans are solid (D037); a calm sea is next (D123). Beyond the Solar System nothing is built yet.

## Laws

- **The "actually exists" rule** (D013, D014, D084): real bodies first, largest first; full bodies have a minimal definition (D015).
- **Bodies are on rails** (D009): ephemerides generated offline by `ephem-tool` from JPL's DE440 and fitted with Chebyshev polynomials, moon systems in their barycentres' frames. The golden tests fail if the shipped file and the code disagree.
- **Terrain is physical** (D047): the ground a craft touches is the ground drawn. Detail below the heightmap's resolution is procedural and lives in `sim` (D059), deterministic from the body.
- **Gravity is measured** (D090): spherical harmonics in each body's rotating frame, truncated by significance, felt on every force path.
- **Light is physical** (D052, D055, D087): star flux and eclipses per body, at a fixed exposure that follows the starlight at the camera.
- **Weather** (D113): deterministic from the save's seed, smooth, cheap to query anywhere, fractal in detail, no gameplay effect yet; open to exotic conditions.

## What exists

- **The Solar System** (D084): 44 bodies: the Sun, the 8 planets, nine dwarf planets and every moon over 100 km mean radius, with physical data; far moon systems pull as one mass (clusters, D023). *Playable.* [solar-system.md](../technical/solar-system.md)
- **Gravity fields** (D090, `sim::gravity`). *Playable.* [solar-system.md](../technical/solar-system.md#gravity-fields-d090)
- **Atmospheres** (`sim::body`): Earth's from the US Standard Atmosphere 1976 to 1,000 km; Venus, Mars and Titan sourced near the surface and assumed above; each names its gas. *Playable.* [aero-thermal.md](../technical/aero-thermal.md)
- **Terrain** (`sim::terrain`, `game::terrain`): bicubic heightmaps, procedural sub-sample detail and crater fields; LOD meshes with geomorphing. *Playable.*
- **Surfaces, the one look** (D073): each body's real maps classified into zones (`asset-tool zones`) and drawn crisp at every distance. Built 2026-10-25. [crisp-surfaces.md](../technical/crisp-surfaces.md), [solar-visuals.md](../technical/solar-visuals.md)
- **Giants and rings:** the giants' colour maps (OPAL), planetary rings with their shadow; opaque cloud decks for bodies with a cloud top (`game::clouds`). [solar-visuals.md](../technical/solar-visuals.md)
- **Starlight** (`sim::light`): flux, eclipses by body spheres, orbit-averaged sunlight. *Playable.*

## Not built

- Weather and Earth's moving clouds (D113): *design*, frozen (D123).
- Water: oceans are solid ground (D037). A calm, flat sea with slamming, added mass and buoyancy comes first (D123), waves later.
- The 1,000-year ephemeris (D005): open for the owner ([solar-system.md](../technical/solar-system.md#the-1000-year-problem-for-the-owner)).
- Stars and the 100-light-year catalog (D004, D010), procedural bodies (D014): *design* only.
- Planet albedo and infrared on vessels ([ship-model.md](../technical/ship-model.md) §12).

## Owners and decisions

- Owners: [owners.md, The world](../owners.md#the-world-bodies-ephemeris-gravity-terrain-light); the drawing of bodies is in [The game: rendering](../owners.md#the-game-rendering).
- Decisions: D004, D005, D009, D010, D013–D016, D037, D046, D047, D051, D052, D055, D059, D084, D086, D087, D090, D113, D123.
- Technical: [solar-system.md](../technical/solar-system.md), [solar-visuals.md](../technical/solar-visuals.md), [crisp-surfaces.md](../technical/crisp-surfaces.md), [motion-model.md](../technical/motion-model.md) §1.
