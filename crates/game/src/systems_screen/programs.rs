//! The ship computer's programs (D126: "the autopilot is a switch with a
//! config menu"; D128 layer 2): each automatic sequence's switch (by
//! itself, on a press, or off), what it does, what it is doing now, and
//! what a press does now. One model (rule 8: pure, table tests) for the
//! systems screen's PROGRAMS tab, the menus' rows (Maneuver, Landing ▸
//! PARACHUTE) and MCP (`read {programs}`, `set_switch`, `press`); a switch
//! or a press becomes a command by one rule ([`set_switch`], [`press`]),
//! whoever gives it.
//!
//! The programs are the sim's (`Vessel::set_switch`, `Vessel::press`): the
//! planned burns' (`maneuver`), the parachutes' deploy (`chutes`, on a
//! craft with canopies) and each scheduled program installed on the
//! computer (`ship_os::program::Installed`, by its name).

use crate::commands::GameCommand;
use crate::format::{distance, duration};
use crate::interface::menus::Tone;
use sim::chute::{CanopyState, ChuteSpec, ChuteState};
use sim::systems::maneuver::Step;
use sim::systems::ship_os::program::{Builtin, Installed, Schedule, Slot};
use sim::systems::ship_os::{chutes, Switch};
use sim::time::Epoch;
use sim::vessel::{Vessel, VesselId};

/// The switch's positions: as the player reads them, and as MCP names them.
pub const SWITCHES: [(Switch, &str, &str); 3] =
    [(Switch::Auto, "AUTO", "auto"), (Switch::Press, "ON PRESS", "press"), (Switch::Off, "OFF", "off")];

/// A switch's label (AUTO, ON PRESS, OFF).
pub fn label(s: Switch) -> &'static str {
    SWITCHES.iter().find(|x| x.0 == s).map_or("", |x| x.1)
}

/// A switch's name for MCP (auto, press, off).
pub fn name(s: Switch) -> &'static str {
    SWITCHES.iter().find(|x| x.0 == s).map_or("", |x| x.2)
}

/// A switch by its MCP name.
pub fn by_name(n: &str) -> Option<Switch> {
    SWITCHES.iter().find(|x| x.2 == n).map(|x| x.0)
}

/// One program as the player reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Its name on the ship computer (`Vessel::set_switch`).
    pub name: String,
    pub title: String,
    /// What it does when it runs: its configuration.
    pub does: String,
    pub switch: Switch,
    /// What it is doing now.
    pub now: String,
    pub tone: Tone,
    /// What a press does now, or why there is nothing to press.
    pub press: Result<String, String>,
}

/// The burn the maneuver program waits on or flies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    /// As the ship's log counts burns (from 1).
    pub number: usize,
    pub step: Step,
    /// Seconds to its wake (negative: passed).
    pub wake_in: f64,
}

/// A craft's parachutes as the deploy reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct Chutes {
    pub spec: ChuteSpec,
    pub deploy: chutes::Chutes,
    pub state: ChuteState,
    /// The craft has no altitude reading (`Vessel::chute_deploy`).
    pub blind: bool,
}

/// What the rows read.
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    /// A flight computer works (`ship_os::Health::computing`): a scheduled
    /// program misses its epochs without one.
    pub computing: bool,
    pub maneuver: Slot,
    pub burn: Option<Burn>,
    /// `None`: the craft has no parachutes.
    pub chutes: Option<Chutes>,
    pub installed: Vec<Installed>,
    pub now: Epoch,
}

impl Inputs {
    /// Vessel `v`'s programs at `now` (`None`: not a craft of parts).
    pub fn of(v: &Vessel, now: Epoch) -> Option<Inputs> {
        v.ship()?;
        let software = v.software();
        let burn = v.program().map(|p| Burn { number: p.node + 1, step: p.step, wake_in: p.wake.seconds_since(now) });
        let chutes = v.craft.design().parts.iter().any(|p| p.chute).then(|| Chutes {
            spec: v.craft.chute.clone(),
            deploy: software.chutes,
            state: v.chute,
            blind: v.chute_deploy().blind,
        });
        Some(Inputs {
            computing: v.flight_health().computing(),
            maneuver: software.maneuver,
            burn,
            chutes,
            installed: software.programs.clone(),
            now,
        })
    }
}

