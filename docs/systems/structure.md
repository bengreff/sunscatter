# Structure: joints, loads, mechanisms, separation, contact

What holds a craft together, what breaks it apart, and what it touches. Every joint carries its load each tick; a joint over its allowable snaps; separations, breakups and debris become vessels of their own; craft touch the ground and each other.

**Status:** joint loads, mechanisms, splits, breakage, debris and ground contact are *playable*. Craft-to-craft contact in the 10 km bubble is *playable* in the sim, wired into the game, not yet seen on screen. Docking and water are not built; both come with the first two flights (D117, D123). D121's debris lumps are next (DEBRIS in the [hand-off](../plans/HANDOFF.md)).

## Laws

- **Rigid until it breaks** (D103, D032): no flex and no wobble; mechanisms move at their actuators' rates.
- **Loads are exact for the rigid body** (D103): each joint carries what accelerates the subtree beyond it; the check is a ring joint's line load against the wall's ultimate in tension and its buckling load in compression. Ratio ≥ 1 snaps the joint (not in debug mode, D064).
- **Splits conserve** momentum and angular momentum exactly, and carry every field of the vessel by its own rule; the split epoch is deterministic, so chunked equals single pass (rule 4).
- **Contact is rigid-body contact** (D066): contact points against the physical surface, gear and hull spring-dampers; a point closing faster than its limit destroys its part.
- **Debris** (D083): a piece is a full vessel until it is dead, uncontrollable and 10 km from every controllable craft. D121 replaces this at separation with a check: a controllable piece becomes a full craft, simulated while inactive; any other becomes a ballistic lump (mass, inertia, drag area, heating, one heat node), deleted when it burns up or hits the ground.
- **The g-load limit** (D111): the proper acceleration the craft may reach is the lower of its structure's and its crew's.
- **Craft in contact advance together** on one worker (D127); bubbles share no craft, so threads change nothing.
- **Universal** (D125): joints, contact and (when built) water impact are one mechanism each, from the parts' parameters.

## What exists

- **Joint loads and the check** (`sim::systems::structure`, gathered by `sim::vessel::ship_loads`): thrust at each mount, the air's force split along the body, each part's peak ratio in its history. The Falcon 9's joints are tested at lift-off, max-Q and near MECO. *Playable*; thruster couples, fatigue and joint stiffness are not modelled.
- **Mechanisms and grid fins** (`sim::systems::{mechanisms, fins}`, `sim::aero::lattice`): legs, grid fins and Dragon's nosecone at their actuators' rates; the lattice-fin aero model with the transonic dip. *Playable.*
- **Splits, breakage and debris** (`sim::vessel::{split, breakage, debris}`): planned separations, decouplers and breakups by temperature, impact or load. *Playable.* [parts.md](../technical/parts.md)
- **Ground contact** (`sim::contact`, D066): legs with crush cores, hull points, slopes that hold or tip, bounces, slides, rest across save and load. *Playable.*
- **Craft-to-craft contact** (D082, `sim::proximity`): exact signed distances to each craft's hull, equal and opposite forces, impacts that break parts; no merging. Each bubble steps on its own thread (D127). *Playable* in the sim. [proximity.md](../technical/proximity.md)

## Not built

- **Docking** (D123): the target is one rigid body, a placeholder station; docking ports and joining two craft into one vessel are not built.
- **Water** (D123): a calm sea with slamming pressure, added mass and buoyancy at each contact point (E until checked against water-impact data). Oceans are solid today (D037).
- **The capsule–trunk joint** (the fidelity review's gap), single-use crush cores, legs' self-levelling, a leg's strut capacity.
- **Debris lumps** and the controllability check at separation (D121).
- **Capacity data for every joint** (D125): a joint with none (an engine mount, a point attachment, a part on a mechanism) never breaks today, though its load is shown.
- **Construction** of structures in space or on the ground (D118): not written.

## Owners and decisions

- Owners: [owners.md, Structure](../owners.md#structure-joints-loads-mechanisms-separation-contact).
- Decisions: D032, D037, D064, D066, D082, D083, D103, D111, D112, D117, D121, D123, D125, D127.
- Technical: [joint-loads.md](../technical/joint-loads.md), [mechanisms.md](../technical/mechanisms.md) (mechanisms, grid fins and legs), [parts.md](../technical/parts.md) (splits, breakage, debris), [proximity.md](../technical/proximity.md), [ship-model.md](../technical/ship-model.md) §5.4, §5.6.
