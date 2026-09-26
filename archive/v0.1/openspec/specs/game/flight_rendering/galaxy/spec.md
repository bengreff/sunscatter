# Procedural Star Field

Procedural stars generated per-sector, cached with LRU eviction, and rendered as colored dots in galaxy view.

## Star Generation

### Requirement: Procedural stars generated per-sector

Stars are generated deterministically from a galaxy seed + sector coordinates. Each star has a position (t=0), spectral properties (temperature, luminosity, mass), and pre-computed render color. Stars are stored in a sector cache with LRU eviction (max 200 sectors, 300-frame age).

### Requirement: Evolved stellar types

After the main-sequence spectral type roll, a secondary "evolution" roll overrides ~8% of stars to evolved types:

| Roll Range | Type | Fraction | Mass (M☉) | Temperature (K) | Luminosity (L☉) |
|-----------|------|----------|-----------|-----------------|------------------|
| < 6.0 | White Dwarf | ~6% | 0.5–0.8 | 4,000–40,000 | 0.0001–0.01 |
| < 8.0 | Red Giant | ~2% | 0.8–8.0 | 3,000–5,000 | 50–2,000 |
| < 8.01 | Supergiant | ~0.01% | 10–70 | 3,500–30,000 | 10,000–500,000 |
| < 8.11 | Neutron Star | ~0.1% | 1.4–2.1 | 100,000–1,000,000 | 0.00001–0.0001 |
| ≥ 8.11 | Main Sequence | ~91.89% | (from spectral roll) | (from spectral roll) | M^3.5 |

For evolved types, mass, temperature, and luminosity are independently re-rolled within their ranges (overriding the main-sequence values). The existing `stellar_color()` function clamps temperature to [2000K, 40000K], so neutron stars render as deep blue. The log-scale luminosity renderer produces:
- Red giants: bright visible dots (L=50–2000)
- Supergiants: largest, brightest dots (L=10k–500k)
- White dwarfs: dim blue-white scatter (L=0.0001–0.01)
- Neutron stars: effectively invisible (L<0.0001)

### Requirement: Elliptical galactic orbital elements

Each ProceduralStar stores orbital elements for an elliptical orbit around the galactic center:
- `semi_major_axis` — orbital semi-major axis (meters), derived so radial position at t=0 matches the star's generated distance from galactic center
- `mean_motion` — angular rate n = sqrt(G * M_enclosed(a) / a^3) (rad/s)
- `mean_anomaly_0` — mean anomaly at t=0 (radians), randomly assigned
- `eccentricity` — orbital eccentricity (f32, range 0..0.6), generated from Rayleigh distribution with radius-dependent σ
- `arg_periapsis` — argument of periapsis (f32, radians), derived so angular position at t=0 matches the star's generated galactic angle

### Requirement: Eccentricity generation

Eccentricity is drawn from a Rayleigh distribution with σ varying by galactic radius:
- `σ(r) = 0.05 + 0.20·exp(−r/2kpc) + 0.03·(r/20kpc)` (1 kpc = 3.086e19 m)
- At r=0 (bulge): σ≈0.25, mean e≈0.31 (high radial oscillation)
- At r=8kpc (Sun): σ≈0.07, mean e≈0.09 (realistic for solar neighborhood)
- At r=30kpc (outer disk): σ≈0.10, mean e≈0.12
- `e = σ · sqrt(−2·ln(max(u, 1e-10)))`, capped at 0.95

### Requirement: Consistent element derivation

Orbital elements are derived so that the star's position at t=0 exactly matches its generated (px, py):
1. Random `mean_anomaly_0` ∈ [0, 2π)
2. Solve Kepler's equation for E₀
3. True anomaly: `ν₀ = 2·atan2(sqrt(1+e)·sin(E₀/2), sqrt(1-e)·cos(E₀/2))`
4. `arg_periapsis = theta_0 − ν₀` (preserves angular position)
5. `semi_major_axis = galactic_r / (1 − e·cos(E₀))` (preserves radial distance)
6. `mean_motion = sqrt(G · M_enclosed(a) / a³)`

## Star Propagation

### Requirement: Elliptical Keplerian position at game time

Each star's current position is computed by solving Kepler's equation:
1. `M(t) = mean_anomaly_0 + mean_motion * game_time`
2. Solve M = E − e·sin(E) via Newton-Raphson (~10 iterations, tolerance 1e-10)
3. True anomaly: `ν = 2·atan2(sqrt(1+e)·sin(E/2), sqrt(1-e)·cos(E/2))`
4. Radial distance: `r = a·(1 − e·cos(E))`
5. Position: `(r·cos(ν + ω_peri), r·sin(ν + ω_peri))`

Stars co-rotate with the Sun at similar galactic radii (differential rotation at different radii). Eccentricity adds realistic radial oscillation — stars drift inward and outward over their orbital period.

