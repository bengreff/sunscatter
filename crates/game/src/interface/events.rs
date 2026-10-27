//! The ship's event log as toasts (`Vessel::ship_events`, the systems'
//! log: stages and functions run, clamps armed and released, failures,
//! aborts): each new event of the active vessel once, bad news as an
//! alert. Switching to a vessel does not replay its history.

use super::toasts::Toasts;
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

/// Toasts the active vessel's new events.
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
            toasts.push(time.elapsed_secs_f64(), format!("Vessel {}: {}", v.id().0, e.text), e.alarm);
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
