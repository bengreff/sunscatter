//! Nothing in the interface flickers (the owner, 2026-10-28). The one owner
//! of the rule every menu, part card, the staging bar and the systems
//! screen follow: a row's presence and a choice between near-equal options
//! change only with hysteresis or a minimum hold, never frame to frame;
//! ties are broken by a stable key.
//!
//! - [`hold`]: a list's entries stay shown for [`HOLD`] seconds after they
//!   last appeared (with their last content), in their place; a row that
//!   comes and goes every other refresh reads as always there.
//! - [`keep`]: a choice (a route, a pick) stays until another is better by
//!   a margin.
//! - [`top`]: the `n` highest of near-equal scores (the hottest parts): a
//!   listed entry leaves only when another beats it by the margin, and the
//!   order changes only past the margin; ties by key.

use std::collections::HashMap;
use std::hash::Hash;

/// Seconds an entry stays shown after it last appeared.
pub const HOLD: f64 = 1.5;

/// When each key was last seen (game-independent seconds, e.g. the app's
/// elapsed time); entries not seen for [`HOLD`] are forgotten.
#[derive(Clone, Debug, Default)]
pub struct Seen<K> {
    at: HashMap<K, f64>,
}

impl<K: Hash + Eq + Clone> Seen<K> {
    fn mark(&mut self, k: &K, now: f64) {
        self.at.insert(k.clone(), now);
    }

    fn held(&self, k: &K, now: f64) -> bool {
        self.at.get(k).is_some_and(|t| now - t < HOLD)
    }

    fn prune(&mut self, now: f64) {
        self.at.retain(|_, t| now - *t < HOLD);
    }
}

/// What to show now: `new`, plus the entries of what was shown (`shown`)
/// that `new` lacks but were seen within [`HOLD`], each after the entry it
/// followed. `key` names an entry (unique within a list); `merge` combines
/// an entry as shown (if it was) and its new self (a section's rows held in
/// turn), or just returns the new one.
pub fn hold<T: Clone, K: Hash + Eq + Clone>(
    shown: &[T],
    new: Vec<T>,
    key: impl Fn(&T) -> K,
    mut merge: impl FnMut(Option<&T>, T) -> T,
    seen: &mut Seen<K>,
    now: f64,
) -> Vec<T> {
    for t in &new {
        seen.mark(&key(t), now);
    }
    let mut out: Vec<T> = new
        .into_iter()
        .map(|t| {
            let before = shown.iter().find(|s| key(s) == key(&t));
            merge(before, t)
        })
        .collect();
    // The ones that vanished: back after the entry before them.
    let mut after: Option<K> = None;
    for s in shown {
        let k = key(s);
        if !out.iter().any(|t| key(t) == k) && seen.held(&k, now) {
            let at = after.as_ref().and_then(|a| out.iter().position(|t| key(t) == *a)).map_or(0, |i| i + 1);
            out.insert(at, s.clone());
        }
        if out.iter().any(|t| key(t) == k) {
            after = Some(k);
        }
    }
    seen.prune(now);
    out
}

/// Whether to keep the current choice (cost `current`, `None` when it is no
/// longer possible) over the best one now (`best`): kept while it costs no
/// more than `margin` (relative) above the best.
pub fn keep(current: Option<f64>, best: f64, margin: f64) -> bool {
    current.is_some_and(|c| c <= best + margin * best.abs())
}

