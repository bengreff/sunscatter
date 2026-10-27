//! CPU time per system, schedule and app thread, from the `tracing` spans
//! Bevy opens with its `trace` feature: a layer that adds up each span's
//! entered time by name. Installed only when profiling (`custom_layer`);
//! otherwise the spans are filtered out at their source and cost nothing.

use super::report::{Acc, Kind};
use bevy::log::tracing::{self, span, Subscriber};
use bevy::log::tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Time per kept span: (kind, name) → total.
type Totals = HashMap<(Kind, Arc<str>), Acc>;

/// The totals since the last [`take`].
static TOTALS: Mutex<Option<Totals>> = Mutex::new(None);

/// Takes the totals gathered since the last call.
pub fn take() -> Vec<(Kind, String, Acc)> {
    let map = TOTALS.lock().map(|mut t| t.take()).unwrap_or_default().unwrap_or_default();
    map.into_iter().map(|((k, n), a)| (k, n.to_string(), a)).collect()
}

/// Which spans are kept, and under what name: Bevy's `system`,
/// `system_commands` and `schedule` spans (their `name` field), the main
/// app's frame and the render app's (on its own thread).
pub fn classify(span: &str, name: Option<&str>) -> Option<(Kind, String)> {
    match (span, name) {
        ("system", Some(n)) => Some((Kind::System, n.to_string())),
        ("system_commands", Some(n)) => Some((Kind::Commands, n.to_string())),
        ("schedule", Some(n)) => Some((Kind::Schedule, n.to_string())),
        ("main app", _) => Some((Kind::App, "main app".into())),
        ("sub app", Some(n)) if n.contains("RenderApp") => Some((Kind::App, "render app".into())),
        _ => None,
    }
}

/// The span's key, kept in its extensions.
struct Key(Kind, Arc<str>);
/// When the span was last entered.
struct Entered(Instant);

/// Reads a span's `name` field.
#[derive(Default)]
struct NameField(Option<String>);

impl tracing::field::Visit for NameField {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

/// The layer: entered time per kept span, added up in [`TOTALS`].
pub struct SpanTimes;

impl<S> Layer<S> for SpanTimes
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span_name = attrs.metadata().name();
        if !matches!(span_name, "system" | "system_commands" | "schedule" | "main app" | "sub app") {
            return;
        }
        let mut field = NameField::default();
        attrs.record(&mut field);
        let Some((kind, name)) = classify(span_name, field.0.as_deref()) else { return };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Key(kind, name.into()));
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        if ext.get_mut::<Key>().is_some() {
            ext.replace(Entered(Instant::now()));
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let ext = span.extensions();
        let (Some(Key(kind, name)), Some(Entered(start))) = (ext.get::<Key>(), ext.get::<Entered>()) else {
            return;
        };
        let ns = start.elapsed().as_nanos() as u64;
        if let Ok(mut t) = TOTALS.lock() {
            t.get_or_insert_with(HashMap::new).entry((*kind, name.clone())).or_default().add(ns);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_spans_are_kept() {
        // (span, its name field, kept as)
        let cases = [
            ("system", Some("game::fleet::advance"), Some((Kind::System, "game::fleet::advance"))),
            ("system_commands", Some("game::x"), Some((Kind::Commands, "game::x"))),
            ("schedule", Some("Update"), Some((Kind::Schedule, "Update"))),
            ("main app", None, Some((Kind::App, "main app"))),
            ("sub app", Some("RenderApp"), Some((Kind::App, "render app"))),
            ("sub app", Some("Other"), None),
            ("system", None, None),
            ("check_conditions", Some("x"), None),
        ];
        for (span, name, want) in cases {
            let got = classify(span, name);
            assert_eq!(got.as_ref().map(|(k, n)| (*k, n.as_str())), want, "{span} {name:?}");
        }
    }
}
