# Grid Rendering

Grid vertex generation for the editor building grid.

### Requirement: Grid line spacing

The editor grid SHALL have minor grid lines spaced at 0.5m (matching `GRID_SIZE`) and major grid lines spaced at 2.5m (every 5 minor lines).

### Requirement: Grid line visual distinction

Minor grid lines SHALL use color `[0.2, 0.2, 0.3, 0.5]`. Major grid lines (and lines at X=0 or Y=0) SHALL use color `[0.3, 0.3, 0.4, 0.7]`. A line is classified as major if `(position / 2.5).fract() < 0.01` or `position.abs() < 0.01`.

### Requirement: Grid line thickness

Grid line thickness in world units SHALL be `0.005 / sqrt(zoom)`, becoming thinner at higher zoom levels to maintain consistent visual weight.

### Requirement: Grid lines as quads

Each grid line SHALL be rendered as a quad (two triangles) with perpendicular thickness applied in world space. Lines with length less than 0.0001 SHALL be skipped.

### Requirement: Grid viewport culling

Only grid lines within the visible camera viewport SHALL be generated. The visible area is computed from camera offset and zoom: `half_extent = (screen_dimension / 2) / zoom`. Grid generation starts at the first grid line at or before the minimum visible coordinate.

### Requirement: Camera-relative grid output

All grid vertices SHALL be output in camera-relative coordinates (world position minus camera offset), not in absolute world coordinates. The shader expects camera-relative input.
