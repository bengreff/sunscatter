//! The ship's event log as notifications (`Vessel::ship_events`, the
//! systems' log: stages and functions run, clamps armed and released,
//! failures, aborts): each new event of the active vessel once, linked to
//! the vessel, bad news as an alert. Switching to a vessel does not replay
//! its history, nor does loading a save (whose vessel ids are those of the
//! flight it was saved from).

use super::toasts::{Link, Toasts};
use crate::state::SimState;
use bevy::prelude::*;
use sim::systems::Event;
use sim::time::Epoch;
use sim::vessel::VesselId;
use std::collections::BTreeMap;

/// The events of `events` (oldest first) not yet shown, given the last one
/// shown (`seen`: its epoch and text): those after it, or after its epoch
/// once it has left the log; with nothing seen yet, none (a vessel's
/// history is not replayed).
pub fn unseen<'a>(events: &'a [Event], seen: Option<&(Epoch, String)>) -> &'a [Event] {
    let Some((at, text)) = seen else { return &[] };
    match events.iter().rposition(|e| e.at == *at && e.text == *text) {
        Some(k) => &events[k + 1..],
        None => {
            let first = events.iter().position(|e| e.at > *at).unwrap_or(events.len());
            &events[first..]
        }
    }
}

/// An event's text as notified: a stage run reads by the stage's name
/// alone ("MECO", "Stage separation"; the log's "Stage: " prefix is the
/// sim's, and the notification is already the vessel's); everything else
/// as logged.
pub fn wording(text: &str) -> String {
    text.strip_prefix("Stage: ").unwrap_or(text).to_string()
}

/// What has been notified: each vessel's last event shown, in the flight
/// as it was after `restores` loads (`SimState::restores`).
#[derive(Default)]
pub struct Seen {
    restores: u64,
    last: BTreeMap<VesselId, (Epoch, String)>,
}

/// The events of vessel `id` to notify now, and what is then seen. A load
/// (`restores` changed) forgets everything seen: the loaded flight's
/// vessels reuse the ids of another timeline, whose last events would hide
/// theirs (after a revert to launch, the new flight's Ignition came before
/// the old one's last event, so was never notified) or not match them.
pub fn to_notify<'a>(events: &'a [Event], id: VesselId, restores: u64, seen: &mut Seen) -> &'a [Event] {
    if seen.restores != restores {
        *seen = Seen { restores, last: BTreeMap::new() };
    }
    let known = seen.last.contains_key(&id);
    let new = if known { unseen(events, seen.last.get(&id)) } else { &[] };
    match events.last() {
        Some(last) => {
            seen.last.insert(id, (last.at, last.text.clone()));
        }
        None if !known => {
            seen.last.insert(id, (sim::sol::sol_epoch(), String::new()));
        }
        None => {}
    }
    new
}

/// Notifies the active vessel's new events.
pub fn toast_events(sim: Res<SimState>, time: Res<Time>, mut toasts: ResMut<Toasts>, mut seen: Local<Seen>) {
    let v = sim.ship();
    for e in to_notify(v.ship_events(), v.id(), sim.restores, &mut seen) {
        toasts.push_linked(time.elapsed_secs_f64(), wording(&e.text), e.alarm, Some(Link::Vessel(v.id())));
    }
}

/// Notifies each warp stop (D128): why the warp dropped, as an alert linked
/// to its vessel.
pub fn toast_warp_stops(mut sim: ResMut<SimState>, time: Res<Time>, mut toasts: ResMut<Toasts>) {
    for (id, stop) in std::mem::take(&mut sim.warp_stops) {
        toasts.push_linked(
            time.elapsed_secs_f64(),
            format!("Warp stopped: {}", stop.why),
            true,
            Some(Link::Vessel(id)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_reads_by_its_name() {
        // (event, notified)
        let table = [
            ("Stage: Ignition", "Ignition"),
            ("Stage: MECO", "MECO"),
            ("Stage: Stage separation", "Stage separation"),
            ("Stage: Second stage ignition", "Second stage ignition"),
            ("Stage: Upstage", "Upstage"),
            ("Function: Grid fins", "Function: Grid fins"),
            ("Liftoff: the clamps released at full thrust", "Liftoff: the clamps released at full thrust"),
        ];
        for (event, want) in table {
            assert_eq!(wording(event), want);
        }
    }

    #[test]
    fn each_event_once_and_no_history_replayed() {
        let t = |s: f64| sim::sol::sol_epoch().add_seconds(s);
        let e = |s: f64, text: &str| Event { at: t(s), text: text.into(), alarm: false };
        let log = [e(1.0, "Stage: Ignition"), e(2.0, "Clamps armed"), e(3.0, "Stage: Liftoff")];
        let texts = |list: &[Event]| list.iter().map(|e| e.text.clone()).collect::<Vec<_>>();
        // (last seen, new)
        type Case<'a> = (Option<(Epoch, String)>, Vec<&'a str>);
        let table: [Case; 4] = [
            (None, vec![]),
            (Some((t(1.0), "Stage: Ignition".into())), vec!["Clamps armed", "Stage: Liftoff"]),
            (Some((t(3.0), "Stage: Liftoff".into())), vec![]),
            // The last seen has left the capped log: what came after its epoch.
            (Some((t(1.5), "gone".into())), vec!["Clamps armed", "Stage: Liftoff"]),
        ];
        for (seen, want) in table {
            assert_eq!(texts(unseen(&log, seen.as_ref())), want, "{seen:?}");
        }
    }

    #[test]
    fn a_load_forgets_what_was_seen_of_another_timeline() {
        let t = |s: f64| sim::sol::sol_epoch().add_seconds(s);
        let e = |s: f64, text: &str| Event { at: t(s), text: text.into(), alarm: false };
        let texts = |list: &[Event]| list.iter().map(|e| e.text.clone()).collect::<Vec<_>>();
        let id = VesselId(1);
        let orbit = [e(10.0, "Stage: Ignition"), e(160.0, "Stage: MECO"), e(6000.0, "Stage: Dragon separation")];
        let launch: [Event; 0] = [];
        let flown = [e(10.0, "Stage: Ignition")];
        let mut seen = Seen::default();
        // (events of vessel 1, loads so far, notified)
        let steps: [(&[Event], u64, Vec<&str>); 6] = [
            (&launch, 0, vec![]),                  // the new game on the pad
            (&orbit, 1, vec![]),                   // a save in orbit loaded: its history is not replayed
            (&orbit, 1, vec![]),                   // nothing new
            (&launch, 2, vec![]),                  // reverted to launch (the launch save)
            (&flown, 2, vec!["Stage: Ignition"]),  // its Ignition, before the old timeline's last event
            (&orbit[..2], 2, vec!["Stage: MECO"]), // and what follows
        ];
        for (k, (events, restores, want)) in steps.into_iter().enumerate() {
            assert_eq!(texts(to_notify(events, id, restores, &mut seen)), want, "step {k}");
        }
    }
}
