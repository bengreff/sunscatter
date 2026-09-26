# Handoff prompt (paste into a fresh session)

You are continuing Sunscatter v0.2 autonomously. The owner is away for about 2 hours and wants to come back to a big step forward.

1. **Read these first:**
   - `CLAUDE.md`
   - `docs/decisions.md`, especially D045–D050
   - `docs/plans/v0.2-earth-moon-prototype.md`, especially its Review section
   - `docs/plans/v0.2-visual-pass-and-foundations.md`, **the plan you are executing**
2. **Execute that plan: Part A (visual pass) first, then Part B (foundations).**
   - Check items off as you go.
   - Commit small, with one purpose each, and push to `main` regularly. CI must stay green: fmt, clippy `-D warnings`, `cargo test -p sim --locked`, and the file-size limit.
3. **Don't stop to ask questions.**
   - Make reasonable choices and record them in the plan.
   - Record choices that affect the whole project in `docs/decisions.md` as new numbered entries.
   - Research prior art when something is hard (`docs/process.md`: KSP mods like Scatterer, Kopernicus, Parallax; Bevy examples and source in `~/.cargo/registry`).
4. **Verify visually with the scripted demo.** Use `SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 cargo run -p game --release`, then look at the screenshots.
   - The owner's screen may be locked, and a locked macOS screen doesn't present windows, so use the offscreen mode.
   - Extend the demo to capture every graphics tier and to measure per-feature costs.
5. **Never break determinism.** Sim golden hashes must pass on macOS and Windows. The sim crate uses `sim::math` (libm) and no engine dependencies.
6. **Finish properly:**
   - Fill in the plan's Review section: measurements, choices, what's not done, what needs the owner's judgement.
   - Commit the screenshots to `docs/images/v0.2-visual/`.
   - Update README and CLAUDE.md if commands changed.
   - Give a short summary.

Priorities, if time runs short:
1. Graphics tiers and settings, with measurements.
2. Earth and Moon heightmap and colour data, with generic terrain LOD.
3. Map mode (icons, orbit lines, Ap/Pe, hover) and slower trackpad zoom.
4. Atmospheric scattering and sun flare.
5. Starfield.
6. Bodies as data, then terrain physics.
7. Saves and settings.
8. Vessel switching and the tracking station.
9. CI and developer speed.
