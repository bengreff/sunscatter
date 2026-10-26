# Owner feedback, 2026-10-25 (after flying the 2026-10-01 playtest save)

The owner flew the low-orbit playtest save for about 18 minutes (deorbit, landing; the craft ended on its side at the pad) and answered with two screenshots: Earth from 298 km (a pale blue blur, no land or water to be seen) and the craft on the ground (a uniform dark grey plain). Decisions from it: D073 (revised), D080. The plans built from it: [crisp-surfaces.md](../features/crisp-surfaces.md) and [flight-menus.md](../features/flight-menus.md).

The four playtest questions (RCS turns, apsides, bell heat, chutes) were not answered yet.

## Graphics (D073, revised)

> "Minimal graphics does not mean blurry voids for the surfaces of planets. It means sharp, crisp, clear surfaces, recognisably water or land or earth, just not necessarily needlessly complex."

- "Realistic skybox based on light and atmosphere."
- "Planet surfaces simply rendered based on an image download sharpened up (coastlines etc) with procedural algorithms then a consistent texture applied for various colors/zones."

## Interface (D080)

> "The HUD needs some work. Make designated, elegant locations for menus and completely cut unnecessary information."

- **Navball:** "essentials like velocity, ap/pe, SAS, option to switch between orbital and surface, etc."
- **Landing/reentry menu:** "specific info on landing or reentry, plus when it is open the impact marker shows up."
- **Orbital menu:** "advanced orbital stats and stats about the target if you have one selected."
- **Utilities menu:** "a brief overview of essential thermal, life support, power, fuel etc system status."
- **Staging menu.**
- "Use your judgement. These menus should be versatile and task oriented, allowing for scaling to a vast scope that this game covers. If we add docking, we add a docking menu. If we add advanced maneuvers, that likely goes in the orbital menu. Hierarchical, you should be able to pull up essentially anything if you want to."

## What the lead found behind the screenshots

- The Minimal tier capped the colour map at 2048 px: 20 km per texel on Earth, magnified ~90× at nadir from 300 km, so everything was a smear. Even the full 8192 map (5 km per texel) is a smear at that range.
- The Minimal tier used Bevy's lookup-table atmosphere everywhere. Its aerial perspective is built for a camera inside the atmosphere; from orbit it washes the ground out in blue. The raymarched mode (adaptive: tables inside, raymarching from outside) costs ~0.6 ms from orbit and nothing at the pad (measured 2026-09-26, `docs/plans/v0.2-visual-pass-and-foundations.md`).
- At the pad the "simple ground pattern" sat on a 20 km colour texel: a uniform grey plain.
