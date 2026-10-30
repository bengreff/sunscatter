# Owner interview, 2026-10-26 evening: the parts system

After the overnight run the owner asked for the realism state and said the next big task is the parts system and internal ship dynamics, with a realism target beyond KSP + RSS + RO + RP-1. The same evening: graphics reduced to one look (GFX), clock and orbit-line fixes (NAV), and gravity fields beyond J2 (GRAV) went to agents.

## First answers (verbatim)

- **First vehicle:** "falcon 9 crew, separated into parts and components that are all adjustable and interchangable in the parts system. (some elements of ships are not a physical part you place but another layer, like an electrical or plumbing layer)."
- **Generality:** "The system must have the capabilities to easily add new parts and procedually generate just about anything (eventually)."
- **Catalog and player parts:** "There can be a fixed catalog including a large variety of real life systems, plus going beyond that eventually, but the player can create their own parts."
- **Building:** "Building can be code-built for now, don't build an editor yet."
- **Engines:** "Published engines before the engine designer."
- **Failures:** "Failures should be fully simulated where justified down to everything simulated by our simulator. Obviously we cannot account for everything, but anything that would cause a failure in real life that we have the data for and that is within simulated ranges should actually cause the failure. If the simulator is not sure, don't trigger the failure. Could be many things: vibrations, ullage, statistical mechanical failure, etc. Many mechanical failures etc are statistic based because we do not simulate pipes and valves within an engine at runtime. Runtime in general is heavily abstracted to behavior, but not too abstracted. Use judgement."

## The lead's twenty questions, and the owner's answers

