# Crisp surfaces and a physical sky (Minimal tier)

**Status:** built 2026-10-25, not yet seen on screen (design the same day). From the owner's feedback ([record](../plans/interview-2026-10-25.md)); decision D073 (revised). What was built and the choices made: [Build notes](#build-notes-2026-10-25).

**Goal:** from any distance, a planet's surface is sharp and recognisable: water is water, land is land, ice is ice, a desert is a desert. Nothing is ever a magnified photograph. The sky is the physical one from Bevy's scattering model, and from orbit the ground stays clear under a thin blue haze.

## Why it is blurry today

- The Minimal tier caps the colour map at 2048 px: 20 km per texel on Earth, ~90× magnified at nadir from 300 km. The full 8192 map is still ~23× magnified.
- The water mask is thresholded with `smoothstep(0.3, 0.7)` on a 5 km texel: the coastline is a 2–3 km gradient.
- The lookup-table atmosphere's aerial perspective, used from orbit in Minimal, washes the ground out in blue.

## Design

The owner's recipe: "an image download sharpened up (coastlines etc) with procedural algorithms, then a consistent texture applied for various colors/zones."

### 1. Zones: baked from the real data (`asset-tool zones`)

- **Data** (`visual.ron`, new `zones` block; generic for any body):
  ```ron
  zones: Some((
      map: "zones.png",
      // How much of the satellite colour's variation survives inside a zone (0 = flat zone colours).
      tint: 0.3,
      list: [
          // `source`: the zone's colour in the colour map (sRGB), the classifier's centre.
          // `color`: the colour it is drawn with (sRGB). `pattern`: its procedural texture.
          (name: "ice", source: (237, 239, 240), color: Some((236, 240, 245)), min_abs_lat: Some(55.0), pattern: (scale_m: 3000.0, contrast: 0.06)),
          (name: "forest", source: (31, 43, 15), color: (40, 62, 28), pattern: (scale_m: 800.0, contrast: 0.25)),
          (name: "lake", source: (20, 30, 45), water: true),
          …
      ],
  )),
  ```
  Optional constraints per zone (`min_abs_lat`, `max_abs_lat`, `min_height_m`, `max_height_m`) keep, for example, bright salt pans from becoming ice. `water: true` zones are drawn as water (lakes above sea level, which the sea mask does not hold).
- **Bake:** the tool reads the committed `color.jpg`, `water.png` and `height.png`. For each output pixel it takes an area average of the colour (in linear light), the sea fraction and the height. Sea (fraction ≥ 0.5) becomes index 0. Land takes the zone whose `source` is nearest in a perceptual space (CIELAB), among the zones whose constraints hold. A 3×3 majority filter removes speckle. The output is `zones.png`: 8-bit grayscale, the index per pixel (0 sea, 1..N the list order), equirectangular in the `grid` convention. Earth and Moon at 4096 × 2048 (~10 km and ~2.7 km per texel). A k-means prototype on Blue Marble (2026-10-25, 9 clusters on land) separated ice, forest, temperate, scrub, steppe and desert cleanly; inland seas such as the Caspian need the `lake` zone.
- **Moon:** mare, highland, bright ejecta, from the LROC map.
- **Size:** each `zones.png` well under 5 MB (D046: ~60 MB in total).

### 2. Shader: crisp at every distance

All of this is in `terrain.wgsl`, behind a `zones` graphics toggle. The toggle is on in Minimal, off in the other tiers (D073: they are not developed). Without a zone map, the current path is used.

