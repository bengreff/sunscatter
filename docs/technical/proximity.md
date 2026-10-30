# Feature: proximity (the 10 km bubble and craft-to-craft contact)

**Status:** playable in the sim, wired into the game (2026-10-26, overnight PROXIMITY workstream): craft near a live controllable craft fly live with it on one tick lattice and collide; tested for conservation, frame independence, save/load and precision. Not seen in the game yet (agents never run it): the lead's offscreen demo has no two craft near each other. No docking (D082: later). Decisions: D082 (the bubble and collisions), D083 (debris fully simulated near controllable craft), D081 (rails warp refused while a craft is live), D066 (rigid-body contact, impacts per point), D032 (parts break), D064 (debug mode), rules 1–4.

The owner (2026-10-26): "physics simulation when crafts get within a certain distance for docking", a 10 km bubble with craft-to-craft collisions. **The lead's reading, to be confirmed:** craft within 10 km of a controllable craft that is itself in live physics fly live with it, in a shared local frame, and collide; when that craft goes back to a coast, the bubble's members coast too. Coasting craft near each other stay on rails (rails warp near a station works when nobody thrusts). The rule's numbers and its leader test are one pure function (`proximity::rule`), so another reading is a small change.

## Prior work

| Source | What it does | Taken | Rejected |
|---|---|---|---|
| KSP physics range | Vessels within 2.5 km of the active one are loaded and simulated; others are on rails | A range around the player's craft | Loading by distance alone (KSP's "packed" vessels jitter and explode on unpacking); our members join on a tick lattice from their stored coasts |
| McNeely, Puterbaugh & Troy, *Six degree-of-freedom haptic rendering using voxel sampling* (SIGGRAPH 1999, "Voxmap PointShell") | Points sampled on one body's surface tested against the other's distance field, penalty forces per point | Point shells against signed distances, both ways | Voxel distance fields (memory, resolution); our solids are exact primitives |
| Catto, *Soft Constraints* (GDC 2011), Box2D v3's soft step | Contact stated as a frequency and damping ratio from the effective mass, stable for any mass | The damping ratio from the pair's reduced mass, the stiffness cap `m_red·(ω_max)²` | An iterative constraint solver (later, with docking ports and joints) |
| Our ground contact (D066, `sim::contact`) | Spring-damper and regularised Coulomb friction per point, impacts per point, a rest rule | The force law, `friction_force` with its one-substep cap, 2 ms substeps | — |

## Design

### 1. Who is in the bubble (`sim::proximity::rule`)

- A **leader** is controllable (D083: intact, a control part, power; `Vessel::controllable`) and live for its own reasons: it would not return to a coast now (thrust, air past the live boundary, a surface within its live height; `Vessel::would_coast`). A craft held live by the bubble does not lead, so two controllable craft cannot keep each other live, and nothing chains.
- A flying craft (live or coasting) is a **member** while within 10 km of a leader, or 10.5 km once a member (hysteresis). Landed and crashed craft stay frozen (not members).
- A member that touched another craft within the last 50 ticks (1 s) stays a member wherever the leader is: coasts do not collide, so a pair in contact must separate before going back on rails.
- `BubbleRule { enter: 10 km, leave: 10.5 km, hold_ticks: 50 }`; distances between centres of mass through the frame tree.

### 2. Stepping (`sim::vessel::bubble::advance`, before the fleet each frame)

- **Nothing to do** without a live controllable craft or a member: one pass over the fleet's phases. Limbo craft, stations and parked craft cost nothing.
- **Separate bubbles:** the craft that may come within reach of a leader or member this frame or the next, joined into connected sets (any two within reach of each other); each set is a bubble with its own lattice and frame, stepped on its own thread (bubbles share no craft, so threads change nothing). A leader alone is flown by the fleet.
- **The lattice** is the bubble's reference's (its live craft with the lowest vessel id): its time plus k·TICK, computed as each vessel computes its own time, so members' times agree bit for bit.
- **Candidates** are the flying craft that could come within 10.5 km of a leader or member during this frame or the next (distance less relative speed over the frame plus 1 s, less 5 g of acceleration, less 1 km). At every epoch each candidate is brought to it (a coast is evaluated there exactly), the rule decides, and joins and leaves happen at that epoch: a coast goes live there on its own anchor (nobody is re-anchored); a live craft on another lattice ticks to just before it and flies one shorter tick onto it; a member that leaves and would coast returns to a coast there. An engine starting near another craft goes live at the frame's start, so the new leader's neighbours join at its first tick.
- **Ticks:** a member whose bounding sphere is farther from every other member's than their relative motion over a tick (plus 0.5 m) flies its tick alone, exactly as outside a bubble. Members that may touch fly the tick together: each starts its tick alone (`begin_tick`: controls latched, parachutes, air, the attitude command; nothing but the attitude control changes until the tick commits, so an aborted tick restores only that), then 10 joint substeps in which every pair's contact forces at the substep's start are applied as impulses on velocity and spin (equal and opposite) before the drift, with ground contact, gravity, thrust and air as in a ground-contact tick, then each ends its tick alone (`finish_tick`: the parachutes' step, the grid fins' and the gimbal's hold, propellant, heating, limits, systems, rest). `vessel::live` was split into these stages (bit for bit for every vessel outside a bubble).
- **Breakups:** when a craft of a bubble breaks up (D032), the whole bubble waits at that tick's epoch (`BubbleState::waiting`: `Vessel::advance` returns at once) until the game applies the split; the pieces then join at that epoch. No craft flies on without the contact, whatever the frames.
- **Order:** members by index, pairs by index, points in each hull's tree order: no sum depends on threads or frames.

