//! Ephemeris generation: integrate a system once with a fixed-step symplectic
//! integrator, then fit each node's motion relative to its parent. Rails
//! (elements + linear drift) are used where they measure within tolerance;
//! otherwise a Chebyshev table. See `docs/design/motion-model.md` §1.
//!
//! Generation is a pure function of its inputs (initial conditions, tree,
//! config): the output bytes are identical on every platform.

mod fit;
mod nbody;

pub use fit::{fit_rails, RailsFit};
pub use nbody::{j2_accel, schwarzschild_accel, Extras, NBody, Oblate};

use crate::ephem::{ChebTable, Ephemeris, Motion, Node, NodeKind};
use crate::frame::NodeId;
use crate::integrate::StepSample;
use crate::time::Epoch;
use glam::DVec3;
use std::collections::VecDeque;

/// A physical body's initial state in the system's inertial frame (m, m/s).
#[derive(Clone, Debug, PartialEq)]
pub struct InitialBody {
    pub name: String,
    pub gm: f64,
    pub r: DVec3,
    pub v: DVec3,
}

/// One node of the output tree.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    pub name: String,
    pub kind: NodeKind,
    /// Index of the parent node (None only for the root, which must be first).
    pub parent: Option<usize>,
    /// Indices into the body list: the body itself, or all members of a
    /// barycenter. The root's members are all bodies.
    pub members: Vec<usize>,
    /// Maximum allowed position error of the fit (m).
    pub tolerance: f64,
}

#[derive(Clone, Debug)]
pub struct GenConfig {
    pub generator: String,
    pub start: Epoch,
    pub end: Epoch,
    /// Fixed integration step (whole seconds).
    pub step: i64,
    pub degree: usize,
    /// Length of the initial span used to choose segment lengths (steps).
    pub trial_steps: usize,
    /// Keep every `rails_stride`-th sample for the rails fit.
    pub rails_stride: usize,
    /// Forces beyond point masses (part of the recorded assumption set).
    pub extras: Extras,
}

/// What the generator decided for one node, and how well it fits.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeReport {
    pub name: String,
    pub motion: &'static str,
    pub seg_len: i64,
    pub table_max_err: f64,
    pub rails_max_err: f64,
}

pub fn generate(bodies: &[InitialBody], tree: &[NodeSpec], cfg: &GenConfig) -> (Ephemeris, Vec<NodeReport>) {
    assert_eq!(cfg.start.fractional_second(), 0.0, "start must be whole seconds");
    let total_steps = (cfg.end.whole_seconds() - cfg.start.whole_seconds()) / cfg.step;
    let trial = run(bodies, tree, cfg, cfg.trial_steps.min(total_steps as usize), None);
    let seg_steps: Vec<usize> = (0..tree.len())
        .map(|n| if n == 0 { 0 } else { choose_segment_steps(&trial.samples[n], tree[n].tolerance, cfg) })
        .collect();
    // Round up so every node's last segment is complete and the window is covered.
    let longest = seg_steps.iter().copied().max().unwrap_or(1).max(1);
    let full_steps = (total_steps as usize).div_ceil(longest) * longest;
    let full = run(bodies, tree, cfg, full_steps, Some(&seg_steps));

    let mut nodes = Vec::with_capacity(tree.len());
    let mut report = Vec::new();
    for (n, spec) in tree.iter().enumerate() {
        let gm: f64 = spec.members.iter().map(|&b| bodies[b].gm).sum();
        // Conic parameter: the parent's and this node's members together (a
        // Jacobi-like choice; a parent that contains the node is not double counted).
        let conic_mu: f64 = spec.parent.map_or(0.0, |p| {
            let mut members: Vec<usize> = tree[p].members.iter().chain(&spec.members).copied().collect();
            members.sort_unstable();
            members.dedup();
            members.iter().map(|&b| bodies[b].gm).sum()
        });
        let motion = if n == 0 {
            Motion::Root
        } else {
            let rails = fit_rails(&full.rails_samples[n], cfg.start, conic_mu);
            let table = &full.tables[n];
            let pick_rails = rails.max_err <= spec.tolerance;
            report.push(NodeReport {
                name: spec.name.clone(),
                motion: if pick_rails { "rails" } else { "table" },
                seg_len: table.seg_len,
                table_max_err: full.table_err[n],
                rails_max_err: rails.max_err,
            });
            if pick_rails {
                Motion::Rails(rails.rails)
            } else {
                Motion::Table(table.clone())
            }
        };
        nodes.push(Node {
            name: spec.name.clone(),
            kind: spec.kind,
            parent: spec.parent.map(|p| NodeId(p as u16)),
            gm,
            motion,
        });
    }
    (Ephemeris::new(cfg.generator.clone(), cfg.start, cfg.end, nodes), report)
}

struct RunOutput {
    /// All samples per node (only kept during the trial run).
    samples: Vec<Vec<StepSample>>,
    rails_samples: Vec<Vec<StepSample>>,
    tables: Vec<ChebTable>,
    table_err: Vec<f64>,
}

