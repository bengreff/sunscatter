# Interview 2026-10-29: after the fixes, and the ship model's fidelity

Ten questions from the lead after the 2026-10-28 fix hour ([fixes-2026-10-28.md](../plans/fixes-2026-10-28.md)); the owner's answers, verbatim where they wrote their own. Decision D115 records what they decide.

## Before the questions

- **Plume:** "The engine plume SHOULD be faint, but it just seems odd that it is such a dull grey color. Make an assumption based on fuel type if unsure."
- **Flickers:** "The flickers happen when landed on the launchpad."

## Answers

1. **Relight after the throttle shut the engines down:** run the stage again (AGAIN); throttling up does not relight (D114 as written).
2. **New work for the queue:** the sequence bug sweep (pad abort past liftoff, `set_stages` index, a release armed with no engine lit, grid fins after a split, an aborted bubble tick not restoring `ShipState`: open-issues.md); planned burns for craft of parts (`PlanError::PartsUnsupported`); the booster after MECO (no vacuum relight, the separation inside 1 s of MECO); and "In general fix inconsistencies against the real falcon 9". Docking was not picked now.
3. **Sensors:** "ONLY add things the real vehicle has." The real vehicle's sensors (Dragon's tank gauging, the stages' propellant level sensing, the strain gauges it flies), dashes only where nothing real measures.
4. **Flight software (guidance):** "Not yet, player flown for now." No autopilot; see 8 for the adaptive functions.
5. **Thermal:** each powered component heats its part by its draw (an engine that is off heats nothing), and Dragon's coolant loop and trunk radiators.
6. **Engines:** per-engine gimbals and the engine-out response (the rest re-trim gimbal and throttle; the burn runs longer, CRS-1); hard starts as a failure mode.
7. **Life support:** "Tanks and leaks, suits can be very abstracted." O₂ / N₂ in their tanks through regulators, leaks; suits abstract.
8. **Navigation:** noise, IMU drift and GNSS, "HOWEVER, wherever possible, the number of sig figs should simply abstract away the uncertainty. I really would not like flat out wrong measurements in most cases. Only where absolutely needed. Estimations where appropriate. Also, I suppose built in flight software (in the form of adaptive functions) is needed."
9. **Published against derived:** published wins: adjust the model's inputs until it reproduces the sourced figure (the legs: 173 kg derived against ~500 kg published), and record the source; derived values stand only where nothing is published.
10. **Adaptive functions first:** all four: engine-out re-trim, launch-commit checks (hold or abort on a bad start between ignition and release), propellant utilisation (mixture-ratio trim so the tanks empty together), the navigation filter (IMU with GNSS and star trackers).

## Not asked (candidates for later)

Slosh pendulums and line transients, craft-specific antennas (Dragon's S-band, TDRS) instead of one `VESSEL_ANTENNA`, the umbilical dropping at T−5 min instead of at release, battery temperature limits, fatigue and pyroshock, docking (IDSS) for the Crew mission, GLOW's rest, CLOUDS.
