# TLI to Moon Scenario

## Purpose

Tests the complete trans-lunar injection (TLI) flight profile: starting from Low Earth Orbit (LEO), computing a Hohmann transfer to the Moon, executing the burn, coasting through the transfer orbit, and verifying Moon SOI entry.

## Phases

### Phase 1: Verify LEO
- Ship spawns via `Ship::spawn_on_earth()` in 400 km circular orbit
- Assert: SOI body = Earth (index 4)
- Assert: altitude 350-450 km
- Screenshot: Ship in LEO with Earth

### Phase 2: Plan Transfer
- Calculate ship orbit elements
- Call `compute_hohmann()` with ship orbit → Moon orbit
- Expected: departure Δv ~830 m/s, transfer time ~4.3 days
- Assert: departure Δv in [500, 1500] m/s range

### Phase 3: Warp to Window
- Warp at 1000x to within 120s of departure window
- Fine-warp at 10x for last 120s
- Screenshot: Ship at departure position

### Phase 4: TLI Burn
- Orient prograde via autopilot (tolerance 1°, timeout 60s)
- Full throttle at 1x warp using fallback 20 m/s² acceleration
- Monitor accumulated Δv until target reached
- Cut throttle, disable autopilot
- Assert: post-burn apoapsis > 300,000 km (reaches Moon distance)
- Screenshot: Transfer orbit

### Phase 5: Coast to Moon
- Warp at 10000x in 1-hour chunks
- Check SOI body each chunk for transition to Moon (index 5)
- Timeout: 1.5x computed transfer time
- Screenshot: Moon SOI entry

### Phase 6: Verify Encounter
- Assert: SOI body = Moon (index 5)
- Calculate orbit around Moon
- Assert: periapsis < 5x Moon radius
- Log: periapsis altitude, eccentricity
- Screenshot: Moon orbit

## Expected Values

| Parameter | Expected | Range |
|-----------|----------|-------|
| LEO altitude | 400 km | 350-450 km |
| TLI Δv | ~3084 m/s | 2500-4000 m/s |
| Transfer time | ~5.0 days | varies |
| Post-burn apoapsis | ~370,000 km | >300,000 km |
| Moon periapsis | ~6,777 km | <5x Moon radius (8,685 km) |

## Pass/Fail Criteria

- PASS: All 6 phases complete, all assertions hold, 5 screenshots generated
- FAIL: Any assertion fails, Moon SOI not reached within timeout, burn incomplete

## Screenshots

Output directory: `data/test_screenshots/tli_to_moon/`

1. `01_leo.png` — Ship in LEO with Earth
2. `02_departure.png` — At transfer window departure position
3. `03_post_burn.png` — Transfer orbit after TLI burn
4. `04_moon_soi.png` — Entering Moon's SOI
5. `05_moon_orbit.png` — Orbit around Moon