### Requirement: Backward rotation for sector lookup

Stars are cached at t=0 positions in sectors. To find stars currently near the camera:
1. Compute camera's galactic radius and angular velocity: `omega_cam = sqrt(G * M_enclosed(r_cam) / r_cam) / r_cam`
2. Rotate camera backward: `theta_0_cam = theta_cam - omega_cam * game_time`
3. Query sectors around the backward-rotated position `(r_cam * cos(theta_0_cam), r_cam * sin(theta_0_cam))`
4. Sector margin = `max(1 sector, 20% of render_radius)` to handle differential rotation and radial drift from eccentricity

#### Scenario: Zero game time
- **WHEN** `game_time == 0`
- **THEN** no backward rotation is applied; lookup center equals camera center; star positions match their generated pos exactly

#### Scenario: Co-rotating stars
- **WHEN** stars are at similar galactic radii to the camera
- **THEN** they have similar mean motion values and appear nearly stationary relative to the camera

#### Scenario: Differential rotation
- **WHEN** stars are at different galactic radii
- **THEN** they rotate at different angular velocities, producing galactic shear

#### Scenario: Radial oscillation
- **WHEN** time is advanced significantly (high warp)
- **THEN** stars with nonzero eccentricity show slight radial drift from their initial circular-like positions

## Kepler Helpers

### Requirement: solve_kepler_nr(M, e) -> E

Newton-Raphson solver for Kepler's equation M = E − e·sin(E). Reduces the input mean anomaly via `m.rem_euclid(TAU)` at entry so trig calls retain precision when callers pass accumulated angles (`M₀ + n·t`) at high warp. Initial guess E = m, up to 10 iterations, tolerance 1e-10. All procedural stars have e < 0.6, so convergence is fast.

### Requirement: kepler_position(a, e, arg_peri, mean_anomaly) -> [x, y]

Full pipeline: reduce `mean_anomaly` via `rem_euclid(TAU)` → solve Kepler → true anomaly → r = a(1 − e·cos(E)) → angle = ν + arg_peri → (x, y). Used by both generation (element derivation) and propagation.

The modulo reduction is mandatory: at high time warp (1e12×) and short orbital periods (binary stars, close-in planets), callers pass `M = M₀ + n·game_time` which grows to 1e10+ radians. `sin`/`cos` lose ~log₂(M/2π) bits of precision on large arguments, causing bodies to drift off their statically-rendered orbit lines. Must match `bodies.rs::Orbit::solve_kepler` semantics.

## Star Classification

### Requirement: StarType enum

Each ProceduralStar stores a `StarType` enum classifying its evolutionary state:
- `MainSequence(char)` — spectral class O/B/A/F/G/K/M, determined from the spectral roll
- `WhiteDwarf` — from evolution roll < 6.0
- `RedGiant` — from evolution roll 6.0–8.0
- `Supergiant` — from evolution roll 8.0–8.01
- `NeutronStar` — from evolution roll 8.01–8.11

`display_name()` returns human-readable strings like "G-type Main Sequence", "White Dwarf", etc.

### Requirement: Physical radius calculation

Star physical radius is computed from luminosity and temperature via Stefan-Boltzmann law:
`R = R_sun * sqrt(L_solar) / (T / T_sun)^2`

This is used for real circle rendering when zoomed in close enough that the star subtends > 1 pixel.

## Rendering

### Requirement: Star visibility threshold

Stars become visible once the screen diagonal covers at least 0.1 light-years (~100 AU) and disappear at galaxy view scale. The galaxy view threshold scales with distance from the galactic center: 144 ly screen span near Sgr A* (dense region), linearly increasing to 640 ly at the Sun's distance (26,000 ly) and beyond. Stars are culled to rectangular screen bounds (no off-screen stars loaded). Sector lookup uses the screen half-diagonal plus a rotation margin, capped at 1,000 ly.

### Requirement: Sector star count cap

Each sector is capped at 2,000 stars (`MAX_STARS_PER_SECTOR`). This bounds galactic center sectors (which would otherwise have ~29,000 stars) to a manageable count while preserving visual density.

### Requirement: Distance-ordered sector iteration

Sectors are sorted by distance from the camera (closest first) before iterating. All on-screen stars from all searched sectors are collected, then if the total exceeds the cap, the closest stars by actual distance from the camera are kept via partial sort.

### Requirement: Star count cap

Total rendered stars are capped at 50,000 (`MAX_STARS`). When the cap is exceeded, stars are trimmed by distance from the camera center (closest kept), producing a smooth circular boundary rather than cutting off at sector edges.

### Requirement: Adaptive circle rendering

When a star's physical radius is large enough on screen (> 1 pixel), it is rendered as a real filled circle with adaptive segment count (`clamp(circumference/3, 16, 256)`) instead of the standard hexagon dot. This provides smooth visual scaling as the camera zooms in on individual stars.

