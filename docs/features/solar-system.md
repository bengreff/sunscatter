# The Solar System (D084)

**Status:** *playable* for motion and physical data (every body is on its real orbit, deterministic, tested against JPL); *skeleton* for looks (one base colour per body, no maps) and for the atmospheres of Venus, Mars and Titan (sourced near the surface, assumed above). Built 2026-10-26 (overnight run, SOL workstream).

**Scope (D084, the D014 rule, largest first):** the 8 planets; the dwarf planets Ceres, Pluto, Eris, Haumea, Makemake, Gonggong, Quaoar, Sedna, Orcus; every moon over 100 km mean radius: Io, Europa, Ganymede, Callisto; Mimas, Enceladus, Tethys, Dione, Rhea, Titan, Hyperion, Iapetus, Phoebe; Miranda, Ariel, Umbriel, Titania, Oberon; Triton, Proteus, Nereid; Charon; Dysnomia, Hi'iaka, Vanth. 44 bodies (was 11). Moons over 5 km: not done (see [Open](#open)).

## What was built

### Motion: the pipeline of motion-model §1, hierarchical

1. **Planetary integration** (`sim::sol::tree`, `gen_config`): the Sun, planets (the outer ones as their systems' barycenters), Earth and the Moon, and now the dwarf planets as point masses (the Eris, Haumea and Orcus systems as barycenters), from DE440 (planets) and JPL Horizons small-body solutions (dwarfs). Yoshida 8 at 1 h, 50 years. Dwarf planets are in this integration, not test particles, so the planets feel them and the cut rule of `forces.rs` (a cut source pulls the vessel as it pulls the anchor) stays exact. Settings unchanged from v3 (power-of-two segments, raw coefficients): see [Choices](#choices).
2. **Moon systems** (`sim::sol::systems`, catalog `data/ephemeris/sol-systems.ron`): each integrated once in the frame of its barycenter node, with mutual point-mass gravity, the primary's zonal field (J2 from its `body.ron`; J4, J6 from the catalog) about its IAU pole, and the tides of the Sun and planets read from the planetary ephemeris (`gen::External`: each body feels a perturber's pull minus its pull on the barycenter, which the planetary integration moves as a point mass of the system's GM: the hierarchy of JPL's satellite ephemerides). Steps 240 s (Saturn) to 3600 s (TNO binaries), a few hundred per orbit of the fastest member. Members are children of the barycenter node (positions never compound two fits). Initial states from Horizons relative to the barycenter, made exactly barycentric with our GMs (Horizons' centre includes unmodelled moons and its GMs are rounded: as given, Uranus's members drifted off their barycenter at 4.9 mm/s, 7,800 km in 50 years). The TNO satellite solutions end at 2030-01-01: their states are taken a day earlier and carried on the two-body conic.
3. **Fit**: rails where the residual meets D042's tolerance (none does: rails residuals are 2 km to 4,000 km), else Chebyshev tables: moons at 100 m, a primary about its barycenter 10 m, dwarf planets 1 km, planets as before. Moon tables use free segment lengths (any whole number of steps; bisection after the power of two) and coefficients rounded to a power of two under `tolerance / (8 (degree+1))`, stored as packed zigzag integers (format tag 4, decoded exactly): Jupiter's system went from 16 MB to 3.5 MB per 50 years at the same tolerance. Degree 20 (degree 31 saves 4 %, costs evaluation time).

The pieces are pure functions of the committed inputs; `ephem-tool sol` regenerates the file bit for bit (checked across runs), and the golden tests regenerate the start of every table (planets: the 2-year trial; moon systems: just over their longest segment, with the shipped segment lengths, `gen::generate_with_segments`) in about 2 s.

### Data

| File | Size | What |
|---|---|---|
| `data/ephemeris/sol-2030.bin` | 19.3 MB (was 2.6) | 63 nodes, 2030–2080 |
| — planets, Moon, barycenters | 2.62 MB | raw tables, as before |
| — dwarf planets | 0.15 MB | |
| — Jupiter / Saturn / Uranus / Neptune systems | 3.55 / 6.61 / 3.42 / 2.24 MB | packed tables |
| — Pluto–Charon, TNO binaries | 0.32, 0.44 MB | |
| `sol-systems.ron` | 10 kB | the catalog, every constant sourced |
| `sol-2030-initial.ron`, `sol-2030-systems-initial.ron` | 4.5, 10 kB | initial states (DE440, Horizons) |
| `horizons-reference.ron`, `de440-reference.ron` | 46, 12 kB | reference states for the golden tests |

