//! Test support (port spec section 5, "Ported tests"): Python's random.Random, so that the ported tests draw the same
//! synthetic signals as the Python tests. Compiled for this crate's tests and, through the `testsupport` feature,
//! for lazymkv-dev's integration tests. Task 4 adds the synthetic level series here.

const N: usize = 624;
const M: usize = 397;

/// Python's random.Random: MT19937 seeded as random.seed(int) (init_by_array over the 32-bit words of the seed,
/// least significant first), with the methods the tests call, giving the same values as Python 3.9. Checked against
/// crates/core/tests/fixtures/pyrandom.json (values Python printed once). Py: random.Random.
pub struct PyRandom {
    state: [u32; N],
    index: usize,
}

impl PyRandom {
    /// random.Random(seed) for a non-negative int seed.
    pub fn new(seed: u64) -> PyRandom {
        let key: Vec<u32> = if seed >> 32 == 0 { vec![seed as u32] } else { vec![seed as u32, (seed >> 32) as u32] };
        let mut mt = [0u32; N];
        mt[0] = 19_650_218;
        for i in 1..N {
            mt[i] = 1_812_433_253u32.wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_add(i as u32);
        }
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..N.max(key.len()) {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
        }
        mt[0] = 0x8000_0000;
        PyRandom { state: mt, index: N }
    }

    /// genrand_uint32: the next 32-bit output, regenerating the 624 words when they are used up.
    fn next_u32(&mut self) -> u32 {
        if self.index >= N {
            for k in 0..N {
                let y = (self.state[k] & 0x8000_0000) | (self.state[(k + 1) % N] & 0x7fff_ffff);
                let mut next = self.state[(k + M) % N] ^ (y >> 1);
                if y & 1 == 1 {
                    next ^= 0x9908_b0df;
                }
                self.state[k] = next;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// random(): 53 bits, (a >> 5) * 67108864 + (b >> 6), over 2**53.
    pub fn random(&mut self) -> f64 {
        let a = f64::from(self.next_u32() >> 5);
        let b = f64::from(self.next_u32() >> 6);
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// getrandbits(k), 1 <= k <= 64: one word shifted right for k <= 32; else the low word whole, then the next
    /// word's top k - 32 bits as the high word (CPython fills the words least significant first).
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!((1..=64).contains(&k), "getrandbits: k must be 1 to 64, not {}", k);
        if k <= 32 {
            return u64::from(self.next_u32() >> (32 - k));
        }
        let low = u64::from(self.next_u32());
        let high = u64::from(self.next_u32() >> (64 - k));
        low | (high << 32)
    }

    /// _randbelow_with_getrandbits(n) of Python 3.9: k = n.bit_length(), getrandbits(k) until it is below n.
    fn randbelow(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let k = 64 - n.leading_zeros();
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        r
    }

    /// randint(a, b) = a + _randbelow(b - a + 1); panics when b < a (Python's ValueError).
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        assert!(b >= a, "empty range for randrange() ({}, {}, {})", a, b + 1, b + 1 - a);
        let width = (i128::from(b) - i128::from(a) + 1) as u64;
        (i128::from(a) + i128::from(self.randbelow(width))) as i64
    }

    /// uniform(a, b) = a + (b - a) * random().
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    /// choice(seq) = seq[_randbelow(len(seq))]; panics on an empty slice (Python's IndexError).
    pub fn choice<'a, T>(&mut self, seq: &'a [T]) -> &'a T {
        let at = self.randbelow(seq.len() as u64) as usize;
        seq.get(at).expect("Cannot choose from an empty sequence")
    }
}

#[cfg(test)]
mod tests {
    use super::PyRandom;
    use serde_json::Value;

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/pyrandom.json")).expect("pyrandom.json is JSON")
    }

    fn floats(fixture: &Value, key: &str) -> Vec<f64> {
        fixture[key].as_array().expect("a list").iter().map(|value| value.as_f64().expect("a number")).collect()
    }

    fn ints(fixture: &Value, key: &str) -> Vec<u64> {
        fixture[key].as_array().expect("a list").iter().map(|value| value.as_u64().expect("an int")).collect()
    }

    #[test]
    fn pyrandom_gives_the_python_values_for_seed_7() {
        let fixture = fixture();
        assert_eq!(fixture["seed"], 7);
        let mut rnd = PyRandom::new(7);
        let random: Vec<f64> = (0..10).map(|_| rnd.random()).collect();
        assert_eq!(random, floats(&fixture, "random"));
        let randint: Vec<u64> = (0..10).map(|_| rnd.randint(0, 99) as u64).collect();
        assert_eq!(randint, ints(&fixture, "randint_0_99"));
        let uniform: Vec<f64> = (0..10).map(|_| rnd.uniform(-1.0, 1.0)).collect();
        assert_eq!(uniform, floats(&fixture, "uniform_m1_1"));
        let bits: Vec<u64> = (0..10).map(|_| rnd.getrandbits(32)).collect();
        assert_eq!(bits, ints(&fixture, "getrandbits_32"));
        let pool: Vec<u64> = (0..5).collect();
        let choice: Vec<u64> = (0..10).map(|_| *rnd.choice(&pool)).collect();
        assert_eq!(choice, ints(&fixture, "choice_range5"));
        let wide: Vec<u64> = (0..3).map(|_| rnd.getrandbits(64)).collect();
        assert_eq!(wide, ints(&fixture, "getrandbits_64"));
        let forty: Vec<u64> = (0..3).map(|_| rnd.getrandbits(40)).collect();
        assert_eq!(forty, ints(&fixture, "getrandbits_40"));
    }

    #[test]
    fn pyrandom_seeds_with_every_32_bit_word_of_a_wide_seed() {
        let fixture = fixture();
        let seed = fixture["big_seed"].as_u64().expect("an int");
        assert_eq!(seed, (1u64 << 40) + 5);
        let mut rnd = PyRandom::new(seed);
        let random: Vec<f64> = (0..3).map(|_| rnd.random()).collect();
        assert_eq!(random, floats(&fixture, "big_seed_random"));
    }
}