### Requirement: Screen position storage for hit testing

During star rendering, each on-screen star's screen pixel coordinates are stored in `procedural_star_screen_positions`. These positions account for camera rotation and are used for hover detection and click handling.

## Interaction

### Requirement: Star hover detection

`update_star_hover()` in `interaction.rs` checks stored screen positions to find the closest star within a 20px screen radius. `update_hover()` uses a three-tier priority so barycenters of multi-star catalog systems display their label correctly:
1. **Tight body hit** (`dist ≤ body.radius`) — wins over everything
2. **Procedural star hit** (20px screen radius) — beats body indicator rings
3. **Body indicator_radius hit** (the soft expanded hit area around small bodies) — fallback

Without this priority, the indicator rings of companion stars in a multi-star system would steal hover from the barycenter dot, hiding the "{name} System" label. Double-click handlers use a different priority order: `body_tight → body_loose → star`. This ensures that clicking on a companion star body (which has a 16px indicator radius matching body_loose) focuses on that companion rather than the ProceduralStar dot (which would re-center on star A). The loose-before-star ordering applies in both flight mode and tracking station.

### Requirement: Star hover labels

When a star is hovered, its catalog name is displayed above the star in both flight mode and tracking station. Labels are rendered in a light blue color (200, 200, 255) to distinguish from body labels. Hover is cleared when the cursor moves onto an egui panel or leaves the window.

### Requirement: Star double-click focus

Double-clicking a procedural star in flight or tracking station:
1. Centers the camera on the star's world position (meters * SCALE)
2. Sets `focused_star` index and `focused_star_name` for tracking
3. Clears body/vessel tracking
4. The camera follows the star each frame via `update_tracking()` using `focused_star_world_pos`

The focused star is re-resolved by name each frame when the star list is rebuilt. If the star goes out of the visible set, the index is cleared (preventing stale data) but the world position is preserved for continued camera tracking.

### Requirement: Unified info panel (tracking station)

Both solar system bodies and procedural stars use the same `BodyInfoData` struct and the same right-side info panel in the tracking station. The panel is shown when a body is tracked OR a procedural star is focused (body tracking takes priority).

Panel sections:
- **Name** — light blue (200, 200, 255) for stars, white for non-stars
- **Star type** (stars only) — star type string (italic, light blue `(160, 160, 200)`)
- **Description** — body/system description (italic, gray `(160, 160, 160)`), if non-empty. Stars show both star type and description.
- **Physical Properties** — Radius (R☉ + metric for stars, metric only for non-stars), surface gravity, mass (M☉ for stars, metric for non-stars)
- **Stellar Properties** (stars only) — luminosity (L☉), temperature (K), SOI
- **Atmosphere** (non-stars only) — pressure + height, or "No atmosphere"
- **Orbit** — labeled "Galactic Orbit" with pc/kpc formatting when `is_galactic_orbit` is true; otherwise standard metric formatting
- **SOI** (non-star bodies only)
- **Colony Prospects** (non-star bodies with resources/habitability)

### Requirement: BodyInfoData extended fields

BodyInfoData includes:
- `luminosity_solar: Option<f64>` — solar luminosities (stars only)
- `star_type: Option<String>` — e.g. "G-type Main Sequence" (stars only)
- `temperature_k: Option<f64>` — surface temperature in Kelvin (stars only)
- `soi_radius_m: Option<f64>` — SOI radius in meters (all bodies with finite SOI)
- `is_galactic_orbit: bool` — true for bodies/stars orbiting Sgr A* (orbit section uses pc/kpc)

For solar system stars: L ∝ M^3.5 luminosity relation, temperature from Stefan-Boltzmann law. Special cases: Sgr A* (supermassive black hole) has no luminosity/temperature; the Sun is hardcoded at 1.0 L☉, 5778 K.

### Requirement: Procedural star SOI

Procedural stars have SOIs computed as `calculate_soi(sma, mass_kg, galactic_enclosed_mass(sma)) / 20.0`, using the same 1/20 scaling factor as the Sun's SOI. The SOI is displayed in the stellar properties section of the info panel.

## Star Naming

### Requirement: Catalog naming scheme

Procedural stars are named `{PREFIX}-{sector_x:04}-{sector_y:04}-{sector_index:04}`, where PREFIX is the star type catalog prefix (e.g. "G" for G-type main sequence, "WD" for white dwarf, "RG" for red giant, "SG" for supergiant, "NS" for neutron star). Catalog stars (catalog_index > 0) display their real name instead (e.g. "Alpha Centauri", "Sirius"). Multi-star catalog systems append " System" to the name (e.g. "Alpha Centauri System", "Sirius System"); single-star catalog systems use the plain name. `num_catalog_stars` field on `StarRenderData` drives this suffix (> 1 = multi-star). Names are stable across frames for the same star and are used for cross-frame identity tracking of focused stars.