**1,000 years (D005) is not affordable this way:** the moon tables cost what their orbits do (about 250 bytes per orbit for an inner moon at any tolerance: Chebyshev convergence is factorial, so tolerance barely matters, and quantization is bounded by the anchors' smoothness, see below). 1,000 years would be ~330 MB of moons. Options for the lead: a much shorter window for fast inner moons, a semi-analytic or frequency-fitted model (secular rates plus a few hundred periodic terms per moon, expanded to tables at load), or a looser D042 for moons with entropy coding. The planets would fit (~50 MB at today's settings, ~12 MB with free segments if C¹ fits make those safe).

### Physical data (`data/bodies/<body>/body.ron`)

Radii and rotation for every body from the IAU WGCCRE 2015 report via NAIF's `pck00011.tpc`: triaxial radii as an oblate spheroid (equatorial √(ab), polar c); slow periodic pole terms linearised about 2055, with the measured worst angle against the full series over 2030–2080 written in each file (Neptune 0.013°, Triton 0.83°, Miranda 4.3° from its fast librations); prime-meridian periodic terms left out. Mars's pole was 1.6° off in the old file (constant terms only); now within 0.0002°. Bodies without an IAU model (Hyperion, Nereid, the TNOs) have placeholders, marked: a pole along the satellite's orbit normal (computed from the JPL states) or the ecliptic pole, a spin from lightcurves. J2 where measured: planets (NASA fact sheet; Juno, Cassini, Jacobson), Io, Europa, Ganymede, Callisto (Schubert et al. 2004), Titan (Iess 2010), Enceladus (Iess 2014), rescaled to each file's radius; J4, J6 for the giants in the catalog. GMs in the catalog (Horizons pages; estimates marked: Sedna, Nereid, Dysnomia's split). Rocky and icy bodies are `solid` (smooth ellipsoids, rails floor 2 km); the gas giants are not (no surface, no atmosphere model yet).

**Atmospheres (skeleton):** Venus: the VIRA table to 65 km (Seiff et al. 1985), then isothermal at 243 K and hydrostatic to a 140 km top (about 3× VIRA's density at 100 km, far too dense in the thermosphere beyond 120 km). Mars: NASA Glenn's model to 40 km, then isothermal at 161 K and hydrostatic to 150 km (within a few times of aerobraking densities near 100 km). Titan: an exponential from Huygens's surface values (1,467 hPa, 93.65 K; H = 21 km) to 600 km, far too thin above ~200 km where the real scale height doubles. The isothermal continuations are our assumption (the table's own exponential extrapolation would be 1,000× too dense at 150 km on Mars); each file says so. Rails floors at the tops (D062). Needed: VIRA's upper tables (Keating et al. 1985), a Mars upper-atmosphere model (Mars-GRAM), Huygens HASI's profile.

### Visual data (`visual.ron`)

Base colour (an approximate visual colour), Bond albedo where published (otherwise the geometric albedo, marked) and a brightened icon colour, for every new body. The game draws them through the terrain system as smooth ellipsoids (Mercury–Neptune were not drawn in 3D before). No maps, no atmospheres in the renderer.

## Accuracy against JPL (golden tests)

- **Fits** against our integration: every table within its tolerance (moons ≤ 86 m, Titania).
- **Planets against DE440:** unchanged quality (Earth–Sun 64 m after 1 year, 4.0 km after 50; Mars 32 km, Jupiter 36 km, Neptune 196 km after 50; Moon–Earth 3.8 km after 1 year), slightly better than before for Earth (the dwarf planets now perturb it, as in DE440).
- **Dwarf planets against Horizons** (heliocentric): under 2 cm at the epoch; after 50 years 15–36 km for the TNOs, 340 km for Ceres (which DE441 perturbs with hundreds of asteroids we do not model).
- **Moons against Horizons** (about their primary): ~1 m at the epoch; then linear drifts, mean-motion offsets of 1e-7 to 3e-6 from constants and model differences (rounded GMs on the Horizons pages, e.g. Charon's printed 106.10; tides, higher harmonics and pole models we do not have). After 1 year / 50 years: Titan 7.6 km / 390 km, Iapetus 7.8 / 400 km, Phoebe 0.8 / 12 km, Ganymede 6 / 650 km, Io 140 km / 6,300 km, Mimas 176 km / 7,900 km, Oberon 280 km / 14,000 km (1.4° of its orbit), Triton 26 km / 1,400 km, Charon 15 km / 770 km. The orbits (size, shape, plane) match; the phase drifts. Fitting the initial states to Horizons over the window instead of taking them at the epoch would remove most of it.

## The force model: clusters (D023)

A moon system far from a vessel pulls it as one point mass at its barycenter (`World::clusters`, a stand-in source per barycenter node): opened where its quadrupole error `3Q/d⁴` could reach a hundredth of the cutoff (Jupiter's within 2.7 AU, the Earth–Moon barycenter's within 1.2e11 m). Decided from the vessel's position, never its anchor (rule 1). With lazy snapshots (`ephem::Snapshot`: the planets eager, nodes inside moon systems on first use) a coast near Earth never evaluates a moon table. `forces.rs` (LIMBO's file) got the minimal change: the source loop skips members of closed clusters and stand-ins of open ones.

**Cost** (`cargo run -p sim --release --example bench_coast`, M2 Pro shared with other agents; "before" is today's code on the previous ephemeris):

| | before (11 bodies) | after (44 bodies) |
|---|---|---|
| LEO coast step | 3.97 µs | 9.25 µs |
| snapshot + one relative position | 0.48 µs | 0.72 µs |
| every body placed (a frame of the map) | 0.38 µs | 2.25 µs |

Where the coast's extra 5.3 µs goes: ~2.1 µs in the per-step searches over solid bodies (`vessel::segment::find_contact`, and `live::nearest_surface` in live flight), which now evaluate 36 moon tables a step: measured by making the moons non-solid. `World::may_reach` (a member of a moon system whose barycenter is farther than its extent plus the reach cannot be near) lets those loops skip far systems without evaluating them; the call sites are in LIMBO's files and not changed tonight. The rest (~3 µs) is the 8 dwarf planets (8 more tables per stage time, always touched because the cut rule adds their pull at the anchor) and the cluster test per force evaluation. Cheaper tables for the dwarf planets (low degree: they move slowly) or skipping cut sources whose pull at the anchor is far below the cutoff would recover most of it.

## Choices

- **Planets stay power-of-two, raw.** Free segment lengths fit closer to the tolerance, and a fit's velocity jumps at segment boundaries in proportion to its error: a year anchored at the Sun drifted 4.8 km from the same coast anchored at the barycenter (19 m before). The planets are the game's anchors (Earth, the Moon), so they keep the old settings. Moon tables are not anchors yet (no anchor zones: a vessel near Io is stored relative to the barycenter of the Solar System), so they can be free and packed. **Before giving moons anchor zones, their fits need C¹ continuity at boundaries** (position and velocity matched at segment ends) or a tighter tolerance.
- **J4/J6 sign:** first written with the opposite sign of J2 (Io drifted 2,000 km a year against Horizons); fixed and tested against one finite-difference potential for J2–J6.
- **Body directory names** are lower-case letters and digits (`Hi'iaka` → `hiiaka`): `sim::body::data::body_dir_name`, which the game now calls instead of its own copy.

## Tests

- `sim/tests/sol_ephemeris.rs`: planets and every moon system equal regeneration bit for bit; DE440 and Horizons bounds (a drift rate per body); the 50-year window; all 44 bodies with physical data; the file under 20 MB.
- `sim/tests/ephemeris_generation.rs`: a planet–moon pair in its barycenter's frame under the star's tide matches one full integration to 0.3 m (golden hash of the packed output: tides, J2–J6, free segments, quantization).
- `sim::gen::nbody`: zonal terms against the potential; `sim::ephem::io`: packed tables round-trip; `sim::ephem`: lazy snapshots equal eager ones; `sim::world`: clusters (roles, opening, point-mass error under 1e-14 m/s²).
- Updated for the new data: the golden LEO coast hash; the gravity-source tests (dwarf planets massless in the cut-versus-full test, Ceres sitting just under the cutoff there; 5 m for the closed systems' quadrupoles; the Pluto flyby's error ratio 2e-3); the playtest save re-stamped to the new ephemeris id.

## Open

- Moons over 5 km (about 80 more): the fastest (Metis 7 h, Naiad 7 h) would cost ~12 MB each as 100 m tables and need 60 s steps. Needs a decision: rails or tables at a looser tolerance for small moons, or test particles.
- Upper atmospheres of Venus, Mars, Titan (above). Gas giants: no surface or atmosphere model (vessels pass through).
- Initial states fitted to Horizons over the window (removes the linear drifts).
- C¹ table fits (anchor zones for moons and planets beyond Earth; free segments for planets).
- 1,000 years (above).
