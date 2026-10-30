# Joint loads: forces, the check, the snap

As built for craft of catalog parts in `sim::systems::structure`, gathered by `sim::vessel::ship_loads` (D103–D106). The design: [ship-model.md](ship-model.md) §5.6. Built by SYSTEMS2 (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [structure](../systems/structure.md).

**Status: playable.** Rigid until it breaks (D103).

- **Loads.** Each live and held tick every intact joint's force and moment, exact for the rigid body: the subtree beyond the joint (its child's side, the capsule being the root) is accelerated by the joint alone, F = Σ mᵢ(a + α×rᵢ + ω×(ω×rᵢ)) − F_ext, the moment about the joint's centre likewise with each part's Iᵢα + ω×Iᵢω (`subtree_loads`, one pass, deepest first). The motion is the one the gathered forces give (`consistent`: a = ΣF/M, α = I⁻¹(τ − ω×Iω)), so a joint carries the same seen from either side. The parts' masses carry their tanks' liquid and ullage gas and their bottles' gas; the pad's mount weighs nothing (ground).
- **Forces** (other than gravity): each engine's thrust at its mount along the direction the tick flew (after the gimbal); the air's hull force and moment (`AeroTick::loads`) split along the slender axis — the forebody's Newtonian pressure on each part's upstream cells, the rest of the drag (friction, base) by wetted area, the normal force by the area facing the crossflow shifted along the axis so its moment is the model's (the totals are the model's exactly; the split is an approximation, the aero model gives no per-part forces); the parachute on its part; the deployed grid fins' forces (`systems::fins`); the ground's normal force at each contact point; the pad's clamps (RUNTIME's held ticks fill the hold-down joint).
- **Check** (`section`, `ratio`): a ring joint's equivalent axial line load N = P/(2πR) ± M/(πR²) in tension against the wall's ultimate (`JointDef::allowable`), in compression against its buckling load where the build gives one (`JointDef::buckling`: a tank's ends sized for an axial load, SP-8007 with its knockdown), shear and torsion flow against the ultimate over √3. A section a tank compartment crosses is relieved by its ullage pressure and liquid head (the gas carries p·πR²) and its buckling raised by SP-8007's pressure gain Δγ; until the plumbing has regulated (`Runtime::pressure_regulated`) the ullage counts at least its loading pressure. Strength falls with temperature as the weaker side's material's ultimate does. No capacity data (an engine mount, a point attachment, an ablator wall, a part on a mechanism — a leg's A-frame and strut, a grid fin's hinge: its feet's overload is the contact's, D066): ratio 0, never breaks; its load is still shown.
- **Snap.** Ratio ≥ 1 (not in debug mode): an alarm ("snapped"/"buckled"), `Effect::Snap(child, load)` → `Breakup::torn`: the child's side tears off with what hangs from it, no separation system fires. Each part's peak ratio goes into its history (`History::peak_load`).
- **Data** (D101): the second stage's axial design load is Dragon and its trunk at the user's guide's 6 g axial limit factor (falcon9.md §6.4), 820 kN; an ablator or core wall carries no stack load (`Wall::allowable` = 0).
- **Falcon 9 numbers** (tests): lift-off: each stack joint carries the mass above × 13.6 m/s², ratios ≤ 0.24; held on the pad at full thrust the octaweb's joint carries the weight above it; max-Q 36 kPa at 4° α with the gimbal balancing: every joint ≤ 0.70 (Dragon ring 0.65); near MECO at 4.9 g the Dragon ring 0.59; the deleted demo's gravity turn (pitch program held directly, ~4° through max-Q): ring peak 0.61; a straight-up full-throttle ascent through MECO, separation and 30 s of MVac: nothing breaks.
- **Cost** (`cost_per_tick`, ignored; release, reference Mac under the night's load): per live tick on the Falcon 9, the joints 0.011 ms, the failures 0.004 ms, power and crew 0.011 ms (ship-model.md §10's budget for the whole systems step: 0.5 ms).
- **Not done:** thruster couples are not among the forces (hundreds of newtons); joints' fatigue (no data); a joint's own stiffness (rigid, D103); a leg's strut capacity (no data: legs are left to the contact).

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Plumbing: tanks, pressurant, feed, settling](plumbing.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
