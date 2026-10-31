//! The staging bar's rows (D111) as pure data with table tests (rule 8),
//! from the craft's view of its program (`sim::craft::program`: after a
//! split, what acts on this piece): the stages still to run, the last at the top and the next at the
//! bottom, then the active stage (the last one run, or what burns now when
//! none has), then the stages run before it, the latest first, each with
//! only its name (all can be run again, D114). Each row shows, where it applies, one bar per propellant
//! (kg/kg of the tanks its engines draw on), the burnout time and the Δv
//! (`sim::systems::estimate`); the active row also the thrust, the
//! thrust-to-weight ratio, the mass now and at burnout and the mean Isp.
//! Each row lists what its commands act on as icons. Values the craft
//! cannot measure are dashes (D106). Also here: what a drop on a stage
//! edits ([`drop_edit`]).

use super::icons::{icons, Icon, Look};
use crate::format::{self, amount};
use sim::craft::command::SequenceEdit;
use sim::craft::program;
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;
use sim::systems::estimate::{Burn, Estimates};

/// Where a row's stage is in the sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Run last (or burning now).
    Active,
    /// Space runs it.
    Next,
    Later,
    /// Run before the active stage: only its name and AGAIN.
    Done,
}

/// One of a stage's commands and the icons of what it acts on.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Its index in the stage's commands (the program's).
    pub action: usize,
    pub label: String,
    pub icons: Vec<Icon>,
}

/// A propellant's bar: how full (0–1, `None` unknown) and its text.
#[derive(Clone, Debug, PartialEq)]
pub struct BarLine {
    pub fluid: String,
    pub fill: Option<f32>,
    pub text: String,
}

/// The active stage's figures.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveFigures {
    pub thrust: String,
    pub twr: String,
    /// Mass now → at burnout.
    pub mass: String,
    pub isp: String,
}

/// What a row's burn shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Figures {
    pub bars: Vec<BarLine>,
    pub burnout: String,
    pub dv: String,
    pub active: Option<ActiveFigures>,
}

/// One row of the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Its index in the sequence, the program's (`None`: what burns now
    /// before any stage).
    pub stage: Option<usize>,
    /// KSP's number: the first stage the highest, the last 0.
    pub number: Option<u32>,
    pub name: String,
    pub place: Place,
    pub items: Vec<Item>,
    pub figures: Option<Figures>,
}

/// The bar's rows, top to bottom (see the module). `chute`: the parts
/// carrying the parachutes and how they look.
pub fn rows(def: &ShipDef, state: &ShipState, est: &Estimates, chute: (&[u32], Look)) -> Vec<Row> {
    // The craft's view (`sim::craft::program`): after a split, the stages
    // and commands acting on this piece, by their indices in the program.
    let shown = program::view(def, &state.stages);
    let n = shown.len();
    let number = |k: usize| Some((n - 1 - k) as u32);
    let items = |k: usize| -> Vec<Item> {
        let stage = &state.stages[shown[k].index];
        (shown[k].actions.iter())
            .map(|&action| {
                let c = &stage.actions[action];
                Item { action, label: crate::staging::parts::label(c), icons: icons(def, state, c, chute) }
            })
            .collect()
    };
    let row = |k: usize, place: Place, burn: Option<&Burn>, active: bool| Row {
        stage: Some(shown[k].index),
        number: number(k),
        name: state.stages[shown[k].index].name.clone(),
        place,
        items: items(k),
        figures: burn.map(|b| figures(b, active)),
    };
    // The first still to run, in the view.
    let next = shown.iter().position(|v| v.index >= state.sequence as usize).unwrap_or(n);
    let mut out: Vec<Row> = (next..n)
        .rev()
        .map(|k| {
            let burn = est.stages.iter().find(|(j, _)| *j == shown[k].index).and_then(|(_, b)| b.as_ref());
            row(k, if k == next { Place::Next } else { Place::Later }, burn, false)
        })
        .collect();
    match (next.checked_sub(1), &est.now) {
        (Some(k), now) => out.push(row(k, Place::Active, now.as_ref(), true)),
        (None, Some(now)) => out.push(Row {
            stage: None,
            number: None,
            name: "Now".into(),
            place: Place::Active,
            items: Vec::new(),
            figures: Some(figures(now, true)),
        }),
        (None, None) => {}
    }
    out.extend((0..next.saturating_sub(1)).rev().map(|k| Row {
        stage: Some(shown[k].index),
        number: number(k),
        name: state.stages[shown[k].index].name.clone(),
        place: Place::Done,
        items: Vec::new(),
        figures: None,
    }));
    out
}

