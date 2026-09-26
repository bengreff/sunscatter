//! Binary ephemeris format: little-endian, versioned, deterministic (the same
//! ephemeris always serializes to the same bytes, so files can be hashed).

use super::{ChebTable, Ephemeris, Linear, Motion, Node, NodeKind, Rails};
use crate::frame::NodeId;
use crate::kepler::Elements;
use crate::time::Epoch;
use glam::DVec3;

const MAGIC: &[u8; 8] = b"SSEPH\0\0\x01";

#[derive(Debug, PartialEq)]
pub enum EphemerisFormatError {
    BadMagic,
    Truncated,
    BadTag(u8),
    BadUtf8,
}

impl std::fmt::Display for EphemerisFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid ephemeris file: {self:?}")
    }
}
impl std::error::Error for EphemerisFormatError {}

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i64(&mut self, v: i64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn str(&mut self, s: &str) {
        self.u32(s.len() as u32);
        self.0.extend_from_slice(s.as_bytes());
    }
    fn epoch(&mut self, e: Epoch) {
        self.i64(e.whole_seconds());
        self.f64(e.fractional_second());
    }
    fn vec3(&mut self, v: DVec3) {
        self.f64(v.x);
        self.f64(v.y);
        self.f64(v.z);
    }
    fn elements(&mut self, e: &Elements) {
        for x in [e.a, e.e, e.i, e.raan, e.argp, e.mean_anomaly] {
            self.f64(x);
        }
    }
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], EphemerisFormatError> {
        if self.0.len() < n {
            return Err(EphemerisFormatError::Truncated);
        }
        let (head, tail) = self.0.split_at(n);
        self.0 = tail;
        Ok(head)
    }
    fn u8(&mut self) -> Result<u8, EphemerisFormatError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, EphemerisFormatError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }
    fn i64(&mut self) -> Result<i64, EphemerisFormatError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes")))
    }
    fn f64(&mut self) -> Result<f64, EphemerisFormatError> {
        Ok(f64::from_bits(u64::from_le_bytes(self.take(8)?.try_into().expect("8 bytes"))))
    }
    fn str(&mut self) -> Result<String, EphemerisFormatError> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| EphemerisFormatError::BadUtf8)
    }
    fn epoch(&mut self) -> Result<Epoch, EphemerisFormatError> {
        let s = self.i64()?;
        Ok(Epoch::new(s, self.f64()?))
    }
    fn vec3(&mut self) -> Result<DVec3, EphemerisFormatError> {
        Ok(DVec3::new(self.f64()?, self.f64()?, self.f64()?))
    }
    fn elements(&mut self) -> Result<Elements, EphemerisFormatError> {
        Ok(Elements {
            a: self.f64()?,
            e: self.f64()?,
            i: self.f64()?,
            raan: self.f64()?,
            argp: self.f64()?,
            mean_anomaly: self.f64()?,
        })
    }
}

impl Ephemeris {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = W(MAGIC.to_vec());
        w.str(&self.generator);
        w.epoch(self.start);
        w.epoch(self.end);
        w.u32(self.nodes().len() as u32);
        for n in self.nodes() {
            w.str(&n.name);
            w.u8(match n.kind {
                NodeKind::Barycenter => 0,
                NodeKind::Body => 1,
            });
            w.u32(n.parent.map_or(u32::MAX, |p| u32::from(p.0)));
            w.f64(n.gm);
            match &n.motion {
                Motion::Root => w.u8(0),
                Motion::Linear(l) => {
                    w.u8(1);
                    w.epoch(l.epoch);
                    w.vec3(l.r0);
                    w.vec3(l.v);
                }
                Motion::Rails(r) => {
                    w.u8(2);
                    w.epoch(r.epoch);
                    w.f64(r.mu);
                    w.elements(&r.el);
                    w.elements(&r.rates);
                }
                Motion::Table(c) => {
                    w.u8(3);
                    w.epoch(c.start);
                    w.i64(c.seg_len);
                    w.u32(c.degree as u32);
                    w.u32(c.coeffs.len() as u32);
                    for &x in &c.coeffs {
                        w.f64(x);
                    }
                }
            }
        }
        w.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Ephemeris, EphemerisFormatError> {
        let mut r = R(bytes);
        if r.take(8)? != MAGIC {
            return Err(EphemerisFormatError::BadMagic);
        }
        let generator = r.str()?;
        let (start, end) = (r.epoch()?, r.epoch()?);
        let count = r.u32()?;
        let mut nodes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = r.str()?;
            let kind = match r.u8()? {
                0 => NodeKind::Barycenter,
                1 => NodeKind::Body,
                t => return Err(EphemerisFormatError::BadTag(t)),
            };
            let parent = match r.u32()? {
                u32::MAX => None,
                p => Some(NodeId(p as u16)),
            };
            let gm = r.f64()?;
            let motion = match r.u8()? {
                0 => Motion::Root,
                1 => Motion::Linear(Linear { epoch: r.epoch()?, r0: r.vec3()?, v: r.vec3()? }),
                2 => Motion::Rails(Rails { epoch: r.epoch()?, mu: r.f64()?, el: r.elements()?, rates: r.elements()? }),
                3 => {
                    let (start, seg_len, degree) = (r.epoch()?, r.i64()?, r.u32()? as usize);
                    let n = r.u32()? as usize;
                    let coeffs = (0..n).map(|_| r.f64()).collect::<Result<_, _>>()?;
                    Motion::Table(ChebTable { start, seg_len, degree, coeffs })
                }
                t => return Err(EphemerisFormatError::BadTag(t)),
            };
            nodes.push(Node { name, kind, parent, gm, motion });
        }
        Ok(Ephemeris::new(generator, start, end, nodes))
    }
}

/// FNV-1a 64-bit hash (stable across platforms; used for determinism checks).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
