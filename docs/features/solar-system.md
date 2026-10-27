# The Solar System (D084)

**Status:** *playable* for motion and physical data (every body is on its real orbit, deterministic, tested against JPL); looks and terrain: see [solar-visuals.md](solar-visuals.md) (2026-10-26: maps, zones and physical terrain for the solid bodies, measured where a global DEM exists and procedural otherwise; the giants' colour maps; atmospheres) and for the atmospheres of Venus, Mars and Titan (sourced near the surface, assumed above). Built 2026-10-26 (overnight run, SOL workstream).

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

**1,000 years (D005) is not affordable this way** (see [The 1,000-year problem](#the-1000-year-problem-for-the-owner)).

### Physical data (`data/bodies/<body>/body.ron`)

Radii and rotation for every body from the IAU WGCCRE 2015 report via NAIF's `pck00011.tpc`: triaxial radii as an oblate spheroid (equatorial √(ab), polar c); the rotation as the IAU series with its periodic terms of at least 0.01° (`body::Rotation::librations`, up to 10 per body; the angular velocity includes their rates), within 0.007° (pole) and 0.03° (prime meridian) of the full series over 2030–2080 for the Moon, whose 18.6-year term moves the pole 1.5° and the meridian 3.6° (tested against the series), and within 0.004° for every other body (Triton's was 0.8° off as a straight-line fit). Mars's pole was 1.6° off in the old file (constant terms only). Bodies without an IAU model (Hyperion, Nereid, the TNOs) have placeholders, marked: a pole along the satellite's orbit normal (computed from the JPL states) or the ecliptic pole, a spin from lightcurves. J2 where measured: planets (NASA fact sheet; Juno, Cassini, Jacobson), Io, Europa, Ganymede, Callisto (Schubert et al. 2004), Titan (Iess 2010), Enceladus (Iess 2014), rescaled to each file's radius; J4, J6 for the giants in the catalog. GMs in the catalog (Horizons pages; estimates marked: Sedna, Nereid, Dysnomia's split). Rocky and icy bodies are `solid` (smooth ellipsoids, rails floor 2 km); the gas giants are not (no surface, no atmosphere model yet).

**Atmospheres (skeleton):** tables from the surface to where the density is negligible (~1e-14 kg/m³, so drag never meets a wall; the first version ended at 1.3e-7 kg/m³ on Venus), with `top` (live flight, rails floor, D062) where the density falls to ~2e-9 kg/m³ like Earth's 150 km. Venus: the VIRA table to 65 km (Seiff et al. 1985), then hydrostatic through sourced temperatures (230 K at 73 km, the 165 K mesopause at 95 km, a 200 K day–night mean of the thermosphere), to 200 km; top 150 km. Mars: NASA Glenn's model to 40 km, then hydrostatic at 161 K (our assumption) to 270 km; top 150 km. Titan: hydrostatic from Huygens HASI's surface values through its temperature profile (70.43 K tropopause at 44 km, 186 K stratopause at 250 km, ~170 K above 500 km; Fulchignoni et al. 2005) to 2,075 km: 7e-10 kg/m³ at 1,000 km, where Cassini flew through a few 1e-10; top 950 km. Mean molar masses held constant (atomic oxygen on Venus and methane's fall on Titan left out). Needed: VIRA's upper tables (Keating et al. 1985), a Mars upper-atmosphere model (Mars-GRAM).

### Visual data (`visual.ron`)

*Superseded for the solid bodies by [solar-visuals.md](solar-visuals.md) (maps, zones, terrain); the base colour and albedo below remain the far-away dot and the fallback.* Base colour (an approximate visual colour), Bond albedo where published (otherwise the geometric albedo, marked) and a brightened icon colour, for every new body. The game draws them through the terrain system as smooth ellipsoids (Mercury–Neptune were not drawn in 3D before). No maps, no atmospheres in the renderer.

## Accuracy against JPL (golden tests)

