//! Compile-time-embedded part definition RONs for the wasm build (where the
//! filesystem is unavailable). Mirror of the files in `data/parts/`.
//!
//! When you add a new RON file under `data/parts/`, append it here.

pub const PARTS_RON: &[(&str, &str)] = &[
    ("aerodynamic.ron",            include_str!("../../data/parts/aerodynamic.ron")),
    ("cargo.ron",                  include_str!("../../data/parts/cargo.ron")),
    ("control.ron",                include_str!("../../data/parts/control.ron")),
    ("electrical.ron",             include_str!("../../data/parts/electrical.ron")),
    ("engines.ron",                include_str!("../../data/parts/engines.ron")),
    ("engines_electric.ron",       include_str!("../../data/parts/engines_electric.ron")),
    ("engines_interstellar.ron",   include_str!("../../data/parts/engines_interstellar.ron")),
    ("engines_ntr.ron",            include_str!("../../data/parts/engines_ntr.ron")),
    ("greenhouses.ron",            include_str!("../../data/parts/greenhouses.ron")),
    ("parachutes.ron",             include_str!("../../data/parts/parachutes.ron")),
    ("pods.ron",                   include_str!("../../data/parts/pods.ron")),
    ("probes.ron",                 include_str!("../../data/parts/probes.ron")),
    ("quarters.ron",               include_str!("../../data/parts/quarters.ron")),
    ("radiators.ron",              include_str!("../../data/parts/radiators.ron")),
    ("rcs.ron",                    include_str!("../../data/parts/rcs.ron")),
    ("reactors_interstellar.ron",  include_str!("../../data/parts/reactors_interstellar.ron")),
    ("reactors_small.ron",         include_str!("../../data/parts/reactors_small.ron")),
    ("shields.ron",                include_str!("../../data/parts/shields.ron")),
    ("structural.ron",             include_str!("../../data/parts/structural.ron")),
    ("tanks.ron",                  include_str!("../../data/parts/tanks.ron")),
    ("tanks_antimatter.ron",       include_str!("../../data/parts/tanks_antimatter.ron")),
    ("tanks_fusion.ron",           include_str!("../../data/parts/tanks_fusion.ron")),
    ("tanks_pulse.ron",            include_str!("../../data/parts/tanks_pulse.ron")),
    ("tanks_xenon.ron",            include_str!("../../data/parts/tanks_xenon.ron")),
];
