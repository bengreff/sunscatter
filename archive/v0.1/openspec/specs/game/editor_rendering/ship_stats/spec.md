# Ship Stats

Vessel statistics calculation from placed parts in the editor.

### Requirement: Dry mass calculation

Dry mass SHALL be the sum of `PartDefinition.mass` (in tonnes) for all placed parts.

### Requirement: Wet mass calculation

Wet mass SHALL be dry mass plus the sum of all resource amounts (in kg, converted to tonnes by dividing by 1000).

### Requirement: Thrust calculation

Total vacuum and sea-level thrust SHALL be the sum of `engine.thrust_vac` and `engine.thrust_asl` (in kN) across all placed engine parts.

### Requirement: Resource accounting

Resources SHALL be accumulated from all tanks with `tank_filled == true` and `fuel_type != Empty`. Each tank contributes oxygen and fuel capacity (from `propellant_capacity(fuel_type)`) to named resource entries: "oxygen", "rp1"/"methane"/"hydrogen".

### Requirement: Electricity capacity

Electricity capacity SHALL be the sum of `battery.capacity_wh` for all placed parts with a `battery` definition. The value is in Watt-hours (Wh).

### Requirement: Power generation

Power generation SHALL be the sum of solar panel output (`solar_panel.output_1au`) and RTG output (`rtg.output_watts`) for all placed parts with those definitions. The value is in Watts.

### Requirement: Power consumption

Power consumption SHALL be the sum of `pod.power_draw` for all placed pod parts. The value is in Watts.

### Requirement: TWR calculation

TWR SHALL be computed as `thrust / (wet_mass * surface_gravity)` where thrust is in kN, mass in tonnes, and gravity in m/s^2. It SHALL return 0.0 if wet mass or gravity is <= 0.
