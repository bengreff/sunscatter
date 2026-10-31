# Landing guidance: the booster's return to a landing zone

**Status: design** (RESEARCH, 2026-10-30, at 835f839). Nothing here is built. Decisions: D117 (the booster flies itself back), D123 (convex landing guidance with a fixed-iteration solver, predictor–corrector for the boostback and entry burns), D126 (a switch), D127 (bit-identical), D128 (layer 2), D129 (the same at every warp), D043 (later WASM). The system: [avionics](../systems/avionics.md); the layer: [software-layers.md](software-layers.md) §6. Data: [falcon9.md](../vehicles/falcon9.md) §2.1, §4.4 and the rows tagged *RESEARCH* there.

**The pick, in one paragraph.** One layer-2 program, `ship_os::landing`, flies the booster from separation to touchdown by the commands a player issues (attitude holds, throttle, ignitions, fins, legs). The **boostback**, the **entry burn** and the **aerodynamic descent** are steered by one numerical **predictor–corrector**: the predictor is the sim's own landing prediction (`sim::landing`, the owner of predicted descents) given a burn plan, and the corrector is a Gauss–Newton step on the predicted miss, a fixed number of steps per wake. The **landing burn** is a second-order cone program by **lossless convexification** (Açıkmeşe & Ploen 2007; pointing by Açıkmeşe, Carson & Blackmore 2013), with drag carried as a known acceleration from the previous plan, the landing error and the fuel in one exactly-penalised objective (equal to Blackmore, Açıkmeşe & Scharf 2010's two steps when the target is reachable), re-solved every 0.2 s by a **primal–dual interior-point solver in pure Rust** (ECOS's algorithm) with a **fixed iteration cap and a fixed-pattern, pivot-free banded factorisation**. Estimated cost: 5–15 ms per solve on one M2 Pro core.

## 1. What the real booster does

| Fact | Value | Source |
|---|---|---|
| Phases | boostback ("sends the rocket shooting back toward the launch pad"), entry, "endo-atmospheric control with fins", landing burn ("the last opportunity to reduce the dispersions, and requires the ability to divert") | Blackmore 2016 (P) |
| Guidance | "SpaceX uses CVXGEN to generate customized flight code, which enables very high-speed onboard convex optimization" | Blackmore 2016 (P) |
| Accuracy | landing dispersions of 10 m or better on a droneship, 30 m or better at Cape Canaveral | Blackmore 2016 (P) |
| Hover | "the rocket has to hit zero velocity at exactly zero altitude": one Merlin at 40 % (338 kN at sea level) exceeds the empty stage's weight | Blackmore 2016 (P); falcon9.md §4.4 (D) |
| A Crew RTLS (Crew-13, 1 Oct 2026, B1101 to LZ-40) | MECO T+2:24; separation 2:27; flip 2:31; boostback 2:41–3:27 (46 s); entry burn 6:20–6:32 (12 s); landing burn 7:24; landing 7:41 (17 s) | SpaceX's timeline via spacelaunchschedule.com; NASA blog, 1 Oct 2026 (P) |
| Engines per burn (RTLS) | boostback 3, entry 3, landing 1 ("every RTLS landing has used only a single engine") | falcon9.md §4.4 [S24]; spacelaunchschedule.com (E) |

The rows added to `falcon9.md` §4.4 derive the propellant these burns take (~55 t) and the relights they need.

## 2. The program and its steps

`ship_os::landing` is a layer-2 program on the API of [software-layers.md](software-layers.md) §6: `View` in (the navigation solution in the body-fixed frame, the propellant sensors, the engines' readouts, the fins' and legs' states), `Commands` out. It never reads the truth, debug mode included (D123). It wakes at fixed epochs from its own epoch: every **1 s** before the landing burn and every **0.2 s** (10 live ticks) during it (E: "several times a second", D123). Each wake stops the vessel's advance like an order, so it acts at the same epoch at every warp (D129). Its state (step, last plan, the corrector's Jacobian) is saved with the vessel.

Steps: *coast-out* (separation, cold-gas flip to the boostback attitude) → *boostback* (3 engines) → *coast* (fins deployed, flip to engines-first) → *entry burn* (3 engines) → *aero descent* (fins) → *landing burn* (1 engine; 3-then-1 when 1 cannot) → *touchdown* (engine cut on contact, legs) → *safe*. The player's switch (D126) runs it by itself, on demand, or not at all; conflicting input hands it to manual, as the maneuver program does (`systems::maneuver::conflicts`).

## 3. The predictor (shared by every phase, and by Dragon's entry)

- **Model:** `sim::landing::model` ([landing-prediction.md](landing-prediction.md)) already integrates the vessel's own force model to the ground: N-body gravity with J2, drag and lift from the aero bake at the real Mach and Knudsen number, thrust at ambient pressure with the mass falling. It becomes the guidance's predictor by taking a **burn plan** as input (a list of segments: an attitude law, a throttle, engines lit, a cutoff rule) instead of today's constant throttle and assumed attitude. One owner (rule 5); no second integrator.
- **Outputs:** the impact point in the body-fixed frame, its epoch, the state at a gate altitude (the landing burn's handoff, ~5 km E), the propellant used, the peak dynamic pressure and stagnation heating on the way.
- **Cost:** the prediction's adaptive integrator evaluates the cell-by-cell aero forces (~10 µs per evaluation for 512 cells, aero-thermal.md), so a 5-minute descent of a few hundred steps costs ~10–30 ms (E, to measure). The corrector needs 1–4 predictions per wake at 1 Hz, so ~2–10 % of a core during the return. If that is too much, the predictor reads the bake's per-direction coefficient table (the craft's own aero data, D125) instead of the cells; the loop closes the difference. Measured first (open point 2).
- **Determinism:** the integrator is deterministic (DOPRI with `sim::math`, fixed tolerance); its step count is data-dependent, but every machine takes the same steps. It is capped at a fixed maximum, which also bounds the program's budget (D043).

## 4. Boostback

- **Unknowns:** the thrust direction (azimuth and elevation in the local horizontal frame) and the cutoff.
- **Rule:** velocity-to-be-gained steering (Battin's Q-guidance family). The corrector finds the **required velocity** v_req at the current position for which the predicted impact, after the planned entry burn and an unpowered aero descent at zero angle of attack, is the landing zone. The burn points along v_go = v_req − v and cuts off when |v_go| reaches the tail-off lead. That reuses the maneuver program's end-of-burn logic (`maneuver::last_ticks`).
- **Corrector:** the miss is 2-D (downrange, crossrange) and v_req has 3 components. Gauss–Newton with the **least-norm step** on the 2×3 Jacobian gives the smallest velocity change that removes the miss. An elevation bound keeps the arc's apogee for the entry. The Jacobian comes from finite differences (3 extra predictions) at the start of the burn, then from a Broyden update at each wake (1 prediction per wake). Two corrector steps per wake; convergence accumulates over wakes, deterministically.
- **Engines:** three (E1, E5 and E9, the ones with TEA-TEB for relights, falcon9.md §2.1), full throttle.
- **Reserve check:** each prediction also returns the propellant the entry and landing burns will need. If the landing zone cannot be reached without spending that reserve, the program aims at the reachable point nearest the target over the sea (the landing burn's minimum-error idea, §7.1, applied early) and notifies.

## 5. Entry burn

- **Purpose:** to limit heating and dynamic pressure in the descent. On Crew-13 it lasted 12 s on three engines, from about 70 km down to about 40 km (E).
- **Trigger and cutoff from the predictor:** before ignition, each wake predicts the unpowered descent's peak dynamic pressure and stagnation heating. It ignites at the last wake that still keeps both under the stage's limits after a burn, and cuts off when the predicted peaks of the rest of the descent fall under them. The limits are the stage's parameters (D125; U for the Falcon 9 → propose a peak q of 60 kPa and a peak stagnation heating of 30 W/cm², E, to tune against the Crew-13 durations).
- **Pointing:** retrograde, plus a small tilt (≤ 5°, E) chosen by the same corrector to trim the predicted miss.

## 6. Aerodynamic descent (grid fins)

- **Command:** the attitude relative to the air (an angle of attack and the plane it is in), so the body's lift and the fins' force move the impact point. The corrector solves for that 2-vector on the predicted miss, bounded by α_max (E 10°). SAS holds it: the fins through `systems::fins::allocate` (least squares within ±20°, 30°/s), the cold-gas thrusters high up. No fin-specific guidance: the fins are actuators of the attitude hold.
- **Authority:** not assumed. The lattice model (`aero::lattice`) gives the fins' force at each state. A test measures the divert footprint of this phase (the reachable impact points from a nominal entry-burn end state) and reports it.

## 7. Landing burn: the convex problem

### 7.1 Formulation

**Frame:** a surface frame at the target, fixed to the body (x up, y and z horizontal). Gravity is constant at the site; the frame's rotation adds the Coriolis and centrifugal terms, both affine in r and v (kept, as Açıkmeşe & Ploen do). Over a 20–30 s burn below ~5 km, the gravity's change with height (≤ 0.2 %) is left to the closed loop.

**Variables at nodes k = 0…N:** position r_k, velocity v_k, z_k = ln m_k, acceleration from thrust u_k = T_k/m_k, slack σ_k ≥ ‖u_k‖ (11 per node).

**Dynamics** (first-order hold between nodes, Δt = t_go/N): r and v from u_k + g + a_D,k + the rotation terms; mass from ż = −α·σ − β·p_k·e^{−z}, with α = 1/v_e,vac and β = n·A_e/v_e,vac. The second term is the back-pressure: the engine model's F = τ·F_vac − p·A_e takes a flow of (F + p·A_e)/v_e,vac (engines.md). p_k comes from the predicted height at node k and e^{−z} is linearised about the reference mass path as in the thrust bounds, so the term stays affine.

**Drag** a_D,k = −½ρ(h_k)·C_D A·|v_k|·v_k / m_k is not convex in v. It enters as a **known acceleration** evaluated on the previous plan's trajectory, with the drag area from the bake at the plan's Mach. Each re-solve updates it, so the guidance loop itself is the successive approximation. Simplício, Marcos & Bennani (2019) and Chen et al. (2023) iterate this inside a solve (successive convexification); at 5 Hz the re-solves do it, and while the drag share is large (just after ignition) a second pass per wake is allowed (cost ×2).

**Thrust bounds (lossless convexification):** ‖u_k‖ ≤ σ_k; ρ1,k·e^{−z_k} ≤ σ_k ≤ ρ2,k·e^{−z_k}, with ρ1,k = n(τ_min·F_vac − p_k A_e) and ρ2,k = n(F_vac − p_k A_e)·(1 − margin). e^{−z} is expanded about the reference mass path z0_k: the lower bound to second order (a cone), the upper to first order (linear), with z0_k ≤ z_k ≤ ln(m_0 − α ρ1 t_k), as in the 2007 paper. The **planning margin** (E 10 %) keeps throttle authority for the loop; it is a parameter of the program, not of the engine.

**Pointing:** n̂ᵀu_k ≥ cos θ_max·σ_k, lossless under Açıkmeşe, Carson & Blackmore (2013). θ_max is a parameter (E 20° from vertical), tightened over the last 3 s (E 3°) so the stage arrives upright. A per-node bound must be checked for tightness by test 3 (§10).

**Glide slope:** ‖(r_k − r_f)_horizontal‖ ≤ (r_k − r_f)_up / tan γ_min, a cone above the target (γ_min E 30°), so the approach never dips toward the ground short of the pad.

**Terminal:** r_N = target (at the legs' height), v_N = (−v_td, 0, 0), with v_td E 1.5 m/s (B1038 touched down at 1.47 m/s vertical, 0.15 m/s lateral, E). z_N ≥ ln m_dry.

**Objective:** Blackmore et al. (2010) solve twice: first the **minimum landing error** (the closest reachable point), then the **minimum fuel** to land within that error. Here both go into one solve with an **exact penalty**: minimise w·‖r_N,h − r_target‖ − z_N, with a norm, not its square. For w above the terminal constraint's multiplier (fuel per metre of miss, bounded), the solution is the two-step one whenever the target is reachable, and the nearest reachable point when it is not. One solve per wake, fixed cost. The two-step version stays as the test oracle (§10, test 7).

**Final time:** t_f is not convex. Açıkmeşe & Ploen line-search it (a golden-section search, one solve per probe). Here the search is **spread over wakes**: each wake solves at t_go carried from the last plan, and every third wake also at t_go ± δ (δ shrinking), keeping the best. For a stage that cannot hover the feasible window is narrow once lit, so t_go from the last plan is nearly right. The ignition decision does the wide search (§7.2).

**Discretisation:** N = 30 nodes (E), Δt = t_go/N down to 0.1 s, then N shrinks (the KKT pattern for each N is fixed). In the last second the last plan is held without re-solving (the open reproduction stops at 0.25 s).

### 7.2 Ignition, the start transient, and 1 or 3 engines

- **Ignition:** from the aero descent, each wake predicts the state one wake plus the engine's start time ahead (Merlin: 2 s to full thrust, falcon9.md §2.1) and solves the problem from there with the margin. It ignites at the last wake whose successor would be infeasible: the latest light that lands, which is the least fuel for a stage that cannot hover. The wide t_f search happens here, by golden section over several wakes, while the margin is still large.
- **Start transient:** the engine's ramp is not controllable, so it is not in the cone constraints. The first nodes carry the ramp as a known thrust along the current attitude, and the plan's free thrust begins where the ramp ends. The cones then hold with ρ1 > 0 throughout, which the losslessness result needs.
- **Engine count:** a discrete choice made before ignition. The program solves with one engine. If the minimum error is not zero, it solves with three engines for the first nodes and one for the rest (the per-node ρ bounds change at the shutdown node), and takes the first that lands. Every RTLS so far used one engine (E).

### 7.3 Between solves, and touchdown

- **Commands:** the thrust command is the current node's u × the estimated mass. Its direction goes to the attitude hold (SAS points the stage; the gimbals trim, engines.md) and its magnitude to the throttle. The plan is applied from the wake that computed it: compute time is zero in sim time, and its cost is charged to the budget (D043). Modelling the latency (solving from the state predicted one wake ahead) is an option.
- **Legs** deploy at t_go = 10 s (E; "legs deploy ~10 s into the burn", falcon9.md §4.4).
- **Touchdown:** the engine is cut on the first leg contact (the leg's stroke sensor) or when the navigation height falls under the legs' length plus 0.5 m (E).

## 8. The solver

### 8.1 Options

| Option | How it works | Accuracy and robustness | Determinism and cost | Verdict |
|---|---|---|---|---|
| **Primal–dual interior point** (ECOS: Domahidi, Chu & Boyd 2013; CVXOPT's algorithm) | Mehrotra predictor–corrector, Nesterov–Todd scaling, homogeneous self-dual embedding; one quasi-definite KKT factorisation per iteration | 1e-8 in 10–30 iterations; insensitive to scaling once nondimensionalised; certifies infeasibility | ECOS: AMD ordering fixed at setup, no numerical pivoting, dynamic regularisation (ε ≈ 1e-14, δ ≈ 1e-7) and iterative refinement, "virtually no variation in run time", 750 lines of C. Flight heritage: BSOCP/G-FOLD on Masten's Xombie (Dueri et al. 2017; the 750 m divert of 2013); CVXGEN-style code at SpaceX | **Picked** |
| CVXGEN (Mattingley & Boyd 2012) | the same IPM family, generated as flat C per problem; default 25 iterations then exits "regardless of the quality of the point" | QPs only (SpaceX's use suggests a QP form of the problem, e.g. polyhedral cones) | "almost branch free", fixed symbolic LDLᵀ with regularisation | its design rules are taken (fixed pattern, fixed cap, regularise instead of pivoting); the generator is closed and QP-only |
| First-order PIPG / xPIPG (Yu, Elango, Açıkmeşe et al. 2022; Li et al. 2026) | projected gradient with a proportional–integral dual update; projections onto cones and balls in closed form | ~1e-3–1e-4 in a few hundred iterations; needs preconditioning; weak infeasibility detection | very cheap: < 5 ms for ~25 nodes (Elango et al.); xPIPG with condensed dynamics, drag and free t_f: 374 µs median, 512 µs p99 (i7-10875H, single precision) against ECOS's 25 ms on the same problem | the upgrade path if the budget bites |
| ADMM conic (SCS) | splitting with one factorisation | like first-order; the factorisation changes with Δt each wake | fixed iterations possible | rejected: no gain over PIPG here |
| ECOS or Clarabel as a library | ECOS is C; Clarabel is a pure-Rust IPM (Goulart & Chen 2024) with crate dependencies and tolerance-based exit | — | a dependency (D123: none), not WASM-portable as is | **reference only**: golden numbers for tests are computed offline with them and committed |

**Why the interior point, not PIPG:** lossless convexification guarantees ‖u‖ = σ (thrust on its bounds) only **at the optimum**. A loosely converged first-order answer can sit with ‖u‖ < σ: a plan whose real thrust is below the minimum throttle, which is exactly the failure a stage that cannot hover must not have. The IPM's accuracy keeps the relaxation tight, its certificates drive the minimum-error fallback, and its cost is bounded by its iteration cap. The problem interface (the matrices and cones) stays solver-agnostic, so PIPG can replace it on a small ship computer (D043's budget) later.

### 8.2 The solver's design (`ship_os::landing::socp`, or a shared `sim::socp` if Dragon's docking wants it)

- **Form:** min cᵀx s.t. Ax = b, Gx + s = h, s ∈ ℝ₊ᵐ × SOC₁ × … (ECOS's form). Homogeneous embedding (τ, κ) for infeasibility detection.
- **Scaling:** nondimensionalised (lengths by the initial height, time by √(h₀/g), mass by the initial mass). An IPM's accuracy depends on it.
- **KKT:** nodes in time order give a block-banded quasi-definite matrix. Eliminating each node's cone block (diagonal plus rank one per cone, as ECOS does) leaves a block-tridiagonal system of ~18-wide blocks. Its LDLᵀ has a fixed elimination order for each N and no pivoting, with static and dynamic regularisation and one or two refinement steps as in ECOS. The dynamic-regularisation test is a data-dependent branch, but a deterministic one.
- **Iterations:** at most K = 30 (E; ECOS and CVXGEN typically converge in 10–25). It stops at convergence (gap and residuals < 1e-9) or at K and returns a status: converged, capped, infeasible.
- **Bit-identity (D127):** only +, −, ×, ÷ and √, all correctly rounded by IEEE 754 on every machine; the one exponential (e^{−z0}) through `sim::math`. No `mul_add`, no parallel or reordered sums, no f32, no reading of time or threads. In WASM later, the same: f64 arithmetic there is IEEE; only NaN bit patterns are nondeterministic (the WebAssembly spec's nondeterminism list; relaxed SIMD excluded), so a NaN anywhere counts as a failed solve and nothing else reads its bits.
- **Failure:** on *capped* or *infeasible*, the last plan, shifted in time, stays in force and the status is shown. A second failure in a row takes the minimum-error result. If that fails too, the stage holds a vertical descent at full throttle (a best-effort mode, E). All of it is logged.

### 8.3 Cost on the M2 Pro (estimate, to measure)

Per iteration: ~30 blocks × (an 18×18 factor plus the Schur update and off-diagonal solves) ≈ 1.4·10⁴ operations per node, so ~4·10⁵ per factorisation, plus the cone scalings and three triangular solves: ~5·10⁵ operations. Over 30 iterations that is ~1.5·10⁷ operations. Scalar f64 code of dependent chains runs ~1–3·10⁹ operations/s on one P-core, so **~5–15 ms per solve**: 2.5–7.5 % of one core at 5 Hz, for the ~20–30 s of the burn, on the booster's own worker (D127). For scale: CVXPY with Clarabel takes 20 ms at p99 for the 50-node Mars case (beriltatli/powered-descent, Python overhead included); ECOS takes 25 ms on the 31-node atmospheric problem (Li et al. 2026). A bench (`examples/bench_landing`) measures it.

## 9. Determinism and warp

The program's inputs are the `View` at its wake epochs. Those are the same bits for any chunking of frames (rule 4, [warp-invariance.md](warp-invariance.md)), so its outputs are too. The booster is live while thrusting or in air (by state, D129), so the guidance never runs at rails warp. Fixed caps (iterations, predictor steps, corrector steps) bound the compute per wake, and the budget charge counts operations, never measured time.

## 10. Tests that would prove it

Solver (unit, CI):
1. Small SOCPs with closed forms (an LP, a projection onto a cone, a QP as an SOCP) to 1e-9.
2. **Açıkmeşe & Ploen 2007, §V** (the Mars case: six 3.1 kN engines canted 27°, 30–80 % throttle, so ρ1 = 4,971.8 N and ρ2 = 13,258.2 N; initial state as in the paper). The paper's optimum is t_f 81 s and 399.5 kg of fuel; an open reproduction gets 79.2 s and 403.6 kg at N = 50. Fuel within 1.5 % of 399.5 kg.
3. **Tightness:** at the optimum ‖u_k‖ = σ_k to 1e-6 relative at every node, and the thrust within [ρ1, ρ2].
4. **Infeasible target:** the solver returns the nearest reachable point, equal to the two-step oracle; infeasibility of the fuel step is certified.
5. **Bits:** the same inputs give the same plan twice, and a committed golden hash; CI compares macOS and Windows (D127).
6. **Cap:** over 1,000 seeded starts drawn from the entry-burn-end dispersion, every solve converges within K. The iteration histogram is reported.
7. **Exact penalty = two-step** on feasible cases, to 1e-6.

Closed loop (sim, the Falcon 9 booster of parts):

8. From seeded states at the 5 km gate (±300 m, ±20 m/s, mass ±3 %, thrust ±2 %): every landing ends within 10 m of the target (Blackmore's droneship figure), at ≤ 2 m/s vertical and ≤ 0.5 m/s horizontal, tilted ≤ 3°, never below the throttle floor (E thresholds).
9. **Warp:** the whole return flown with different frame sizes gives the same bits (as `tests/warp_course.rs`).
10. **The Crew-13 return:** from the separation state of a full ascent, it lands at LZ-40 with propellant left; the burn durations are reported against 46 s, 12 s and 17 s (within ±30 %, E: the sim's ascent and the stage's limits differ).
11. **Mission test** (D122): the RTLS end to end, headless through MCP, with shots.

## 11. Prior work: what was taken and what was rejected

| Source | Taken | Rejected |
|---|---|---|
| Açıkmeşe & Ploen, *Convex programming approach to powered descent guidance for Mars landing*, JGCD 30(5), 2007 | the formulation, the convexified thrust bounds, the rotation terms, the t_f line search (spread over wakes) | a probe solve per t_f point every cycle |
| Blackmore, Açıkmeşe & Scharf, *Minimum-landing-error powered-descent guidance…*, JGCD 33(4), 2010 | minimum error, then minimum fuel (here as one exact-penalty solve) | two solves per wake |
| Açıkmeşe, Carson & Blackmore, *Lossless convexification of nonconvex control bound and pointing constraints…*, IEEE TCST 21(6), 2013 | the pointing cone | — |
| Blackmore, *Autonomous Precision Landing of Space Rockets*, The Bridge 46(4), 2016 | the phases, the accuracy targets, the CVXGEN design rules | — |
| Dueri, Açıkmeşe, Scharf & Harris, *Customized real-time interior-point methods for onboard powered-descent guidance*, JGCD 40(2), 2017; G-FOLD on Xombie (JPL/Masten, 2013) | a custom IPM exploiting the problem's structure | — |
| Domahidi, Chu & Boyd, *ECOS*, ECC 2013; Mattingley & Boyd, *CVXGEN*, Optim. Eng. 13, 2012 | the algorithm; the fixed ordering, regularisation and refinement; the cap | C, and dynamic orderings |
| Yu, Elango, Topcu & Açıkmeşe, *Proportional-integral projected gradient method for conic optimization*, Automatica 142, 2022; Elango et al., *A customized first-order solver for real-time powered-descent guidance*, AIAA SciTech 2022; Li et al., *Submillisecond sequential convex optimization for powered landing via dynamics condensation and xPIPG*, arXiv:2608.10395, 2026 | the upgrade path, condensation, the cost reference | as the first solver (accuracy at a fixed cap) |
| Chen et al., *A fast algorithm for onboard atmospheric powered descent guidance*, IEEE TAES 2023; Simplício, Marcos & Bennani, *Guidance of reusable launchers: improving descent and landing performance*, JGCD 42(10), 2019 | drag as a successively convexified term; boostback, entry and aero phases | many inner passes per wake |
| Malyuta et al., *Convex optimization for trajectory generation*, IEEE CSM 42(5), 2022 | discretisation and scaling practice | — |
| beriltatli/powered-descent (GitHub, CVXPY + Clarabel) | the 2007 case's numbers as the test oracle | — |
| MechJeb's landing autopilot (KSP) | impact prediction with drag, corrected by small burns | its background-thread simulation (timing-dependent, not deterministic) |
| kOS boostback scripts (KSP) | impact-point steering for the boostback | patched conics and spheres of influence (rule 1) |
| Lu, *Entry guidance: a unified method*, JGCD 37(3), 2014 | the numerical predictor–corrector pattern (shared with Dragon's entry, entry-ablation.md §2) | — |

## 12. Open for the owner

1. **Where the predictor runs** for a layer-2 program: as a call into the ship OS (one owner, the same bits, its operations charged to the program's budget), or compiled into the program (a second copy once programs are WASM). Recommended: a ship-OS call.
2. **The predictor's aerodynamics:** the cells (exact, ~10–30 ms per prediction, E) or the bake's coefficient table (fast, the software's own model of its craft). Recommended: measure first, then the table if needed.
3. **The guidance's limits as craft parameters (E until sourced):** the throttle margin 10 %, θ_max 20°, glide slope 30°, touchdown 1.5 m/s, the entry burn's q 60 kPa and heating 30 W/cm².
4. **An offshore aim point:** whether the stage aims short of land until the landing burn diverts it in. This is not sourced; it would be a parameter of the target.
5. **The solver:** an interior point (exact, ~5–15 ms) now, PIPG (sub-millisecond, approximate) later if the ship computer's budget needs it. Recommended as written.
