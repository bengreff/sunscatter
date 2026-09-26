# Sunscatter

Sunscatter is a real-scale spaceflight, mission-design and logistics game. It is being rebuilt from scratch in 3D.

| Version | Status | Where |
|---|---|---|
| **v0.1** | Archived 2D prototype in Rust/wgpu. It is still runnable. | [`archive/v0.1/`](archive/v0.1), whose README has screenshots, features and build instructions |
| **v0.2** | In development. The first Earth–Moon flight prototype is playable: launch, orbit, trajectory prediction, parachute landing (placeholder graphics). | [`crates/`](crates), [`docs/`](docs) |

## Running the v0.2 prototype

```bash
cargo run -p game --release          # fly: Z full throttle, W/A/S/D/Q/E steer, P parachute, . and , warp
cargo test -p sim                    # simulation tests (deterministic; CI runs them on macOS and Windows)
SUNSCATTER_DEMO=/tmp/shots cargo run -p game --release   # scripted flight + screenshots
```

![Orbit with the stored trajectory](docs/images/v0.2-prototype/03_orbit_with_prediction.jpg)

The build plan and its review (what works, measurements, what's next) are in [docs/plans/v0.2-earth-moon-prototype.md](docs/plans/v0.2-earth-moon-prototype.md).

## Planning documents

- [Vision](docs/vision.md): identity, pillars, non-goals and roadmap.
- [Process](docs/process.md): documents, research strategy and engineering rules.
- [Decision log](docs/decisions.md): what has been decided, and why.
- [Lessons from v0.1](docs/lessons-from-v0.1.md): what to keep and what to change.
- [Motion model](docs/design/motion-model.md): how bodies and ships move without spheres of influence, plus the analysis of numerical precision.