| # | Question (lead's default) | Owner's answer |
|---|---|---|
| 1 | Three kinds of thing: **parts** (physical, a shape and a place), **components** (inside a part: mass, volume, power, heat, no outer shape), **layers** (networks: structure joints, plumbing, electrical, data, coolant) | Yes. |
| 2 | Everything from generators with parameters; a catalog entry = generator + parameters + real data; player parameter sets now, player generators (WASM) later | Yes. |
| 3 | Derived numbers, plus a visible calibrated "unmodelled mass"; or published numbers override | "We tune all the parameters (including tech progression making things more efficient etc) so procedural parts are realistic, and we have truly published numbers overrule procedural numbers, for example thrust or battery capacity. Obviously there are gaps, so overrules are generally minimal and most things are derived. Build to minimize contradiction." |
| 4 | Career parts' properties come from physics; free numbers only in sandbox, marked unverified | Yes (a). |
| 5 | A declarative craft file, placement by rules or coordinates, MCP tools for the AI agent | "a with (obviously) a simple 3d modeling software integration AND defined mounting points rather than just anything touching is attached, and eventually a visual editor." |
| 6 | Rigid body, joint loads every tick from the real accelerations, strength vs temperature; bending and pogo later where needed | "No bending. Everything else is fine, just the crafts should be rigid until they break, with potentially LIMITED give implemented as a specified allowance in specific scenarios. Don't even touch wobbly rockets, instead just find where they break and model the effects of the minimal structural vibrations etc. I make this decision because there will likely pretty much never be a need for a bendy rocket. Cable and stuff like that can still bend. Intentionally dynamic parts can still move, springs can bounce." |
| 7 | Vibration as an environment per part (engines through the joints, buffeting, pyroshock) against its qualification level, damage accumulating; failure only above qualification with data | Yes. |
| 8 | A lumped fluid network: tank and pressurant pressures, propellant temperature and density, boil-off, line losses, pump inlet pressure, settling, slosh | Correct. |
| 9 | Engines from published data: start (limited ignitions, spin-up), steady map, throttle limits, shutdown, gimbal, heat, failure modes; the engine designer later gives the same maps | Yes. |
| 10 | Statistical failures seeded from the save: no re-roll on reload, the same at every warp | "a, but of course if a failure is caused by something it may not happen, like if accumulated vibration is multiplied by a chance or max load or something to decide if a failure happens, a different ascent after reload would have a different outcome. Same exact ascent (scripted?) results in same exact outcome IF launch time is identical." |
| 11 | Class-level historical rates where part-specific data is thin, at the low end of the rate's range | Not answered: the default stands. |
| 12 | True rates hidden; the organisation estimates them | "A bit more information. For example, a valve could have a reliability score. A more expensive valve is more reliable. It could be 1-10, rated operation hours before 50% chance of defect, rated max temp, etc. Comparison is easy, but a 1500K max temp for a period of 1 day does not mean that the part explodes after 1 day at 1500K. Other factors play in, plus statistics ONLY where it is appropriate. For example, if a heat shield has a rated max temp of 4000K for complete failure, the part is destroyed at 4000K, unless that 4000K is sustained for a certain period of time (likely should be, integrated heat for part destruction/internal temp). Parts should have various failure modes, for example something snapping is a different outcome from something being melted. Feel free to simplify this, I feel like I am giving mixed signals." |
| 13 | Part history across flights changes failure rates; refurbishment later | Yes. |
| 14 | Explosions and cascades: blast and fragments damage neighbours; no fire | "yes. A certain failure mode for a specific part triggers a given outcome." |
| 15 | Electrical as power flow in watts, voltage derived, batteries, solar cells per skin cell, load shedding | Good. |
| 16 | Instruments show the true state now; sensor estimates later | "Interesting. I would say the UI never shows something wrong. If your ship lacks the sensors for a particular measurement, that measurement is simply replaced with a dash or something. We simplify the errors in such processes somewhat, another thing we can do is limit sig figs to a reasonable value for what your current sensors can measure to. You can't get your velocity to 50 decimal places while in orbit (or similar). Of course, we still simulate time delay, occlusion, relay networks, signal strength, and I would say the actual mechanisms behind how your ship gets the data it gives you. If you have no way to measure your velocity during reentry, your display does not show that. If you do not have a sensor in your fuel tank, you don't know how much fuel is in there (sensor would be a simple component)." |
| 17 | Safety logic now, the vehicles' own flight software once scripting exists | "a for now [fly by hand], scripting comes soon, should be integrated with the data model I mentioned just now. BTW debug mode should give you perfect info instantly." |
| 18 | Cabin air, consumables, suits, crew g-limits in the first slice; radiation later | Yes. |
| 19 | Clamps, pre-launch propellant state, the droneship; full countdown later | "Yes, full countdown procedure later. I would lean towards abstracting that procedure though, modeling as a time+funds sink. Anything not part of the ship, launchpad, and abstracted procedures is out of scope, no cars driving around the launch site." |
| 20 | Done = a whole Crew mission (launch, booster landing, ISS docking, entry, splashdown) | "that is the final goal. Start with just building the complete falcon 9 model within the part system foundation, controls and systems taken to the proper level of realism, allowing me to fly it." |

Also: the gravity-field work goes to an agent (GRAV); the owner allowed the night's work ("You may do whatever you want all night, up to 2-3 subagents. Polish things up as you go. … I would like to see a falcon 9 at the proper level of simulation when I wake up"), the GFX measurement demos and the pre-push hook in this checkout.

## The final five questions, and the owner's answers

| # | Question (lead's default) | Owner's answer |
|---|---|---|
| 1 | If the night runs short, simplify Dragon's insides first, then the non-physical failures | Default (not contested). |
| 2 | A Systems menu in the flight view; Space steps a sequence from the craft file | "Systems should be an entire new screen separate from flight. Flight has essential info, but to truly control every bit of the craft you need a whole separate screen. Here the primary visualization is of your craft and can switch between layers, like physical, structural, thermal, electrical, and more. Plus all the features you mentioned. The idea is you can define stages and save functions to a mini-systems panel shortcut that you can activate during flight." |
| 3 | Start on the pad at LC-39A, fuelled at loading temperatures, clamps holding, crew aboard; a new-flight choice of craft | Default (not contested). |
| 4 | The booster becomes a controllable vessel you can switch to and land by hand; left alone the debris rule handles it | "The booster is controllable, so it stays simulated until it hits the ground and is destroyed." |
| 5 | Blender glTF (.glb) for shapes, named empties as mounting points, custom properties as data; procedural shapes tonight | Default (not contested). |

"Go."
