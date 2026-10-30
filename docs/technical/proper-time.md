# Proper time

Each vessel's own clock (D012, D088). Extracted from the archived [realism-1](../archive/plans/realism-1.md) §4a and corrected from the code (2026-10-30). The system: [motion](../systems/motion.md).

**Status:** *playable* (`sim::relativity`, `sim::vessel::clock`; tests in `crates/sim/tests/proper_time.rs`).

## The law

- Each vessel carries δ = τ − t, its proper time minus the coordinate time t (TDB, the ephemeris' time scale). Storing the offset, not τ, keeps it precise (motion-model.md §3).
- A clock's rate against TCB is `1 − (U + v²/2)/c²` (weak field, slow motion), with `U = Σ GMᵢ/rᵢ > 0` over every gravity source and `v` the barycentric velocity, formed through the frame tree. The sign of U is the classic bug, and a test pins it.
- TDB runs at `1 − L_B` against TCB (IAU 2006 Resolution B3), so `dδ/dt = (L_B − (U + v²/2)/c²) / (1 − L_B)`. A clock on the geoid then keeps TT, which matches TDB on average: its δ has no drift, only TDB − TT's periodic terms. Without L_B every clock lost 1.34 ms a day against the game clock.
- U counts every source, cut from the forces or not: cutting is a force-precision policy, and a clock would otherwise jump by a source's whole potential when it is cut (point masses; Earth's J2 potential, ~0.1 %, is left out).
- δ is an extra component of the integrator's state (coasts, burns and live ticks), outside error control, with compensated summation, so trajectories and goldens do not change and chunked = single pass. Landed vessels integrate the same rate at their body-fixed position on a 600 s lattice (3-point Gauss–Legendre) from where they came to rest.

## Tests

- A GPS clock (20,200 km) gains 38.6 µs a day over one at the equator, within 1 %.
- A launch-site clock reads TT through a year, within 10 µs, over a swing of 3.3 ms.
- An ISS-like clock (400 km, 51.6°) loses about 24.7 µs a day against the site.
- Chunked = single pass, live ticks carry the clock, and a save and load keep it to the bit.

## Shown

The flight panel shows the ship clock and its offset from the game clock (the origin's, D088); MCP's `get_state` gives the active ship clock's offset. Special-relativistic motion (u = γv) is not needed at Earth–Moon speeds (γ − 1 < 10⁻⁹; the 1PN term is in the forces) and waits for fast ships.
