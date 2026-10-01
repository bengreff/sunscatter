//! Parachutes: a drogue and a reefed main, deployed in sequence, filling
//! over time and failing when overloaded (the owner of the chute rules;
//! the vessel wires them up, `aero` turns the drag area into force).
//!
//! * **Sequence:** the command arms it. The drogue opens at once; the
//!   main opens when the craft is below `main_height` above the ground
//!   and the drogue is out of its reefed stages (or gone), and the drogue
//!   is cut away at that moment (Apollo, Orion and Soyuz all fly this
//!   order; sources in the craft file). A chute without a drogue opens
//!   its main on the command.
//! * **Reefing:** each canopy opens to its reefed stages in turn, on
//!   timers from line stretch (pyrotechnic reefing-line cutters), then to
//!   full.
//! * **Filling:** a canopy's mouth grows at V/n (Knacke's filling time
//!   t_f = n·D/V, *Parachute Recovery Systems Design Manual*, NWC TP 6575,
//!   1991, §5), from its last stage's diameter to the next; the drag area
//!   goes with the diameter squared.
//! * **Strength:** a canopy whose drag q·Cd·A exceeds its `max_load` fails
//!   (torn canopy or broken risers) and is lost: deployed too fast, the
//!   opening load breaks it (D079). Nothing refuses the command, and
//!   `deploy_max_q` is no cliff: real canopies hold above their design
//!   load (Apollo's mains passed ultimate-load tests at 1.39× it, TN
//!   D-7437; a CPAS main took 40,000 lb, above its design limit,
//!   undamaged, NTRS 20110011562). The opening load is q·Cd·A as the
//!   canopy fills, without the added-mass overshoot of a real opening
//!   (Knacke's opening-force factor Cx); a cluster shares it evenly
//!   (real clusters inflate unevenly, Apollo's lead canopy took ~1.9× the
//!   mean).

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

/// One parachute, or a cluster flown as one (craft data).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canopy {
    /// Drag coefficient × area, fully open, the whole cluster (m²).
    pub cd_area: f64,
    /// Nominal diameter D₀ of one canopy (m): sets the filling time.
    pub diameter: f64,
    /// Knacke's canopy fill constant n (t_f = n·D/V).
    pub fill_constant: f64,
    /// Reefed stages before full: the fraction of the full Cd·A and the
    /// time after line stretch at which the stage's cutters fire (s).
    pub reefing: Vec<Reef>,
    /// The highest dynamic pressure it is designed to open at (Pa): its
    /// opening load there is within `max_load`.
    pub deploy_max_q: f64,
    /// The drag force that breaks it, the whole cluster (N).
    pub max_load: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reef {
    /// Fraction of the full Cd·A while reefed.
    pub cd_fraction: f64,
    /// Seconds after line stretch when the stage ends.
    pub until: f64,
}

/// The parachute system of a craft (craft data).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChuteSpec {
    /// Attachment point (body axes, m).
    pub mount: glam::DVec3,
    pub drogue: Option<Canopy>,
    pub main: Canopy,
    /// Height above the ground at which the main opens (m).
    pub main_height: f64,
}

/// Where one canopy is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum CanopyState {
    #[default]
    Stowed,
    /// Out since line stretch `age` seconds ago, its mouth `diameter` m.
    Open { age: f64, diameter: f64 },
    /// Cut away (the drogue when the main opens).
    Released,
    /// Torn by an overload.
    Failed,
}

/// The state of a craft's parachutes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChuteState {
    /// The deploy command was given.
    pub armed: bool,
    pub drogue: CanopyState,
    pub main: CanopyState,
}

/// Something that happened in a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChuteEvent {
    DrogueOpened,
    MainOpened,
    DrogueFailed,
    MainFailed,
}

/// The flight condition a step sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    /// Dynamic pressure ½ρV² (Pa).
    pub q: f64,
    /// Airspeed (m/s).
    pub speed: f64,
    /// Height above the ground (m).
    pub height: f64,
}

impl Canopy {
    /// The fraction of full Cd·A the canopy may open to `age` seconds
    /// after line stretch.
    pub fn stage_fraction(&self, age: f64) -> f64 {
        self.reefing.iter().find(|r| age < r.until).map_or(1.0, |r| r.cd_fraction)
    }

