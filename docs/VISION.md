# Vision: Sunscatter v0.2+

> A challenging, vast, scientifically honest game about designing missions, flying them and running logistics. You lead a spaceflight organization starting in 2030, and expand from Earth into the nearest 100 light-years over 1,000 years.

This is not a casual game. The target is several steps beyond KSP in realism. It is complex enough that a player's own AI agent, connected to the game, acts as the tutorial and assistant.

## Pillars

Every feature must serve at least one pillar. A feature that serves none needs a decision-log entry that changes the pillars.

1. **Nothing contradicts science.**
   - Real scale.
   - N-body gravity with no patched conics deciding motion.
   - Relativity, radiation, and light-speed delay.
   - Complex engineering can be abstracted generously, but never falsified.
2. **Designing and running missions is the core.**
   - Advanced N-body trajectory planning, and burn plans that can run under time warp.
   - Many missions in flight at once.
   - Every ship is fully physical, with no abstract cargo.
   - Trade routes are tools for creating and managing missions efficiently.
3. **Realistic progression.**
   - Funding and science drive the expansion of an organization over centuries.
   - A sandbox mode exists alongside.
4. **Ships are engineered, not assembled.**
   - Parts are placed programmatically or with constraints, and specifying coordinates is valid.
   - Parts can be procedural or designed, and designs can be shared.
   - Power and fuel follow semi-physical paths, and the layout has consequences.
   - Parts can break.
   - Probes are controlled through a ship operating system with scripting, because light delay makes direct control impossible.
5. **Performance is a feature.**
   - 60 fps on an M2 Pro with 10 or more ships in flight.
   - Warp up to 1,000,000x, with every warp level producing identical results.

## What we keep from v0.1

- Its scope and its creative touches.
- The physics-first identity.
- The design documents.

See [lessons-from-v0.1.md](lessons-from-v0.1.md).

## Non-goals (for now)

- The whole Milky Way. The scope is a 100 ly bubble, with more as optional downloads.
- A web build.
- Multiplayer before v1.
- Detailed planetary surfaces.
- Real-time CFD. The engine simulator produces performance maps when a design is made.
- Casual accessibility. Depth comes first, and the AI assistant bridges the gap.

## Roadmap

- **v0.2 (milestone 1):**
  - One ship flying in the Earth–Moon system, on solid foundations.
  - Graphics close to final: atmosphere and lighting, heavily optimized.
  - Launch, fly, and land by parachute.
  - Blocks with made-up thrust first, then predefined ships.
- **Beyond v0.2:** defined as we go, recorded in [decisions.md](decisions.md).
