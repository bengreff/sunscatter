# The Solar System in Earth's visual style

**Status:** in progress, 2026-10-26 (owner away; plan: [solar-visuals-2026-10-26.md](../plans/solar-visuals-2026-10-26.md)). Decisions: D073 (crisp, simple surfaces), D084, D086 (map budget), D087 (exposure), D052/D055 (atmospheres, lighting). Two agents, each editing its own sections: LANDS (solid bodies' maps and terrain) and SKY (tour, exposure, atmospheres, giants, lighting). Every source is cited with its licence, resolution and URL.

## Seeing a body: the tour (SKY)

`SUNSCATTER_TOUR=<body,body,…> SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 cargo run -p game --release` (`game::tour`): per body, in the Minimal tier with the clock frozen and no interface, the whole disc from 3 radii at a 30° phase angle (`disc`), the limb from low orbit (1.5 % of the radius up, or 5 % above the atmosphere's top; `limb`), and the ground from 2 km above the terrain with the Sun 10° up (`ground`, solid bodies only). The low views look along the terminator (light from the side) with the horizon 8° above the frame's centre. Each shot waits for the terrain and its maps (`Terrain::busy`, 1.5–45 s).

## Exposure follows the starlight (SKY, D087)

`lighting::exposure_ev100`: EV100 = 14.5 + log₂(E_cam / E_1AU), where E_cam is the Sun's flux at the camera (inverse square, no eclipses: the light where the photographer is, never the scene's luminance), clamped to −14…+4 stops; a setting (`starlight_exposure`, Graphics, not part of the tiers) turns it off. Numbers: at Earth (0.983 AU in January) +0.05 stops; Jupiter (5.2 AU) −4.76 (EV 9.74); Saturn (9.5 AU) −6.50; Neptune (30 AU) −9.81; Mercury (0.387 AU) +2.74. A day side then reads like Earth's on screen at any distance, and a night side stays as many stops below its day side as at Earth (D055). Stars are drawn in display units, so they do not change.

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

**First tour (2026-10-26):** with D087's exposure the giants' day sides read like Earth's. Jupiter: pale cream with pink-tan belts and the Great Red Spot, paler and less saturated than the familiar enhanced images (it is calibrated). Saturn: a pale peach disc with faint bands. Uranus: pale cyan with a whiter north polar region. Neptune: pale blue, close to Uranus (as Irwin et al. 2024, MNRAS 527, 11521, found for the true colours). From low orbit the cloud tops are smooth: no detail below the map's texel yet.