## Catalog Stars

### Requirement: Named star catalog

67 real star systems from `docs/nearby_stars.md` are injected into the procedural galaxy as predefined entries. Each catalog star is a `ProceduralStar` with `catalog_index` set to a 1-based index (1–67). Procedural stars have `catalog_index = 0`.

Catalog systems span 5 zones:
- Zone 1 (0–12 ly): 18 systems — Alpha Centauri, Barnard's Star, Wolf 359, etc.
- Zone 2 (12–25 ly): 19 systems — YZ Ceti, Luyten's Star, Altair, etc.
- Zone 3 (25–50 ly): 12 systems — Vega, Fomalhaut, Pollux, etc.
- Zone 4 (50–100 ly): 8 systems — Arcturus, Canopus, Achernar, etc.
- Zone 5 (Galactic Center): 10 systems — orbiting Sgr A* at AU scale

### Requirement: Catalog data structures

Each catalog system contains:
- `CatalogSystem` — name, description (system-level from `nearby_stars.md`), zone, distance from Sol, galactic position (ly), orbital elements, list of stars, bodies, and binary orbits
- `CatalogStar` — name, per-star description (empty for single-star systems where the system description is used), spectral type, mass/radius/luminosity in solar units (multi-star systems store all components)
- `CatalogBody` — planet/moon with name, designation, orbital parameters, physical properties (mass, radius, gravity, temperature), atmosphere, habitability score, science value, resources, description
- `CatalogAtmosphere` — pressure (atm), composition string, scale height (km)
- `StarOrbitData` — binary/hierarchical stellar pair orbital parameters: star_a/star_b indices, semi-major axis (AU), eccentricity, period (years)

All data is stored in a static array `CATALOG` in `src/galaxy/catalog/catalog_data.rs` (~4800 lines, 67 systems, ~318 bodies total).

### Requirement: Catalog injection into sector system

At `GalaxyState::new()`, `build_catalog_stars()` converts all 67 catalog systems into `ProceduralStar` instances and groups them by sector in `catalog_by_sector: HashMap<SectorCoord, Vec<ProceduralStar>>`. When `get_sector()` generates a sector, it appends any catalog stars for that sector and removes the nearest procedural star within 2 ly to avoid visual overlap.

### Requirement: Solar neighborhood exclusion zone

Procedural stars are suppressed within 15 light-years of the Sun's galactic position (`SUN_EXCLUSION_RADIUS_LY = 15.0` in `generation.rs`). This prevents randomly generated stars from appearing closer than Alpha Centauri (4.37 ly), which would be unrealistic. The catalog's 67 named systems cover the real solar neighborhood. The Sun's t=0 position is computed from its orbital elements (a=1.996e20 m, e=0.07, M₀=4.8534 rad) using Kepler's equation, matching the same calculation in `catalog.rs`.

Zone 5 systems use `is_sgr_a_orbit = true` flag — their `orbit_sma_ly` field stores AU (not ly), and mean motion uses Sgr A* point mass (8.26e36 kg) instead of enclosed galactic mass.

Multi-star systems (Alpha Centauri, Sirius, etc.) appear as a single dot in the galaxy star field using the primary star's properties; the dot represents the **single system barycenter** position (not the primary star — the primary orbits this barycenter like every other component). When a multi-star system is focused, the procedural star dot is hidden (component star bodies replace it visually), and all component stars (including the primary) are positioned using `binary_orbits` data and injected as synthetic bodies alongside planets. Every component star orbits the same common barycenter on its own individual ellipse; there are no nested/hierarchical barycenters. The dot label shows "{name} System" for multi-star systems (e.g. "Alpha Centauri System").

### Requirement: Multi-star system rendering

19 of the 67 catalog systems are multi-star. Each has a `binary_orbits` array of `StarOrbitData` entries encoding stellar pairs (tightest binary first, wider orbits subsequent).

Multi-star systems use a **flat single-barycenter model**: every component star orbits one common system barycenter at `(star_x, star_y)` — the position stored on `StarRenderData`. There are no nested/hierarchical barycenters; there is exactly one barycenter per system, and every star (including the primary) has its own individual ellipse around that single point.

