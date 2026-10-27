//! Segments for display only (D089): a path predicted by another force
//! model (`crate::landing::air_path`: the vessel's own, through air that
//! needs live flight), and a stored segment cut where such a prediction
//! takes over. Both are finished: sampled, never extended (rule 4).

use super::{CoastStart, EndKind, Sample, Segment, SegmentEnd, SegmentKind};
use crate::ephem::SnapshotCache;
use crate::forces::ActiveSources;
use crate::frame::NodeId;
use crate::integrate::{StepSample, StepState};
use crate::math::{Compensated, CompensatedScalar};
use crate::time::Epoch;
use crate::world::World;

impl Segment {
    /// A finished coast through `samples` (relative to `anchor`, local times
    /// from `t0`, the first at 0), ending as `end` at the last. The ship
    /// clock is held at `proper_time` (the other model does not carry it).
    pub fn recorded(
        world: &World,
        t0: Epoch,
        anchor: NodeId,
        samples: &[StepSample],
        end: EndKind,
        mass: f64,
        proper_time: f64,
    ) -> Self {
        let (first, last) = (samples[0], *samples.last().expect("a first sample"));
        let start = CoastStart {
            anchor,
            r: first.r,
            v: first.v,
            drag: None,
            contact_height: 0.0,
            horizon: last.t,
            fixed_anchor: true,
            proper_time,
        };
        let active =
            SnapshotCache::new(&world.eph).with(t0, |snap| ActiveSources::select(world, snap, anchor, first.r));
        Segment {
            t0,
            kind: SegmentKind::Coast,
            samples: samples.iter().map(|&s| Sample { anchor, s, delta: proper_time, delta_rate: 0.0 }).collect(),
            end: Some(SegmentEnd { t: last.t, kind: end }),
            state: StepState {
                t: last.t,
                r: Compensated::new(last.r),
                v: Compensated::new(last.v),
                a: last.a,
                h: 1.0,
                x: CompensatedScalar::new(proper_time),
                x_rate: 0.0,
            },
            anchor,
            active,
            drag: None,
            contact_height: 0.0,
            horizon: last.t,
            fixed_anchor: true,
            mass0: mass,
            horizon_end: end,
            start,
            plan_index: 0,
            entry: None,
            flight: None,
            limbo: None,
        }
    }

    /// A copy that ends at local time `t` (already computed) as `end`: the
    /// samples before it and the state there, its acceleration interpolated
    /// linearly between the samples around it (for display). `None` if `t`
    /// is not computed yet.
    pub fn cut_at(&self, t: f64, end: EndKind) -> Option<Segment> {
        let (anchor, r, v) = self.eval(t)?;
        let keep = self.samples.partition_point(|s| s.s.t < t);
        let a = match (keep.checked_sub(1).map(|i| self.samples[i]), self.samples.get(keep)) {
            (Some(p), Some(n)) if p.anchor == n.anchor && n.s.t > p.s.t => {
                let f = (t - p.s.t) / (n.s.t - p.s.t);
                p.s.a + (n.s.a - p.s.a) * f
            }
            (_, Some(n)) => n.s.a,
            (Some(p), None) => p.s.a,
            (None, None) => return None,
        };
        let delta = self.proper_time_at(t)?;
        let mut seg = self.clone();
        seg.samples.truncate(keep);
        seg.samples.push(Sample { anchor, s: StepSample { t, r, v, a }, delta, delta_rate: 0.0 });
        seg.end = Some(SegmentEnd { t, kind: end });
        seg.state.t = t;
        Some(seg)
    }
}