- **One warp.** A 2D domain-warp vector from 3D fBm of the body-fixed position, in units of a zone texel: amplitude ~0.6 texel in the first octave, halving per octave. Octaves are added until their wavelength drops under ~2 pixel footprints, and the last one fades in by footprint, so nothing shimmers. Both the zone and the sea lookups use the same warp, so coastlines and zone edges are fractal, line up, and keep gaining detail as the camera closes in.
- **Coastline.** The warped, bilinear sea fraction is thresholded at 0.5 over one pixel (`fwidth`). That gives a hard, antialiased edge at every zoom, replacing the 0.3–0.7 smoothstep.
- **Zone edges (near).** A `textureGather` of the 4 zone indices around the warped texel position, with their bilinear weights summed per distinct zone. The zone with the largest weight wins, antialiased against the runner-up over one pixel. This works like marching squares on a one-hot field, so edges are smooth curves, not texel staircases.
- **Zone colour (far).** When a pixel covers more than ~1 zone texel, a mipmapped zone-colour texture is used instead. It is built at load time from the index map and the palette. The two are crossfaded between 0.5 and 1 texel per pixel, so minification is filtered and magnification is crisp.
- **Consistent texture per zone.** The zone's `color` × (1 + `contrast` × fBm tone at the zone's `scale_m` and its octaves). The octaves fade by footprint as in the simple ground pattern, so a forest looks like the same forest from 300 km and from 3 km. Close to the ground, the existing simple ground pattern (5–320 m) adds depth perception.
- **Tint.** `color *= clamp((satellite / source)^tint, 0.5, 2)`. This keeps a gentle, regional variation from the real map inside a zone. Only a ratio is taken, so it never shows up as blur.
- **Water.** Open water keeps its physical albedo (`ocean.albedo`). New `ocean.shallow_albedo` and `shallow_depth_m` lighten the continental shelves by depth, taken from the mesh height (ETOPO bathymetry), so coasts read from orbit. Lakes use the same shading.
- **Kept as they are:** rock on steep slopes, the snow line (both physical), lighting, eclipses, planetshine.

### 3. Sky