When a multi-star system is focused, `inject_catalog_planets()`:
1. **Positions each component star around the single system barycenter**: For each star `si` in `sys.stars`, the code finds the first `StarOrbitData` entry in which the star appears (as either `star_a` or `star_b`) — this is the tightest pair containing that star. Using the **raw two-star masses** of that pair (not any accumulated group mass), the star's individual semi-major axis around the system barycenter is `pair.sma_au * m_partner / (m_star + m_partner)`. The star's position at time `t` is `(star_x, star_y) + kepler_position(star_sma, pair.eccentricity, arg_peri, mean_anomaly)`, where `arg_peri = π` when the star is `star_a` of its pair and `0` when it is `star_b`. A deterministic per-pair phase offset from `(catalog_index, pair_idx)` sets the mean anomaly at t=0 so stars in different pairs aren't all at periapsis; two stars that share the same pair share the same phase, keeping them diametrically opposite at all times.
2. **Records per-star orbit info**: Each star's `StarOrbitInfo` stores `(star_sma_m, eccentricity, period_s, arg_peri)`. There are no local offsets — all orbits are relative to the common system barycenter. Wider hierarchical entries in `binary_orbits` (e.g. Alpha Centauri's `(A, Proxima, 8700 AU)`) only contribute to the star that hasn't already been matched by an earlier entry (here, Proxima) — they do not shift or nest the stars matched by tighter pairs.
3. **Injects all component stars (including the primary) as synthetic bodies**: Each star gets a colored dot based on spectral type temperature, with stellar radius (minimum 12 km floor — real neutron star radius — to ensure zero-radius catalog entries still produce a nonzero body that renders as an indicator ring at stellar scales). Each star also gets an orbital ellipse rendered around the system barycenter (color: star color × 0.4, alpha 0.5, 5120 segments). All star orbit lines share the same `parent_x, parent_y = star_x, star_y` — the single system barycenter. There are no group-level orbit lines.
4. **Builds BodyInfoData for each component star**: Star type derived from spectral type via `spectral_to_star_type()`, physical properties from `CatalogStar`, planets filtered to only those orbiting this specific star, the star's own flat-model orbit parameters for the orbit section, `is_galactic_orbit = false`.
5. **Routes planets to correct host star**: Uses `host_star_index(designation, stars)` to determine which star each planet orbits. The function searches the stars array by name — for designation "Bb", it finds the first star whose name has a word starting with 'B' (e.g. "Regulus B" at index 2, skipping "Regulus A companion" at index 1). Parenthetical words like "(YY Gem)" are skipped. Fallback rules: "Proxima ..." → last star, "AB..." → primary, bare lowercase "b" → star 0.

#### Scenario: Binary star orbit computation
- **GIVEN** a `StarOrbitData` with `star_a=0, star_b=1, sma_au=23.7, ecc=0.52, period_years=79.9` and masses `m_a, m_b`
- **THEN** star A's individual SMA around the system barycenter is `23.7 * m_b / (m_a + m_b)` AU with `arg_peri = π`
- **AND** star B's individual SMA around the system barycenter is `23.7 * m_a / (m_a + m_b)` AU with `arg_peri = 0`
- **AND** both stars share the same mean anomaly (pair phase offset + `mean_motion * game_time`), keeping them diametrically opposite at all times
- **AND** both orbit lines are drawn around the same point `(star_x, star_y)`

#### Scenario: Hierarchical triple system flattened (Alpha Centauri)
- **GIVEN** `binary_orbits = [(A, B, 23.7 AU), (A, Proxima, 8700 AU)]` and masses `m_A, m_B, m_P`
- **WHEN** `inject_catalog_planets()` assigns each star its first matching pair
- **THEN** A takes pair 0 (is_a=true): `sma_A = 23.7 * m_B / (m_A + m_B)`, `arg_peri = π`
- **AND** B takes pair 0 (is_a=false): `sma_B = 23.7 * m_A / (m_A + m_B)`, `arg_peri = 0`
- **AND** Proxima takes pair 1 (is_a=false, since A is already matched): `sma_P = 8700 * m_A / (m_A + m_P)`, `arg_peri = 0`
- **AND** all three stars orbit the **single** system barycenter at `(star_x, star_y)` — there is no AB sub-barycenter
- **AND** exactly three orbit ellipses are emitted, one per star, all sharing `parent_x, parent_y = star_x, star_y`
- **AND** no group-level orbit lines are produced

#### Scenario: Planet host star routing
- **GIVEN** a planet with designation "Bb" in a 3-star system
- **THEN** `host_star_index("Bb", 3)` returns 1 (star B)
- **AND** the planet orbits star B's computed position, not the system barycenter

#### Scenario: Single star system (unchanged)
- **GIVEN** a system with `binary_orbits: &[]`
- **THEN** all planets orbit the focused star's position (no companion star positioning)

#### Scenario: Focused multi-star system dot suppression
- **GIVEN** a focused star with `num_catalog_stars > 1`
- **WHEN** `add_procedural_stars_impl` renders the star field
- **THEN** the focused star's procedural dot (hexagon/circle) is suppressed (no vertices generated)
- **AND** the star's screen position is NOT recorded for hit testing (preventing the invisible barycenter dot from capturing clicks and re-centering the camera)
- **AND** companion star bodies from `inject_catalog_planets()` replace the dot visually

