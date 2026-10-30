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