### 3. The shared frame and precision (rule 3)

Members keep their own anchors (live ticks and ground contact stay on the anchor their coast chose). Members on one anchor (the usual case) meet as the difference of their anchor-relative positions, their lowest common ancestor; members on different anchors meet through the frame tree (the anchors' relative state at each substep added). An anchor-relative position is rounded to its distance from the anchor at every tick: 1e-9 m in low orbit, but 3e-5 m at 1 AU from the barycentre and 1e-3 m at 30 AU, and the same rounding every tick adds up (60 s at 30 AU moved two members' separation by 0.42 m). So **a member's live ticks carry the low part of its position** (compensated summation through the tick's integration and the contact substeps; `BubbleState::lo`, saved), and contact bodies are placed relative to the first member's centre of mass with it: metres, not orbit radii. Measured after a minute, two members 10 km apart against the same pair coasting: 0.6 µm in low orbit, 14 µm at 1 AU, 0.10 mm at 30 AU. (A vessel anchor, motion-model §3's ad-hoc anchor, would make this exact everywhere; not needed at these numbers.)

### 4. Craft-to-craft contact (`sim::proximity::{hull, contact}`)

- **Hull** (built once per craft id from its definition, `proximity::hull_of`): the union of the geometry's convex primitives with exact signed distances (2D profile distance for frustums, cylinders and struts; arc and base disc for sphere caps; boxes). A point inside leaves through the nearest face, of every primitive holding it, whose outer side is not inside another primitive (a point under the cabin's joint goes out the side, not into the cabin; in a concave corner, the nearest face). **Sample points** on the outer surface at most 0.2 m apart (rims, caps and box edges included; points inside other primitives dropped), each with its area and part, in a tree of bounding spheres (median splits, 16 a leaf). The thermal cells are too coarse for contact (16 per part on a stress craft: two touching tanks could sink 20 cm between centroids). Test craft: ~3,000 points; a 300-part stress craft: 43,519 points, built in a few ms.
- **Where they touch:** every sample point of either craft inside the other's solid, both ways. Bounding spheres, then the point tree against the other hull's primitive boxes (a leaf's sphere against the solids), then the points. Once a tick the bubble finds each close pair's points within the tick's relative motion of the other craft (`near_points`: relative speed, rims turning, 1 m/s of speed change, 1 cm) and its substeps test only those (`pair_forces_near`; the same forces as the full search while the craft move less than the margin, tested).
- **Force** per touching point along the other solid's normal: `(k·depth + c·closing)⁺` with `k = 5e7 N/m per m² of the point's area` (stiffness grows with the touching area, not with the sampling), the pair's damping `2ζ√(K·m_red)` (ζ = 0.3) shared by stiffness, and regularised Coulomb friction (μ = √(μ_A μ_B) of the hulls' friction, the larger stick speed) capped to what one substep can stop at the point's effective mass. The total stiffness is capped at `m_red·(0.5/dt)²`: explicit substeps stay stable for any piece (a small one's contact is softer: it sinks v·dt/0.5, millimetres at docking speeds).
- **Equal and opposite:** B takes exactly `−F` at the same point; torques about each centre of mass satisfy `τ_A + τ_B + d × F_B = 0`. Contact impulses change spin exactly (`ΔL = τ·h` in inertial axes); the rotation between them is the usual RK4 tick.
- **Impacts (D066, per point):** a touching point closing faster than a craft's impact limit destroys it (one part) or the part that was hit (a breakup, D032; the vessel stops at that tick for `Vessel::split`). `Destruction::Impact { point: u32::MAX }` marks a hit by another craft (no ground contact point). Debug mode disables it (D064).
- **Rest:** a pair pressed together settles with no jitter (depth constant to 0.1 µm); a pair in contact stays live (§1). There is no merging: two craft resting against each other keep the bubble live until they separate (docking will join them into one vessel).

### 5. Warp (D081)

A craft held live by a bubble refuses rails warp like any live craft; for the active vessel the reason names the craft that holds it: `RailsBlock::Near(id)`, shown as "limited: near Vessel N" (`game::state`, `game::hud`).

## Numbers

| What | Result |
|---|---|
| Two test craft nose to nose at 0.5 m/s (free space) | contact 0.19 s, 20 mm deep, restitution 0.45; momentum changed by 4e-12 kg·m/s (of 1e4), angular momentum by 6e-14 |
| Off-centre hit (a fin's edge on the other's flared skirt) | spin exchanged; angular momentum to 1e-8 of 1.5e4 (8e-13 relative), energy lost |
| Pressed together at 0.02 m/s² | depth 0.0696 mm, constant to 1e-7 m; relative motion < 3e-5 m/s |
| In a bubble in low orbit, 0.5 m/s | they join at the first epoch, touch from 17.2 s, separate at 0.12 m/s, intact |
| 12 m/s (over the 8 m/s limit) | both test craft destroyed by impact; in debug mode neither; 8-part stress craft each lose the parts hit |
| A minute in a bubble without touching, against the craft's own coast | 0.72 mm at 400 km, 0.28 mm at 2,000 km: the live tick's force evaluation against the coast's (a vessel held live alone differs by the same bits; the coast against a bare coast by nothing measurable), a steady ~4e-7 m/s², not the bubble. With Earth's gravity field to degree 30 (D090), 1.12 mm at 400 km and 0.14 mm at 2,000 km |
| Relative position of two members 10 km apart after 60 s | 0.6 µm (low orbit), 14 µm (1 AU), 0.10 mm (30 AU) |
| Cost, two 300-part stress craft touching in low orbit plus the leader (`examples/bench_bubble`, release, the reference Mac loaded by other agents) | full pair search 0.40 ms; points within 3 cm (46 + 46) found in 0.47 ms once a tick; forces 5.9 µs a substep; **a frame of the three craft 1.26 ms per 20 ms tick while touching**, 0.64 ms apart: within the live tick budget at 1× and 4× |

## Tests (the specification)

- `proximity::rule`: membership table (enter, leave, hysteresis, landed craft, contact hold, leaders) and who gathers members (no chains, no landed leader).
- `proximity::hull`: signed distance against the inside test and the sampled surface for every primitive of the test craft and a stress craft; samples on the union surface, covering it within the spacing, areas within 10 %; exits at joints; the point tree.
- `proximity::contact`: equal and opposite at 12 poses; collide and separate (momentum, angular momentum, energy, restitution); off-centre hit; resting without jitter; near points equal the full search.
- `vessel::bubble`: the review's scenario (a leader in low orbit, a craft on the Moon's anchor drifting into its member at 2 m/s, an unrelated lower-id leader in lunar orbit: separate bubbles, nobody re-anchored, contact through the frame tree); two bubbles far apart advance exactly as each alone; a breakup in contact waits for the split, frames of 1/60 s and 1/7 s giving the same bits; collide and separate in a bubble; 1/60 s frames equal one call (bits, through joins and contact); save and load mid-contact continue bit for bit; a minute in a bubble against the coast; members return to coasts when the leader does, and the pass then touches nothing; relative position precision at 10 km; impacts (one part, parts, debug).
- `game::state`: a craft held in a bubble names its leader as the warp limit.

## Limits and what remains

- **Coasting craft pass through each other.** Without a live controllable craft nearby nobody collides (the lead's reading: rails warp near a station). A player drifting into a station with the engine off is on rails unless something is live. A coast–coast conjunction search on the stored trajectories (closest approach within a margin → both go live at that epoch) would close it; not built.
- **Thin air:** live ticks see no air above the atmosphere's live top (150 km on Earth), so a member above it flies without the thin-air drag and heating its coast would carry (limbo): ~1e-7 m/s² at 400 km, part of the 0.72 mm a minute above. Giving live ticks the coast's thin-air model is not built.
- **Landed craft are not members:** a lander touching down next to a parked rover does not collide with it.
- **Frame independence** is exact once the members are on the bubble's lattice. A leader that goes live inside a frame for a reason other than its engine (descending into air, a planned coast's end) takes its neighbours from the next frame's first epoch, which differs from the exact join by one frame of coast against live ticks (~1e-9 m): deterministic for the same frames, not bit-identical across frame rates. (A craft becomes a candidate a second of relative motion and 1 km before it can reach 10.5 km, so the fleet never carries a joining craft past its epoch.)
- **Point sampling** misses edge-on-edge contact between sample points (two thin struts crossing) until a point enters; sharp convex features penetrate up to ~5 mm before a point does. Thin parts can be tunnelled at speeds where a substep crosses half their thickness (the test craft's 8 cm fins at 20 m/s).
- **Cost:** every member flies live ticks (thermal network, systems), not only those in contact: ten 300-part craft near a thrusting one cost ten live ticks each frame (`docs/archive/research/scale-baseline-2026-10-26.md`). Members could stay on their coasts until a contact is possible (bit-exact invariance for free); that is a change to the reading, for the owner.
- **No merging or docking:** resting pairs stay live. Docking ports, the Docking menu (D080) and merging two vessels into one rigid body come later.
- The contact constants (`MODULUS`, `DAMPING_RATIO`, `OMEGA_STEP`, `SPACING`) are placeholders from the behaviour wanted (centimetre penetration at docking speeds), not from materials.