/// Every program aboard, in the order the screen lists them: the planned
/// burns, the parachutes, then the installed ones.
pub fn rows(x: &Inputs) -> Vec<Row> {
    let mut out = vec![maneuver(x.maneuver, x.burn)];
    out.extend(x.chutes.as_ref().map(parachutes));
    out.extend(x.installed.iter().map(|p| installed(p, x.now, x.computing)));
    out
}

/// The planned burns' program (`maneuver`).
fn maneuver(slot: Slot, burn: Option<Burn>) -> Row {
    let (now, tone, press) = match burn {
        None => ("no burn planned".to_string(), Tone::Dim, Err("no burn planned".to_string())),
        Some(b) if b.step != Step::Wait => {
            (format!("burn {}: {}", b.number, step_label(b.step)), Tone::Good, Err("it is flying a burn".into()))
        }
        Some(b) => {
            let n = b.number;
            let due = if b.wake_in >= 0.0 {
                format!("wakes in {}", duration(b.wake_in))
            } else {
                format!("was due {} ago", duration(-b.wake_in))
            };
            match (slot.switch, slot.pressed) {
                (Switch::Auto, _) => (format!("burn {n} {due}"), Tone::Normal, Err("it runs by itself".into())),
                (Switch::Press, true) => (
                    format!("pressed: flies burn {n} when it wakes ({due})"),
                    Tone::Good,
                    Err(format!("pressed already: burn {n} flies when it wakes")),
                ),
                (Switch::Press, false) => {
                    (format!("burn {n} {due}: waits for a press"), Tone::Warn, Ok(format!("fly burn {n}")))
                }
                (Switch::Off, _) => {
                    (format!("off: burn {n} is not flown ({due}); fly it by hand"), Tone::Dim, Err(off()))
                }
            }
        }
    };
    Row {
        name: "maneuver".into(),
        title: "Planned burns".into(),
        does: "flies each planned burn to its node: turns, settles the propellant, ignites, cuts off on the target"
            .into(),
        switch: slot.switch,
        now,
        tone,
        press,
    }
}

/// What the maneuver program is doing, in the menus' words (the ship's log
/// tells how a burn ended).
pub fn step_label(step: Step) -> &'static str {
    match step {
        Step::Wait => "waiting",
        Step::Turn => "turning to the burn",
        Step::Settle => "settling the propellant",
        Step::Burn => "burning",
        Step::TailOff => "cutting off",
        Step::Done => "on target",
        Step::Manual => "taken over",
        Step::Short => "ended short",
    }
}

fn off() -> String {
    "switched off: set it to ON PRESS or AUTO".into()
}

/// The parachutes' deploy (`chutes`; `ship_os::chutes`' rules).
fn parachutes(c: &Chutes) -> Row {
    let (spec, d, s) = (&c.spec, &c.deploy, &c.state);
    let main = distance(spec.main_height);
    let arm = chutes::arm_height(spec).map_or_else(|| "its height".to_string(), distance);
    let drogue = spec.drogue.is_some();
    let does = if drogue {
        format!(
            "AUTO arms the drogues below {arm} while descending; the mains open below {main} by the altitude \
             reading, or on a second press"
        )
    } else {
        format!("AUTO opens the mains below {arm} while descending; a press opens them")
    };
    let (now, tone) = match s.main {
        CanopyState::Open { .. } => ("mains open".to_string(), Tone::Good),
        CanopyState::Failed => ("mains torn".into(), Tone::Alert),
        CanopyState::Released => ("mains cut away".into(), Tone::Dim),
        CanopyState::Stowed if s.armed => {
            if d.main_called || !drogue {
                ("armed: the mains open once the drogues let them".into(), Tone::Good)
            } else if d.switch == Switch::Off {
                ("armed: the mains open on a second press".into(), Tone::Warn)
            } else if c.blind {
                ("armed: no altitude reading, so the mains open on a second press".into(), Tone::Warn)
            } else {
                (format!("armed: the mains open below {main}"), Tone::Good)
            }
        }
        CanopyState::Stowed => match d.switch {
            Switch::Auto if c.blind => {
                ("stowed: no altitude reading, so it cannot arm by itself: press to arm".into(), Tone::Warn)
            }
            Switch::Auto => (format!("stowed: arms by itself below {arm} while descending"), Tone::Normal),
            Switch::Press => ("stowed: arms on a press".into(), Tone::Normal),
            Switch::Off => ("stowed: nothing opens by itself; each press opens the next canopy".into(), Tone::Dim),
        },
    };
    // A press is the manual deploy (D126: the same function): it arms, or,
    // armed, calls the main.
    let press = match (s.armed, s.main) {
        (false, _) if drogue => Ok("arm the drogues".to_string()),
        (false, _) => Ok("open the mains".into()),
        (true, CanopyState::Stowed) if drogue && !d.main_called => {
            Ok("open the mains once the drogues let them".into())
        }
        _ => Err("nothing left to open".into()),
    };
    Row { name: "chutes".into(), title: "Parachutes".into(), does, switch: d.switch, now, tone, press }
}

