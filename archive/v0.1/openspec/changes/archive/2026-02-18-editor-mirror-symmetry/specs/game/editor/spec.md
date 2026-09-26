## MODIFIED Requirements

### Requirement: Symmetry mode cycling

The symmetry mode SHALL toggle between Off and Mirror via the R key or the toolbar button. The current mode SHALL be displayed in the toolbar.

#### Scenario: Symmetry mode display

- **WHEN** the symmetry mode is `Mirror`
- **THEN** the toolbar SHALL display "Mirror"

#### Scenario: Symmetry mode toggle

- **WHEN** the symmetry mode is `Off` and the user presses R
- **THEN** the symmetry mode SHALL change to `Mirror`

#### Scenario: Symmetry mode toggle back

- **WHEN** the symmetry mode is `Mirror` and the user presses R
- **THEN** the symmetry mode SHALL change to `Off`
