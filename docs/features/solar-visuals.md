# The Solar System in Earth's visual style

**Status:** in progress, 2026-10-26 (owner away; plan: [solar-visuals-2026-10-26.md](../plans/solar-visuals-2026-10-26.md)). Decisions: D073 (crisp, simple surfaces), D084, D086 (map budget), D087 (exposure), D052/D055 (atmospheres, lighting). Two agents, each editing its own sections: LANDS (solid bodies' maps and terrain) and SKY (tour, exposure, atmospheres, giants, lighting). Every source is cited with its licence, resolution and URL.

## Seeing a body: the tour (SKY)

`SUNSCATTER_TOUR=<body,body,…> SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 cargo run -p game --release` (`game::tour`): per body, in the Minimal tier with the clock frozen and no interface, the whole disc from 3 radii at a 30° phase angle (`disc`), the limb from low orbit (1.5 % of the radius up, or 5 % above the atmosphere's top; `limb`), and the ground from 2 km above the terrain with the Sun 10° up (`ground`, solid bodies only). The low views look along the terminator (light from the side) with the horizon 8° above the frame's centre. Each shot waits for the terrain and its maps (`Terrain::busy`, 1.5–45 s). Options: `SUNSCATTER_TOUR_DATE=YYYY-MM-DD[THH:MM]` (TDB) starts a new game at that date (the ephemeris runs 2030–2080; Saturn's rings are lit from the south until 2039, from the north after); `SUNSCATTER_TOUR_VIEWS=disc,limb,ground,eclipse,transit` picks the views. The event views search forward from the date (up to 120 days, 10-minute steps, refined to 1 s) and frame like `disc`: `eclipse`, a moon at its eclipse's ingress (half the Sun hidden by its primary); `transit`, the primary when the moon's shadow axis passes within 0.6 radii of its centre. Names match ignoring case and punctuation (`hiiaka`).

## Exposure follows the starlight (SKY, D087)

`lighting::exposure_ev100`: EV100 = 14.5 + log₂(E_cam / E_1AU), where E_cam is the Sun's flux at the camera (inverse square, no eclipses: the light where the photographer is, never the scene's luminance), clamped to −14…+4 stops; a setting (`starlight_exposure`, Graphics, not part of the tiers) turns it off. Numbers: at Earth (0.983 AU in January) +0.05 stops; Jupiter (5.2 AU) −4.76 (EV 9.74); Saturn (9.5 AU) −6.50; Neptune (30 AU) −9.81; Mercury (0.387 AU) +2.74. A day side then reads like Earth's on screen at any distance, and a night side stays as many stops below its day side as at Earth (D055). Stars are drawn in display units, so they do not change. **Under a haze** (Titan's, Venus's: bodies with `raymarch_inside`) the flux at the camera is the light that reaches it through the column above (`lighting::haze_exposure_share`: the direct and diffuse transmission of `haze_above`, Rec. 709-weighted, for an overhead Sun so a night side stays dark): on Venus's ground about 1 % of the light at the top (6–7 stops), as Venera's photometers found a few per cent; Earth is unchanged.

**One atmosphere at a time.** Bevy draws one atmosphere per camera, the nearest. Before, with Earth's the only one, it was always drawn; with Mars's added, Mars's air tinted Jupiter dark red from 4 AU (f32 round-off in its ray tests). Now only the atmosphere the camera is deepest in, within 100 outer radii of its body, is drawn (`atmosphere::active`; Earth's still covers the Moon at 60 radii).

## Atmospheres (SKY)

Bevy's Hillaire model (D052) at 680, 550 and 440 nm. New in the data (`visual.ron`): `mie_asymmetry_rgb` (the Mie term's asymmetry per channel: Bevy's phase function has one asymmetry per term, so such a term becomes one term per channel, `atmosphere::mie_terms`) and `layers` (further aerosol layers with exponential or tent profiles).

### Mars

| Quantity | Value (R, G, B) | Source |
|---|---|---|
| CO₂ Rayleigh cross-section | 4.89, 11.6, 29.2 × 10⁻³¹ m² | Sneep & Ubachs 2005, JQSRT 92, 293 (refractivity and King factor of CO₂; 13.3 × 10⁻³¹ at 532 nm against their measured 12.4 ± 0.9) |
| Surface number density | 2.09 × 10²³ m⁻³ | body.ron (699 Pa, 242 K, NASA Glenn's Mars model) |
| Rayleigh scattering | 1.02, 2.43, 6.11 × 10⁻⁷ m⁻¹; scale height 11.1 km | the two above; body.ron's pressure law |
| Dust optical depth | 0.5 at 880 nm | typical outside dust storms: Lemmon et al. 2015, Icarus 251, 96 (0.3–1) |
| Dust single-scattering albedo | 0.95, 0.92, 0.87 | Wolff et al. 2009, JGR 114, E00D04 (0.94–0.96 at 650 nm, 0.86–0.90 at 440 nm); Tomasko et al. 1999, JGR 104, 8987 (0.84 at 443 nm to 0.92 at 671 nm) |
| Dust extinction relative to 880 nm; asymmetry | 0.962, 0.934, 0.910; g 0.73, 0.75, 0.78 | Mie theory for the measured sizes (gamma distribution, r_eff 1.5 µm, v_eff 0.3: Wolff et al. 2009, Lemmon et al. 2015), n = 1.5, k fitted to the albedos above (0.0019, 0.0027, 0.0039). It gives g = 0.69 at 965 nm, as Tomasko et al. measured (0.7) |
| Dust profile | well mixed, the gas's 11.1 km scale height | |
| Dust coefficients | scattering 4.11, 3.87, 3.57 × 10⁻⁵ m⁻¹; absorption 2.17, 3.36, 5.33 × 10⁻⁶ m⁻¹ | τ / H × extinction ratio, split by the albedo |
| Ground albedo (multiple scattering) | 0.27, 0.16, 0.09 | Bond albedo 0.25 with Mars's red slope |

The larger asymmetry in blue concentrates blue light around the Sun (the blue sunset); the dust's blue absorption makes the day sky butterscotch. First tour (2026-10-26, without LANDS' surface maps): the sky from 2 km is a dull tan, the disc from 3 radii is a hazy ochre; the haze looks strong from orbit, to be checked against Viking/MGS images once the surface maps are in.

### Titan

| Quantity | Value (R, G, B) | Source |
|---|---|---|
| N₂ Rayleigh | 2.20, 5.22, 13.0 × 10⁻⁵ m⁻¹; scale height 21.0 km | air's cross-sections (Bucholtz 1995, Appl. Opt. 34, 2765) × 1.02 for N₂ × 1.135 × 10²⁶ m⁻³ (146.7 kPa, 93.65 K: body.ron) |
| Haze optical depth | 2.9, 3.6, 4.6 (vertical) | Huygens DISR: 2 at 950 nm to 4.5 at 450 nm (Tomasko et al. 2008, PSS 56, 669), a power law between |
| Haze profile | exponential, 65 km | ibid. (above 80 km); the layers below 80 km (linear in their papers) are folded in |
| Haze single-scattering albedo | 0.95, 0.90, 0.80 | ibid. (lower in blue: the orange tholin), as summarised in the literature, not re-read |
| Haze asymmetry | 0.65 | representative of their fractal aggregates' phase functions (not a fitted value) |
| Ground albedo | 0.15, 0.12, 0.08 | DISR surface reflectance ~0.1–0.2, red slope |

Thickness 600 km (the haze reaches ~500 km). From orbit (tour 2026-10-26): an orange-tan fuzzy ball without surface features and a soft, bluish-grey limb, as Cassini's natural-colour views show; the ground is dim orange-brown under a dark orange sky.

### Venus

| Quantity | Value (R, G, B) | Source |
|---|---|---|
| CO₂ Rayleigh | 4.44, 10.6, 26.5 × 10⁻⁴ m⁻¹; scale height 8.95 km | Sneep & Ubachs 2005 × 9.07 × 10²⁶ m⁻³ (9.21 MPa, 735 K: body.ron); the height puts VIRA's 3.69 kPa at 70 km (Seiff et al. 1985), so the CO₂ above the clouds is right (15.9 km, RT/Mg at the surface, put 30 times too much there: a blue veil over the disc) |
| Cloud deck | tent from 34 to 80 km peaking at 57 km, vertical optical depth 30 | the main deck at 47–70 km, τ 25–40 (Pioneer Venus LSAR, Ragent & Blamont 1980) |
| Cloud asymmetry | 0.75 | sulfuric-acid droplets, r_eff 1.05 µm, n 1.44 (Hansen & Hovenier 1974) |
| Cloud single-scattering albedo | 0.9979, 0.9973, 0.9967 | from the albedo of a deep layer, A = 1 − 4 √((1 − ω) / (3 (1 − g))), set to the Bond albedo 0.76 coloured by the geometric albedos 0.712, 0.689, 0.664 (Mallama et al. 2017): A = 0.786, 0.761, 0.733 |
| Cloud top (seen from above) | a Lambert sphere 74 km up, albedo 1.0, 0.973, 0.929 | the unit-optical-depth level at low and middle latitudes (Ignatiev et al. 2009, JGR 114, E00B43, VMC/VIRTIS); albedo 1.5 × the geometric albedos in Johnson R, V, B (0.708, 0.689, 0.658: Mallama et al. 2017, Icarus 282, 19, table 7), scaled to 1 in red. The blue drop is the clouds' unknown UV absorber, whose absorption reaches to ~600 nm (Pérez-Hoyos et al. 2018, JGR Planets 123, 145) |

**The deck seen from above** (`game::clouds`, `cloud_top` in `visual.ron`): raymarching 30 optical depths of cloud from outside in Bevy's 24 even steps drew concentric ripples at the limb, where each step crossed the cloud top (the deck's top few kilometres hold the first optical depth, the steps are 5 km at the disc's centre and ~60 km at the limb). From above the cloud top the deck is now an opaque shell, a Lambert sphere lit by the Sun's flux at Venus, and the sky above it is drawn with only the gas (`atmosphere::DeckMedia`: the medium without the layers; the shell's depth ends the raymarch); below the top the shell is hidden and the full medium is drawn. Lambert is an approximation: Venus is brighter than a Lambert sphere near full phase (its geometric albedo exceeds 2/3) and the albedo 1.5 p does not fit under 1, so red is scaled to 1 (a disc ~15 % darker than measured near 30° phase). From orbit (tour 2026-10-26, second pass): a smooth, pale cream-white disc without ripples and a thin blue rim of CO₂ above the clouds. The ground under it is lit through the deck (below) and exposed for the light that arrives there (D087, above): dim and red-brown.

### Light under a haze (Titan, Venus)

Bevy's atmosphere does not dim the Sun on our terrain, so Titan's ground first came out in full sunlight under τ ≈ 4 of haze. Bodies with `raymarch_inside: true` now light their ground through it (`lighting::haze_light`, from the same coefficients the sky is drawn with): the direct beam exp(−τ m(μ₀)) with the spherical-shell air mass m for the extinction-weighted scale height, plus the diffuse light the haze sends down, Lambertian, ∝ μ₀. The diffuse fraction is a diffusion estimate, not a radiative-transfer solution: a slab's total transmission 1 / (1 + ¾ (1 − g) τ_s) (Eddington, conservative), damped by exp(−k τ) with k = √(3 (1 − ω)(1 − ω g)), minus the direct beam. Titan: about 3–7 % of the light at the top arrives diffusely, least in blue; Venus well under 1 %. These atmospheres are also raymarched from inside: their aerial-perspective table broke into coloured rings on the ground.

### Venus and Titan surfaces (SKY)

Baked by `asset-tool planets venus titan` (LANDS' pipeline, `specs.rs`), physical terrain (D047: `body.ron` `heightmap`, `roughness`, `terrain_detail`; golden hashes and Maxwell Montes in `sim/tests/solar_terrain.rs`):

| Body | Heights | Colour (classifies only) | Zones and albedo | Size |
|---|---|---|---|---|
| Venus | Magellan global topography v2, altimetry and stereo (USGS Astrogeology, 4641 m, public domain; `Venus_Magellan_Topography_Global_4641m_v02.tif`), metres above 6,051 km; 4096 × 2048 (9.3 km); 8 % gaps filled smoothly; −3.7 to +10.7 km | Magellan SAR C3-MDIR global mosaic (USGS, 2025 m; `Venus_Magellan_C3-MDIR_Global_Mosaic_2025m.tif`), 2048 wide; radar backscatter is roughness, not colour: no tint | radar-dark plains, lava plains, rough terrain (k-means of the radar grey); reflectance ~0.07, nearly grey: Venera 13's images corrected for the orange light show a dark surface without significant colour (Pieters et al. 1986, Science 234, 1379) | 10.2 MB |
| Titan | procedural (no public-domain global DEM: Corlies et al. 2017's map is a journal supplement): 11 IAU Gazetteer craters, shallow (depth cap 800 m: Neish et al. 2013, Icarus 223, 82), a sparse population, regional fBm 300 m RMS; 2048 × 1024; −1.4 to +1.1 km | Cassini ISS 938 nm global mosaic (USGS, 4 km; `Titan_ISS_P19658_Mosaic_Global_4km.tif`), the surface through the haze's window | dune fields, plains, bright terrains; the plains take Huygens DISR's reflectance (0.15, 0.12, 0.08: Tomasko et al. 2005, Nature 438, 765), the others that times their brightness ratio in the mosaic^0.5 (LANDS' rule) | 2.1 MB |

Ground views (tour, second pass): Titan's ground is a dim orange-brown under an orange sky. Venus's ground is lit through the deck and exposed for that light, but it is still nearly black at 2 km: Bevy's atmosphere has no multiple scattering through 30 optical depths of cloud and ~10 of CO₂ Rayleigh, so the air under the deck only extinguishes (distant ground fades to black instead of into a bright orange haze) and the sky is black. Real Venera images show a bright, uniform orange overcast. **Open:** below the deck, draw the sky as the shell's underside at the diffuse sky radiance (E_top · diffuse · μ₀ / π, `lighting::haze_above`) without Bevy's atmosphere, and fog the terrain towards that radiance with the gas's extinction (a conservative medium's source function is its mean intensity).

### Lighting checks (SKY, 2026-10-26)

With the tour's event views: Io and Europa at their eclipse ingress (2030) show Jupiter's shadow across half the disc with a soft penumbral edge, as `sim::light::eclipse_factor` gives (correct). Saturn's rings shadow the planet and the planet shadows the rings, from the south (2030) and from the north (`SUNSCATTER_TOUR_DATE=2047-03-01`). **Fixed (SKY3):** Io's shadow transit on Jupiter (the `transit` view, Io's shadow axis 0.33 radii from the centre in January 2030) showed no shadow: each body took one occluder, the body nearest the Sun's direction seen from its centre, and from Jupiter that was an inner planet (Mercury lies within 4.3° of the Sun there, Io's shadow axis 3.2° off). Now every body takes up to `MAX_OCCLUDERS` (4) bodies whose penumbra cone reaches it (`lighting::shadow_depth`: the cone tangent to the star's and the occluder's discs), ranked by the share of the star's disc they can hide there (Mercury ~10⁻⁵, Io 1), ties by index, none under 0.1 %; the shader sums their hidden areas as `sim::light::eclipse_factor` does, with the separation from atan2 (acos lost a third of the Sun's radius in f32). Io's shadow is now a sharp black disc on Jupiter's clouds. Tests: a table (transit, Mercury in line, the penumbra grazing or missing the limb, Io beside or beyond Jupiter), the four deepest kept in a fixed order, and every surface point the eclipse rule dims belongs to a selected occluder. Planetshine from Jupiter on its moons' night sides is not yet checked.

## Giants (SKY)

`asset-tool giants [jupiter saturn uranus neptune]` (`asset-tool::giants`), inputs in `data/external/giants/`:

| Planet | Source | Map |
|---|---|---|
| Jupiter | Hubble OPAL 2025a, F631N, F502N, F467M as R, G, B (3600 × 1800, 0.1°) | the map in full; uncovered pixels and the 15° next to the coverage edges (the limb) from the latitude profile |
| Saturn | OPAL 2025a, F631N, F502N, F467M (1800 × 900) | zonal: the median of each latitude row (the rings, edge-on in 2025, removed as spikes > 15 % off their ±3° neighbourhood) |
| Uranus | OPAL 2025a, F657N, F547M, F467M (721 × 361) | zonal; the south (unlit in 2025, below ~5° S after trimming) is the last covered row extended |
| Neptune | OPAL 2025b, F657N, F547M, F467M (721 × 361) | zonal; the north beyond ~55° N extended likewise |

OPAL: Outer Planet Atmospheres Legacy, PI A. Simon (GO 13937), NASA/ESA Hubble, archived by STScI, DOI [10.17909/T9G593](http://dx.doi.org/10.17909/T9G593) (`https://archive.stsci.edu/hlsps/opal/`). The FITS maps are calibrated (unlike display-stretched mosaics such as Cassini's PIA07782, which was tried first: its contrast had to be compressed to γ = 0.2 to fit the measured albedo). Zonal maps are 64 × 2048 (the renderer's width cap keeps every latitude row); Jupiter's 3600 × 1800. Sizes: Jupiter 0.5 MB, the others 4–5 kB each.

**Calibration.** Each map's area mean per channel is the Lambert albedo 1.5 p of the geometric albedo p in Sloan r′, Johnson V and Sloan g′ (Mallama, Krobusek & Pavlov 2017, Icarus 282, 19, table 7): Jupiter 0.575, 0.538, 0.509; Saturn 0.618, 0.499, 0.436; Uranus 0.355, 0.488, 0.559; Neptune 0.307, 0.442, 0.533. Where the brightest 0.5 % would exceed 0.98, the contrast is reduced by a power law (each channel relative to its mean, raised to γ) until they fit: Jupiter γ = 0.76, Saturn 0.59, Uranus and Neptune 1. The longitudes are the map's own (column 0 at −180° in the grid convention); cloud features drift against any rotation system.

**First tour (2026-10-26):** with D087's exposure the giants' day sides read like Earth's. Jupiter: pale cream with pink-tan belts and the Great Red Spot, paler and less saturated than the familiar enhanced images (it is calibrated). Saturn: a pale peach disc with faint bands. Uranus: pale cyan with a whiter north polar region. Neptune: pale blue, close to Uranus (as Irwin et al. 2024, MNRAS 527, 11521, found for the true colours). From low orbit the cloud tops were smooth (no detail below the map's texel).

**Cloud detail (D073).** `visual.ron` `clouds`: the colour map is looked up through the zones' domain warp (fBm of the body-fixed position, octaves fading by footprint), here stretched east-west like zonal flow, plus a fine tone, so band edges stay crisp and turbulent and the clouds keep gaining detail closer in (`terrain.wgsl`, the colour-map path). The parameters are representative, not measured: Jupiter first wavelength 1,000 km, amplitude 0.35 of it, stretch 4, tone ±15 %; Saturn 1,500 km, 0.2, 6, ±8 %; Uranus and Neptune 1,000 km, 0.15, 6, ±3 %. From 3 radii Jupiter's belts have wavy, turbulent edges; from low orbit its cloud tops show a faint mottling and Saturn's are still nearly uniform (their zones are bright and the contrast is low).

### Rings (SKY; *playable*)

`asset-tool rings` bakes `data/bodies/saturn/rings.png` (12 kB): the normal optical depth from the Voyager 2 photopolarimeter's occultation of δ Scorpii, 10 km bins from 72,000 to 142,500 km (PDS Rings Node, VG_2801 `EASYDATA/KM010/PS1P01.TAB`, public domain; Esposito et al. 1983, JGR 88, 8643; τ × 10⁴ in 16 bits; the PPS saturates in the B ring's core, τ > ~2.5, where the ring is opaque anyway). `visual.ron` `rings`: the profile, its radii, and the particles' albedo by region (edges from NASA's Saturnian Rings Fact Sheet; values representative of Cuzzi et al. 2009, *Saturn from Cassini-Huygens* ch. 15: A and B ring particles ~0.4–0.6 and red, the C ring and Cassini Division darker and greyer).

`game::rings`: an annulus in the equatorial plane (alpha-blended, both faces). Each fragment is a slab of optical depth τ: coverage 1 − exp(−τ/μ_v); brightness from the single-scattering slab law with isotropic particles (lit face ϖ/4 · μ_s/(μ_s+μ_v) · (1 − e^{−τ(1/μ_s+1/μ_v)}), unlit face ϖ/4 · μ_s/(μ_s−μ_v) · (e^{−τ/μ_s} − e^{−τ/μ_v})), lit by the Sun's illuminance at Saturn; the planet's shadow falls on them (a sphere test). The planet receives the rings' shadow: `terrain.wgsl` dims sunlight by exp(−τ/|μ|) where the ray to the Sun crosses the ring plane (`ring_transmission`, mirrored in Rust and tested). The tour frames a ringed body's disc at 3 ring radii.

**Not a bug: the rings seen from the south.** The tour's first shot looked as if the rings were hidden wherever they passed in front of the planet. In 2030 the Sun lights the rings' south face (Saturn's equinox was in 2025; the next is in 2039), the disc view stands near the Sun's direction, so south of the ring plane, and keeps north up: from the south with north up the rings' near arc is the upper one, crossing the northern hemisphere (as in Hubble's 2003–2004 images), and the lower arc, behind the planet, is correctly hidden. The planet's sizes and the ellipse's offset match the perspective of a camera 3 ring radii out at 18° south. `SUNSCATTER_TOUR_DATE=2047-03-01` shows the north face, near arc below. Two real faults were found on the way and fixed: the tour's shot frame rendered from the camera rig's own position (the shot is taken after the tour moves to the next view; the last view's framing is now kept for that frame), and the rings and their shadow on the planet aliased into speckles where the occultation's τ is noisy (10 km bins against ~360 km pixels): the profile now has mip levels averaging the albedo and the transmission e^−τ (`rings::mip_chain`), sampled at the pixel's radial footprint, on the rings and in the terrain shader's ring shadow.

## Any star, any body: the audit (SKY3, 2026-10-26)

The owner: "an engine that will be applicable with any planet and any number of light sources without any bugs." Every place that assumed one star (the Sun), one occluder or reflector, or a named body, at commit 65e9797 (file:line there). Severity: **H** wrong with two stars or in another system, **M** approximate, **L** tooling. Status: *fixed* (with the commit's tests), *open* (with the reason).

| Where (65e9797) | Assumption | Sev. | Status |
|---|---|---|---|
| scene.rs:71-73, :135 | `BodyDefs.light_source`: one star; with two emissive bodies the last wins | H | open |
| scene.rs:98, :173 | one `SunLight` (`single_mut`), white, Bevy's default sun disc (Earth's) | H | open |
| lighting.rs:133-150 `body_light` | one star; one occluder (the body nearest the Sun's direction: Mercury beat Io seen from Jupiter); one planetshine source | H | occluders fixed (`occluders`, `shadow_depth`); stars, shine open |
| lighting.rs:154-165 `object_light` | one star; the brightest planetshine instead of the sum | M | open |
| lighting.rs:195-206 exposure | one star's flux, referenced to *that* star's flux at 1 AU (a dim star's habitable zone exposed like Earth's) | M | open |
| lighting.rs:318-354 `update_terrain` | one star feeds `star`, `occluder`, `shine`; the shine tint ignores the star's colour; the haze diffuse uses one star | H | open |
| terrain.wgsl:60-63, :746-776 | one `star`/`occluder`/`shine`; the whole PBR output (every light, the sky light) scaled by one star's eclipse, rings and elevation; the haze by one star's air mass | H | open |
| terrain.wgsl (Bevy's light direction) | the light's direction is camera→star, not fragment→star: a body far from the camera (Earth seen from Jupiter) gets a slightly wrong phase | M | open |
| terrain.wgsl:112-142 | ring shadow only on the ring's own planet, one star | M | open |
| scene.rs:341-389 (ships) | one star, one eclipse factor on the whole material; occluders and reflectors from `world.surfaces()` (solid bodies only: Jupiter never eclipsed or lit a ship) | H | open |
| rings.rs:226-243, rings.wgsl:58-62 | one star, white; the parent's shadow only (hard, no penumbra, no moons); no planetshine on the unlit face | H/M | open |
| clouds.rs:100-125, clouds.wgsl:20-24 | one star, white, no eclipses, no planetshine (the deck itself is data-driven, `cloud_top`) | H/M | open |
| atmosphere.rs:170-177 | one atmosphere drawn per camera (Bevy) | M | open (engine limit) |
| atmosphere (Bevy) | the sky's light ignores eclipses: the sky stays bright in a total solar eclipse | M | open |
| sky/mod.rs:43-96, stars.rs:185-202 | sky ambient, `sun_visibility` and `daylight_of` for one star; occluders from `surfaces()` only | M | open |
| sky/flare.rs:47-73 | one star's flare, warm colour hard-coded | L | open |
| tour.rs:281-300, :364, :405 | event search with one star and the primary as the only occluder | L | open |
| demo.rs, state.rs, tracking.rs, trajectory, map.rs | "Earth", "Moon", "Sun" by name (pad, test ships, plot frames) | L/M | open (gameplay setup, not rendering) |
| sim light.rs:63-88 `sunlight` | the stars' beams returned as one vector sum: thermal (heating.rs:218-229, live.rs:429) and power (systems/power.rs:69-75) treat it as one beam; two stars on opposite sides cancel | H | open |
| sim light.rs:146-170, coast_heat.rs:216-237 | orbit-averaged sunlight: the orbited body the only occluder, stars summed into one vector | M/H | open |
| sim world.rs:128-130 | `World.star` = the first luminous source (used for distance shortcuts only) | L | open |
| visual.ron `emissive` | no star temperature: a light's colour cannot follow the star | M | open |

## Solid bodies (LANDS)

**Status:** *playable* data for 35 bodies (maps baked, physical terrain loaded by `sim`, zones drawn by the generic shader), first seen in the tour on 2026-10-26 (below). Bake: `cargo run -p asset-tool --release -- planets [body…]` (`asset-tool::planets`, the per-body table in `asset-tool::specs`; sources are downloaded into `data/external/`). Total committed: **78.9 MB** (D086's ~90 MB for these bodies; every file under 19 MB).

### Method

- **Heights** (`height.png`, 16-bit, 1 m steps; `body.ron` `height_scale` where the relief exceeds ±32.7 km: Haumea 4 m, Quaoar and Hyperion 2 m). A DEM is area-averaged in its own datum (no-data excluded; unmapped areas filled smoothly by push-pull), then converted to radii and to heights above the body's `body.ron` spheroid (geocentric latitude, the formula of `BodyPhysical::ellipsoid_radius`), so landing, collision and drawing use the measured radius (D047). Heights above the spheroid differ from the sources' datums: Olympus Mons is 22.9 km above Mars's IAU spheroid (21.2 km above MOLA's areoid), the Hellas floor −7.6 km (−8.2 km).
- **Procedural relief** where no global DEM exists (stated in each `body.ron`): the IAU triaxial shape (long axis at longitude 0), the IAU Gazetteer's named craters at their real centres and diameters, a random population `N(>D) = n1 D⁻²` per km² down to three map pixels (largest first; each crater overprints the older surface inside its rim, measured from its mean there), simple bowls of depth 0.2 D, complex craters (flat floor, central peak, depth ∝ D^0.3 above the transition diameter) capped at a relaxed depth, r⁻³ ejecta; crater relief soft-limited to 1.5 depth caps (the ejecta's infill of saturated surfaces is not modelled), plus regional fBm. Densities: near empirical saturation (`n1` 0.1–0.3) on Callisto, Rhea, Tethys, Mimas, Iapetus, Oberon, Umbriel, Phoebe, Hyperion, Proteus; 0.03 Ganymede, Miranda, Titania; 0.01 Ariel and the TNOs (assumed); 2–3·10⁻⁴ Europa and Triton (young surfaces); none on Io. Sources for densities, transition diameters and relaxed depths: Melosh 1989 (*Impact Cratering*, ch. 6, 7, 10), Pike 1977, Schenk 2002 (Nature 417, 419), Zahnle et al. 2003 (Icarus 163, 263), Kirchoff & Schenk 2010 (Icarus 206, 485), Thomas et al. 2007 (Nature 448, 50, Hyperion).
- **Sub-sample detail** (D059): `roughness.png` baked from each heightmap, `terrain_detail` octaves from the map spacing down to ~2 m. Cratered bodies add crater-shaped detail (`sim::terrain::craters`, `terrain_detail.craters` in `body.ron`): per octave, from the map spacing down to ~100 m, cubic cells each holding a bowl with probability `density` (0.4–0.5 on saturated surfaces, 0.2 Mars, 0.02–0.03 Europa and Triton, none on Io), radius 0.1–1/3 of the cell, `depth_ratio` 0.12–0.2 of the diameter, a rim 0.2 of the depth, falling to zero at 1.5 radii; only integer hashes, floor and sqrt, so it is bit-identical everywhere (golden hashes per body). The densities are assumptions scaled from each body's crater statistics above. Cost (release, M2 Pro): a surface sample takes 2.3 µs instead of 1.6 µs on Mercury, 2.6 instead of 1.6 on Callisto (7 octaves).
- **Zones** (D073): k-means centres of each colour map in CIELAB (or chosen classes), classified by the existing `asset-tool zones`; every zone's `albedo` is a linear reflectance at ~645/555/470 nm, cited in its `visual.ron`. Convention: the area mean is the body's measured mean albedo (Bond where published, else geometric; Ceres and Mercury normal albedo, like the Moon's zones), the zone's brightness ratio in the (display-stretched) mosaic compressed by a power 0.5–0.7, the colour from measured B−V and V−R (converted with the Sun's 0.653 and 0.356) or from the map where it is true colour (Ganymede); Mars, Io, Iapetus, Ceres, Pluto and Triton have per-zone values from the literature. SKY's giants use the Lambert albedo 1.5 p instead (open: one convention for both).
- **Bodies without any global map** (Uranus's moons, Proteus, Nereid, the TNOs and their moons): the colour map is the measured colour, lighter (×1.6, assumed) around a small fraction of fresh craters; one or two zones (terrain, fresh ejecta).

### Bodies

| Body | Heights (px, spacing) | Colour / zones (px) | MB | Sources |
|---|---|---|---|---|
| Mars | 5760 (3.7 km) | 4096 / 4096, 6 zones incl. polar ice (≥ 55°) | 20.9 | MOLA MEGDR radius 32 ppd (Smith et al. 2001), [PDS Geosciences](https://pds-geosciences.wustl.edu/missions/mgs/megdr.html); Viking colour mosaic 925 m, USGS |
| Mercury | 4096 (3.7 km) | 4096 / 4096, 5 | 15.9 | MESSENGER MDIS DEM v2 665 m (Becker et al. 2016); MDIS MD3 colour basemap 665 m (enhanced; classification only) |
| Ceres | 2048 (1.5 km) | 2048 / 2048, 4 incl. faculae | 3.9 | Dawn FC HAMO DTM 60 ppd (Preusker et al. 2016, DLR); Dawn FC mosaic 20 ppd |
| Pluto | 4096 (1.8 km) | 2048 / 2048, 5 | 6.7 | New Horizons DEM and mosaic 300 m (Schenk et al. 2018); 54 % unmapped, filled |
| Charon | 2048 (1.9 km) | 2048 / 2048, 4 | 2.0 | as Pluto; 54 % unmapped |
| Enceladus | 720 (2.2 km) | 2048 / 2048, 3 | 1.5 | Cassini ISS shape model (Bland et al. 2020); Cassini mosaic 110 m |
| Ganymede, Callisto | 2048 (8.1, 7.4 km), procedural | 2048 / 2048, 4 | 2.8, 3.4 | Voyager–Galileo SSI mosaics 1–1.4 km |
| Io, Europa | 1024 (11, 10 km), procedural | 2048 / 2048, 7 and 4 | 1.2 each | Galileo SSI–Voyager mosaics (Io colour merge 1 km, Europa 500 m) |
| Mimas, Tethys, Dione, Rhea, Iapetus | 1024 (1.2–4.7 km), procedural | 2048 / 1024–2048, 3–4 | 1.4–2.0 | Cassini (DLR basemap for Mimas; USGS Cassini–Voyager mosaics 154–783 m) |
| Triton | 1024 (8.3 km), procedural | 2048 / 2048, 4 | 0.8 | Voyager 2 colour mosaic with global fill 600 m (LPI/USGS) |
| Hyperion, Phoebe, Proteus, Nereid, Miranda, Ariel, Umbriel, Titania, Oberon | 512–1024, procedural | procedural | 0.2–0.8 | albedos: Thomas et al. 2007, Simonelli et al. 1999, Karkoschka 2001/2003 |
| Eris, Haumea, Makemake, Gonggong, Quaoar, Sedna, Orcus, Dysnomia, Hi'iaka, Vanth | 512–1024, procedural | procedural | 0.1–0.5 | albedos and colours: Sicardy 2011, Ortiz 2012/2017, Kiss 2019, Braga-Ribas 2013, Pál 2012, Fornasier 2013, Brown & Butler 2018, MBOSS (Hainaut et al. 2012) |

All maps are USGS Astrogeology / NASA PDS products (public domain): `https://asc-pds-services.s3.us-west-2.amazonaws.com/mosaic/<file>` (file names in `asset-tool::specs`); named craters from the IAU Gazetteer of Planetary Nomenclature GIS downloads (`https://asc-planetarynames-data.s3.us-west-2.amazonaws.com/<TARGET>_nomenclature_center_pts.zip`, public domain).

### Checks and tests

`sim/tests/solar_terrain.rs`: landmarks (Olympus Mons, Hellas, Rachmaninoff, Caloris), every heightmap loads with its detail, rails floors follow D062, per-body golden hashes of the sampled surface (2,000 directions, base + detail: macOS and Windows must agree). `asset-tool`: datum conversion, GeoTIFF georeferencing (degrees or metres, central meridian, extra columns), masked resampling and gap filling, crater profiles and population statistics, the dBASE reader, and every shipped body's maps (equirectangular, zone indices within their lists, land albedos in (0, 1)).

### Gaps

- No global DEM exists for the Galileans, Saturn's mid-size moons (Schenk's stereo/photoclinometry products are not in the USGS archive), Triton or Uranus's moons: their relief is procedural, and only the named craters line up with the photographs. Io's 26 named mountains, 9 mensae, 8 plana and 170 paterae are placed from the Gazetteer with typical heights (mountains 0.04 × diameter, 2–17.5 km: Schenk et al. 2001; paterae ~1 km deep: Radebaugh et al. 2001). Not modelled: Iapetus's 20 km equatorial ridge, Io's unnamed mountains, Europa's ridges, Miranda's coronae and cliffs, Tethys's Ithaca Chasma.
- Sub-pixel craters are a statistical field, not the real small craters; the Moon (not LANDS') does not use them yet.
- Haumea's shape is the JPL triaxial 1050 × 840 × 537 km (Ortiz et al. 2017 measured 1161 × 852 × 513).
- Loading: the game loads every body's colour and zone maps at start (about 35 more bodies), and `World::sol` decodes every heightmap (~0.9 s in tests); loading by distance would bound GPU memory.