/// A scheduled program installed on the computer, by its kind.
fn installed(p: &Installed, now: Epoch, computing: bool) -> Row {
    let next = p.schedule.next(p.last).map(|e| e.seconds_since(now));
    let when = next.map_or("nothing due".to_string(), |s| {
        if s > 0.0 {
            format!("next run in {}", duration(s))
        } else {
            "due now".into()
        }
    });
    let (title, does, state, tone) = match &p.program {
        Builtin::PowerGuard(g) => {
            let every = match p.schedule {
                Schedule::Every { period, .. } => format!(", every {}", duration(period)),
                Schedule::At(_) => String::new(),
            };
            let does = format!(
                "switches {} off below {:.0} % of the charge it reads in the batteries, on again at {:.0} %{every}",
                g.loads.join(", "),
                g.low * 100.0,
                g.high * 100.0
            );
            let (state, tone) = if g.shed {
                (format!("{} off until {:.0} %", g.loads.len(), g.high * 100.0), Tone::Warn)
            } else {
                ("loads on".to_string(), Tone::Normal)
            };
            ("Load manager", does, state, tone)
        }
    };
    let (now_text, tone, press) = match (computing, p.slot.switch, p.slot.pressed) {
        (false, ..) => (
            format!("{state}; no working flight computer: it misses its epochs"),
            Tone::Alert,
            Err("no working flight computer".to_string()),
        ),
        (true, Switch::Auto, _) => (format!("{state}; {when}"), tone, Err("it runs by itself".into())),
        (true, Switch::Press, true) => {
            (format!("{state}; pressed: runs at its next epoch ({when})"), Tone::Good, Err("pressed already".into()))
        }
        (true, Switch::Press, false) => {
            (format!("{state}; waits for a press"), tone, Ok(format!("run once at its next epoch ({when})")))
        }
        (true, Switch::Off, _) => (format!("{state}; off"), Tone::Dim, Err(off())),
    };
    Row { name: p.name.clone(), title: title.into(), does, switch: p.slot.switch, now: now_text, tone, press }
}

/// The row of `program` among `rows`, or why there is none (naming those
/// there are).
pub fn find<'a>(rows: &'a [Row], program: &str) -> Result<&'a Row, String> {
    rows.iter().find(|r| r.name == program).ok_or_else(|| {
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        format!("no program `{program}` aboard (it has {})", names.join(", "))
    })
}

/// The command that sets `program`'s switch on `vessel` (its `rows`): the
/// screen's, the menus' and MCP's one rule.
pub fn set_switch(rows: &[Row], vessel: VesselId, program: &str, switch: Switch) -> Result<GameCommand, String> {
    let r = find(rows, program)?;
    Ok(GameCommand::SetSwitch { vessel, program: r.name.clone(), switch })
}

/// The command that presses for `program` on `vessel` (its `rows`), or why
/// a press does nothing now.
pub fn press(rows: &[Row], vessel: VesselId, program: &str) -> Result<GameCommand, String> {
    let r = find(rows, program)?;
    match &r.press {
        Ok(_) => Ok(GameCommand::Press { vessel, program: r.name.clone() }),
        Err(why) => Err(format!("{}: nothing to press ({why})", r.title)),
    }
}

#[cfg(test)]
#[path = "programs_tests.rs"]
mod tests;
