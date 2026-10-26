# Owner interview, 2026-10-26 (before an overnight run)

The owner judged the 2026-10-25 builds ("Looks great") and asked for a night of work on something well defined: "multi craft optimization/foundations, full solar system creation, in general optimizations needed to make this game maximally stable with dozens of crafts flying each with hundreds of parts. Also maybe fine tuning our final aero/aero heating model. Don't want to dive into craft creation yet because I need to guide that." The lead asked ten questions. Decisions from it: D081–D085. The plan: [overnight-2026-10-26.md](overnight-2026-10-26.md).

## Answers

| # | Question | Owner's answer |
|---|---|---|
| 1 | Main track | Scale foundations first, then aero/heating; the full Solar System in the background (in parallel). |
| 2 | Scale target | 50 craft, ~300 parts each, "but up to 1 million timewarp". "The 10 live craft wouldn't really be in atmosphere exactly and thus not in full physics simulation, I would say only 1-3 craft at low timewarp are needed as the physics warp capability, but absolutely ensure 10+ craft can be in a limbo state where there are constant forces acting on them and their internal systems are evolving according to extrapolated models that DO NOT change behavior as timewarp changes." |
| 3 | Craft in live physics that you are not flying | Full fidelity, warp capped: rails warp is refused while any vessel is live, with the reason shown. |
| 4 | Debris | "Debris can be deleted IFF it is totally uncontrollable and dead, and once it goes 10+ km from a controllable craft. Until then full simulation (physics simulation when crafts get within a certain distance for docking BTW)." |
| 5 | Stress-test craft | Yes: a synthetic generator for benchmarks and tests only. "You may assign placeholder properties to parts as well to confirm high timewarp behavior equivalence." |
| 6 | Proximity | A 10 km bubble, with craft-to-craft collisions. |
| 7 | Limbo effects, warp-independent | All four: continuous low thrust, upper-atmosphere drag, radiation pressure, systems evolving. |
| 8 | Solar System scope | The D014 rule, moons over 100 km radius first (planets, dwarf planets, ~30 bodies); smaller moons after. |
| 9 | Aero first | Close the D070 model gaps. "Also ensure you are modeling things at the fidelity I want, full body aero, basic turbulence effects, while staying fast enough for 1-2 crafts (or far more smaller ones on breakup)." |
| 10 | The machine overnight | Full use: all cores, offscreen demos, parallel agents on separate files; one game process at a time; try backhouse at the end. |

## Read by the lead (to confirm in the morning)

- "Basic turbulence effects" is read first as **boundary-layer turbulence**: laminar–turbulent transition and its effect on skin friction and heating over the whole body. Atmospheric turbulence (gusts) needs winds, which Earth does not have yet; it is noted, not built.
- The 10 km bubble is read as: any craft within 10 km of a controllable craft that is itself in live physics is simulated live with it, in a shared local frame, and craft collide.