    /// Whether it is still in a reefed stage at `age`.
    pub fn reefed(&self, age: f64) -> bool {
        self.reefing.iter().any(|r| age < r.until)
    }

    /// Drag area (m²) of a canopy in `state`.
    pub fn cd_area_of(&self, state: CanopyState) -> f64 {
        match state {
            CanopyState::Open { diameter, .. } => self.cd_area * (diameter / self.diameter).powi(2),
            _ => 0.0,
        }
    }

    /// Advances an open canopy by `dt`: the mouth grows at V/n towards
    /// the stage's diameter. Returns the new state.
    fn grow(&self, age: f64, diameter: f64, speed: f64, dt: f64) -> CanopyState {
        let age = age + dt;
        let target = self.diameter * self.stage_fraction(age).sqrt();
        let diameter = (diameter + speed.max(0.0) / self.fill_constant * dt).min(target).max(diameter);
        CanopyState::Open { age, diameter }
    }
}

impl ChuteState {
    /// Whether any canopy is open.
    pub fn open(&self) -> bool {
        matches!(self.drogue, CanopyState::Open { .. }) || matches!(self.main, CanopyState::Open { .. })
    }

    /// Total drag area of the open canopies (m²).
    pub fn cd_area(&self, spec: &ChuteSpec) -> f64 {
        spec.drogue.as_ref().map_or(0.0, |d| d.cd_area_of(self.drogue)) + spec.main.cd_area_of(self.main)
    }

    /// One step of `dt` seconds: arms on `command`, opens what the
    /// sequence calls for, fills the open canopies and fails the
    /// overloaded ones (the drag of the step's end against `max_load`;
    /// none if `unbreakable`, debug mode, D064).
    pub fn step(&mut self, spec: &ChuteSpec, command: bool, flow: Flow, dt: f64, unbreakable: bool) -> Vec<ChuteEvent> {
        let mut events = Vec::new();
        self.armed |= command;
        if !self.armed {
            return events;
        }
        let drogue_done = match (&spec.drogue, self.drogue) {
            (None, _) | (_, CanopyState::Failed | CanopyState::Released) => true,
            (Some(d), CanopyState::Open { age, .. }) => !d.reefed(age),
            (Some(_), CanopyState::Stowed) => false,
        };
        if spec.drogue.is_some() && self.drogue == CanopyState::Stowed && self.main == CanopyState::Stowed {
            self.drogue = CanopyState::Open { age: 0.0, diameter: 0.0 };
            events.push(ChuteEvent::DrogueOpened);
        } else if self.main == CanopyState::Stowed
            && drogue_done
            && (spec.drogue.is_none() || flow.height <= spec.main_height)
        {
            self.main = CanopyState::Open { age: 0.0, diameter: 0.0 };
            if matches!(self.drogue, CanopyState::Open { .. }) {
                self.drogue = CanopyState::Released;
            }
            events.push(ChuteEvent::MainOpened);
        }
        if let (Some(d), CanopyState::Open { age, diameter }) = (&spec.drogue, self.drogue) {
            self.drogue = d.grow(age, diameter, flow.speed, dt);
            if !unbreakable && flow.q * d.cd_area_of(self.drogue) > d.max_load {
                self.drogue = CanopyState::Failed;
                events.push(ChuteEvent::DrogueFailed);
            }
        }
        if let CanopyState::Open { age, diameter } = self.main {
            let m = &spec.main;
            self.main = m.grow(age, diameter, flow.speed, dt);
            if !unbreakable && flow.q * m.cd_area_of(self.main) > m.max_load {
                self.main = CanopyState::Failed;
                events.push(ChuteEvent::MainFailed);
            }
        }
        events
    }
}

impl ChuteState {
    /// The drag area a landing prediction assumes at `height` above the
    /// ground (m²): each canopy fully open from the moment the sequence
    /// would open it, no filling and no failures.
    pub fn predicted_cd_area(&self, spec: &ChuteSpec, height: f64) -> f64 {
        use CanopyState::{Open, Stowed};
        if !self.armed {
            return 0.0;
        }
        let main_next = self.main == Stowed && (spec.drogue.is_none() || height <= spec.main_height);
        match (&spec.drogue, self.drogue) {
            _ if matches!(self.main, Open { .. }) || main_next => spec.main.cd_area,
            (Some(d), Stowed | Open { .. }) if self.main == Stowed => d.cd_area,
            _ => 0.0,
        }
    }
}
