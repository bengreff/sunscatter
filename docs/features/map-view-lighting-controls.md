# Feature: map view, lighting and controls (fix round 1)

**Status:** accepted direction (owner answers of 2026-09-26 are folded in; decisions D054–D056).
- Items marked **Q** still wait on an answer.
- Everything else is the plan; the owner can override any default.

**Source:** owner feedback of 2026-09-26 after the visual pass ([plan review](../plans/v0.2-visual-pass-and-foundations.md#review)).

**Decisions:** D054 (per-object map view, replacing D049 and D053), D055 (lighting), D056 (orbit-line length).

## Scope

In scope:
1. Zoom speed.
2. One rule that decides whether each object is in map view.
3. Physical lighting from stars.
4. A steady fps readout.
5. Unified orbit-line length, with settings.
6. A pause menu with a settings screen.
7. Camera collision.
8. Ground jitter while thrusting.
9. Surface flicker and dark patches.
10. Hover priority.

Out of scope: new content (craters, clouds), the burn planner.

---

## 1. Zoom speed

Zoom half as fast as now, for both the wheel and the trackpad.
- Wheel: 1.15 → 1.072 per line.
- Trackpad: 0.25 → 0.125 lines per pixel.

The speed is also a setting (§6), stored in `ControlsSettings`.

## 2. Per-object map view (replaces the global map mode)

### The rule

Each object *o* is a body or a vessel. All sizes are measured with **one scale**: pixels per metre at the camera's distance to its **focus**.

    k = viewport_height_px / (2 · tan(fov/2) · focus_distance)

Using one scale is the owner's wording, and it has a property we want. Objects in the same scene switch together however far they are from the camera, so the Moon doesn't flicker in and out as the camera orbits the Earth. It also makes the behaviour independent of perspective depth.

- `sprite_px(o) = k · radius(o)`. A vessel uses its bounding radius, currently 10 m.
- `orbit_px(o) = k · orbit_radius(o)`, where the orbit radius is the osculating semi-major axis about the object's primary, or the current distance to it if unbound.
- **o is in map view ⇔ `sprite_px(o) < 1` and `orbit_px(o) ≥ 1`,** with hysteresis: it enters below 1 px and leaves above 2 px, and likewise for the orbit bound.

### What depends on it

This is the only place any of these behaviours are decided:

| Thing | Shown when |
|---|---|
| Icon | o is in map view |
| Orbit line of o | o is in map view (and o's orbit lines are enabled, §6) |
| Hover highlight ring and name | o is in map view |
| Ap/Pe markers (vessels) | o is in map view |
| The 3D model/terrain | always, from any distance (it just becomes sub-pixel) |

There is no global "map mode" flag any more. The old ship test (<1 px) is the vessel case of the same rule.

### Hover priority (item 9)

Only objects in map view can be hovered: a body drawn at full size shows no ring and no name. Among the in-map-view objects under the cursor:
1. the one with the **largest** mass wins, so a planet beats its moons when their icons overlap, and a star beats its planets;
2. ties go to the nearest icon centre.

A full-size body's disc blocks hovering on icons behind it.

Moons are still reachable: zoom in until their icons separate. Icons whose screen distance to a heavier object's icon is under 6 px are hidden (the KSP convention), so hovering there shows the parent.

### Tracking station (**Q3**)

Proposed: the same rule, with the tracking station's own camera, plus two differences:
- every *tracked* vessel's icon and line is shown even when its orbit is under 1 px, drawn at a minimum size, so nothing tracked is ever invisible;
- the station has no flight HUD, and clicking an icon focuses it.

### Tests (pure functions, no Bevy)

`map_view::classify(objects, k) -> Vec<Visibility>` is one pure function, tested by a table:
- a sub-pixel ship in LEO, seen from Earth-scale zoom, is in map view;
- a ship with a 10 m sprite is not;
- the Moon, seen from a Moon-scale zoom, is not in map view and its icon is gone;
- at solar-system scale, Earth is in map view and the Moon is not (its orbit is under 1 px);
- hysteresis: no flip-flop across a 1.5 px band;
- hover priority: a planet beats its moon; a full-size disc blocks icons behind it and is not hoverable itself.

## 3. Lighting from stars (physical, per body)

### Why the Moon's night side is lit blue (image 2)

The current Earthshine is one *directional light* whose strength is computed at the **camera**. From low Earth orbit that is ~25,000 lux (Earth fills half the sky), and a directional light applies it to *everything*, including the Moon 384,000 km away. The main sunlight has the same flaw: its 1/r² falloff is evaluated at the camera, not at each body. In an interstellar setting this is exactly KSP's single-sun problem.

### Proposal

- **Stars are data.** Each star gets a luminosity in `visual.ron` (Sun: 3.828e26 W; the lux conversion follows from its spectrum, about 93 lm/W). There is no separate "light" object.
- **Per-body illumination.** Every frame, for each lit object (terrain body or vessel) and each star within range (flux above 1e-6 of the brightest), the CPU computes the **flux at that object's position**, `E = L/(4πd²)`, and its direction. The terrain shader and a ship material take up to 4 star directions and illuminances as uniforms and light the surface themselves: Lambert plus the existing PBR specular. The global `DirectionalLight` is used only for shadows. The illuminance stays physical everywhere: Earth 1 AU from the Sun gets 128,000 lux; a planet 5 AU from a second star gets its own value from its own distance.
- **Reflected light** ("planetshine"). Each lit body is also a weak source: a Lambert sphere with albedo from data, whose flux is computed at each *receiving* object with the phase angle. That is Earthshine on the Moon and ships, and moonlight on Earth. 
- **Exposure (decided: eye adaptation, D055).** With physical values, Earthshine on the Moon is 10,000× weaker than sunlight and invisible at a fixed exposure. We use Bevy's `AutoExposure` (histogram-based, compute) with slow adaptation and limits: a close-up of the Moon's night side adapts to show Earthshine, while a sunlit scene keeps night sides near-black. The tiers get auto exposure from Low up; Minimal keeps the fixed EV.
- **Ambient.** The fixed floor (30) is removed. Night sides get only planetshine and the starlight background (about 0.001 lux).
- **Eclipses.** An analytic sphere-occlusion factor per star per object on the CPU gives Earth's shadow on the Moon, a ship in Earth's shadow, and so on. Cheap, and consistent with the flare's occlusion test. It is in scope unless the owner says otherwise.

**Tests:**
- flux falls off as 1/r² between two positions;
- two stars add;
- the eclipse factor is 0 behind the planet's centre and 1 outside its shadow cone;
- the Earthshine phase function peaks at a "full Earth";
- the demo shows the Moon's night side at the same brightness wherever the camera is (it no longer depends on camera position).

## 4. FPS readout

The fps readout is the frame count over the last ≥ 0.5 s, divided by the elapsed time, updated every 0.5 s. It appears in both the HUD and the overlay.

## 5. Orbit-line length (unified)

### Default

An object's line runs forward until it has completed **one full revolution about its dominant body**, with that body dominant the whole way.
- Measure the angle swept about the current dominant body in the object's orbit plane.
- Stop when it reaches 360° with no change of dominant body in that stretch.
- If the dominant body changes, restart counting about the new one, so a transfer is drawn until one full revolution about the destination.
- Hard caps prevent unbounded work:
  - the computed segment horizon;
  - a time cap (setting, default 1 year for vessels);
  - for escape trajectories, until the object leaves the plot's region of interest.

This is display logic only. Physics never uses "dominance" (rule 1).

### Which body is dominant (decided: tidal, D056)

The largest raw gravitational pull makes the **Sun** dominant for the Moon: it pulls the Moon about twice as hard as Earth does. The Moon's line would then be a year-long heliocentric loop.

**Decided: relative (tidal) dominance.** The dominant body B for object o maximises *B's pull on o relative to B's pull on o's other candidate primaries*: the perturbation ratio used for Laplace spheres of influence. This makes Earth dominant for the Moon and ships in LEO, and the Sun for Earth. It is a display rule, not physics.

### Settings (in the new settings screen)

- **Vessel line length:** "1 revolution" (default), "2 revolutions", "until impact/escape", or a fixed time.
- **Body line length:** "1 revolution" (default) or off.
- **Per-body show/hide** (a list of bodies with checkboxes), plus "show only orbits in the current system".

Bodies' lines come from the ephemeris with the same revolution rule. The ship's line needs its coast segment computed far enough ahead, so the look-ahead budget follows the rule instead of the old 1.5-orbit guess.

**Tests:**
- a circular LEO coast gives a line whose end angle is 360° ± 0.5°;
- a Moon flyby trajectory changes its dominant body and ends after one revolution about the Moon (or at the cap);
- the Moon's line is one lunar orbit about Earth (tidal dominance);
- the settings round-trip.

## 6. Pause menu, settings and general GUI

### Pause menu

**Esc** pauses (the sim clock stops, and a "PAUSED" banner shows) and opens a centred menu:
- Resume;
- Settings;
- Save;
- Load (the save list);
- Revert flight to launch;
- Tracking station;
- Quit to desktop.

Revert and Quit ask for confirmation. Esc again resumes. While paused, the camera still works and the game ignores flight keys.

### Settings screen

Tabs, all persisted in `settings.ron`, each with "Reset to defaults":
- **Graphics:** tier buttons plus every toggle, as F3 has now, and the benchmark.
- **Orbits:** line length (§5), per-body show/hide, "only the current system".
- **Controls:** zoom speed, trackpad zoom, mouse sensitivity, invert, and a key reference.
- **Interface:** UI scale, fps readout, HUD elements.

F3/F5/F6/F7/F9 stay as shortcuts; F3 opens Settings → Graphics.

### General GUI

These fix what is rough now; **Q6** adjusts the look.
- **One style.** A shared egui theme module holds colours, fonts, spacing and window frames. Panels are dark and slightly translucent, with an accent colour for the active vessel, and nothing is left at egui's default look.
- **HUD layout, no overlaps.**
  - Top left: date, time and warp.
  - Bottom left: the flight panel (altitude above terrain, speeds, Ap/Pe, throttle, SAS, chute).
  - Top right: target/hover info.
  - Right edge: messages.
  - Debug lines (anchor, plot frame) move to an optional debug panel.
- **Units and numbers.** One formatter for distance, speed and time: m, km, Mm, AU; s, min, h, d, y. Numbers are right-aligned monospace so they don't jiggle; rates update at most 10 times a second.
- **Warp indicator.** Arrows show the level, with "physics"/"rails" and the reason warp is limited (throttle, compute).
- **Messages.** One queue of short toasts: saves, loads, warp limited, crashed. They replace ad-hoc labels.
- **Keys.** A help overlay (**H**) replaces the collapsible Controls window.
- **Windows remember their position**; the UI scales with the display's DPI plus the UI-scale setting.

## 7. Camera collision

- **Bodies.** The camera never goes below `surface_height + max(2 m, 0.002 · focus_distance)` of the nearest body, using `sim`'s terrain sampling (the same surface as physics and rendering). A zoom or rotation that would violate it is clamped: the camera stops, as requested. For a body focus, the minimum distance is its radius plus that margin along the view direction.
- **Ships.** The camera stays outside the ship's bounding sphere (1.2 × its radius) when it is the focus.
- **Tests:** a pure function `clamp_camera(...)` with cases for zooming into terrain, orbiting below a mountain, and zooming into the ship.

## 8. Ground jitter while thrusting (item 7)

**Cause (confirmed in code):** a powered vessel advances in fixed ticks and reports its state at *its own* time, which trails the clock by up to one tick. Bodies are drawn at the clock time. The camera follows the ship, so the ground jumps by `v · Δt` every frame: up to metres at launch speeds. Landed vessels have the same mismatch in principle.

**Fix:** the renderer asks for every vessel's state **at the clock time** (`Vessel::state_at(world, clock)`), interpolating between the last two powered ticks with the same quintic Hermite as coast segments. It never integrates (rule 4). Test: a powered vessel's rendered position is continuous across ticks.

## 9. Surface flicker and dark patches (item 8)

Likely causes, to be confirmed with repro captures before fixing:
- **The water mask is per vertex at LOD resolution** (the blue square in image 3). A finer chunk samples points below −5 m that the coarser neighbour didn't, so a whole chunk turns into "water" and switches as the LOD changes. Fix: bake a water mask texture (from the colour map or an ETOPO land mask, at the asset tool) and look it up per fragment, like the colour map. This also resolves the question of an ocean mask for the Dead Sea (the plan's B3 note).
- **LOD popping:** abrupt chunk swaps. Fix: morph vertices between levels (geomorphing) over the last 20 % of a node's range.
- **The sky light and atmosphere mode switch abruptly at the atmosphere's top.** Fix: cross-fade over a band instead of switching.
- **Aerial perspective ends at 400 km**, so distant ground can be darker. Fix: raise the limit or fade.

**Tests:** the water mask is independent of LOD level (sample the same point via different chunk levels); demo captures during a zoom sweep (new demo step) are checked by eye.

---

## Open questions

- **Q3.** Tracking station differences (proposed above)?
- **Q6.** GUI taste (colours, density, KSP-like or not): see the questions asked on 2026-09-26.

Resolved:
- Q1: tidal dominance.
- Q2: no hover outside map view.
- Q4: eye adaptation.
- Q5: the pad crash was steered (not a bug).