/// A burn's figures as text (dashes for what is not measured).
pub fn figures(b: &Burn, active: bool) -> Figures {
    let bars = (b.bars.iter())
        .map(|bar| BarLine {
            fluid: bar.fluid.clone(),
            fill: bar.kg.filter(|_| bar.capacity > 0.0).map(|r| (r.shown() / bar.capacity).clamp(0.0, 1.0) as f32),
            text: amount(bar.kg, bar.capacity),
        })
        .collect();
    let burnout = match b.time {
        None => format::DASH.to_string(),
        Some(r) if !r.value.is_finite() => "∞".into(),
        Some(r) => format::duration(r.value),
    };
    let active = active.then(|| ActiveFigures {
        thrust: format::force(b.thrust),
        twr: b.twr.map_or(format::DASH.into(), |r| format!("{:.*}", format::decimals(r.precision, 1.0, 2), r.shown())),
        mass: format!("{} → {}", format::mass(Some(b.mass)), format::mass(b.burnout_mass)),
        isp: format!("{:.0} s", b.isp),
    });
    Figures { bars, burnout, dv: format::speed_of(b.dv), active }
}

/// What is dragged on the bar: a stage, or one of a stage's commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drag {
    Stage(usize),
    Action { stage: usize, action: usize },
}

/// The sequence edit a drop of `drag` on stage `onto` makes, if any: a
/// stage moves to the stage's place, a command to the end of that stage.
pub fn drop_edit(drag: Drag, onto: usize) -> Option<SequenceEdit> {
    let u = |k: usize| k as u32;
    match drag {
        Drag::Stage(from) if from != onto => Some(SequenceEdit::Move { from: u(from), to: u(onto) }),
        Drag::Action { stage, action } if stage != onto => {
            Some(SequenceEdit::MoveAction { from: u(stage), action: u(action), to: u(onto) })
        }
        _ => None,
    }
}

/// Whether a row's stage can be edited on the bar (dragged, deleted, its
/// commands dragged, a drop onto it): the stages still to run.
pub fn editable(row: &Row) -> bool {
    matches!(row.place, Place::Next | Place::Later) && row.stage.is_some()
}

/// Whether a row has AGAIN (D114): the active stage and those run before.
pub fn runs_again(row: &Row) -> bool {
    matches!(row.place, Place::Active | Place::Done) && row.stage.is_some()
}

/// An edit of the bar, by the stages' indices in the program
/// ([`Row::stage`]): a stage dragged onto another, one of its commands
/// dragged to another stage, × (delete), AGAIN. The agent's
/// `edit_stages` (D044).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Move { stage: usize, to: usize },
    MoveCommand { stage: usize, command: usize, to: usize },
    Delete(usize),
    Again(usize),
}

/// What an [`Op`] sends: a sequence edit, or a stage to run again.
#[derive(Clone, Debug, PartialEq)]
pub enum Sent {
    Edit(SequenceEdit),
    Again(u32),
}

/// What `op` does on the bar showing `rows`, refused with why where the
/// bar would not let a player do it (the same rules: [`editable`],
/// [`runs_again`], [`drop_edit`]).
pub fn check(op: Op, rows: &[Row]) -> Result<Sent, String> {
    let row = |j: usize| rows.iter().find(|r| r.stage == Some(j)).ok_or_else(|| format!("no stage {j} on the bar"));
    let pending = |j: usize| {
        let r = row(j)?;
        editable(r).then_some(r).ok_or_else(|| format!("stage {j} ({}) has run: only AGAIN", r.name))
    };
    let dropped = |drag: Drag, to: usize| {
        pending(to)?;
        drop_edit(drag, to).map(Sent::Edit).ok_or_else(|| format!("already in stage {to}"))
    };
    match op {
        Op::Move { stage, to } => {
            pending(stage)?;
            dropped(Drag::Stage(stage), to)
        }
        Op::MoveCommand { stage, command, to } => {
            if !pending(stage)?.items.iter().any(|i| i.action == command) {
                return Err(format!("stage {stage} has no command {command} acting on this piece"));
            }
            dropped(Drag::Action { stage, action: command }, to)
        }
        Op::Delete(j) => pending(j).map(|_| Sent::Edit(SequenceEdit::Delete(j as u32))),
        Op::Again(j) => {
            let r = row(j)?;
            runs_again(r).then_some(Sent::Again(j as u32)).ok_or_else(|| format!("stage {j} ({}) has not run", r.name))
        }
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
