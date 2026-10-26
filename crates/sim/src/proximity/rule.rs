//! The bubble rule (D082): which craft fly live because they are near a
//! controllable craft in live physics. One pure function ([`members`]),
//! evaluated at every tick epoch of the bubble's lattice
//! (`sim::vessel::bubble`), never per frame.
//!
//! * A **leader** is controllable (D083: intact, a control part, power) and
//!   live for its own reasons (thrust, air, the ground): it would not
//!   coast now. A craft held live by the bubble does not lead, so two
//!   controllable craft cannot keep each other live once both would coast.
//! * A flying craft (live or coasting; landed and crashed craft stay
//!   frozen) is a **member** while within [`BubbleRule::enter`] of a
//!   leader, or [`BubbleRule::leave`] if it already was one (hysteresis: a
//!   craft on the edge does not alternate between coast and live).
//! * A member that touched another craft within the last
//!   [`BubbleRule::hold_ticks`] stays a member (coasts do not collide: a
//!   pair in contact must separate before going back on rails).
//!
//! Distances are between centres of mass through the frame tree (rule 3).

use serde::{Deserialize, Serialize};

/// The rule's numbers (data, D082).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BubbleRule {
    /// A craft this close to a leader joins (m).
    pub enter: f64,
    /// A member this far from every leader leaves (m).
    pub leave: f64,
    /// Ticks after its last craft contact a member is held.
    pub hold_ticks: u32,
}

/// D082's bubble: 10 km, left at 10.5 km; a contact holds for 1 s.
pub const BUBBLE: BubbleRule = BubbleRule { enter: 10_000.0, leave: 10_500.0, hold_ticks: 50 };

/// What a vessel carries of the bubble between ticks (saved with it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BubbleState {
    /// In the bubble at the last tick epoch.
    pub member: bool,
    /// Ticks since its last contact with another craft (`None`: none
    /// while a member).
    pub since_contact: Option<u32>,
}

impl BubbleState {
    /// Not in a bubble (what a save leaves out).
    pub fn is_idle(&self) -> bool {
        *self == BubbleState::default()
    }
}

/// What the rule reads of one vessel at a tick epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Live or coasting (not landed or crashed).
    pub flying: bool,
    /// Controllable and live for its own reasons.
    pub leader: bool,
    /// Its state from the last epoch.
    pub state: BubbleState,
}

/// Which candidates are in the bubble at this epoch, given the distance
/// (m) between candidates `i` and `j`. Leaders are members.
pub fn members(rule: &BubbleRule, c: &[Candidate], distance: impl Fn(usize, usize) -> f64) -> Vec<bool> {
    (0..c.len())
        .map(|i| {
            let me = &c[i];
            let radius = if me.state.member { rule.leave } else { rule.enter };
            let near = |j: usize| j != i && c[j].flying && c[j].leader && distance(i, j) < radius;
            let held = me.state.member && me.state.since_contact.is_some_and(|t| t < rule.hold_ticks);
            me.flying && (me.leader || held || (0..c.len()).any(near))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(flying: bool, leader: bool, member: bool, since_contact: Option<u32>) -> Candidate {
        Candidate { flying, leader, state: BubbleState { member, since_contact } }
    }

    /// One leader (index 0, live and controllable) and one other craft at
    /// a distance: (other craft, distance, joins).
    #[test]
    fn membership_table() {
        let lead = cand(true, true, true, None);
        let cases = [
            // Joining: strictly within 10 km.
            (cand(true, false, false, None), 9_999.0, true),
            (cand(true, false, false, None), 10_000.0, false),
            (cand(true, false, false, None), 10_200.0, false),
            // Hysteresis: a member leaves only beyond 10.5 km.
            (cand(true, false, true, None), 10_200.0, true),
            (cand(true, false, true, None), 10_499.0, true),
            (cand(true, false, true, None), 10_500.0, false),
            (cand(true, false, true, None), 20_000.0, false),
            // Landed or crashed craft stay frozen.
            (cand(false, false, false, None), 10.0, false),
            (cand(false, false, true, None), 10.0, false),
            // A contact holds a member for 50 ticks, wherever the leader is.
            (cand(true, false, true, Some(0)), 20_000.0, true),
            (cand(true, false, true, Some(49)), 20_000.0, true),
            (cand(true, false, true, Some(50)), 20_000.0, false),
            // Holding needs membership (a stale counter does nothing).
            (cand(true, false, false, Some(0)), 20_000.0, false),
            // A leader is a member wherever it is.
            (cand(true, true, false, None), 1e9, true),
        ];
        for (other, d, joins) in cases {
            let c = [lead, other];
            let m = members(&BUBBLE, &c, |_, _| d);
            assert_eq!(m, vec![true, joins], "{other:?} at {d} m");
        }
    }

    /// Who leads: only leaders gather members; a member never extends the
    /// bubble (no chains), and a landed leader gathers none.
    #[test]
    fn only_flying_leaders_gather_members() {
        // Positions on a line (km): 0 leader, 8 member, 16 beyond the
        // leader but within 10 km of the member.
        let x = [0.0, 8_000.0, 16_000.0];
        let d = |i: usize, j: usize| f64::abs(x[i] - x[j]);
        let c = [cand(true, true, true, None), cand(true, false, true, None), cand(true, false, false, None)];
        assert_eq!(members(&BUBBLE, &c, d), vec![true, true, false]);
        // Two controllable craft that would both coast: nobody leads.
        let c = [cand(true, false, true, None), cand(true, false, true, None), cand(true, false, false, None)];
        assert_eq!(members(&BUBBLE, &c, d), vec![false, false, false]);
        // A leader that is not flying (landed) gathers nobody.
        let c = [cand(false, true, false, None), cand(true, false, false, None), cand(true, false, false, None)];
        assert_eq!(members(&BUBBLE, &c, d), vec![false, false, false]);
        // Two leaders: each gathers its own neighbours.
        let c = [cand(true, true, false, None), cand(true, false, false, None), cand(true, true, false, None)];
        assert_eq!(members(&BUBBLE, &c, d), vec![true, true, true]);
    }
}