#### Scenario: Focused star preserved when outside sector scan radius
- **GIVEN** the user is tracking a companion star body in a multi-star system (e.g. Fomalhaut B at ~0.9 ly from A) via `tracked_body = Some(synthetic_body_idx)` with `focused_star_id` pointing at the primary
- **WHEN** the camera is zoomed close enough to the companion that the sectored scan in `build_procedural_star_data` does not reach the primary's sector (primary is more than `radius + margin` away from the scan's lookup center)
- **THEN** the primary must still be pushed into the output star list via `lookup_focused_star()` (in addition to `sync_focused()` for the 1-frame-lag fix)
- **AND** this ensures `inject_catalog_planets()` can locate the focused star and inject all companion bodies (including the tracked one)
- **AND** `track_catalog_body()` finds the tracked companion's synthetic index in the bodies array and keeps the camera on it
- **AND** the camera does NOT snap back to the system barycenter (primary star) on the next frame

#### Scenario: Hierarchical quadruple system flattened (Regulus)
- **GIVEN** `binary_orbits = [(A, WD, 0.35 AU), (B, C, 100 AU), (A, B, 4200 AU)]`
- **WHEN** `inject_catalog_planets()` assigns each star its first matching pair
- **THEN** A and WD both take pair 0 and get tight orbits (~0.2 AU scale) around the system barycenter
- **AND** B and C both take pair 1 and get wider orbits (~50 AU scale) around the **same** system barycenter
- **AND** the third entry `(A, 4200 AU, B)` does not produce any additional orbit — A and B are already matched by earlier (tighter) pairs, and the flat model has no concept of a "wide hierarchical orbit" since there is only one barycenter
- **AND** exactly four orbit ellipses are emitted, one per star, all sharing `parent_x, parent_y = star_x, star_y`

### Requirement: Catalog star positioning from Sun's actual t=0 position

Non-Zone-5 catalog stars are positioned at the correct `distance_ly` from the Sun's actual galactic t=0 position. The Sun's position is computed using exact Newton-Raphson (matching `bodies.rs Orbit::position_at()` which renders the Sun dot), NOT `galaxy::kepler_position()` which uses a first-order approximation for e < 0.1. Sun orbital elements: a=1.996e20m, e=0.07, ω=0, M₀=4.8534. The catalog `galactic_pos_ly` is used only for direction — each star is placed at `sun_pos + distance_ly * direction`.

The star's galactic orbital elements are then recomputed so `kepler_position()` reproduces the corrected position at t=0. The inverse must match the same math branch that `kepler_position()` uses for the star's eccentricity:

