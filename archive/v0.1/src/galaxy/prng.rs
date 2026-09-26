/// Deterministic xorshift64 PRNG — no external dependencies.
/// Seeded from a u64; same seed always produces the same sequence.
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Create a new PRNG from a seed. Seed must be non-zero.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }

    /// Mix two u64 values into a single seed (splitmix-style).
    pub fn mix(a: u64, b: u64) -> u64 {
        let mut z = a.wrapping_add(b.wrapping_mul(0x9E3779B97F4A7C15));
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Next raw u64.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform f64 in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform f64 in [lo, hi).
    #[inline]
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.next_f64() * (hi - lo)
    }

    /// Uniform u32 in [0, n).
    #[inline]
    pub fn next_u32(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }
}
