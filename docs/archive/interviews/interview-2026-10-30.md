# Interview 2026-10-30: simplifying the codebase, scale, testing, the first flights

Twenty-four questions from the lead, after F1–F3, D119 and D120 had landed. Part of the brief was "simplifying this codebase". Before the questions, the lead reviewed the code and the scale baseline (`scale-baseline-2026-10-26.md`). The owner's answers are given verbatim where they wrote their own. The decisions are D121–D124, and D081 is revised.

## The quick review the questions rest on

- **Code:** 110k lines of Rust (sim 65k, game 39k, tools 5k), 1,152 tests, and 181k words of docs (plans 71k, features 70k).
- **Scale (1f39836):** a live tick costs ~7.8 µs a part; a design (bake, volume grid, network) takes 2 s at 300 parts and 13.5 s at 1,000; a craft build 0.37 s and 2.5 s. The hot spots are:
  - the aero cells' boundary layer, 57–59 % of a live tick, evaluated 5–11× a tick;
  - the thermal network, 84 % of a coast's systems;
  - the bake, every triangle for each of 642 directions;
  - the geometry scans, n².
- **Bookkeeping that will not scale:** 46 linear id searches in the per-tick systems code and 63 string fields in the craft definitions.
- **Complexity:**
  - two kinds of craft, with ~270 places in the code telling them apart;
  - two state structs (`ShipState`, `Runtime`);
  - six execution modes, each subsystem a little different in each;
  - the engine rule duplicated (review F8).

## Answers

1. **The test craft:** delete it entirely (not rebuilt from parts).
2. **`ShipState` and `Runtime`:** merge them.
3. **Execution modes:** one law per subsystem for advancing over any interval; the modes only choose the interval.
4. **How to simplify:** "Do a quick review, then ask me FAR more specific questions with more detail. For example, the ship model needs to be INCREDIBLY scalable to various situations, including thousands of parts, with efficiency."
5. **The scale target:** both: a 5,000-part craft flown live, and 50 craft of 300 parts at 10⁶×.
6. **Live aero:** once per tick, held through the stages and substeps.
7. **New pieces:** "We don't need full fidelity for debris. Every time a craft is separated, we do a quick check: is this thing controlable (radio/crew/onboard computer, power). If not, simplify it to a lump until it gets deleted. If it is controlable, it becomes a new craft, fully simulated as inactive."
8. **Coast heat:** analytic relaxation toward the orbit-averaged equilibrium.
9. **Lumps:** ballistic (mass, inertia, drag area, heating, one heat node); deleted on burn-up or ground impact; lumps in orbit stay.
10. **Designs for controllable pieces:** precompute the planned pieces at craft build.
11. **Finished plans:** move them to `docs/archive/`. "Also consolidate architecture docs, we need a vision doc, and general docs for each of the main systems, plus more specific docs on technical details, plus a guide for reading in the readme."
12. **The 700-line cap:** "I never made this cap, you can remove it."
13. **The docs layout:** by system (the lead's reading of the owner's note), with `systems/` left open: "plus, keep in mind systems would have even more stuff such as colonies, economics, construction, fusion, various interstellar techs, etc, just leave that open."
14. **The live budget:** no cap.
15. **Live heat:** at the network's own rate.
16. **The ISS:** "It will be deorbited soon anyways, it will likely not even be in the game. One rigid body, placeholder test for docking for now."
17. **Landing guidance:** convex, as SpaceX does it.
18. **Water:** a calm sea first.
19. **Visuals:** frozen. "I don't know if we really need the last plume things."
20. **Testing:** "We just need the headless mission test with screenshots for large integrations of new features, plus maybe specific cases if needed. The game can run 100% autonomously after all with an AI agent driving. Tests should be real flight software plus an AI agent making decisions, not a scripted demo. Clean, ONE testing framework. Ensure you have unit tests too."
21. **Who decides in a mission test:** a development agent, through MCP.
22. **The demo, the tour and the `SUNSCATTER_*` variables:** replaced by MCP tools.
23. **An engine whose computers or controller are lost:** fail-safe shutdown (E).
24. **Debug mode and dead computers:** the computers still count.

## Taken by the lead from the owner's words

- **An abort needs escape thrust enough to get away:** "would actually save" (D120). An uncrewed Crew Dragon is offered it, since it saves the vehicle.

## The second twenty (the same day, after DOCS, MISSION and ONEKIND started)

Before these, the lead corrected an earlier example: Crew Dragon flies without a fairing, so a crew launch has no fairing halves. The decisions are D125–D127, plus D043, D034, D044, D115, D123 and D027 revised, and vision.md's v0.2.

25. **The booster's guidance and the ship OS:** "Why are we writing the ship OS In WASM? I just don't remember. There should be a single flight-computer layer." The lead explained D043's reasons: any language, sandboxed, deterministic, metered. See 29.
26. **The compute budget:** the real processors. "Also, every computer on the ship is summed to one abstracted operating system."
27. **The vehicle's flight software:** shipped with the craft.
28. **Uploads:** "This gets at a CRUCIAL razor: ships with a crewmember or (later) superintelligent AI system get to be instantly "controlled" by the player as if the player is right there on the ship. If the ship is autonomous scripts can run on it, but everything new comes from the nearest mission control (initially on earth, eventually on other planets)."
29. **The programs' format on the one layer:** all WASM, one path.
30. **Uncrewed ships are commanded from:** the nearest mission control.
31. **Planned burns for craft of parts:** right after ONEKIND.
32. **How a burn executes:** closed-loop to the target.
33. **Supersonic retropropulsion:** "We CANNOT have ship specific physics. I would say 2, clean independence, as this is an edge case anyways."
34. **Ablation:** "Once again, ablation must be a universal mechanism for all heat shields based on material etc. EVERY part must be FULLY described by a set of parameters. 1-D ablation."
35. **The parachutes:** "Real logic. However, keep in mind that that is the AUTOMATIC option. It ALSO must be possible for a human to drive this spacecraft, so for example a human presses deploy as a manual override and the function for the deploying sequence runs. Autopilot should be a switch and a config menu, plus a button you can press to automatically perform an action such as a planned burn or timewarp to a given point (increasing and decreasing timewarp, never jumping through time). Manual control is always an option with simplified (obviously) controls as exist today (auto engine vectoring for example), even if it leads to catastrophic failure (part of the fun of driving your own spacecraft)."
36. **Random failures in the first flights:** off until clean.
37. **Threads:** per vessel.
38. **The live tick rate:** one fixed rate.
39. **Across machines:** bit-identical, tested.
40. **A performance gate:** by hand.
41. **Checkouts:** the shared tree, as today.
42. **The agent cap:** 3 + the lead.
43. **What completes v0.2:** the two flights plus the scale target.
44. **The career's design:** after v0.3.

Taken by the lead: MISSION's `rotate` tool stays, since the player's manual controls are the agent's too (D126). The developer flag is `SUNSCATTER_DEV`.
