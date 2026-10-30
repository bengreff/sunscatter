# Interview, 2026-10-27 (afternoon): lighting, the flight interface, physical visuals

The owner's request after playing the Falcon 9 (verbatim, with a screenshot of the second stage lit half white, half dark red), the questions asked, and the answers (verbatim; a question not answered took the default in brackets). Decisions taken from it: D109–D113. Research done for it: [research-2026-10-27-flight-ui.md](../research/research-2026-10-27-flight-ui.md), [research-2026-10-27-visuals.md](../research/research-2026-10-27-visuals.md).

## The request

> Fix this please, lighting should NOT be dependent on the position of the camera but rather the position of the craft. NO occulsion of the camera, occulsion of BODIES AND CRAFTS. Replace V/S on the nav ball with current g-load out of max g-load (in direction the current g's are going). Timewarp should not be limited if the throttle is on but no thrust is being produced, only limited when thrust starts being produced, and of course there will eventually be a timewarp while burning setup menu. The biggest issue with the UI is I have no idea where the essential info is. Stages should have a better UI to the very right of the screen, a very thin bar, each stage (ONLY if it applies) should have a reading of the amount of fuel left in that stage to the best that the algorithms can determine what fuel this stage is drawing from (in the form of a simple bar with kg/kg), plus time the stage will last on the current trajectory. All estimations within a stage assume no input, and the estimations change when the rocket is controlled. Each stage should have a list of parts/components/scripts they affect, with a SIMPLE icon for each one denoting ONLY part type (engine, decoupler bolt, solar panel servo, etc). You can click on each icon and bring up a parts live-menu. This is ONE CONSTANT menu for EACH part, separate from the comprehensive menu in F8 (choose a different shortcut BTW F8 doesn't work on mac). It can be pulled up by clicking on a part in flight on the actual ship and by clicking on an icon in a stage. The live menu gives essential stats on that part/component on THIS SHIP, right now. Thrust, fuel/max fuel, percent charged, temp/max, anything else essential for knowing what needs to be known about this part right now. Make sure you make it clear the SMI in elements (should it be "osculating"?) is from the center of the body, and resolve that confusion with consistency if you can. Move all the stuff from "propulsion" to other tabs, TWR and mass (current/end) plus thrust go into the staging menu, plus avg ISP, everything RCS goes to a new RCS menu that appears automatically ONLY when you press an "RCS" button on the nav ball. This menu has all the stats about your RCS thrusters and when it is on, ijkl translate and wasd use RCS as well as engine gimbals to turn. Overrides and stats about your RCS system are also here, a list of thrusters. I would say the menus should be redesigned a bit, no submenus, instead every menu has a button at the top, menus are thoughtfully designed to each fulfill a task and only have necessary information, and if anything more is needed a link is provided to a specific screen in the ship view F8 screen. That screen should also maybe be redesigned somewhat, information strategically placed, when you have a layer selected all info about that layer as a whole should be provided. Some other tasks include toggleable volumetric clouds and waterfall engine effects and reentry effects […]. Actually on second thought I remembered a principle: no visuals for the sake of visuals. All visuals are derived from either a truth about how physics works, simplified, or an element of the simulation. Reentry is derived directly from what our aero model computes, and if our aero model is not high fidelity enough to see plasma buildup, we improve it. Waterfall is a rendering model (simple and fast) based on a seed of exhaust parameters the instant it exits the nozzle (which will be the output of the engine testbench) and the local atmosphere parameters. Expandable to relativistic exhaust and different mixtures/temps of exhaust. […] maybe it also looks into efficiency techniques and how we can make our visuals VERY fast and optimize out anything off screen/far away. Volumetric clouds can be less rigor, but still based on a general atmosphere model configurable for each planet with simple pressure, scale height, composition, temp etc fields, plus an optional empirical large scale weather pattern model+cloud characteristics (for earth only). The clouds should be pasted onto the visual texture for the earth (only when the toggle is on of course), smoothly transistion from a 2d pattern filled with fractal detail when viewed from orbit to a volume from a similar altitude.

## Questions and answers

