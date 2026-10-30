# Comms: control locations, the relay network, light delay

Where the player and their agent act from, and how commands and telemetry travel (D034, D067, D068, D069). Extracted from the archived [realism-1](../archive/plans/realism-1.md) §4b–§4c and checked against the code (2026-10-30). The system: [avionics](../systems/avionics.md).

**Status:** the network, light time and the control locations are *playable* (`sim::comms`, `game::comms`, `game::commands`). D034's revision of 2026-10-30 is not built: commands still leave from wherever the player is (see [Not built](#not-built)).

## Control locations (D067)

- The player (or their agent) is at a **control location**: aboard a crewed vessel, or at a mission control site (`game::comms::Location::{Vessel, Site}`).
- The rule (`location_rule`, table-tested): at the tracking station, or flying an uncrewed vessel, the player is at mission control; flying a crewed vessel, they are aboard it. A vessel counts as crewed when its ship has crew (`ship().crew() > 0`).
- A command to the vessel the player is aboard applies at once. Any other command is scheduled at its arrival over the network, or refused with "no signal" when there is no path (`game::commands::route`). An uncrewed vessel's helm carries the light delay itself (`controls_route`); a crewed one's controls travel to its crew. Orders reach a craft of parts at their epoch (`Vessel::order_at`, [ship-commands.md](ship-commands.md)).
- The agent's tools follow the same rule ([mcp.md](mcp.md)).

## The relay network (D068)

All of it is pure functions of positions and data (`sim::comms`); nothing moves anything.

- **Sites** (`data/comms.ron`): one mission control (Houston), the Kennedy pad, the Cape's tracking stations, and the three DSN complexes (Goldstone, Madrid, Canberra), each with an antenna and an elevation mask.
- **Links:** a link exists when the line of sight is clear (bodies are their reference ellipsoids; a ground site also needs its partner above its mask) and the Shannon rate of the link budget, `B · log2(1 + P·Gt·Gr·(λ/4πd)² / (k·T·B))` at X band (1 MHz, data), is at least 10 bit/s.
- **The ground network** joins sites on one body at 0.7 c along the great circle, with no rate limit.
- **Light time** per hop solves the light-time equation (`light_time`), with the relative position through the frame tree (rule 3).
- **A path** is the least-delay chain of usable links (`best_path`, ties to the lowest index, deterministic); its rate is the slowest hop's. The game rebuilds the graph each frame and keeps a route while it stays within a margin of the best (`game::comms`).
- **Telemetry:** from a location, every other vessel is shown at its retarded state (the state whose light arrives now), with its age; without a path, its last received state, "no signal" and the time since contact.
- **The blackout** (D109): the shock layer's plasma frequency against each link's band cuts the link ([reentry.md](reentry.md)).

## Not built

- **D034's routing:** commands and programs come only from the nearest mission control, and a crewed ship nearby does not command probes. Today a command to another vessel leaves from the player's location, so from a crewed vessel a probe is commanded with the delay between them. With one mission control there is no choice of the nearest yet.
- **Programs over the network** (D034, D043): no programs exist yet.
- **Antennas from the parts** (D125): every vessel carries one antenna constant, `game::comms::VESSEL_ANTENNA` (20 dBi, 20 W).
- **Knowledge files per location** (what its computers know) and data rates limiting science or telemetry detail (realism-1 §4b–§4c).
