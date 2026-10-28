//! The ship's event log as notifications (`Vessel::ship_events`, the
//! systems' log: stages and functions run, clamps armed and released,
//! failures, aborts): each new event of the active vessel once, linked to
//! the vessel, bad news as an alert. Switching to a vessel does not replay
//! its history.

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

/// Notifies the active vessel's new events.
pub fn toast_events(
    sim: Res<SimState>,
    time: Res<Time>,
    mut toasts: ResMut<Toasts>,
    mut seen: Local<BTreeMap<VesselId, (Epoch, String)>>,
) {
    let v = sim.ship();
    let events = v.ship_events();
    let known = seen.contains_key(&v.id());
    if known {
        for e in unseen(events, seen.get(&v.id())) {
            toasts.push_linked(time.elapsed_secs_f64(), wording(&e.text), e.alarm, Some(Link::Vessel(v.id())));
        }
    }
    match events.last() {
        Some(last) => {
            seen.insert(v.id(), (last.at, last.text.clone()));
        }
        None if !known => {
            seen.insert(v.id(), (sim::sol::sol_epoch(), String::new()));
        }
        None => {}
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
}
