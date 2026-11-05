# Interview 2026-11-05: game layer + its interactions with parts and science (owner + Director, live)

## Warp questions (from the warp spike, section 6)
1. Chattering thermostats/relief valves become "hold at the set point"; wear from a cycle law: YES.
2. Quiet craft on repeating orbits: orbits copied from a certified cycle behind the no-switch guard: YES.
3. Physics warp: "narrow the craft proximity warp drop to confirmed collision, padded to avoid glitchy rapid switching between warp allowed and not. Restrict the warp is correct ... 1x warp is all that is needed for full physics warp." (Director: so the real requirement is performance at 1x: today a 1,000-part craft costs 103 ms per 20 ms tick at 1x, about 5x slower than real time; this goes to PERF.)
4. Stochastic processes beyond fates (micrometeoroids, SEUs) in v0.2: YES, seeded by entity and epoch, each with a modelled cause.
5. Across code versions a better integrator may move events by seconds: YES, saves record their version.

## Game loop
6. Early-career evening: design a vehicle for a contract, test, fly.
7. Contracts: real-world-style specs + deadlines + penalties (customers post requirements; you bid price and date; late or failed delivery costs money and reputation).
8. Pace over 10,000 years: money + windows + world progress, "Plus the career overall is a sort of competition to accomplish the most, hit milestones first. You can always go slowly, but to fully step into the roleplay rapid progress is part of the game."
9. "First" is NOT a literal competition: "There are no milestones being achieved in the world. This is just a general philosophy to avoid the KSP meta of warping 100 years just for one mission. Instead, sunscatter has decisions and navigation at least monthly/daily in some cases, a VERY advanced alarm clock system, as well as advanced automation of what the player does not want to do manually (the AI assistant could do some things). Plus there could be a global leaderboard: earliest interstellar probe, etc."
10. Money sinks: ALL of hardware (parts, propellant); design programs + R&D; facilities; operations + people.
11. Testing: "no test stands. That is too much. All of that iteration is abstracted inside the design program. The design program is assumed to include tests. A design program could raise TRL, but some tests need real flights/could benefit from real flights. The 'benefit' is science points and TRL and maybe prestige though, not literal determination of stats except for integrated systems in a couple long term cases. A design program gives you all the performance stats WITHIN REASON."
12. Mission failure: real consequences + an investigation (penalties, insurance, reputation; the investigation uses only what your sensors recorded; a fix may need a delta design program; grounding until cleared).
## Science layer pacing (with the science-nodes draft, docs/design/foundation/science-nodes.md)
13. Node pacing: "Bundles are fine as a shortcut, still separated into nodes. However, I would guess that most of the SCIENCE nodes to achieve a small launcher would be already world-known. In addition, many components could be purchased. You maybe do 5-10 design programs for your first small launcher."
14. Size regimes: "I lean towards [per-class regimes], but I would need more info. Lots of classes could be fine. Also, often two size classes are NOT different part classes, they use the same logic, but there are separate science nodes unlocking the various sizes." (Director to bring the trade-off with examples.)
15. Planetary science (and parts of astronomy, life sciences, aero): global level + per-body sub-levels.
16. Proprietary 2030 tech: the node is world-known; designs are proprietary (buy parts or license by contract); nothing locked forever.
17. Size regimes, Director's working default (owner leaned per-class, wanted more info): a shared size-ladder rung is the base requirement, plus a class-specific size node only where the enabling technology really differs (large tank welding, large composite fairings, high-thrust engines). Estimated 60-90 size nodes rather than 133. Owner can overrule.
18. Alarm clock: ONE event queue everything posts to (craft scripts, burns, windows, contract deadlines, design programs, factory output, thresholds, comms passes, failures); warp runs to the next event you care about; filters, priorities, snooze, auto-handle by rule or by the AI; the same queue drives warp stops.
19. AI assistant: anything the player can do, through MCP parity, by permission scopes the player grants; everything logged.
20. Design programs: months to years by complexity x novelty x (1 - similarity); parallel programs limited by engineering staff (payroll), more staff shortens with diminishing returns.
21. Facilities: abstract buildings with capacities, cost, build time, upkeep and location; bought, built or rented.
(Session stopped here by the owner, 2026-11-05; game layer to be continued: economy numbers, contracts' customers, reputation, leaderboard.)
