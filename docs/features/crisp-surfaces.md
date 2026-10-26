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
          (name: "ice", source: (237, 239, 240), albedo: Some((0.96, 0.97, 0.98)), min_abs_lat: Some(55.0), pattern: (scale_m: 3000.0, contrast: 0.06)),
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

- **Zone lists.** Earth: 12 zones from k-means centres of the Blue Marble land pixels in CIELAB: ice, dark forest, forest, woodland, scrub, red earth, steppe, desert, tundra, bare rock, pale ground (salt pans, patchy snow), lake. Two zones were added to the prototype's clusters: *red earth* (Australia's and the Kalahari's red interiors fell into scrub) and *dark forest* (the eucalyptus forests of south-east Australia were taken for lakes). Lakes are near-black in the map (the Caspian is sRGB (0, 4, 3)), so the lake source is (3, 6, 8). No constraints are used: everything the unconstrained ice zone takes below 55° is a real glacier (Patagonia, Kerguelen, South Georgia, high mountains), and salt pans (Uyuni, L* 86) are nearer pale ground than ice (L* 94). Moon: mare, highland, bright highland (the far side's highlands are brighter than the near side's), bright ejecta. Zones are drawn with a physical albedo (below), not their Blue Marble colour.
- **Albedo (2026-10-25, after the first screenshots).** Blue Marble is a display stretch, not reflectance: its dark forest, sRGB (31, 43, 15), is linear (0.014, 0.024, 0.005), 2–3× darker than measured forest, and under the physical haze from orbit land vanished into the sea. Each zone is now drawn with a physical albedo (`albedo`, linear reflectance for roughly MODIS bands 1, 4, 3: ~645, 555, 470 nm); Blue Marble only classifies and gives the tint ratio. The values are representative within the published ranges below. The per-class tables of Gao et al. (2005) and Tsvetsinskaya et al. (2002) were behind paywalls when this was written, so their ranges are quoted from the literature as known, not re-read; check them when the papers are at hand.

  | Zone | Albedo (R, G, B), linear | Source |
  |---|---|---|
  | ice | 0.96, 0.97, 0.98 | Antarctic snow, 0.96–0.98 over the visible: Grenfell, Warren & Mullen 1994, JGR 99(D9) 18669 |
  | dark forest | 0.025, 0.040, 0.018 | MODIS dense dark vegetation, ρ0.47 ≈ ρ2.1/4, ρ0.66 ≈ ρ2.1/2, ρ2.1 0.05–0.1: Kaufman et al. 1997, IEEE TGRS 35, 1286; visible albedo of evergreen forest 0.03–0.04: Gao et al. 2005, JGR 110 D01104 (MODIS MCD43 by IGBP class) |
  | forest | 0.035, 0.055, 0.025 | as above; broadleaf and mixed forest 0.04–0.05 (Gao et al. 2005) |
  | woodland | 0.055, 0.070, 0.035 | croplands, woody savanna, 0.05–0.07 (Gao et al. 2005) |
  | scrub | 0.12, 0.10, 0.06 | open shrubland, dry savanna, 0.08–0.12 (Gao et al. 2005) |
  | red earth | 0.20, 0.11, 0.06 | iron-rich arid soils, steep red slope (Tsvetsinskaya et al. 2002, GRL 29(9) 1353) |
  | steppe | 0.18, 0.15, 0.10 | dry grassland, 0.1–0.15 (Gao et al. 2005) |
  | desert | 0.40, 0.30, 0.20 | Sahara and Arabian sand seas, MODIS white-sky band albedos (Tsvetsinskaya et al. 2002) |
  | tundra | 0.11, 0.11, 0.09 | sparse tundra and cold plateaus, between open shrubland and barren (Gao et al. 2005) |
  | bare rock | 0.18, 0.17, 0.15 | barren / sparsely vegetated outside the sand seas, 0.15–0.2 (Gao et al. 2005) |
  | pale ground | 0.70, 0.72, 0.72 | Salar de Uyuni, a calibration site, 0.7–0.8: Lamparelli et al. 2003, IEEE TGRS 41(6) 1461 |
  | lake | open water | `ocean.albedo` |
  | Moon mare | 0.070, 0.062, 0.054 | LROC WAC normal albedo at 643 nm, maria ~0.06–0.08, highlands ~0.13–0.18: Sato et al. 2014, JGR Planets 119, 1775; the red slope (415/643 nm ≈ 0.6 in the highlands, flatter in the maria) for green and blue |
  | Moon highland | 0.145, 0.124, 0.102 | as above |
  | Moon bright highland | 0.175, 0.150, 0.123 | as above (the far side) |
  | Moon bright ejecta | 0.240, 0.206, 0.170 | as above (fresh craters and rays) |

  The ordering by brightness matches Blue Marble's, so the tint ratio never inverts a zone. The Moon becomes much darker than its colour map (which is normalised to a bright grey): its mean normal albedo is now ~0.14, consistent with its Bond albedo of 0.12.