**For e < 0.1** (first-order inverse, matching kepler_position's first-order approximation):
1. `angle = atan2(y, x)` (this equals `ν + ω = M + 2e·sin(M) + ω` in the forward path)
2. Solve `M + 2e·sin(M) = angle − ω` via fixed-point iteration: `M ← (angle − ω) − 2e·sin(M)` (6 iterations, converges for e < 0.1)
3. `a = r / (1 − e·cos(M))`
4. Mean motion from corrected SMA using enclosed galactic mass

**For e ≥ 0.1** (exact inverse, matching kepler_position's Newton-Raphson branch):
1. True anomaly: `ν = atan2(y, x) − ω`
2. SMA: `a = r·(1 + e·cos(ν)) / (1 − e²)`
3. Eccentric anomaly: `E = atan2(√(1−e²)·sin(ν), e + cos(ν))`; mean anomaly: `M₀ = E − e·sin(E)`
4. Mean motion from corrected SMA using enclosed galactic mass

At runtime, stars are propagated purely via their corrected Kepler elements (no Sun-relative offset). Because the inverse uses the same approximation branch as the forward propagation, the star dot lies exactly on its rendered galactic orbit ellipse at t=0.

### Requirement: Galactic orbit line rendering

Galactic orbit ellipses are drawn around the galactic center (0,0) using each star's orbital elements (SMA, eccentricity, argument of periapsis). The orbit is rendered as a thick line strip (dual-vertex with perpendicular offset for width). Segment count is adaptive via `orbit_segments()` in `render/types.rs`: ~1 segment per 3 pixels of screen-space circumference, clamped to [64, `ORBIT_SEGMENTS`=5120]. This single function governs every orbit in the game (body orbits, galactic orbits, patched-conic arcs, hyperbolic trajectories).

The point generator in `add_galactic_orbit_line()` (`src/render/scene.rs`) must match the same math branch that `kepler_position()` uses for the star's eccentricity, so the rendered line passes exactly through the star dot:

- **For e < 0.1** (first-order): step by mean anomaly `M ∈ [0, 2π)` and compute `r = a·(1 − e·cos M)`, `ν = M + 2e·sin M`, `(x, y) = (r·cos(ν+ω), r·sin(ν+ω))`. This curve is *not* a true Keplerian ellipse — it's the first-order approximation that `kepler_position()` produces for low-eccentricity orbits. Drawing a true ellipse here would be off by up to `a·e²` (tens of light-years at galactic scales), causing visible separation between the star dot and its orbit line.
- **For e ≥ 0.1** (true ellipse): parametrize by eccentric anomaly `E ∈ [0, 2π)` as `(a·cos E, b·sin E)` with `b = a·√(1−e²)`, with the ellipse center offset by `-a·e` from the focus at the galactic origin, then rotated by `ω`. This matches the Newton-Raphson branch of `kepler_position()` which places stars on a true ellipse.

All orbits (including the focused star) are gated by two conditions: not in galaxy view, and the star's physical disk must be sub-pixel (< 1px on screen). The sub-pixel check uses `star.radius_m` (the physical stellar radius in meters), matching the same threshold that controls whether the star is rendered as a hexagon dot vs a real circle. This means each star's orbit naturally disappears at a different zoom level as you zoom in and its physical disk grows past 1px.

Within 1000 ly of Sgr A* (distance computed from star.x/star.y in meters), all catalog stars (catalog_index > 0) and the focused star (even if procedural) show galactic orbit lines when sub-pixel. Outside this radius, only the focused star shows its orbit (when sub-pixel).

The Sun's galactic orbit follows this same catalog star pipeline — it has no special-case rendering through the body orbit system. **Stars** whose parent is the root (Sgr A*) skip body-orbit rendering entirely, deferring to the catalog star orbit pipeline. This means the Sun's orbit appears only in star field view and only when focused, consistent with other distant catalog stars. A body is classified as a star for this purpose when its mass exceeds `MIN_STAR_MASS_KG` (1e28 kg, ~5 Jupiter masses) and it has no `accretion_disc` — matching `build_body_is_star()`. Non-star bodies orbiting the root (e.g. Crucible, a planet at 13 AU from Sgr A*) fall through to the normal body-orbit pipeline so their orbits render like any other planet's.

Sgr A* catalog stars are always included in the procedural star list (`current_procedural_stars`) regardless of viewport culling or zoom level. This ensures their orbit visibility is determined solely by the sub-pixel check, not by whether the star's dot happens to be on-screen. Without this, non-focused Sgr A* stars could drop out of the list (due to viewport culling or the 0.1 ly close-zoom early return), causing their orbits to disappear at a different zoom level than when focused.

### Requirement: Catalog planet and moon indicator rings

For each visible catalog star (catalog_index > 0), planet and moon indicator rings are drawn at orbital positions around the relevant parent:
- Each planet gets a 10px outer / 70% inner ring indicator at its orbit position around its host star
- Each moon is rendered the same way, orbiting its parent planet's **current-frame position** (not the host star)
- Planet/moon angular position uses a deterministic initial angle from body index (`body_idx * 2.399`) propagated by game time
- Planet orbit radius: `orbit_sma_au * 1.496e11 * scale` (AU → meters → world units)
- Moon orbit radius: `orbit_sma_km * 1000.0 * scale` (km → meters → world units)
- Planet rings are drawn BEFORE the star dot and star indicator ring (z-order: planets behind star)
- Color coding: gold `[0.9, 0.75, 0.2]` for life worlds, green `[0.3, 0.85, 0.4]` for habitability > 30, gray `[0.6, 0.6, 0.6]` otherwise

`inject_catalog_planets()` iterates `sys.bodies` in order. Moons come after their parent in every catalog entry, so a per-body `body_positions: Vec<Option<[f64; 2]>>` can be populated as planets are processed, and later referenced by index when moons are reached. A moon whose parent position is unavailable is skipped.

### Requirement: Exoplanet info panel

When a catalog planet or moon is tracked in the tracking station (`tracked_body >= num_real_bodies`), the info panel shows body-specific data built from `CatalogBody`:

- **Name** — planet/moon name in white
- **Description** — body description (italic, gray)
- **Physical Properties** — radius, surface gravity, mass
- **Atmosphere** — pressure (converted from atm to Pa) and scale height, or "No atmosphere"
- **Orbit** — semi-major axis, eccentricity, orbital period
- **Colony Prospects** — habitability score, mineable resources
- **Moons (N)** — for planets that host moons, a compact listing of each moon (name, gravity, temperature, habitability, atmosphere, life indicator). Moons themselves don't host sub-moons, so their panels don't include this section.

`BodyInfoData` for catalog bodies is built during `inject_catalog_planets()` and stored in `RenderState.catalog_body_info: HashMap<usize, BodyInfoData>` keyed by synthetic body index. For non-moon bodies, `catalog_planets` is populated with all children whose `parent_body_idx == Some(body_idx)` (each tagged `is_moon: true`). For moons, `catalog_planets` is empty. The tracking station panel checks `catalog_body_info` when `body_info.get(idx)` returns `None` for synthetic indices.

### Requirement: Catalog star info panel

When a catalog star is focused (catalog_index > 0), the info panel shows additional sections after the standard stellar properties:

**Single-star systems**: standard star info panel with all planets listed.

**Multi-star systems (barycenter view)**:
- Name: "{system name} System" (e.g. "Alpha Centauri System")
- Description: "Binary/Triple/Quadruple star system" (italic, gray)
- Physical properties and stellar properties sections are **hidden** (radius_m == 0.0 guard)
- **Component Stars** section: each star listed with name (light blue, 180/200/255), spectral type / mass / luminosity on second line (gray)
- No planets listed at barycenter level (planets are on individual star info panels)
- System Info section: zone, distance, spectral types
- Galactic orbit section preserved

**Individual companion stars** (double-click on a companion star):
- Standard star info panel with physical/stellar properties
- Description: per-star description from `CatalogStar.description` (falls back to system description if empty)
- Planetary System section shows **only planets orbiting this specific star** (filtered by `host_star_index()`)
- Orbit section shows the stellar orbit around the barycenter (not galactic orbit)
- `is_galactic_orbit = false` for stellar orbits

Common sections across all catalog star views:
- **System Info** — zone number, distance from Sol (ly), spectral type(s), star count
- **Planetary System** — compact listing of planets and moons with:
  - Designation and name
  - Temperature (K), surface gravity (g), habitability score
  - Atmosphere indicator
  - Life indicator (gold text for worlds with life)
  - Color coding: gold for life worlds, green for habitability > 30, gray otherwise

### Requirement: CatalogPlanetInfo and CatalogStarInfo in BodyInfoData

`BodyInfoData` includes catalog-specific fields:
- `catalog_stars: Vec<CatalogStarInfo>` — component star data for multi-star system barycenter info panel
- `catalog_planets: Vec<CatalogPlanetInfo>` — planet/moon summary data for the info panel
- `catalog_zone: Option<u8>` — zone number (1–5)
- `catalog_distance_ly: Option<f32>` — distance from Sol in light-years
- `catalog_spectral: Option<String>` — spectral type string(s) for all stellar components (e.g. "G2V / K1V / M5.5Ve")

`CatalogStarInfo` contains: name, spectral_type, mass_solar, radius_solar, luminosity_solar.

`CatalogPlanetInfo` contains: name, designation, temperature_k, gravity_g, habitability, has_atmosphere, has_life, is_moon, is_gas_giant.

## Files

- `src/galaxy/mod.rs` — ProceduralStar struct (with catalog_index field), StarType enum, GalaxyState cache (with catalog_by_sector), solve_kepler_nr(), kepler_position()
- `src/galaxy/generation.rs` — Per-sector star generation with elliptical orbital elements and StarType classification
- `src/galaxy/density.rs` — Sector star count from galactic position (exponential disk + Gaussian bulge), capped at MAX_STARS_PER_SECTOR (2000)
- `src/galaxy/catalog.rs` — CatalogSystem/CatalogStar/CatalogBody/CatalogAtmosphere/StarOrbitData data structures, host_star_index() helper, build_catalog_stars() builder, lookup_system() lookup function
- `src/galaxy/catalog/catalog_data.rs` — Static CATALOG array with all 67 named star systems (318 bodies), binary_orbits data for 19 multi-star systems
- `src/main.rs` — `build_procedural_star_data()`: backward rotation, distance-ordered sectors, Kepler propagation, MAX_STARS cap, star metadata population, catalog name lookup; `lookup_focused_star()`: resolves focused star by name from galaxy cache when below visibility threshold, catalog name lookup; focused star info builder: catalog data lookup for planets/zone/spectral info; star double-click handling in flight and tracking station
- `src/render/scene.rs` — StarRenderData struct (with catalog_name, catalog_index, num_catalog_stars), format_name() returns real name for catalog stars (with " System" suffix for multi-star), adaptive dot/circle rendering, screen position storage
- `src/render/state.rs` — Star hover/focus state fields on RenderState, catalog_body_info HashMap for exoplanet info panels
- `src/render/interaction.rs` — `update_star_hover()`, `star_at_screen_pos()`, star focus tracking in `update_tracking()`
- `src/render/menus.rs` — Star hover labels and info panel in tracking station, catalog system info and planetary system sections
- `src/render/flight.rs` — Star hover labels in flight mode
- `src/render/types.rs` — BodyInfoData with luminosity_solar, star_type, temperature_k, soi_radius_m, is_galactic_orbit, catalog_stars, catalog_planets, catalog_zone, catalog_distance_ly, catalog_spectral fields; CatalogStarInfo struct; CatalogPlanetInfo struct; OrbitRenderData struct; ORBIT_SEGMENTS constant (5120); orbit_segments() adaptive function
- `src/bodies.rs` — `calculate_soi()` (public), `galactic_enclosed_mass()` used for star SOI computation