- **Fits** against our integration: every table within its tolerance (moons ≤ 86 m, Titania).
- **Planets against DE440:** unchanged quality (Earth–Sun 64 m after 1 year, 4.0 km after 50; Mars 32 km, Jupiter 36 km, Neptune 196 km after 50; Moon–Earth 3.8 km after 1 year), slightly better than before for Earth (the dwarf planets now perturb it, as in DE440).
- **Dwarf planets against Horizons** (heliocentric): under 2 cm at the epoch; after 50 years 15–36 km for the TNOs, 340 km for Ceres (which DE441 perturbs with hundreds of asteroids we do not model).
- **Moons against Horizons** (about their primary): the initial states are fitted to JPL's satellite ephemerides over the whole window (`ephem-tool`'s `fit`: at 21 epochs 2.5 years apart, each moon's along-track residual is fitted with a line, the moon moved along its orbit by its offset and its semi-major axis changed for its rate, 4 rounds of one integration each, keeping the best; observations from Horizons when cached, else from the same ephemerides as NAIF SPK kernels via ANISE: JUP365, SAT441, URA184, NEP097/NEP105, PLU060, in `data/external/kernels`). Worst error over 2030-2080, before the fit -> after: Charon 770 km -> 110 m, Iapetus 400 -> 1.5 km, Titan 380 -> 8 km, Io 6,400 -> 24 km, Oberon 14,000 -> 54 km, Ariel 4,600 -> 36 km, Ganymede 650 -> 35 km, Mimas 7,600 -> 390 km, Tethys 5,700 -> 170 km, Triton 1,900 -> 500 km, Nereid 4,700 -> 450 km. Proteus (in a 1.8 GB kernel, not downloaded) is not fitted: 320 km. What remains is model differences (orbit shapes and planes: higher harmonics, tides, the primaries' pole models), not drift. The TNO moons are not fitted (their JPL solutions end at the epoch).

## The force model: clusters (D023)

