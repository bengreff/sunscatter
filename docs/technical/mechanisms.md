# Mechanisms, grid fins and legs

As built for craft of catalog parts in `sim::systems::{mechanisms, fins}`, `sim::aero::lattice`, legs in `sim::contact` (D103–D106). The design: [ship-model.md](ship-model.md) §5.4. Built by RUNTIME (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [structure](../systems/structure.md).

**Status: playable.**

- **Mechanisms** move at their actuators' rates (legs 120° in 3 s, grid fins 90° in 2 s, Dragon's nosecone 100° at 1°/s); the shape (a configuration of its own) changes when they arrive. A mechanism whose actuator is lost or unpowered stays where it is.
- **Lattice fins** (`aero::lattice`): C_N = 1.2·tanh(C_Nα(M)·α/1.2) on the cells' frontal area; C_Nα 4.7, 4.1, 3.9 per radian at Mach 0.5, 1.2, 2.0 (Hiroshima & Tatsumi, ICAS 2004, wind tunnel), the **transonic choking dip** at Mach 0.95 to 65 % (Washington & Miller, AIAA 93-0035; E), ∝ 1/√(M²−1) beyond Mach 2 (E). Their frontal faces' drag stays the bake's Newtonian plate (an overstatement, noted).
- **Grid fins in flight** (`systems::fins`): deployed, each pushes along its tangent at mid-span for the flow's incidence plus its deflection; SAS and the pilot get their authority (least-squares allocation within ±20°, 30°/s); their real forces replace the commanded share, and they reach the joint loads (SYSTEMS2). The booster from 12 km at 300 m/s engines first, upset 5°: it tumbles without them and holds surface retrograde within 0.1° with them.
- **Legs** (`contact`, D066): a foot with a crush core (`Spring::crush`; the Falcon 9's 250 kN per leg, U) pushes at most its plateau while it strokes; a craft of parts' hull contact points leave out a deployed leg's end around its pad (`CellOptions::foot_reach` 0.6 m). The booster lands on one engine from 2 km at 150 m/s (hoverslam, E9 at 75–90 %): touchdown 1.1 m/s, at rest, intact.
- **Not done:** single-use crush cores (they recover), legs' self-levelling, the nosecone's aero while hinged, grid-fin heating and their pump's hydraulic fluid.

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
