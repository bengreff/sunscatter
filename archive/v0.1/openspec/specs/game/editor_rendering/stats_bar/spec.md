# Stats Bar

The statistics bar below the toolbar showing vessel mass, thrust, TWR, delta-v, and resource totals.

### Requirement: Stats bar layout

The stats bar SHALL appear as a top panel below the toolbar with a dark background (`rgba(25, 30, 40, 240)`) and horizontal layout. Statistics SHALL be displayed at heading text size.

### Requirement: Mass display

The stats bar SHALL display total wet mass in tonnes as "Mass: {value} t" with 2 decimal places.

### Requirement: Thrust display

The stats bar SHALL display total thrust in kN as "Thrust: {value} kN" with 1 decimal place. The displayed thrust SHALL be ASL or vacuum depending on the TWR settings toggle.

### Requirement: TWR display with body selector

The stats bar SHALL display TWR as "TWR: {value}" with 2 decimal places. A combo box SHALL allow selecting the reference body for gravity. The default body index SHALL be 3 (Earth) with `show_asl = true`. An ASL/Vac toggle button SHALL switch between atmospheric and vacuum values for both thrust and TWR.

### Requirement: Delta-v in stats bar

When total delta-v > 0, the stats bar SHALL display "dv: {formatted}" after a separator. Delta-v SHALL be formatted as "{value} m/s" when < 1000 m/s, or "{value} km/s" with 1 decimal when >= 1000 m/s.

### Requirement: Vessel cost display

When vessel cost > 0, the stats bar SHALL display "Cost: {formatted}" after delta-v. The cost SHALL be colored green (`rgb(100, 200, 100)`) when affordable (cost <= company money) or red (`rgb(220, 80, 80)`) when too expensive.

### Requirement: Resource totals display

The stats bar SHALL display resource totals at body text size in consistent order: O2 (oxygen), RP1 (rp1), CH4 (methane), LH2 (hydrogen). Only resources that exist in the vessel SHALL be shown. Mass values SHALL be formatted as "{value} t" for >= 1000 kg, or "{value} kg" for < 1000 kg.

### Requirement: Power stats display

When the vessel has any electricity capacity, power generation, or power consumption > 0, the stats bar SHALL display after a separator:
- "EC: {value}k Wh" for capacity >= 1000 Wh, or "EC: {value} Wh" otherwise
- "Power: +{generation}W / -{consumption}W"
- **WHEN** net power (generation - consumption) < 0 AND electricity capacity > 0, "Duration: {formatted}" SHALL be shown
- Duration formula: `seconds = (electricity_capacity / |net_watts|) * 3600`
- Duration formatted as `format_duration()`: "Xd Xh Xm Xs" / "Xh Xm Xs" / "Xm Xs" / "Xs"