/// Integrates `steps` steps. Without `seg_steps` it keeps every sample (trial
/// run); with it, it streams Chebyshev fits and discards consumed samples.
fn run(
    bodies: &[InitialBody],
    tree: &[NodeSpec],
    cfg: &GenConfig,
    steps: usize,
    seg_steps: Option<&[usize]>,
) -> RunOutput {
    let mut sys = NBody::new(cfg.start, bodies, cfg.extras.clone());
    let n_nodes = tree.len();
    let mut buffers: Vec<VecDeque<StepSample>> = vec![VecDeque::new(); n_nodes];
    let mut out = RunOutput {
        samples: vec![Vec::new(); n_nodes],
        rails_samples: vec![Vec::new(); n_nodes],
        tables: (0..n_nodes)
            .map(|n| ChebTable {
                start: cfg.start,
                seg_len: seg_steps.map_or(0, |s| s[n] as i64 * cfg.step),
                degree: cfg.degree,
                coeffs: Vec::new(),
            })
            .collect(),
        table_err: vec![0.0; n_nodes],
    };
    for k in 0..=steps {
        if k > 0 {
            crate::integrate::yoshida8_step(&mut sys, cfg.step as f64);
        }
        let t = (k as i64 * cfg.step) as f64;
        let body_kin = sys.kinematics();
        for n in 1..n_nodes {
            let s = relative_sample(&body_kin, bodies, tree, n, t);
            if k % cfg.rails_stride == 0 {
                out.rails_samples[n].push(s);
            }
            match seg_steps {
                None => out.samples[n].push(s),
                Some(seg) => {
                    buffers[n].push_back(s);
                    let len = seg[n];
                    if buffers[n].len() > len {
                        let seg_samples: Vec<StepSample> = buffers[n].iter().take(len + 1).copied().collect();
                        let (coeffs, err) = fit_segment(&seg_samples, cfg.degree);
                        out.tables[n].coeffs.extend(coeffs);
                        out.table_err[n] = out.table_err[n].max(err);
                        buffers[n].drain(..len);
                    }
                }
            }
        }
    }
    out
}

/// State of node `n` relative to its parent from body kinematics (r, v, a).
fn relative_sample(
    kin: &[(DVec3, DVec3, DVec3)],
    bodies: &[InitialBody],
    tree: &[NodeSpec],
    n: usize,
    t: f64,
) -> StepSample {
    let bary = |spec: &NodeSpec| {
        let total: f64 = spec.members.iter().map(|&b| bodies[b].gm).sum();
        let mut acc = (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO);
        for &b in &spec.members {
            let w = bodies[b].gm / total;
            acc.0 += kin[b].0 * w;
            acc.1 += kin[b].1 * w;
            acc.2 += kin[b].2 * w;
        }
        acc
    };
    let me = bary(&tree[n]);
    let parent = bary(&tree[tree[n].parent.expect("non-root node has a parent")]);
    StepSample { t, r: me.0 - parent.0, v: me.1 - parent.1, a: me.2 - parent.2 }
}

/// Fits one segment spanning `samples` (equally spaced, both ends included):
/// interpolates at Lobatto nodes (values from quintic Hermite between samples)
/// and returns the coefficients plus the max position error at the samples.
fn fit_segment(samples: &[StepSample], degree: usize) -> (Vec<f64>, f64) {
    let (t0, t1) = (samples[0].t, samples[samples.len() - 1].t);
    let h = samples[1].t - t0;
    let len = t1 - t0;
    let nodes = crate::ephem::lobatto_nodes(degree);
    let at = |t: f64| {
        let j = (((t - t0) / h) as usize).min(samples.len() - 2);
        crate::integrate::hermite5(&samples[j], &samples[j + 1], t).0
    };
    let pts: Vec<DVec3> = nodes.iter().map(|x| at(t0 + (x + 1.0) * 0.5 * len)).collect();
    let mut coeffs = Vec::with_capacity(3 * (degree + 1));
    for axis in 0..3 {
        let vals: Vec<f64> = pts.iter().map(|p| p.to_array()[axis]).collect();
        coeffs.extend(crate::ephem::interpolate_lobatto(&vals));
    }
    let mut max_err: f64 = 0.0;
    for s in samples {
        let tau = 2.0 * (s.t - t0) / len - 1.0;
        let (tn, _, _) = crate::ephem::cheb_basis(tau, degree);
        let mut p = [0.0; 3];
        for (axis, pa) in p.iter_mut().enumerate() {
            let c = &coeffs[axis * (degree + 1)..(axis + 1) * (degree + 1)];
            *pa = c.iter().zip(tn.iter()).map(|(c, t)| c * t).sum();
        }
        max_err = max_err.max((DVec3::from_array(p) - s.r).length());
    }
    (coeffs, max_err)
}

/// Largest power-of-two number of steps whose segments fit the trial samples
/// within half the tolerance.
fn choose_segment_steps(samples: &[StepSample], tolerance: f64, cfg: &GenConfig) -> usize {
    let mut steps = 1usize << 16;
    while steps > 4 {
        let fits = steps < samples.len()
            && samples.windows(steps + 1).step_by(steps).all(|w| fit_segment(w, cfg.degree).1 <= 0.5 * tolerance);
        if fits {
            return steps;
        }
        steps /= 2;
    }
    steps
}
