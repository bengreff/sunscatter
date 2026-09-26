# Contract System

## Purpose
Contracts are the player's manual income bridge until automated trade routes take over. They require the player to fly each mission, phasing out naturally as trade routes automate income.

## Contract Types

### Payload Delivery
- Player delivers a physical payload (mass_kg) to a destination
- Payload is a `ContractPayload` object placed into cargo containers in the editor
- Payout = mass_kg * destination price_per_kg
- Completion: payload is on vessel (non-decoupled cargo container) and destination condition is met

### Tourism
- Player flies N passengers to a destination and returns safely
- Ship needs crew_capacity >= passengers
- Payout = passengers * destination tourism_price_per_seat
- Two-phase completion:
  1. Fly to destination -> `destination_reached` flag set
  2. Return to Earth, recover vessel -> contract completes on recovery
- Tourism only available for: Suborbital, LEO, Lunar Orbit

### Government Milestones
- Automatic one-time awards checked each frame
- No acceptance needed; fires when a first achievement is detected
- 9 milestones from FirstSuborbital ($5M) to FirstCrewedMars ($1B)

## Destinations

| Destination | Price/kg | Tourism/seat | Unlock Condition |
|---|---|---|---|
| Suborbital | $3K | $750K | Always |
| Low Earth Orbit | $3K | $60M | After first suborbital |
| Lunar Orbit | $20K | $125M | After first orbit |
| Lunar Surface | $50K | N/A | After lunar orbit |
| Mars Orbit | $80K | N/A | After lunar landing |
| Mars Surface | $100K | N/A | After Mars orbit |

## Pool System

- `POOL_SIZE = 5` available contracts shown to player
- ~70% payload / ~30% tourism weighted
- Refilled: at game start, on save load, after each contract completion
- Only generates contracts for unlocked destinations
- Won't generate tourism for destinations that don't support it

## Size Scaling

Payload mass ranges scale with destination completion count:
- 0-4 completions: base range
- 5-14 completions: 1x-5x scale
- 15+ completions: 5x-25x scale

Tourism passengers: 1-2 early, scaling to 4-6 with experience.

## Contract Generation

Deterministic PRNG using `hash(next_id + sim_time_bits)`. No external rand crate dependency.

## Payload Flow

1. Accept payload contract from contract board
2. In editor, cargo container UI shows "Add Payload" for unplaced payloads from active contracts
3. Payload consumes cargo capacity by mass_kg
4. Launch and fly to destination
5. Completion detected automatically (no warp stop), money awarded, notification shown
6. Pool refills

## Tourism Flow

1. Accept tourism contract
2. Fly to destination -> destination_reached flag set
3. Return to Earth, land, use "Recover Vessel" in pause menu
4. Recovery triggers tourism completion, awards payout

## Edge Cases

- Cancelled contract: matching payloads removed from editor parts
- Recovery without destination reached: tourism contracts stay active
- Payload mass > capacity: editor UI prevents placement (grayed out)
- Old saves: fresh ContractManager via serde defaults
- **New game reset**: `reset_for_new_game()` creates a fresh `ContractManager` so `awarded_milestones` don't carry over from a previous save
- **Save load milestone sync**: `restore_to_game()` calls `ContractManager::sync_milestones_from_discoveries()` to insert milestones (without payouts) for any discovery flag already set. Prevents re-awarding milestones when loading old saves created before the milestone system existed

## Key Files

- `src/colony/contracts.rs` — Data model, generation, completion logic
- `src/colony/notification.rs` — ContractCompleted, MilestoneAchieved variants
- `src/colony/tech.rs` — DiscoveryTracker (crewed discovery fields)
- `src/parts/blueprint.rs` — cargo_payloads on BlueprintPart/PlacedPart
- `src/parts/vessel.rs` — cargo_payloads on FlightPart, all_payloads()
- `src/game.rs` — check_contracts(), check_government_milestones()
- `src/render/management_ui.rs` — Contract board UI (pool-based)
- `src/editor/ui.rs` — Payload placement in cargo containers
- `src/main.rs` — Recovery hook, pool refill wiring, milestone checks
