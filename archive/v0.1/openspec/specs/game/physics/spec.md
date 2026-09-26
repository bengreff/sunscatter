# Physics

Physical interactions between the ship and its environment: atmospheric effects (drag, heating), collisions with celestial bodies, and collisions with other craft. Covers all situations where the ship comes into contact with other matter.

## Capabilities

- [Atmosphere Model](atmosphere_model/spec.md) - Atmospheric density/pressure profiles, body rotation for surface-relative airspeed
- [Aerodynamic Drag](aerodynamic_drag/spec.md) - Orientation-dependent drag force opposing surface-relative velocity
- [Aerodynamic Heating](aerodynamic_heating/spec.md) - Convective heating, radiative cooling, thermal destruction, and heat HUD
- [Terrain Collision](terrain_collision/spec.md) - Collision detection with celestial body surfaces and launchpad structures
- [Craft Collision](craft_collision/spec.md) - OBB collision detection and response between vessels
- [Fuel System](fuel/spec.md) - Fuel zones, consumption, and drain priority (asparagus/onion staging)
- [Relativity](relativity/spec.md) - Threshold-based special and gravitational relativity (thrust limiting, time dilation, HUD)
