# Plumbing: tanks, pressurant, feed, settling

As built for craft of catalog parts in `sim::systems::plumbing` (D103–D106). The design: [ship-model.md](ship-model.md) §5.2. Built by RUNTIME (2026-10-27); the Falcon 9 Crew's numbers are tested in `sim`. The system: [propulsion](../systems/propulsion.md).

**Status: playable** (slosh pendulums not built).

- **Tanks:** pressure = the pressurant's ideal-gas pressure in the ullage + the liquid's vapour pressure; the liquid's density from its table at its temperature. The liquid warms from the air through its wetted wall (9 W/m²K: falcon9.md §3.6's 2 kW/m² on a frosted LOX tank, U) and from the sunlight its wetted wall absorbs; tanks inside components (Dragon's) are held by their heaters. Above the relief pressure (the tank's MEOP) the ullage vents, its vapour with it; at saturation the heat boils off Q/L, vented. Densified LOX warms 0.72 K per 10 min on the pad (DATA ~0.6) and its 2 % ullage vents a little helium as it expands.
- **Pressurisation** (falcon9.md §3.3): each tank's ullage is held at its loading pressure from the bottles plumbed to it, within the lines' choked flow (Cd 0.8, U); heated helium (250 K into LOX, 300 K into RP-1, U) while a pump-fed engine it feeds is lit, else at the bottle's temperature; bottles blow down isentropically and exchange heat with the liquid they sit in (τ 60 s, U); the ullage expands adiabatically, mixes, relaxes towards the liquid (τ 300 s, U). A first-stage burn holds 3.5 bar on 251 kg of helium (DATA ~260 kg). Sets `Runtime::pressure_regulated` (the structure then reads the real pressures). Ullage gas counts in the vessel's mass.
- **Feed:** NPSP = ullage + ρ·a·(liquid height + the line's drop to the inlet) − 0.05 MPa at full flow (U) − vapour pressure: 9.4 bar at the Merlins at liftoff (DATA 10.1).
- **Settling** (falcon9.md §3.5): the liquid's bulk moves along the tank's axis under the proper acceleration at the tank (centre of mass + rotation) while the Bond number exceeds 1, and disperses over the capillary time √(ρR³/σ) below it; the outlet is covered within 5 cm + 10 % of the liquid's height (U, generous). A separation system's push jolts the liquids (the stage pushed forward settles). MECO's coast moves them centimetres; after a long coast they float until settled in √(2x/a).
- **On the lattice** (standing, 60 s, from where the slow part last stepped): heat, relief, boil-off, the bottles' and ullages' exchange, dispersal. **On a coast** the tanks and bottles follow one law of time instead (D119, `systems::coast_law`; ship-model.md §5.10), which the coast segment's mass reads too, so the two agree to the bit; the lattice keeps the dispersal, power and the crew. A day on rails: the second stage's densified LOX warms 2.66 K, vents 1.4 kg of helium; at saturation it boils off 1,452 kg/day (the 60 s lattice of the instant's sunlight: 1,450); the same bits in any frames.
- **Not done:** slosh pendulums (D103's "limited give"), line transients, valves' effect on flow (prevalves are state only), propellant temperature stratification, the thermal network's wall nodes coupling to the liquids (the liquid takes its own wall's heat directly).

## The other subsystems

- [Ship commands, the sequence and the pad](ship-commands.md)
- [Engines, RCS and the gimbal](engines.md)
- [Mechanisms, grid fins and legs](mechanisms.md)
- [Knowledge: what a craft measures](knowledge.md)
- [Flight software](flight-software.md)
- [Joint loads: forces, the check, the snap](joint-loads.md)
- [Failure modes: fates, damage, bursts](failure-modes.md)
- [Power: buses, sources, shedding](power.md)
- [Crew and life support](life-support.md)