/// The `n` highest-scoring candidates (key, score), given those listed
/// before (`listed`, in their order): a listed one stays unless another
/// scores more than `margin` (absolute) above it, and two listed swap
/// places only past the margin. New ones enter by score, ties by key.
pub fn top<K: Ord + Clone>(listed: &[K], candidates: &[(K, f64)], n: usize, margin: f64) -> Vec<K> {
    let score = |k: &K| candidates.iter().find(|(c, _)| c == k).map(|c| c.1);
    let mut by_score: Vec<&(K, f64)> = candidates.iter().collect();
    by_score.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut out: Vec<(K, f64)> = listed.iter().filter_map(|k| Some((k.clone(), score(k)?))).take(n).collect();
    for (k, s) in by_score {
        if out.iter().any(|(o, _)| o == k) {
            continue;
        }
        if out.len() < n {
            out.push((k.clone(), *s));
            continue;
        }
        // Replace the weakest listed one only past the margin.
        let weakest =
            (0..out.len()).min_by(|&a, &b| out[a].1.total_cmp(&out[b].1).then_with(|| out[b].0.cmp(&out[a].0)));
        if let Some(w) = weakest.filter(|&w| *s > out[w].1 + margin) {
            out.remove(w);
            out.push((k.clone(), *s));
        }
    }
    // Order: highest first, a swap only past the margin (bubble passes).
    let mut moved = true;
    while moved {
        moved = false;
        for i in 1..out.len() {
            if out[i].1 > out[i - 1].1 + margin {
                out.swap(i - 1, i);
                moved = true;
            }
        }
    }
    out.into_iter().map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(
        shown: &[(&'static str, u32)],
        new: &[(&'static str, u32)],
        seen: &mut Seen<&'static str>,
        now: f64,
    ) -> Vec<(&'static str, u32)> {
        hold(shown, new.to_vec(), |r| r.0, |_, t| t, seen, now)
    }

    /// A row that comes and goes every refresh stays, in its place, with
    /// its last value; one gone for longer than the hold goes.
    #[test]
    fn a_row_that_comes_and_goes_stays_in_its_place() {
        let mut seen = Seen::default();
        let mut shown = step(&[], &[("A", 1), ("VIA", 2), ("B", 3)], &mut seen, 0.0);
        // (time, rows given, rows shown)
        type Rows = &'static [(&'static str, u32)];
        let table: [(f64, Rows, Rows); 5] = [
            (0.1, &[("A", 1), ("B", 4)], &[("A", 1), ("VIA", 2), ("B", 4)]),
            (0.2, &[("A", 1), ("VIA", 5), ("B", 4)], &[("A", 1), ("VIA", 5), ("B", 4)]),
            (0.3, &[("B", 4)], &[("A", 1), ("VIA", 5), ("B", 4)]),
            (1.6, &[("B", 4)], &[("A", 1), ("VIA", 5), ("B", 4)]),
            (1.85, &[("B", 4)], &[("B", 4)]),
        ];
        for (now, given, want) in table {
            shown = step(&shown, given, &mut seen, now);
            assert_eq!(shown, want, "at {now}");
        }
        // A new row shows at once, where it is given.
        shown = step(&shown, &[("C", 1), ("B", 4)], &mut seen, 2.0);
        assert_eq!(shown, [("C", 1), ("B", 4)]);
    }

    /// Nested lists (a menu's sections and their rows) hold at both levels.
    #[test]
    fn sections_and_their_rows_hold_together() {
        type S = (&'static str, Vec<&'static str>);
        let mut outer: Seen<&'static str> = Seen::default();
        let mut inner: Seen<(&'static str, &'static str)> = Seen::default();
        let mut run = |shown: &[S], new: Vec<S>, now: f64| {
            hold(
                shown,
                new,
                |s| s.0,
                |old, s| {
                    let id = s.0;
                    let rows = old.map_or(&[][..], |o| o.1.as_slice());
                    (id, hold(rows, s.1, |r| (id, *r), |_, r| r, &mut inner, now))
                },
                &mut outer,
                now,
            )
        };
        let shown = run(&[], vec![("summary", vec!["IMPACT IN"]), ("parachute", vec!["MAINS", "q / DESIGN q"])], 0.0);
        let shown = run(&shown, vec![("parachute", vec!["MAINS"])], 0.1);
        assert_eq!(shown, [("summary", vec!["IMPACT IN"]), ("parachute", vec!["MAINS", "q / DESIGN q"])]);
    }

    #[test]
    fn a_choice_changes_only_past_the_margin() {
        // (current, best, kept)
        let table = [
            (Some(1.0), 1.0, true),
            (Some(1.005), 1.0, true),
            (Some(1.02), 1.0, false),
            (None, 1.0, false),
            (Some(0.0), 0.0, true),
        ];
        for (current, best, want) in table {
            assert_eq!(keep(current, best, 0.01), want, "{current:?} vs {best}");
        }
    }

    /// The hottest parts on the pad: near-equal scores keep their list and
    /// order; one clearly hotter enters, ties by name.
    #[test]
    fn the_top_n_changes_only_past_the_margin() {
        let c = |v: &[(&'static str, f64)]| v.to_vec();
        // (listed, candidates, want)
        type Keys = &'static [&'static str];
        type Case = (Keys, Vec<(&'static str, f64)>, Keys);
        let table: [Case; 5] = [
            (&[], c(&[("b", 0.25), ("a", 0.25), ("c", 0.24)]), &["a", "b"]),
            (&["a", "b"], c(&[("a", 0.250), ("b", 0.251), ("c", 0.2505)]), &["a", "b"]),
            (&["a", "b"], c(&[("a", 0.250), ("b", 0.251), ("c", 0.30)]), &["c", "b"]),
            (&["a", "b"], c(&[("a", 0.20), ("b", 0.25), ("c", 0.19)]), &["b", "a"]),
            (&["a", "gone"], c(&[("a", 0.20), ("b", 0.19)]), &["a", "b"]),
        ];
        for (listed, candidates, want) in table {
            let listed: Vec<&str> = listed.to_vec();
            assert_eq!(top(&listed, &candidates, 2, 0.01), want, "{listed:?} {candidates:?}");
        }
    }
}