A moon system far from a vessel pulls it as one point mass at its barycenter (`World::clusters`, a stand-in source per barycenter node with its members' total GM, so opening or closing changes the pull only by the quadrupole, tested at the opening distance): opened where its quadrupole error `3Q/d⁴` could reach a hundredth of the cutoff (Jupiter's within 2.7 AU, the Earth–Moon barycenter's within 1.2e11 m). Decided from the vessel's position, never its anchor (rule 1). With lazy snapshots (`ephem::Snapshot`: the planets eager, nodes inside moon systems on first use) a coast near Earth never evaluates a moon table. `forces.rs` (LIMBO's file) got the minimal change: the source loop skips members of closed clusters and stand-ins of open ones.

**Cost** (release, M2 Pro shared with other agents; each pair measured alternately from archived trees so the load is the same):

| | before SOL (077af0c) | with the new bodies (3b2ad2c) | now |
|---|---|---|---|
| `bench_coast`: LEO coast step | 3.5 µs | 9.2 µs | 5.3 µs (4.4 µs on the old ephemeris) |
| every body placed (a frame of the map) | 0.4 µs (11) | 2.3 µs (44) | 2.3 µs (44) |
| `bench_limbo`: bare coast at 250 km | | 10.0 µs | 5.8 µs |
| perturbed coast at 250 km | | 15.7 µs | 12.7–13.9 µs |
| 10 limbo craft at 1,000,000×, per vessel per frame | | 4.3–4.5 ms | 3.8–4.0 ms |

What got it back (all decided from the vessel's position, the same for every anchor up to the stated budgets; chunked = single pass, invariance tests unchanged):
- **Per-step searches skip far bodies** (`World::may_reach`): contact, atmosphere entry and ambient pressure test a moon system by its barycenter and extent, and a body or system that keeps farther from the Sun than the vessel by its least distance from the Sun over the window (`Cluster::star_distance`, `Source::faint_min_distance`), so far moons and dwarf planets are never evaluated (was ~2 µs a step).
- **Faint bodies left out where they cannot matter** (`World::counts_in`): a dwarf planet or a dwarf's system (under 1e-8 of the system's GM) whose pull on the vessel and on the anchor is provably under a hundredth of the cutoff (1e-14 m/s²) is not summed; its uncancelled pull on the anchor is the error, inside D023's budget. Near Earth that is every dwarf but Ceres and Pluto. Faint nodes and moon systems are evaluated lazily by snapshots (the planets, Earth and the Moon eagerly).
- **The active set without evaluating far moons** (`World::may_be_active`, `may_be_active_all`): a closed system's member is tested only where `2 GM ρ / d³` (ρ the vessel's offset from the anchor, d the least distance from the anchor–vessel segment to the system) could reach the cutoff; the set is the one the exact test over every source gives. (Testing only the sources that count, tried first, left a source counted but cut for part of a step when its cluster closed: 2 km in a year.)
- **Cheaper loops:** the gravity loop skips a closed system's members at once (`World::counted`), positions relative to the anchor reuse the anchor's walk to the root (`Snapshot::path_to`, bit-identical), clusters far from the Sun close by the bound above without a walk, and a body's gravity field is evaluated only to the degree that can matter at the vessel ([Gravity fields](#gravity-fields-d090), which replaced the J2 rule of the time).

Golden LEO coast hash 0x52e3680dc0baea81 (the faint bodies and far J2 terms now left out).

## Gravity fields (D090)

**Status: playable.** Vessels feel each body's measured gravity field beyond its point mass (`sim::gravity`), through every force path (coasts, limbo, live ticks, planned burns, the landing predictor, the bubble).

**Model.** Fully normalized spherical harmonics (geodesy 4π, no Condon–Shortley phase) with each model's reference radius and GM, in the body's rotating frame (`body.ron`'s rotation, `Rotation::matrix`), evaluated by Pines' singularity-free recursion in direction cosines (normalized as Lundberg & Schutz 1988 and Eckman et al. 2014; the radius ratio folded into the column recursion and the degree sums lumped per order as Holmes & Featherstone 2002; the radial term from `A(n+1,m+1) = (n+m+1) A(n,m) + u A(n,m+1)`, so no column runs past the field's degree). No trigonometry, no division by the cosine of the latitude (the poles are ordinary points), a fixed summation order. A zonal field needs only the pole. A body without a published field has the degree-2 zonal case of its `body.ron` J2. Checked against independent evaluations to 2.0e-14 relative at worst (pyshtools 4.14 at degrees 12 and 300; within 0.1° of a pole, where pyshtools is off by 1e-12, a 40-digit mpmath sum and pyshtools through rotated coefficients), and the shipped files against pyshtools reading the published files directly (Earth, the Moon, Jupiter, to 1e-12; `tests/gravity_fields.rs`).

**Truncation** (`Field::included_degree`, `forces::field_degree`): a vessel includes a body's degrees up to where the rigorous bounds of all higher degrees, `(GM/r²)(R/r)ⁿ (2n+1) √(n+1) ‖cₙ‖` from the addition theorem, sum to under a hundredth of the cutoff (1e-14 m/s²) at the vessel. The field acts on the vessel only (the anchor's acceleration is its ephemeris'), so the rule depends on the vessel's distance from the body alone, never on the anchor. The degree only grows within a segment and is stored with it (`forces::Active`). Near a body every shipped degree is in (LEO, low lunar orbits); at GEO Earth's degree 10, near the Moon Earth's 3; a heliocentric vessel none.

**Steps that resolve the field** (`ForceContext::max_step` through `Dynamics::max_step`): a step spans at most three half-wavelengths of the highest degree that can move a step's position by the coasts' tolerance (bounds over 1e-9 m/s²), `3 π d / (N v)` at the vessel's speed over the body's turning surface. Without it the error control does not see the high degrees and aliases them: at the coasts' tolerance degree 60 in LEO erred by 700 m a day against its 8 m truncation error (degree 10: 350 m), and the Earth- and barycentre-anchored coasts drifted 13 m apart in 6 h.

### Data (`data/bodies/<body>/gravity.ron`, 556 kB)

Written by `cargo run -p ephem-tool --release -- gravity` from the raw files in `data/external/gravity/` (each file's header gives its URL), truncated, with the model's GM and reference radius (the harmonics scale with the model's GM; the point mass is the ephemeris'):

| Body | Model (source) | Shipped / available degree | Frame |
|---|---|---|---|
| Earth | GGM05C (Ries et al. 2016, doi:10.5880/icgem.2016.002; CSR, GRACE + GOCE + DTU13; zero-tide) | 30 / 360 | ITRF taken as the body-fixed frame of the IAU rotation (below) |
| Moon | GRGM1200A (Lemoine et al. 2014, GRL 41, 3382; PDS) | 100 / 1200 | DE430 principal axes, rotated from our mean-Earth frame by Rz(67.573″) Ry(78.580″) Rx(0.285″) (Williams, Boggs & Folkner 2013, JPL IOM 335-JW,DB,WF-20130722-016) |
| Mars | MRO120F (Konopliv et al. 2020, GRL 47, e2020GL090568; PDS; GM of the Mars system as the ephemeris' Mars) | 50 / 120 | Konopliv's frame, within ~25 m at the surface of IAU 2015's (its nutation and seasonal spin terms, < 0.5″, left out) |
| Venus | MGNP180U (Konopliv, Banerdt & Sjogren 1999, Icarus 139, 3; PDS) | 40 / 180 | the IAU frame of the Magellan era, the same as `body.ron`'s |
| Mercury | HgM008 (Genova et al. 2019, GRL 46, 3625; PDS) | 40 / 100 | the model's rotation (pole 281.0082°, 61.4164°; W 329.75° + 6.138505441°/d), turned into ours at 2030: 0.088° in longitude, drifting 0.1° by 2080 |
| Jupiter | Durante et al. 2020 (GRL 47, e2019GL086572; Juno, PDS) | J2–J12 / 12 (zonals; the degree-2 tesserals, ~1e-9 and within ~2σ of zero, left out) | the pole only |
| Saturn | Iess et al. 2019 (Science 364, eaat2965; Cassini Grand Finale), J2–J10 even, as tabulated by Fortney et al. 2023 (arXiv:2304.09215) | 10 (zonal; the odd zonals, ~1e-7, left out) | the pole only |
| Io | Park et al. 2025 (Nature 638, 69; Juno + Galileo): J2, C21, S21, C22, S22 | 2 | `body.ron`'s IAU frame (synchronous, x near the sub-Jupiter point, as the solution's) |
| Europa | Gomez Casajus et al. 2021 (Icarus 358, 114187), SOL-B (J2/C22 = 10/3 imposed, as Anderson et al. 1998) | 2 | as Io |
| Ganymede | Gomez Casajus et al. 2022 (GRL 49, e2022GL099475; Juno's flyby; doi:10.5281/zenodo.6949645) | 5 / 5 | as Io |
| Callisto | Anderson et al. 2001 (Icarus 153, 157) as tabulated by Schubert et al. 2004 | 2 | as Io |
| Titan | Durante et al. 2019 (Icarus 326, 123), Table 2 | 3 / 5 (degrees 4–5 mostly within their uncertainties) | as Io |
| Enceladus | Iess et al. 2014 (Science 344, 78), SOL1: J2, C21, S21, C22, S22, J3 | 3 | as Io |
| Ceres | CERES18D (Konopliv, Dawn, PDS SBN DWNCGRS_2 v4; Konopliv et al. 2018, Icarus 299, 411) | 18 / 18 | the model's rotation turned into IAU 2015's at 2030 (0.36° in longitude; the rates differ by 3.5e-5°/day, 0.64° by 2080) |

Every other body keeps its `body.ron` J2. Vesta is not one of our bodies. `body.ron`'s `j2` stays what the ephemeris generator uses (the shipped ephemeris regenerates bit for bit); Earth's differs from GGM05C's by 8e-6 relative.

**Frames, the errors stated.** *Earth:* our rotation is IAU 2015's (the pole's linear precession, W linear in TDB): no nutation (≤ 17″ in longitude, 9″ in obliquity), no polar motion (≤ 0.5″) and no UT1. Against precession and GMST with ΔT ≈ 70 s, our Greenwich is 0.047° west of the real one in 2000, 0.091° in 2030, 0.12° by 2080 (10–14 km at the surface; 0.0042° per second of ΔT): the tesseral field turns with it, which moves the GEO stable points by that much. Terrain and launch sites share it (not this workstream's to fix). *Moon:* the IAU series realises the mean-Earth frame to ~150 m (NAIF: 72 m mean, 155 m amplitude against DE440's); GRAIL's principal-axis frame is ~860 m from it and is rotated in. *Synchronous moons:* the published J2/C22 are about the mean sub-planet direction, which the IAU prime meridians approximate to within their libration (≲ 1°).

### Convergence: how many degrees (`examples/gravity_convergence`)

A day in each body's lowest common orbit through the body alone (its point mass and field, its rotation), integrated tightly (1 µm a step) at each degree against the highest available; worst position difference over the day:

| Orbit | Degree → worst difference in a day |
|---|---|
| Earth 400 km, 51.6° | 10: 346 m · 20: 154 m · **30: 70 m** · 40: 60 m · 60: 8 m · 100: 1.0 m · 120: 0.2 m (vs 160) |
| Earth 200 km, 51.6° | 10: 492 m · **30: 134 m** · 60: 32 m · 100: 13 m · 120: 4 m |
| GEO, 10 days | 2: 3,962 m · 3: 84 m · 6: 0.66 m · 8: 0.02 m (the rule includes 10) |
| Moon 100 km, polar | 30: 719 m · 50: 449 m · 70: 41 m · **100: 2.4 m** · 150: 0.08 m (vs 300) |
| Moon 50 km, polar | 70: 353 m · **100: 50 m** · 150: 5 m · 200: 0.9 m (vs 400) |
| Moon 20 km, polar | **100: 398 m** · 150: 113 m · 200: 40 m · 250: 22 m (vs 400; GRAIL's own high degrees matter here) |
| Mars 250 km, polar | 30: 486 m · 40: 229 m · **50: 48 m** · 70: 7.5 m (vs 120) |
| Venus 200 km | 30: 494 m · **40: 87 m** · 70: 41 m · 100: 5 m (vs 180) |
| Mercury 200 km | 30: 178 m · **40: 35 m** · 50: 19 m · 70: 0.9 m (vs 100) |
| Ceres 385 km (Dawn's LAMO) | 10: 7.4 m · 14: 0.8 m (**18** shipped) |

At the coasts' tolerance with the step cap (`GRAVITY_COAST=1 GRAVITY_HCAP=3`) the errors are these to within 10 % or 1.3 m; with J2 alone a LEO day errs by ~900 m. **The shipped degrees** bring a day in the lowest common orbit to tens of metres, a few times the coasts' own error (~10 m a day in LEO): below the drag model's uncertainty wherever Earth's dropped degrees matter (km a day at 400 km), and to metres in lunar orbits from 100 km. Each doubling of the degree costs ~8× (evaluations go as N², steps as N), so the next steps (Earth 60, the Moon 150) are a budget decision ([below](#cost)).

### Validation (`tests/gravity_fields.rs`, `examples/lunar_orbits`)

- **The Apollo subsatellites.** PFS-2 (released 1972-04-24 into 91 × 130 km at 169.3°) hit the Moon after 35 days, its periapsis having fallen to ~10 km, risen to ~50 km and fallen again; PFS-1 (103 × 136 km at 151.3°) was at 52 × 186 km 13 months later and lasted about a year and a half (NSSDC; Bell 2006, NASA Science). The same orbits in 2030 from eight node longitudes and start dates: **PFS-2-like orbits crash after 33–54 days**, with the same dip and recovery (the test's: 35.9 days; with the Moon's J2 alone the orbit stays up); **PFS-1-like orbits: seven of eight are up after 400 days** (periapsis 21–55 km at worst), one crashes after 333 days (the test: above 70 km for 60 days).
- **Frozen inclinations.** Circular at 100 km for 150 days from three node longitudes: 27°, 50°, 76°, 86° stay up (periapsis at worst 37–73 km); polar falls to 14–20 km, about 0.5 km a day (Lunar Prospector and LRO maneuvered every few weeks to keep theirs); 63° crashes after 81–103 days, 11° after 17–49 (Bell 2006; Ely 2005).
- **Geostationary drift.** At rest over 30°E and 120°E the longitude accelerates at +0.00183 and −0.00200°/day² toward 75°E (−3 a_E/a: +0.00178, −0.00199; J22 alone ±0.00170; Soop: at most ~0.002); over 75°E, 255°E (105°W) and 165°E (unstable) under 2e-4 (the Sun and the Moon's).
- **Sun-synchronous:** 700 km at the J2 inclination (98.19°) precesses 0.9811°/day against the Sun's 0.9856.
- **Mars's frozen orbit:** MRO's 255 × 320 km polar orbit started at J3's frozen eccentricity with its periapsis over the south pole stays near it (its mean eccentricity vector moves 0.28 e in ten days) where one a quarter turn away circulates (1.43 e).
- **Invariance:** a low lunar orbit (50 km, polar, degree 100) anchored at the Moon and at the Earth–Moon barycentre agrees to 6 cm over 4 h (the barycentre's precision at 3.8e8 m), and chunked = single pass bit for bit; LEO anchored at Earth and at the barycentre, the cut-against-full-model year, the perturbed coast's anchors (7e-5 m) and the golden coast all pass with the fields.

### Cost

Coasts (DOP853 at the coasts' tolerance, the step cap), release, M2 Pro shared with other agents (load 5–7), `examples/bench_gravity` before the fields (8943203) and now:

| Orbit (fields included) | Before: µs a step × steps a day | Now | Per simulated day |
|---|---|---|---|
| LEO 400 km (Earth 30, Moon 2) | 10.6 × 215 | 26.5 × 303 | 2.3 → 8.0 ms |
| LEO 200 km | 12.9 × 225 | 28.9 × 318 | 2.9 → 9.2 ms |
| GEO (Earth 10, Moon 2) | 11.0 × 19 | 16.4 × 19 | 0.21 → 0.31 ms |
| Low lunar orbit, 100 km (Moon 100, Earth 3) | 13.5 × 143 | 122 × 823 | 1.9 → 100 ms |
| Low lunar orbit, 50 km | 12.3 × 149 | 137 × 1,101 | 1.8 → 151 ms |
| Mars 250 km (Mars 50) | 8.8 × 159 | 43.6 × 441 | 1.4 → 19 ms |
| Heliocentric, 1.3 AU (none) | 8.5 × 8 | 9.2 × 8 | unchanged |

One evaluation costs ~1.6 ns a term (`(N+1)(N+2)/2` terms): Earth's degree 30 ~1.2 µs, the Moon's 100 ~8.4 µs, 150 ~17 µs. **Budgets at 1,000,000× (D081)**, 11.6 simulated days a real second: a LEO craft needs ~93 ms of compute a second (27 before), so 50 of them ~4.6 s on the 7 compute threads, within D081's reach; a craft in a 100 km lunar orbit ~1.2 s, and one vessel is advanced by one thread at a time, so with ~8 ms of a frame for it the clock holds at ~4×10⁵× while one is there (shown as limited by compute). Raising Earth to 60 (8 m a day at 400 km) would cost ~4× in LEO; the Moon at 150 (5 m a day at 50 km) ~2.7× in lunar orbit. The coasts' work budgets charge the fields (`forces::field_units`: ~1.6 ns a term, 12 evaluations a step; ~100 units a step in a low lunar orbit, against 15 for a plain step), so a frame's time stays bounded when a vessel is there. `bench_fleet` (11 test craft in LEO): the look-ahead 485 → 1,250 ms per 1,000,000 s; the game's frame loop at 1,000,000× is bound by the thermal lattices either way (~296,000× reached, 8.5 → 11.3 ms a frame).

## Choices

- **Planets stay power-of-two, raw.** Free segment lengths fit closer to the tolerance, and a fit's velocity jumps at segment boundaries in proportion to its error: a year anchored at the Sun drifted 4.8 km from the same coast anchored at the barycenter (19 m before). The planets are the game's anchors (Earth, the Moon), so they keep the old settings. Moon tables are not anchors yet (no anchor zones: a vessel near Io is stored relative to the barycenter of the Solar System), so they can be free and packed. **Before giving moons anchor zones, their fits need C¹ continuity at boundaries** (position and velocity matched at segment ends) or a tighter tolerance.
- **J4/J6 sign:** first written with the opposite sign of J2 (Io drifted 2,000 km a year against Horizons); fixed and tested against one finite-difference potential for J2–J6.
- **Body directory names** are lower-case letters and digits (`Hi'iaka` → `hiiaka`): `sim::body::data::body_dir_name`, which the game now calls instead of its own copy.

## Tests

- `sim/tests/sol_ephemeris.rs`: planets and every moon system equal regeneration bit for bit; DE440 and Horizons bounds (a drift rate per body); the 50-year window; all 44 bodies with physical data; the file under 20 MB.
- `sim/tests/ephemeris_generation.rs`: a planet–moon pair in its barycenter's frame under the star's tide matches one full integration to 0.3 m (golden hash of the packed output: tides, J2–J6, free segments, quantization).
- `sim::gravity::zonal`: the generator's J2–J6 against the potential and against the field model; `sim::ephem::io`: packed tables round-trip; `sim::ephem`: lazy snapshots equal eager ones; `sim::world`: clusters (roles, opening, point-mass error under 1e-14 m/s²).
- Updated for the new data: the golden LEO coast hash; the gravity-source tests (dwarf planets massless in the cut-versus-full test, Ceres sitting just under the cutoff there; 5 m for the closed systems' quadrupoles; the Pluto flyby's error ratio 2e-3); the playtest save re-stamped to the new ephemeris id.

- `sim::gravity` (unit) and `sim/tests/gravity_fields.rs`: the evaluation against pyshtools and mpmath, the gradient of the potential, the bound, the shipped files against the published ones, the subsatellites, GEO drift, sun-synchronous precession, Mars's frozen orbit, a low lunar orbit's anchor and chunking invariance (the frozen-inclination sweep is `--ignored`: 34 s in release).

## The 1,000-year problem (for the owner)

D005 asks for about 1,000 years; the shipped window is 2030–2080. Nothing here is decided.

**Why it does not scale.** A moon's table costs what its orbits do: about 250 bytes per orbit for an inner moon, nearly whatever the tolerance (Chebyshev convergence is factorial, so 100 m → 1 km saves ~15 %; quantization is bounded by the anchors' smoothness). Measured for 50 years: planets 2.6 MB, dwarf planets 0.15 MB, moons 16.6 MB. Scaled to 1,000 years: **planets ~52 MB, moons ~330 MB** (Mimas alone 2.3 MB per 50 years). Generation is 5 minutes for 50 years, so ~1.7 hours for 1,000 (one thread; the systems are independent, so ~25 minutes on 10 cores).

**Options:**

| | What | Size shipped | Accuracy / consistency | Cost and risk |
|---|---|---|---|---|
| A | Tables for the first N decades, rails + drift beyond | today's 19 MB for N = 5 | beyond N, moons are off our integration by their rails residual: 2 km (Phoebe) to ~4,000 km (Iapetus); a vessel near Io would feel a fictitious ~7e-4 m/s² (400 km at Io's frequency): low orbits around moons stop being trustworthy after year N | cheap; planets could stay tables (52 MB) or go rails (residuals 30 km–4,000,000 km, far over D042's 1 km) |
| B | Moon tables generated on demand per decade, deterministically, from shipped checkpoints | ~1 MB of checkpoints (each system's exact integrator state, compensated sums included, at every decade boundary) + the planets | identical to one 1,000-year integration (resuming from an exact state equals a single pass, as coasts do); bit-exact on both OSes (golden-tested per decade) | ~1–2 minutes of background CPU per decade (Saturn's 10 years at 240 s steps), cached on disk (~33 MB per decade); at 1,000,000× a decade passes in 5 real minutes, so generation must run ahead; the planets' 1,000 years still need ~52 MB or the same treatment |
| C | A frequency-fitted model per moon (secular rates + a few hundred periodic terms from our integration, NAFF-style), expanded to tables at load | a few MB | km-level against our integration, so the same (smaller) fictitious-force issue as A | research work: fitting resonant systems (Io–Europa–Ganymede, Titan–Hyperion) to D042's 100 m is not routine |
| D | Ship all 1,000 years of tables | ~385 MB (or a separate download) | exact | far over D046's ~60 MB; every regeneration adds that much to git history unless kept out of the repository |
| E | A looser D042 for moons (1 km), with entropy coding | ~250 MB | 1 km | saves little (see above) |

**Notes for deciding:** B keeps every rule (one integration, pure function of time, consistent forces) at the price of a background job and a disk cache; A and C trade consistency near moons for size; D only trades size. The planets alone (52 MB) are already near D046's total; with C¹ (velocity-continuous) fits they could take the moons' free segments and quantization (~4× smaller), or the checkpoint scheme of B.

## Open

- Moons over 5 km (about 80 more): the fastest (Metis 7 h, Naiad 7 h) would cost ~12 MB each as 100 m tables and need 60 s steps. Needs a decision: rails or tables at a looser tolerance for small moons, or test particles.
- Sourced upper atmospheres of Venus and Mars (above). Gas giants: no surface or atmosphere model (vessels pass through).
- C¹ table fits (anchor zones for moons and planets beyond Earth; free segments for planets).
- 1,000 years: [the options above](#the-1000-year-problem-for-the-owner), for the owner.
- Gravity fields: the degrees are a budget decision (Earth 60, the Moon 150 or more for orbits under 50 km; [Cost](#cost)); the ephemeris generator keeps `body.ron`'s J2–J6 (regenerating the moon systems with the fields' zonals would align them); Earth's rotation model lacks nutation, polar motion and UT1 (0.09° in longitude in 2030); tides, Mars's seasonal J3 and the Love-number terms are not modelled; Uranus's and Neptune's J4, the mid-sized Saturnian moons' and Vesta's fields are not shipped.
