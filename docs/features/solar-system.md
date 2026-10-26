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
- **Cheaper loops:** the gravity loop skips a closed system's members at once (`World::counted`), positions relative to the anchor reuse the anchor's walk to the root (`Snapshot::path_to`, bit-identical), clusters far from the Sun close by the bound above without a walk, and another body's J2 is applied only where its field's change over the anchor–vessel segment (`60 |J2| GM R² ρ / d⁵`) reaches a hundredth of the cutoff (the anchor's own always): far away the vessel and the anchor feel the same J2 pull, which the anchor's ephemeris leaves out too (the Moon's J2 at LEO: ~1e-19 m/s² tidal).

Golden LEO coast hash 0x52e3680dc0baea81 (the faint bodies and far J2 terms now left out).

## Choices

- **Planets stay power-of-two, raw.** Free segment lengths fit closer to the tolerance, and a fit's velocity jumps at segment boundaries in proportion to its error: a year anchored at the Sun drifted 4.8 km from the same coast anchored at the barycenter (19 m before). The planets are the game's anchors (Earth, the Moon), so they keep the old settings. Moon tables are not anchors yet (no anchor zones: a vessel near Io is stored relative to the barycenter of the Solar System), so they can be free and packed. **Before giving moons anchor zones, their fits need C¹ continuity at boundaries** (position and velocity matched at segment ends) or a tighter tolerance.
- **J4/J6 sign:** first written with the opposite sign of J2 (Io drifted 2,000 km a year against Horizons); fixed and tested against one finite-difference potential for J2–J6.
- **Body directory names** are lower-case letters and digits (`Hi'iaka` → `hiiaka`): `sim::body::data::body_dir_name`, which the game now calls instead of its own copy.

## Tests

- `sim/tests/sol_ephemeris.rs`: planets and every moon system equal regeneration bit for bit; DE440 and Horizons bounds (a drift rate per body); the 50-year window; all 44 bodies with physical data; the file under 20 MB.
- `sim/tests/ephemeris_generation.rs`: a planet–moon pair in its barycenter's frame under the star's tide matches one full integration to 0.3 m (golden hash of the packed output: tides, J2–J6, free segments, quantization).
- `sim::gen::nbody`: zonal terms against the potential; `sim::ephem::io`: packed tables round-trip; `sim::ephem`: lazy snapshots equal eager ones; `sim::world`: clusters (roles, opening, point-mass error under 1e-14 m/s²).
- Updated for the new data: the golden LEO coast hash; the gravity-source tests (dwarf planets massless in the cut-versus-full test, Ceres sitting just under the cutoff there; 5 m for the closed systems' quadrupoles; the Pluto flyby's error ratio 2e-3); the playtest save re-stamped to the new ephemeris id.

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
