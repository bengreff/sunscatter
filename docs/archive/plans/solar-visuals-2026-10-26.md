# Plan, 2026-10-26 (day): the Solar System in Earth's visual style

The owner, leaving for the day: "launch an agent or two on something (like building the solar system) while I am gone. Similar visual style to earth, accounting for lighting and terrain and atmosphere." Decisions: D073 (crisp, simple surfaces), D084 (the bodies), D086 (map budget ~150 MB, default), D087 (exposure by the starlight at the camera, default), D047/D059 (terrain is physical, procedural detail in `sim`), D052/D055 (atmospheres, lighting).

The owner, later the same day: "make sure they are building an engine that will be applicable with any planet and any number of light sources without any bugs." So every visual feature is generic and data-driven (D048): no code matches on a body's name or assumes Sol (one star, rings only on Saturn, clouds only on Venus); every lit object takes the light of every relevant star at its own position, with that star's eclipses and colour, and planetshine from every relevant body (D055); where an engine limit forces a bounded set of lights, a deterministic, tested rule picks it (the brightest few by flux, the rest as ambient); multi-star and multi-occluder cases have table tests and a synthetic binary-star test world.

Two agents, separate files. Status words: *design / skeleton / playable / polished*.

## TOUR (first, shared): see any body

`SUNSCATTER_TOUR=<body,body,…> SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 cargo run -p game --release`: offscreen screenshots of each body (Minimal tier): the whole disc sunlit from ~3 radii, the limb from low orbit, and the ground from ~2 km at a low Sun. Built by agent SKY in its first hour. One game process at a time: hold the lock `mkdir /tmp/sunscatter-demo.lock` while it runs, `rmdir` after.

## Agent LANDS: solid bodies

Mercury, Mars (surface), Io, Europa, Ganymede, Callisto, Ceres, Mimas, Enceladus, Tethys, Dione, Rhea, Hyperion, Iapetus, Phoebe, Miranda, Ariel, Umbriel, Titania, Oberon, Triton, Pluto, Charon, Eris, Haumea, Makemake, Gonggong, Quaoar, Sedna, Orcus and the dwarf planets' moons.
- Real maps where public-domain global data exist (USGS Astrogeology / NASA PDS: MESSENGER, MOLA and Viking/THEMIS, Galileo/Voyager, Cassini, Dawn, New Horizons): heights (DEM) and colour, baked by `asset-tool` like Earth's and the Moon's (`grid` convention), zones (D073) and roughness. Physical terrain in `sim` (D047): `body.ron` heightmap, `solid: true`.
- No global data: a procedural surface from the body's measured statistics (albedo, colour, crater density), stated as such.
- Budget (D086): ~90 MB for all of these together; every file under 50 MB.

## Agent SKY: atmospheres, giants, lighting

- **TOUR** (above), first.
- **Atmospheres** in Bevy's scattering model (D052) from sourced data: Mars (CO₂ Rayleigh, dust Mie: butterscotch day sky, blue sunset), Venus (thick CO₂, the sulfuric-acid cloud deck: an opaque yellowish disc from orbit), Titan (N₂ with orange tholin haze). Venus's and Titan's surfaces (Magellan, Cassini radar) under them.
- **Giants:** Jupiter, Saturn, Uranus, Neptune drawn from global colour mosaics (Juno/Cassini/Voyager) on their 1-bar ellipsoids, with their limbs; Saturn's rings if affordable (Cassini radial profile, with shadows).
- **Lighting:** D087's exposure; check eclipses, ring and planet shadows, and planetshine on moons of the giants.
- Budget (D086): ~30 MB for these bodies.

## Rules

- Each agent builds in `target/agents/<name>` and deletes it at the end. The pre-push hook is not enabled in this checkout yet: run CI's checks yourself before every push (fmt, file sizes, decision refs, clippy `--all-targets`, `--no-fail-fast` tests for sim, and for game when touched).
- Commit only your own paths; never stash or reset; no branches or worktrees.
- Cite every source (licence, resolution, URL) in `docs/technical/solar-visuals.md`.
