//! Sunscatter simulation core.
//!
//! Pure Rust, f64, deterministic, no engine dependencies. See
//! `docs/design/motion-model.md` for the model this crate implements.
//!
//! Rules for everything in this crate:
//! * all transcendental math goes through [`math`] (libm-backed); no `mul_add`;
//! * bodies are pure functions of time (see [`ephem`]);
//! * vectors carry their frame kind ([`frame`]); relative quantities are formed
//!   through the lowest common ancestor in the frame tree, never by subtracting
//!   two large absolute coordinates.

pub mod aero;
pub mod approach;
pub mod body;
pub mod comms;
pub mod contact;
pub mod craft;
pub mod ephem;
pub mod forces;
pub mod frame;
pub mod gen;
pub mod integrate;
pub mod kepler;
pub mod light;
pub mod math;
pub mod relativity;
pub mod rigid;
pub mod save;
pub mod sol;
pub mod terrain;
pub mod thermal;
pub mod time;
pub mod vessel;
pub mod world;
