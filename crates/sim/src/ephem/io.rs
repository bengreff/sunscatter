//! Binary ephemeris format: little-endian, versioned, deterministic (the same
//! ephemeris always serializes to the same bytes, so files can be hashed).
//!
//! Motion tags: 0 root, 1 linear, 2 rails, 3 table of f64 coefficients, 4
//! table of quantized coefficients ([`ChebTable::quantum`]): each coefficient
//! is `n · 2^e`, and the integers `n` are stored zigzag-encoded in a fixed
//! number of bits per coefficient slot (axis and degree, the widest over the
//! table's segments), packed LSB first. Decoding multiplies by the power of
//! two, which is exact, so a packed table reads back bit for bit.

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
                Motion::Table(c) if c.quantum > 0.0 => {
                    w.u8(4);
                    w.epoch(c.start);
                    w.i64(c.seg_len);
                    w.u32(c.degree as u32);
                    w.u32(c.coeffs.len() as u32);
                    pack(&mut w, c);
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
                    Motion::Table(ChebTable { start, seg_len, degree, coeffs, quantum: 0.0 })
                }
                4 => {
                    let (start, seg_len, degree) = (r.epoch()?, r.i64()?, r.u32()? as usize);
                    let n = r.u32()? as usize;
                    let (coeffs, quantum) = unpack(&mut r, degree, n)?;
                    Motion::Table(ChebTable { start, seg_len, degree, coeffs, quantum })
                }
                t => return Err(EphemerisFormatError::BadTag(t)),
            };
            nodes.push(Node { name, kind, parent, gm, motion });
        }
        Ok(Ephemeris::new(generator, start, end, nodes))
    }
}

/// `2^e` exactly (for the normal range of f64).
fn pow2(e: i32) -> f64 {
    f64::from_bits(((e + 1023) as u64) << 52)
}

/// The exponent of a power of two.
fn log2_exact(q: f64) -> i32 {
    let e = ((q.to_bits() >> 52) & 0x7ff) as i32 - 1023;
    assert_eq!(pow2(e), q, "a table quantum must be a power of two");
    e
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(z: u64) -> i64 {
    (z >> 1) as i64 ^ -((z & 1) as i64)
}

/// Writes a quantized table's coefficients: the quantum's exponent, the bit
/// width of each slot, then the packed integers (byte length first).
fn pack(w: &mut W, c: &ChebTable) {
    let e = log2_exact(c.quantum);
    let slots = 3 * (c.degree + 1);
    let ints: Vec<u64> = c
        .coeffs
        .iter()
        .map(|&x| {
            let n = x / c.quantum;
            assert!(n == n.round() && n.abs() < 9.0e15, "coefficient {x} is not a multiple of {}", c.quantum);
            zigzag(n as i64)
        })
        .collect();
    let mut widths = vec![0u8; slots];
    for (k, z) in ints.iter().enumerate() {
        let bits = (64 - z.leading_zeros()) as u8;
        widths[k % slots] = widths[k % slots].max(bits);
    }
    w.0.extend_from_slice(&e.to_le_bytes());
    w.0.extend_from_slice(&widths);
    let mut bytes = Vec::new();
    let (mut acc, mut filled) = (0u128, 0u32);
    for (k, z) in ints.iter().enumerate() {
        acc |= u128::from(*z) << filled;
        filled += u32::from(widths[k % slots]);
        while filled >= 8 {
            bytes.push(acc as u8);
            acc >>= 8;
            filled -= 8;
        }
    }
    if filled > 0 {
        bytes.push(acc as u8);
    }
    w.u32(bytes.len() as u32);
    w.0.extend_from_slice(&bytes);
}

/// Reads what [`pack`] wrote: `n` coefficients and the quantum.
fn unpack(r: &mut R, degree: usize, n: usize) -> Result<(Vec<f64>, f64), EphemerisFormatError> {
    let e = i32::from_le_bytes(r.take(4)?.try_into().expect("4 bytes"));
    if !(-1000..1000).contains(&e) {
        return Err(EphemerisFormatError::Truncated);
    }
    let slots = 3 * (degree + 1);
    let widths = r.take(slots)?.to_vec();
    if widths.iter().any(|&b| b > 64) {
        return Err(EphemerisFormatError::BadTag(4));
    }
    let len = r.u32()? as usize;
    let bytes = r.take(len)?;
    let quantum = pow2(e);
    let (mut acc, mut filled, mut next) = (0u128, 0u32, 0usize);
    let mut coeffs = Vec::with_capacity(n);
    for k in 0..n {
        let bits = u32::from(widths[k % slots]);
        while filled < bits {
            let byte = *bytes.get(next).ok_or(EphemerisFormatError::Truncated)?;
            acc |= u128::from(byte) << filled;
            next += 1;
            filled += 8;
        }
        let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
        let z = (acc as u64) & mask;
        acc >>= bits;
        filled -= bits;
        coeffs.push(unzigzag(z) as f64 * quantum);
    }
    Ok((coeffs, quantum))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantized_tables_pack_and_read_back_bit_for_bit() {
        let q = pow2(-3);
        // Two segments of degree 2: a large leading term, mixed signs, zeros
        // and one coefficient needing all 53 bits of an f64 mantissa.
        let ints: [i64; 18] = [
            3_400_000_000,
            -12,
            0,
            1,
            -1,
            7,
            255,
            -256,
            0, //
            -3_400_000_123,
            5,
            0,
            (1 << 52) - 1,
            -(1 << 52),
            0,
            2,
            -3,
            4,
        ];
        let coeffs: Vec<f64> = ints.iter().map(|&n| n as f64 * q).collect();
        let table = ChebTable { start: Epoch::J2000, seg_len: 86_400, degree: 2, coeffs, quantum: q };
        let node =
            |name: &str, parent, motion| Node { name: name.into(), kind: NodeKind::Body, parent, gm: 1.0, motion };
        let eph = Ephemeris::new(
            "packed".into(),
            Epoch::J2000,
            Epoch::J2000.add_seconds(2.0 * 86_400.0),
            vec![node("root", None, Motion::Root), node("moon", Some(NodeId(0)), Motion::Table(table.clone()))],
        );
        let bytes = eph.to_bytes();
        let back = Ephemeris::from_bytes(&bytes).unwrap();
        let Motion::Table(t) = &back.nodes()[1].motion else { panic!("a table") };
        assert_eq!(t.quantum.to_bits(), q.to_bits());
        let bits = |c: &[f64]| c.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&t.coeffs), bits(&table.coeffs));
        assert_eq!(back.to_bytes(), bytes);
        // Packed: far smaller than 8 bytes per coefficient once the widths
        // are paid for (here 18 coefficients in about 9 bytes each, because
        // one slot needs 54 bits; real tables decay to a few bits).
        assert!(Ephemeris::from_bytes(&bytes[..bytes.len() - 1]).is_err(), "truncation is an error");
    }

    #[test]
    fn zigzag_round_trips_the_extremes() {
        for v in [0, 1, -1, 2, -2, i64::MAX, i64::MIN, 1 << 52, -(1 << 52)] {
            assert_eq!(unzigzag(zigzag(v)), v);
        }
        assert_eq!(zigzag(-1), 1);
        assert_eq!(zigzag(1), 2);
        assert_eq!(log2_exact(0.125), -3);
        assert_eq!(pow2(10), 1024.0);
    }
}