- **Bake results** (by area, cos-latitude weighted): Earth sea 70.7 % (the real figure is 71 %); of land, forest 30.1 %, woodland 21.3 %, scrub 11.2 %, ice 10.4 % (about 10 % of land is glaciated), desert 9.7 %, steppe 9.5 %, red earth 2.6 %, bare rock 1.3 %, dark forest 1.2 %, pale ground 1.2 %, tundra 0.9 %, lake 0.6 %. Moon: highland 61.0 %, bright highland 22.9 %, mare 14.9 % (the maria cover ~16 %), bright ejecta 1.2 %. `zones.png` is 0.29 MB (Earth) and 0.38 MB (Moon). Lakes narrower than ~20 km do not survive the 10 km grid and the majority filter; Lake Okeechobee is dark forest.
- **Sea fill.** At load, sea pixels take the nearest land zone that is not water (breadth-first, `ZoneMap::fill_sea`): the coast comes from the water mask, so a land fragment by the coast always finds a land zone in its gather, and lakes never spread into the sea.
- **Tint.** Baked into the far texture at load, from the land part of the colour map only (pixels where the sea mask is under half), so coastal land is not darkened by the sea. Near, the tint is the far texture's base level over the palette (both bilinear), so the near path needs no colour-map lookup and its colours match the far ones.
- **Water mask resolution.** With zones on, the water mask loads at least at the zone map's width (4096 for Earth), so coasts and zone edges have the same detail.
- **Warp.** First octave 4 zone texels (39 km on Earth, 10.7 km on the Moon), 0.6 texel, halving per octave; an octave is fully drawn at 3 pixel footprints and gone at 2; at most 10 octaves and none under 100 m (the body-fixed position is good to ~0.5 m in f32). From 300 km (220 m pixels at nadir) that is 7 octaves, at the pad 9. Zone patterns: octaves from `scale_m` down, at most 6, none under 40 m; close up, the simple ground pattern takes over. Noise: 3D value noise with an integer lattice hash.
- **No new samplers.** The fragment stage allows 16 samplers and High's view bindings already use the rest (two zone samplers crashed the demo at High, 2026-10-25): the zone indices are read with `textureLoad` (longitude wraps, latitude clamps, `zones::texel_block`), and the zone colours use the colour map's sampler (same settings), so zones draw only once the colour map is loaded.
- **Exact coasts and edges.** The coast is computed from a `textureGather` (the water mask's own sampler) with its own bilinear weights (the hardware's 8-bit filter weights would staircase the 0.5 contour when magnified), and both the coast and the zone edges are antialiased with analytic gradients (no `fwidth`), so the near path can be skipped in branches.
- **Shallow water.** The two-flow model: albedo + (shallow − albedo)·exp(−depth / 10 m), shallow (0.02, 0.09, 0.07) (π·Rrs over the Bahamas Banks at a few metres). Physically only water shallower than ~20 m is visibly lighter, so the shallow banks (Bahamas, reefs, the first cells off a coast) light up and continental shelves at 50–200 m stay dark, as they do in photographs; the design's "lighten the continental shelves" is not followed where it would be unphysical. The depth comes from the sim's heightmap (the mesh is raised to sea level, so its height has no bathymetry).
- **Rock and the snow line** were not drawn in Minimal before (they were part of the detail layer); with zones they are, as shared shader functions.
- **Sky.** The adaptive mode (lookup tables inside an atmosphere, raymarching from outside) had been tied to the sky light since 2026-09-27, so the tiers without it (Medium, High) raymarched at the pad too; it now depends on the camera's depth only (`atmosphere::raymarch`).
- **GPU memory** (Earth): zone map 8.4 MB, far texture 44.7 MB with mips, water mask at 4096 instead of 2048 +8.4 MB: ~61 MB. Moon ~53 MB. About the budget per body, twice it for both.
- **Known limits.** Zone edges are bilinear contours of a one-hot field: smooth curves with a slight bead where a third zone has a single texel along an edge; a smoother kernel (a B-spline over 4×4 texels) would round them further at ~4× the gathers. The texture coordinates are f32, good to ~2–5 m on Earth, so a coastline seen from under ~100 m shows small steps.
- **Checked so far:** the bake's previews; a CPU re-implementation of the near path over the baked data (Florida, the Nile delta, Hudson Bay: crisp, recognisable coasts and zones); the shader parses, validates and compiles to MSL with naga 29 (main and prepass variants). Not yet seen in the game, and not yet measured.