**Lighting**
1. Every craft shadows itself and other craft nearby at any zoom; craft shadow the ground; bodies eclipse craft. — "The issue is on the screenshot I showed, that shadow changes as the camera rotates around the craft. Orbital sunset would happen on the whole craft instantly. Earthshine is fine. Yes."
2. Earthshine near the terminator from the lit crescent. — "um, duh, that should be automatically derived from the area reflecting light, intensity of light, and angle of the craft relative the reflecting surface. No additional edge case ideally."
3. The brown disc in the screenshot. — "no clue" (it is the MVac's flame cone seen end-on).
4. Running the game offscreen to reproduce. — "doing that is allowed as long as it is not more than a couple times. Just don't open 20 windows. It is a very specific camera angle, so I doubt you can reproduce it. Just know the lighting of the craft is dependent on the camera which is wrong."

**Principles and scope**
5. "No visuals for the sake of visuals" as a decision; the camera's optics as physics. — "sunflare is mostly fine as long as it applies to any light producing body. Parachutes should be meshes that collide with the craft. Physics of the camera is odd, this is not a camera simulator and the exterior camera is NOT a real camera floating in space, it is a magical visualization. Thus, the magical visualization should only do what is clearest and most efficient. Later we may add an "only on board cameras" mode, in which case the physics of the camera should be taken into account."
6. Clouds the only toggle. — "No, exhaust and reentry should be toggleable b/c they are technically non-vital and compute heavy. Exhaust defaults to the cone we see today."
7. Order of work. — [lighting → warp fix → flight interface → systems screen → exhaust → reentry and aero → clouds → the Crew mission's next steps]
8. Agents at once. — [2; the owner is on the machine]

**Staging bar**
9. Next stage at the bottom. — [yes]
10. Each step of the Falcon 9's sequence is a stage. — [yes]
11. Two propellants. — "one bar each, with burnout time based on the one that burns out first."
12. Δv per stage. — "yes, and below the stage bar should be a toggle between current atmosphere real thrust/delta v, and vaccum thrust/delta-v."
13. TWR, mass, thrust, Isp for the active stage only. — "yes, for other stages we only need delta-v."
14. Stages edited only in the systems screen. — "You can drag stages up and down or drag parts/components/functions between stages or delete a stage from flight."
15. Monochrome icons per part type, coloured by state. — [yes]

**Part cards (the live menu)**
16. Where. — "They should show up near the part in question (fixed on the screen and draggable) when a part on the ship is selected, stack left of the staging bar over whatever menu is there if an icon is selected. Line should go between the live menu and the part/icon."
17. A single click on a part opens it. — [yes]
18. Commands on the card, irreversible ones confirmed. — [yes]
19. Only measured values, a dash otherwise; debug shows all. — "Yes, as a QOL feature, if you hover over a dash for 2s it shows a little tooltip about what sensor would measure this quantity."
20. Components inside their part's card. — "only if appropriate to the live state, so not a sensor or something."

**Navball**
21. G-load is proper acceleration (1.0 g on the pad). — [yes]
22. Max g: the lowest of the structure's limit in that direction and the crew's tolerance, the limiting item named. — [yes]
23. V/S moves to the Landing menu. — [yes]
24. RCS on R; reset only from Esc. — [yes]
25. Translation on IJKL + H/N; help to `?`. — [yes; the planner's key: see 35]

**RCS menu**
26. Authority per axis, propellant, the thruster list, imbalance, overrides. — [all]
27. Whose RCS. — "All thrusters of the entire vessel, plus the ability to select which thrusters are on with the default of all of them. A vessel is everything connected structurally, plus information being sent EITHER along a wire or wirelessly."

**Warp**
28. RCS and settling thrust count as thrust. — [yes]
29. The warp-while-burning setup later. — [later]

**Menus**
30. A button per menu along the top; flat pages; links into the systems screen. — "yes"
31. The set: Orbit, Landing/Reentry, Maneuver, Crew/ECLSS, Power, Comms; RCS from the navball; staging is the bar. — "Plus docking eventually, which will include more tools for docking with the assumption that the RCS window is open as well."
32. Left dock for flight tasks; right edge for the bar and cards. — [yes]

**Elements**
33. Altitude reference. — "Altitude has two settings, toggleable on the nav bar: sea level (or lowest surface elevation) plus terrain. Switching there changes any stat in the menu referencing altitude above surface. Gas giants get an arbitrary boundary that is sea level."
34. "Osculating". — "I did not know what osculating meant. Pick a better word."

**Keys**
35. F-keys go: Tab systems screen, M tracking station, Cmd+S/L. — "yes. Plus menus along the top can be number keys 1-0, we will likely have 10 eventually."

**Systems screen**
36. Layers first. — [structure, fluids, electrical]
37. Layout. — "yes, plus maybe be a bit more clear and organized about part stats. Pop up menus welcome."

**Exhaust, reentry, clouds**
38. Build the engine testbench's first piece. — "NO NO NO! I am building this in "inquiry". You can check there, we almost have an RL10 simulated. Use placeholders from published engines for now." (`~/inquiry-project`, CRUCIBLE: reacting compressible flow in a nozzle, Cantera.)
39. Aerothermal upgrade with blackout. — [yes]
40. Earth's weather. — "smoothly moving weather, moving smoothly+obeying weather trends on a large scale so it is VERY cheap and queryable from seed (deterministic), everything else is still smooth but fractal detail. No gameplay effect yet. Leave atmosphere+weather open to be influenced by exotic conditions such as temperature swings on a planet with an eccentric orbit. For example the whole atmosphere could freeze."
41. Other worlds later, the model general. — [yes]
42. Clouds off by default. — [yes]

**Flying**
43. Per-vessel controls. — "BTW switching crafts is between nearest first and is [ or ], moving forwards and backwards in the list of crafts so you can go back and forth. Yes, plus ability for a script to control the inactive vessel."
44. An abort key. — "Just an action group in the bar above the nav ball. No keybinds specific to a craft."
45. The mini systems panel. — (see 44: the action-group bar above the navball)
46. Anything else while flying. — (a screenshot of the right dock: Utilities and Staging run off the bottom of the screen) "This gets cut off BTW, make sure the menus are versatile. Notifications should show up in a window in the corner where they can be scrolled through and the relevant vessel/screen linked where relevant. Add a pause timewarp by clicking , again."

**Next**
47. After this round. — [the Crew mission]
48. The night's open items. — "One thing I noticed is the derived masses were terrible. Of course, we can't make generalizations until we have data on hundreds of systems across aerospace. That system will come later, just keep in mind that eventually we will have a system that creates realistic, tech-dependent part stats procedually, tuned on comprehensive research over hundreds or thousands of published rocket systems. That is part of the procedual part system, we don't dive into that now, for now we stay with real life parts with the assumption that we might edit internal elements of them later to work with procedual generation."

**Follow-ups (2026-10-27 evening)**
- The number keys: plain 1–0 open menus; the hold modes and pinned functions? — "No need for hold modes on 1-6. Not everything needs a key."
- The Falcon 9 starts its booster's cold gas and Dragon's Dracos disabled (its sequence enables them); does "all on by default" override it? — "Use the real default RCS config and whatever is logical."
