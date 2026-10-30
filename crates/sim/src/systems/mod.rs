//! A vessel's internal systems on a fixed time lattice
//! (`docs/technical/limbo.md` §3, D081, D033).
//!
//! The systems step at the epochs `start + k·STEP`, whatever the frames:
//! each step reads the vessel's state at its own lattice epoch (from the
//! stored trajectory and its pointing while coasting, the tick that holds
//! the epoch while live, the body's rotation while landed) and advances by
//! exactly [`STEP`]. So a year of systems is the same bits advanced in one
//! call or in frames at any warp, and a save taken between lattice points
//! continues exactly (the state is the last lattice point's).
//!
//! What a system may and may not do: a system that changes what the
//! trajectory integrates (mass, thrust) must give the segment a continuous
//! law it can integrate exactly (the propellant boil-off rate of
//! [`Consumables`] is a mass flow of the vessel's segments and live ticks),
//! never step it; anything else (power, life support, comms, crew later)
//! steps here. A real system is one more field and one more call in
//! [`Systems::step`], reading [`Env`].
//!
//! Placeholders until parts carry systems (D060): [`Power`] (a solar
//! panel, a battery, constant loads) and [`Consumables`] (boil-off).

pub mod coast_law;
pub mod command;
pub mod crew;
pub mod data;
pub mod estimate;
pub mod exhaust;
pub mod failure;
pub mod fins;
pub mod flight;
pub mod g_limit;
pub mod mechanisms;
pub mod plumbing;
pub mod power;
pub mod propulsion;
pub mod rcs;
pub mod runtime;
pub mod ship;
pub mod structure;

pub use data::{Knowledge, Quantity, Reading};
pub use power::{Power, PowerSpec};
pub use runtime::{Event, Order, OrderKind, Runtime};

use crate::time::Epoch;
use glam::{DQuat, DVec3};

/// The systems lattice spacing (s): resolves a low orbit's eclipse
/// (≈ 35 min) to a minute.
pub const STEP: f64 = 60.0;

/// What the systems read at a lattice point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// Sunlight at the vessel (W/m², inertial, along the light; eclipses
    /// included, `sim::light`'s rule).
    pub sun: DVec3,
    /// Attitude (body → inertial).
    pub attitude: DQuat,
    /// Propellant aboard (kg).
    pub propellant: f64,
}

/// Propellant boil-off: a constant mass flow (kg/s) while there is
/// propellant (a placeholder for a tank's heat leak over the latent heat;
/// zero for the test craft's storable propellants). The vessel's segments
/// and live ticks take it as a mass flow; the lattice keeps the ledger.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Consumables {
    pub boil_off: f64,
    /// Propellant boiled off so far (kg), at the lattice points.
    pub boiled: f64,
}

impl Consumables {
    fn step(&mut self, env: &Env, dt: f64) {
        if env.propellant > 0.0 {
            self.boiled += self.boil_off * dt;
        }
    }
}

/// A vessel's systems and where their lattice stands.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Systems {
    /// The last lattice point stepped; `None` until the vessel first
    /// advances (the lattice then starts at its time).
    pub epoch: Option<Epoch>,
    pub power: Power,
    pub consumables: Consumables,
}

impl Systems {
    /// Systems whose lattice starts at `epoch`.
    pub fn starting(epoch: Epoch) -> Self {
        Systems { epoch: Some(epoch), ..Systems::default() }
    }

    /// The next lattice point (`None` before the lattice has started).
    pub fn next(&self) -> Option<Epoch> {
        self.epoch.map(|e| e.add_seconds(STEP))
    }

    /// Starts the lattice at `epoch` if it has not started.
    pub fn start_at(&mut self, epoch: Epoch) {
        self.epoch.get_or_insert(epoch);
    }

    /// Steps every system to the next lattice point with the environment
    /// there.
    pub fn step(&mut self, env: &Env) {
        let Some(next) = self.next() else { return };
        self.power.step(env, STEP);
        self.consumables.step(env, STEP);
        self.epoch = Some(next);
    }

    /// Steps every lattice point up to `until`, the environment at each
    /// from `env_at`. Returns the number of steps.
    pub fn advance(&mut self, until: Epoch, mut env_at: impl FnMut(Epoch) -> Env) -> usize {
        let mut n = 0;
        while let Some(e) = self.next().filter(|e| e.seconds_since(until) <= 0.0) {
            self.step(&env_at(e));
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(sun: f64) -> Env {
        Env { sun: DVec3::new(-sun, 0.0, 0.0), attitude: DQuat::IDENTITY, propellant: 10.0 }
    }

    #[test]
    fn any_chunking_of_the_lattice_gives_the_same_bits() {
        let t0 = Epoch::new(1_000_000_000, 0.25);
        // Sunlight that switches on and off (an eclipse every 50 min).
        let env_at = |e: Epoch| env(if (e.seconds_since(t0) / 3_000.0) as i64 % 2 == 0 { 1_361.0 } else { 0.0 });
        let mut one = Systems::starting(t0);
        one.consumables.boil_off = 1e-5;
        let mut chunked = one.clone();
        one.advance(t0.add_seconds(86_400.0), env_at);
        let mut t = t0;
        for k in 0..2_000 {
            t = t.add_seconds(if k % 3 == 0 { 7.3 } else { 61.9 });
            if t.seconds_since(t0) > 86_400.0 {
                t = t0.add_seconds(86_400.0);
            }
            chunked.advance(t, env_at);
        }
        chunked.advance(t0.add_seconds(86_400.0), env_at);
        assert_eq!(one, chunked);
        assert_eq!(one.epoch, Some(t0.add_seconds(86_400.0)));
        assert!((one.consumables.boiled - 1e-5 * 86_400.0).abs() < 1e-9);
    }
}