- Minimal uses the adaptive raymarched atmosphere (`AtmosphereQuality::Raymarched`): lookup tables inside an atmosphere, raymarching from outside. The cost was measured on 2026-09-26: +0.6 ms from orbit, nothing at the pad. Bump `persist::GRAPHICS_VERSION` so saved settings pick up the new preset.
- Check against photographs (ISS nadir and limb views, a sea-level sky): the ground is clear at nadir from orbit, hazier towards the limb, and the limb has a thin blue band. If the haze is still too strong from orbit after this, report it with numbers; do not tune the scattering coefficients by eye (they are Hillaire's).

## Owners

- `asset-tool::zones` (new): the classification. `game::terrain::zones` (new): loading the zone map, the zone-colour texture and the palette uniform. Also there: a CPU mirror of the shader's zone pick and coast step, which is tested (like `terrain::water`).
- `game::body_visual`: the `zones` data. `game::settings`: the `zones` toggle and the Minimal preset.

## Tests

- asset-tool: nearest zone with constraints (table), sea → 0, the majority filter (table), deterministic output.
- game: the zone pick from 4 indices and weights, including the runner-up and its margin (table); the coast step at 0.5 (table); the octave count from the footprint (table); palette packing; every shipped `visual.ron` with zones parses; every index in the shipped `zones.png` is below the list length + 1.
- Budget: Minimal at 1600×900 on the reference Mac: terrain fragment cost ≤ +0.3 ms, GPU memory ≤ +60 MB (two 4096 maps per body).

## Acceptance (screenshots from the demo, Minimal)

- Orbit at ~300 km, looking at the horizon as in the owner's screenshot: oceans, coasts, land zones and ice are clearly visible, sharp, with haze only towards the limb.
- Orbit, nadir: coastline crisp at full window resolution.
- The pad: the ground reads as land (the zone's colour and pattern), not a grey plain; the horizon has no step.
- The Moon close: maria and highlands with crisp edges.

## Build notes (2026-10-25)

Commits cfaf932 (bake), d83cbb3 (shader), f47a1e3 (sky). Choices where the design was silent, and where the build differs from it:

- **Zone lists.** Earth: 12 zones from k-means centres of the Blue Marble land pixels in CIELAB: ice, dark forest, forest, woodland, scrub, red earth, steppe, desert, tundra, bare rock, pale ground (salt pans, patchy snow), lake. Two zones were added to the prototype's clusters: *red earth* (Australia's and the Kalahari's red interiors fell into scrub) and *dark forest* (the eucalyptus forests of south-east Australia were taken for lakes). Lakes are near-black in the map (the Caspian is sRGB (0, 4, 3)), so the lake source is (3, 6, 8). No constraints are used: everything the unconstrained ice zone takes below 55° is a real glacier (Patagonia, Kerguelen, South Georgia, high mountains), and salt pans (Uyuni, L* 86) are nearer pale ground than ice (L* 94). Moon: mare, highland, bright highland (the far side's highlands are brighter than the near side's), bright ejecta. Each zone is drawn in its source colour (`color` is optional, default `source`): the map's own reflectance, so the drawn albedo stays physical.
- **Bake results** (by area, cos-latitude weighted): Earth sea 70.7 % (the real figure is 71 %); of land, forest 30.1 %, woodland 21.3 %, scrub 11.2 %, ice 10.4 % (about 10 % of land is glaciated), desert 9.7 %, steppe 9.5 %, red earth 2.6 %, bare rock 1.3 %, dark forest 1.2 %, pale ground 1.2 %, tundra 0.9 %, lake 0.6 %. Moon: highland 61.0 %, bright highland 22.9 %, mare 14.9 % (the maria cover ~16 %), bright ejecta 1.2 %. `zones.png` is 0.29 MB (Earth) and 0.38 MB (Moon). Lakes narrower than ~20 km do not survive the 10 km grid and the majority filter; Lake Okeechobee is dark forest.
- **Sea fill.** At load, sea pixels take the nearest land zone that is not water (breadth-first, `ZoneMap::fill_sea`): the coast comes from the water mask, so a land fragment by the coast always finds a land zone in its gather, and lakes never spread into the sea.
- **Tint.** Baked into the far texture at load, from the land part of the colour map only (pixels where the sea mask is under half), so coastal land is not darkened by the sea. Near, the tint is the far texture's base level over the palette (both bilinear), so the near path needs no colour-map lookup and its colours match the far ones.
- **Water mask resolution.** With zones on, the water mask loads at least at the zone map's width (4096 for Earth), so coasts and zone edges have the same detail.
- **Warp.** First octave 4 zone texels (39 km on Earth, 10.7 km on the Moon), 0.6 texel, halving per octave; an octave is fully drawn at 3 pixel footprints and gone at 2; at most 10 octaves and none under 100 m (the body-fixed position is good to ~0.5 m in f32). From 300 km (220 m pixels at nadir) that is 7 octaves, at the pad 9. Zone patterns: octaves from `scale_m` down, at most 6, none under 40 m; close up, the simple ground pattern takes over. Noise: 3D value noise with an integer lattice hash.
- **Exact coasts and edges.** The coast is computed from a `textureGather` with its own bilinear weights (the hardware's 8-bit filter weights would staircase the 0.5 contour when magnified), and both the coast and the zone edges are antialiased with analytic gradients (no `fwidth`), so the near path can be skipped in branches.
- **Shallow water.** The two-flow model: albedo + (shallow − albedo)·exp(−depth / 10 m), shallow (0.02, 0.09, 0.07) (π·Rrs over the Bahamas Banks at a few metres). Physically only water shallower than ~20 m is visibly lighter, so the shallow banks (Bahamas, reefs, the first cells off a coast) light up and continental shelves at 50–200 m stay dark, as they do in photographs; the design's "lighten the continental shelves" is not followed where it would be unphysical. The depth comes from the sim's heightmap (the mesh is raised to sea level, so its height has no bathymetry).
- **Rock and the snow line** were not drawn in Minimal before (they were part of the detail layer); with zones they are, as shared shader functions.
- **Sky.** The adaptive mode (lookup tables inside an atmosphere, raymarching from outside) had been tied to the sky light since 2026-09-27, so the tiers without it (Medium, High) raymarched at the pad too; it now depends on the camera's depth only (`atmosphere::raymarch`).
- **GPU memory** (Earth): zone map 8.4 MB, far texture 44.7 MB with mips, water mask at 4096 instead of 2048 +8.4 MB: ~61 MB. Moon ~53 MB. About the budget per body, twice it for both.
- **Known limits.** Zone edges are bilinear contours of a one-hot field: smooth curves with a slight bead where a third zone has a single texel along an edge; a smoother kernel (a B-spline over 4×4 texels) would round them further at ~4× the gathers. The texture coordinates are f32, good to ~2–5 m on Earth, so a coastline seen from under ~100 m shows small steps.
- **Checked so far:** the bake's previews; a CPU re-implementation of the near path over the baked data (Florida, the Nile delta, Hudson Bay: crisp, recognisable coasts and zones); the shader parses, validates and compiles to MSL with naga 29 (main and prepass variants). Not yet seen in the game, and not yet measured.

