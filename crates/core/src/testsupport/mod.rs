//! Test support (port spec section 5, "Ported tests"): Python's random.Random, so that the ported tests draw the same
//! synthetic signals as the Python tests. Compiled for this crate's tests and, through the `testsupport` feature,
//! for lazymkv-dev's integration tests. Task 4 adds the synthetic level series here; phase 1's Task 2 adds the robot
//! log builders (spec 9.4) and the TMDb answer builders (spec 8.1, in TMDb's documented shapes).

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

// ------------------------------------------------------------------------------------------------------------------
// Added by Task 4: the synthetic level series of test_audio_similarity.py and test_audio_dataset.py (both files define
// them identically). Each draws from PyRandom in the Python's order, so the ported tests measure the Python's numbers.
// ------------------------------------------------------------------------------------------------------------------

use crate::audio::measures::py_pow;

/// 20 ms frames per second. Py: FPS.
pub const FPS: usize = 50;

/// Syllables: `loud + uniform(-1, 1)` for randint(4, 7) frames, then `quiet + uniform(-1, 1)` for randint(4, 8)
/// frames, until `frames` levels are made; cut to `frames`. Py: speech(rnd, frames, loud, quiet).
pub fn speech(rnd: &mut PyRandom, frames: usize, loud: f64, quiet: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    while out.len() < frames {
        // `[loud + rnd.uniform(-1, 1)] * rnd.randint(4, 7)`: Python evaluates the list before the count.
        let level = loud + rnd.uniform(-1.0, 1.0);
        let count = rnd.randint(4, 7) as usize;
        out.extend(std::iter::repeat(level).take(count));
        let level = quiet + rnd.uniform(-1.0, 1.0);
        let count = rnd.randint(4, 8) as usize;
        out.extend(std::iter::repeat(level).take(count));
    }
    out.truncate(frames);
    out
}

/// A dialogue-driven programme: lines of randint(100, 200) frames of speech(-20, -30), the centre 1 dB below the mix,
/// then randint(75, 150) frames of room tone (the mix at -60 + uniform(-1, 1) for every frame, then the centre at
/// -75 + uniform(-1, 1) for every frame). Returns (mix, centre), each cut to seconds * FPS.
/// Py: programme(seconds=420, seed=1).
pub fn programme(seconds: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
    let mut rnd = PyRandom::new(seed);
    let total = seconds * FPS;
    let mut mix: Vec<f64> = Vec::new();
    let mut centre: Vec<f64> = Vec::new();
    while mix.len() < total {
        let n = rnd.randint(2 * FPS as i64, 4 * FPS as i64) as usize;
        let line = speech(&mut rnd, n, -20.0, -30.0);
        centre.extend(line.iter().map(|v| v - 1.0));
        mix.extend(line);
        // int(1.5 * FPS) is 75
        let n = rnd.randint((1.5 * FPS as f64) as i64, 3 * FPS as i64) as usize;
        for _ in 0..n {
            mix.push(-60.0 + rnd.uniform(-1.0, 1.0));
        }
        for _ in 0..n {
            centre.push(-75.0 + rnd.uniform(-1.0, 1.0));
        }
    }
    mix.truncate(total);
    centre.truncate(total);
    (mix, centre)
}

/// 10 * log10(10 ** (a / 10) + 10 ** (b / 10)). Py: power_sum.
pub fn power_sum(a: f64, b: f64) -> f64 {
    10.0 * (py_pow(10.0, a / 10.0) + py_pow(10.0, b / 10.0)).log10()
}

/// Two people talking all the way through: speech(-18, -27) for randint(150, 300) frames (the count drawn before the
/// speech), then randint(15, 30) frames at -100 dB, over `mix` ducked by 30 dB: power_sum(voice, mix - 30) per frame,
/// as long as `mix`. Py: commentary(mix, seed=7).
pub fn commentary(mix: &[f64], seed: u64) -> Vec<f64> {
    let mut rnd = PyRandom::new(seed);
    let mut voice: Vec<f64> = Vec::new();
    while voice.len() < mix.len() {
        let frames = rnd.randint(3 * FPS as i64, 6 * FPS as i64) as usize;
        let talk = speech(&mut rnd, frames, -18.0, -27.0);
        voice.extend(talk);
        let pause = rnd.randint(15, 30) as usize;
        voice.extend(std::iter::repeat(-100.0).take(pause));
    }
    voice.iter().zip(mix).map(|(&v, &m)| power_sum(v, m - 30.0)).collect()
}

/// A compact level file as audio-levels.ps1 writes it: the header line, then `'%.1f' % v` per line, "\n" ends.
/// Py: level_file (its write_text body).
pub fn level_file_text(values: &[f64]) -> String {
    let lines: Vec<String> = values.iter().map(|&v| crate::recording::py_fixed(v, 1)).collect();
    format!(
        "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n{}\n",
        lines.join("\n")
    )
}

/// MANIFEST.format(fc=..., main=..., cand=...) of test_audio_similarity.py: what audio-levels.ps1 writes for main
/// stream 1 (DTS-HD MA 5.1, English) and candidate 3 (AC-3 stereo).
pub fn manifest(fc: &str, main: &str, cand: &str) -> String {
    format!("# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\nmain=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc={fc} frames=1 md5={main}\ncand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5={cand}\n")
}

/// ffmpeg's ametadata output, one frame every `step_s` seconds: "frame:k    pts:k    pts_time:<k * step_s :g>" and
/// "lavfi.astats.Overall.RMS_level=<v :.6f>" per value. Py: test_audio_similarity.raw_file (its write_text body).
pub fn raw_file_text(values: &[f64], step_s: f64) -> String {
    let mut out = String::new();
    for (k, &v) in values.iter().enumerate() {
        out.push_str(&format!(
            "frame:{k}    pts:{k}    pts_time:{}\nlavfi.astats.Overall.RMS_level={}\n",
            crate::recording::py_g(k as f64 * step_s),
            crate::recording::py_fixed(v, 6)
        ));
    }
    out
}

/// A spec holding only section 17's table head and row 13 with test_audio_dataset.SPEC's numbers (3 verdict discs,
/// 8 commentaries from 4 discs, 2 on feature films, 20 programme tracks of which 6 separately encoded, one title of
/// each kind, the fitting disc `fit-disc`). Py: test_audio_dataset.SPEC.
pub const ROW13_SPEC: &str = concat!(
    "# spec\n\n## 17. Experiments (phase 0)\n\n| # | Question | If no |\n|---|---|---|\n",
    "| 13 | Measured over whole titles on the owner's own discs, does the content rule give its commentary verdict to ",
    "**no** programme track, and to at least one commentary on each of at least 3 different discs? Before ",
    "the answer may be yes, the labelled tracks must include: at least 8 commentaries from at least 4 discs, ",
    "at least 2 of them on feature films and at least 1 with the film clearly audible under the voices; at ",
    "least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least ",
    "20 programme tracks other than the reference, at least 6 of them separately encoded lossy companions or ",
    "alternate mixes; at least 1 title whose reference has no centre channel (mono or 2.0) and at least ",
    "1 dialogue-heavy or speech-only title, each with an extra track. Every threshold was fitted on two tracks of ",
    "one disc (`fit-disc`: the S03E16 commentary), so no track of that disc counts. | fallback |\n",
);

/// A track label: how "listened", sure, the verdict not seen, dated 2026-10-05, no note. Py: test_audio_dataset.entry.
pub fn entry(label: &str, stream: i64, kind: &str) -> serde_json::Value {
    serde_json::json!({"label": label, "stream": stream, "kind": kind, "how": "listened", "sure": true,
                       "date": "2026-10-05", "saw_verdict": false, "note": ""})
}

/// `dict(value, key=v)`: `value` (an object) with `key` set to `v`, in place when it is there, else added at the end.
pub fn with(mut value: serde_json::Value, key: &str, v: serde_json::Value) -> serde_json::Value {
    value.as_object_mut().expect("an object").insert(key.to_string(), v);
    value
}

/// The text of an ffprobe-streams.json (or a level folder's streams.json) with a video stream, index 0, then the given
/// audio streams numbered from 1, each as (codec, profile, channels, language). Py: test_audio_dataset.probe_file's
/// content.
pub fn probe_streams_json(streams: &[(&str, Option<&str>, i64, &str)]) -> String {
    let mut list = vec![serde_json::json!({"index": 0, "codec_type": "video"})];
    for (k, (codec, profile, channels, language)) in streams.iter().enumerate() {
        list.push(serde_json::json!({"index": k + 1, "codec_type": "audio", "codec_name": codec, "profile": profile,
                                     "channels": channels, "tags": {"language": language}}));
    }
    serde_json::to_string(&serde_json::json!({"streams": list})).expect("serialisable")
}

/// One level folder's manifest.txt and streams.json as audio-levels.ps1 writes them (main stream 1 with six
/// channels, streams.json listing every stream up to the highest measured one), as (path under the label folder,
/// bytes). Py: test_audio_dataset.folder, without the level series (`labels auto` reads none).
fn level_folder(label: &str, main_md5: &str, cands: &[(i64, &str)]) -> Vec<(String, Vec<u8>)> {
    let numbers: Vec<String> = std::iter::once(1).chain(cands.iter().map(|(n, _)| *n)).map(|n| n.to_string()).collect();
    let dir = format!("{label}/audio-levels-20ms-{}", numbers.join("-"));
    let mut lines = vec![
        "# lazymkv audio-levels v2 (experiment 13)".to_string(),
        "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400".to_string(),
        format!("main=1 codec=dts profile=- channels=6 layout=5.1(side) language=eng fc=yes frames=100 md5={main_md5}"),
    ];
    for (n, md5) in cands {
        lines.push(format!("cand={n} codec=ac3 profile=- channels=2 layout=stereo language=eng frames=100 md5={md5}"));
    }
    let top = cands.iter().map(|(n, _)| *n).max().unwrap_or(1).max(1);
    let mut streams = vec![serde_json::json!({"index": 0, "codec_type": "video"})];
    for k in 1..=top {
        let (codec, channels) = if k == 1 { ("dts", 6) } else { ("ac3", 2) };
        streams.push(serde_json::json!({"index": k, "codec_type": "audio", "codec_name": codec, "channels": channels,
                                        "tags": {"language": "eng", "title": format!("t{k}")}}));
    }
    vec![
        (format!("{dir}/manifest.txt"), (lines.join("\n") + "\n").into_bytes()),
        (format!("{dir}/streams.json"), serde_json::to_vec(&serde_json::json!({"streams": streams})).expect("serialisable")),
    ]
}

/// The recording of the Python's Routine auto test, "disc-d", without its audio-labels.json: the files under the
/// recording folder as (relative path, bytes). t0 measured with streams 1 to 7 (2 and 5 the same packets, 3 and 7
/// the main's, 6 flagged 2304 by the scan), t1 with streams 1 and 2 while the scan's title 1 has three audio
/// streams; each label folder has its rip-summary.json, and the folder has scan.txt. Callers put the files in a
/// `MemTree` or on disk, and add the labels file themselves. Py: test_audio_dataset.Routine (its auto test).
pub fn disc_d_files() -> Vec<(String, Vec<u8>)> {
    let mut files = level_folder("t0", "aa", &[(2, "bb"), (3, "aa"), (4, "cc"), (5, "bb"), (6, "dd"), (7, "aa")]);
    files.push(("t0/rip-summary.json".to_string(), br#"{"ok": true, "title": 0}"#.to_vec()));
    files.extend(level_folder("t1", "ee", &[(2, "ff")]));
    files.push(("t1/rip-summary.json".to_string(), br#"{"ok": true, "title": 1}"#.to_vec()));
    let flags = [(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 2304), (7, 0)];
    let mut scan = vec!["TINFO:0,9,0,\"0:07:00\"".to_string(), "SINFO:0,0,1,0,\"Video\"".to_string()];
    scan.extend(flags.iter().map(|(s, _)| format!("SINFO:0,{s},1,0,\"Audio\"")));
    scan.extend(flags.iter().map(|(s, f)| format!("SINFO:0,{s},22,0,\"{f}\"")));
    scan.push("SINFO:1,0,1,0,\"Video\"".to_string());
    scan.extend((1..=3).map(|s| format!("SINFO:1,{s},1,0,\"Audio\"")));
    files.push(("scan.txt".to_string(), (scan.join("\n") + "\n").into_bytes()));
    files
}

/// The SHA-1 of `bytes` as lowercase hex: the digest the evidence tests compare recorded outputs by.
pub use crate::recording::sha1_hex;

/// A fresh folder under the system's temporary directory, removed when dropped.
pub struct Scratch(pub std::path::PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Scratch {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lazymkv-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes `text` to `path`, making its folders first.
pub fn write(path: &std::path::Path, text: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A file written under a scratch folder by a path relative to it, its folders made first.
pub trait ScratchExt {
    fn write(&self, rel: &str, data: impl AsRef<[u8]>);
    /// Like `write`, and returns the full path of the file.
    fn write_path(&self, rel: &str, data: impl AsRef<[u8]>) -> std::path::PathBuf;
}

impl ScratchExt for Scratch {
    fn write(&self, rel: &str, data: impl AsRef<[u8]>) {
        write(&self.0.join(rel), data);
    }

    fn write_path(&self, rel: &str, data: impl AsRef<[u8]>) -> std::path::PathBuf {
        let path = self.0.join(rel);
        write(&path, data);
        path
    }
}

#[cfg(test)]
mod generator_tests {
    //! The Task 4 generators against values Python 3.9.6 printed for the same calls of test_audio_similarity.py's
    //! programme(), commentary(), speech(), power_sum(), level_file() and raw_file().
    use super::*;

    #[test]
    fn the_generators_give_the_pythons_series() {
        let (mix, centre) = programme(420, 1);
        assert_eq!((mix.len(), centre.len()), (21000, 21000));
        assert_eq!((mix[0], mix[20999]), (-19.861592250355574, -60.76686815012025));
        assert_eq!((centre[0], centre[20999]), (-20.861592250355574, -74.49592297734323));
        let voice = commentary(&mix, 7);
        assert_eq!(voice.len(), 21000);
        for (k, want) in [(0usize, -17.101968171039434), (12345, -89.31715843496981), (20999, -27.86279645475714)] {
            assert!((voice[k] - want).abs() <= 1e-12, "commentary[{k}] = {}, python {want}", voice[k]);
        }
        let s = speech(&mut PyRandom::new(3), 30, -18.0, -27.0);
        assert_eq!(s.len(), 30);
        assert_eq!(&s[..5], &[-18.524070745816218; 5]);
        assert_eq!(&s[5..12], &[-27.26008966690384; 7]);
        assert!((power_sum(-20.0, -50.0) - (-19.995659225206815)).abs() <= 1e-12);
    }

    #[test]
    fn level_and_raw_file_texts_are_the_pythons() {
        assert_eq!(
            level_file_text(&[-45.25, -100.0, 0.05, -0.04]),
            "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n\
             -45.2\n-100.0\n0.1\n-0.0\n"
        );
        assert_eq!(
            raw_file_text(&[-20.5, -100.0, -3.25], 0.1),
            "frame:0    pts:0    pts_time:0\nlavfi.astats.Overall.RMS_level=-20.500000\n\
             frame:1    pts:1    pts_time:0.1\nlavfi.astats.Overall.RMS_level=-100.000000\n\
             frame:2    pts:2    pts_time:0.2\nlavfi.astats.Overall.RMS_level=-3.250000\n"
        );
    }
}

/// A robot-mode value in quotes, with `"` and `\` backslash-escaped, as spec 9.4 says MakeMKV writes them.
fn robot_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// A robot log from a compact description: the CINFO lines, then per title (number, attrs) its TINFO lines and per stream
/// (number, attrs) its SINFO lines; `attrs` are (attribute id, value) pairs written as `CINFO:id,0,"value"`,
/// `TINFO:title,id,0,"value"` and `SINFO:title,stream,id,0,"value"`, in the order given, each line ending in "\n". A
/// title with no attrs writes no TINFO line (its SINFO lines are still written). Tests of Tasks 3 to 16 build scans with
/// it instead of long literals. Spec 9.4.
pub fn scan_text(disc: &[(u32, &str)], titles: &[(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])]) -> String {
    let mut out = String::new();
    for (id, value) in disc {
        out.push_str(&format!("CINFO:{},0,{}\n", id, robot_quoted(value)));
    }
    for (title, attrs, streams) in titles {
        for (id, value) in attrs.iter() {
            out.push_str(&format!("TINFO:{},{},0,{}\n", title, id, robot_quoted(value)));
        }
        for (stream, stream_attrs) in streams.iter() {
            for (id, value) in stream_attrs.iter() {
                out.push_str(&format!("SINFO:{},{},{},0,{}\n", title, stream, id, robot_quoted(value)));
            }
        }
    }
    out
}

/// The same text with the MSG:1005 banner for version `version` (`MakeMKV v<version> win(x64-release)`) and, when `java`
/// is true, an MSG:3344 line naming `C:\Program Files\Java\bin\java.exe`, both before the CINFO lines as in the recorded
/// scans. Spec 9.4, 6.1 step 4.
pub fn scan_text_with_messages(
    version: &str,
    java: bool,
    disc: &[(u32, &str)],
    titles: &[(u32, &[(u32, &str)], &[(u32, &[(u32, &str)])])],
) -> String {
    let banner = format!("MakeMKV v{version} win(x64-release)");
    let mut out = format!(
        "MSG:1005,0,1,{},\"%1 started\",{}\n",
        robot_quoted(&format!("{banner} started")),
        robot_quoted(&banner)
    );
    if java {
        let java_path = r"C:\Program Files\Java\bin\java.exe";
        out.push_str(&format!(
            "MSG:3344,0,1,{},\"Using Java runtime from %1\",{}\n",
            robot_quoted(&format!("Using Java runtime from {java_path}")),
            robot_quoted(java_path)
        ));
    }
    out.push_str(&scan_text(disc, titles));
    out
}

/// A `/movie/{id}` answer as TMDb documents it: id, title, original_title, release_date, original_language, runtime
/// (None writes null), external_ids {imdb_id}. The IMDb ID is synthetic: `tt` and the TMDb ID in seven digits. Spec 8.1.
pub fn tmdb_movie_json(
    id: i64,
    title: &str,
    original_title: &str,
    release_date: &str,
    original_language: &str,
    runtime: Option<u32>,
) -> String {
    serde_json::json!({
        "id": id,
        "title": title,
        "original_title": original_title,
        "release_date": release_date,
        "original_language": original_language,
        "runtime": runtime,
        "external_ids": {"imdb_id": format!("tt{id:07}")},
    })
    .to_string()
}

/// A `/tv/{id}` answer with its seasons expanded as `/tv/{id}/season/{n}` gives them: id, name, original_name (the name
/// again), first_air_date, original_language, external_ids {imdb_id, tvdb_id}, seasons[] {season_number, episode_count
/// (the episodes given), episodes[] {episode_number, name, runtime}}. The IDs are synthetic: `tt` and the TMDb ID in seven
/// digits, and 100000 plus the TMDb ID. Spec 8.1, 8.5.
pub fn tmdb_tv_json(
    id: i64,
    name: &str,
    first_air_date: &str,
    original_language: &str,
    seasons: &[(u32, &[(u32, &str, Option<u32>)])],
) -> String {
    let seasons: Vec<serde_json::Value> = seasons
        .iter()
        .map(|(season_number, episodes)| {
            let episodes: Vec<serde_json::Value> = episodes
                .iter()
                .map(|(episode_number, episode_name, runtime)| {
                    serde_json::json!({"episode_number": episode_number, "name": episode_name, "runtime": runtime})
                })
                .collect();
            serde_json::json!({"season_number": season_number, "episode_count": episodes.len(), "episodes": episodes})
        })
        .collect();
    serde_json::json!({
        "id": id,
        "name": name,
        "original_name": name,
        "first_air_date": first_air_date,
        "original_language": original_language,
        "external_ids": {"imdb_id": format!("tt{id:07}"), "tvdb_id": id + 100_000},
        "seasons": seasons,
    })
    .to_string()
}

/// A `/search/multi` answer: results[] {media_type, id, title|name, original_title|original_name,
/// release_date|first_air_date} from (media_type, id, title, original title, date), the movie fields for "movie" and the
/// TV fields for any other media type, wrapped as TMDb pages it (page 1, total_pages 1, total_results). Spec 8.1.
pub fn search_json(results: &[(&str, i64, &str, &str, &str)]) -> String {
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|&(media_type, id, title, original, date)| {
            if media_type == "movie" {
                serde_json::json!({"media_type": media_type, "id": id, "title": title, "original_title": original, "release_date": date})
            } else {
                serde_json::json!({"media_type": media_type, "id": id, "name": title, "original_name": original, "first_air_date": date})
            }
        })
        .collect();
    serde_json::json!({"page": 1, "results": rows, "total_pages": 1, "total_results": results.len()}).to_string()
}

#[cfg(test)]
mod builder_tests {
    //! The scan and TMDb fixture builders of phase 1 Task 2: the exact text each writes, and that the parser reads a
    //! built scan back.
    use super::*;
    use crate::robot::parse;

    #[test]
    fn scan_text_writes_the_cinfo_lines_then_each_titles_tinfo_and_sinfo_lines() {
        let text = scan_text(
            &[(1, "Blu-ray disc"), (2, "Test Disc")],
            &[
                (0, &[(9, "0:10:00"), (16, "00001.mpls")], &[(0, &[(1, "Video")]), (1, &[(1, "Audio"), (3, "eng")])]),
                (3, &[(9, "0:01:00")], &[]),
            ],
        );
        assert_eq!(
            text,
            concat!(
                "CINFO:1,0,\"Blu-ray disc\"\n",
                "CINFO:2,0,\"Test Disc\"\n",
                "TINFO:0,9,0,\"0:10:00\"\n",
                "TINFO:0,16,0,\"00001.mpls\"\n",
                "SINFO:0,0,1,0,\"Video\"\n",
                "SINFO:0,1,1,0,\"Audio\"\n",
                "SINFO:0,1,3,0,\"eng\"\n",
                "TINFO:3,9,0,\"0:01:00\"\n",
            )
        );
        let scan = parse(&text);
        assert!(scan.skipped.is_empty(), "{:?}", scan.skipped);
        assert_eq!(scan.disc[&2], "Test Disc");
        assert_eq!(scan.titles[&3][&9], "0:01:00");
        assert_eq!(scan.streams[&(0, 1)][&3], "eng");
    }

    #[test]
    fn scan_text_escapes_quotes_and_backslashes_as_makemkv_does() {
        let text = scan_text(&[(2, r#"Say "Hi" \ Bye"#)], &[]);
        assert_eq!(text, "CINFO:2,0,\"Say \\\"Hi\\\" \\\\ Bye\"\n");
        assert_eq!(parse(&text).disc[&2], r#"Say "Hi" \ Bye"#);
    }

    #[test]
    fn scan_text_with_messages_puts_the_banner_and_the_java_line_first() {
        let text = scan_text_with_messages("2.0.0", true, &[(1, "Blu-ray disc")], &[]);
        assert_eq!(
            text,
            concat!(
                r#"MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)""#,
                "\n",
                r#"MSG:3344,0,1,"Using Java runtime from C:\\Program Files\\Java\\bin\\java.exe","Using Java runtime from %1","C:\\Program Files\\Java\\bin\\java.exe""#,
                "\n",
                "CINFO:1,0,\"Blu-ray disc\"\n",
            )
        );
        let scan = parse(&text);
        assert_eq!(scan.messages[0].code, 1005);
        assert_eq!(scan.messages[0].params, ["MakeMKV v2.0.0 win(x64-release)"]);
        assert_eq!(scan.messages[1].code, 3344);
        assert_eq!(scan.messages[1].params, [r"C:\Program Files\Java\bin\java.exe"]);
        assert_eq!(
            scan_text_with_messages("1.18.4", false, &[], &[]),
            concat!(
                r#"MSG:1005,0,1,"MakeMKV v1.18.4 win(x64-release) started","%1 started","MakeMKV v1.18.4 win(x64-release)""#,
                "\n",
            )
        );
    }

    #[test]
    fn tmdb_movie_json_has_the_documented_movie_fields() {
        assert_eq!(
            tmdb_movie_json(1417, "Pan's Labyrinth", "El laberinto del fauno", "2006-10-11", "es", Some(119)),
            concat!(
                r#"{"id":1417,"title":"Pan's Labyrinth","original_title":"El laberinto del fauno","#,
                r#""release_date":"2006-10-11","original_language":"es","runtime":119,"#,
                r#""external_ids":{"imdb_id":"tt0001417"}}"#,
            )
        );
        assert_eq!(
            tmdb_movie_json(7, "A", "A", "", "en", None),
            concat!(
                r#"{"id":7,"title":"A","original_title":"A","release_date":"","original_language":"en","runtime":null,"#,
                r#""external_ids":{"imdb_id":"tt0000007"}}"#,
            )
        );
    }

    #[test]
    fn tmdb_tv_json_has_the_documented_tv_fields_with_its_seasons_expanded() {
        let text = tmdb_tv_json(
            2316,
            "The Office",
            "2005-03-24",
            "en",
            &[(3, &[(15, "Phyllis' Wedding", Some(22)), (16, "Business School", None)]), (0, &[])],
        );
        assert_eq!(
            text,
            concat!(
                r#"{"id":2316,"name":"The Office","original_name":"The Office","first_air_date":"2005-03-24","#,
                r#""original_language":"en","external_ids":{"imdb_id":"tt0002316","tvdb_id":102316},"#,
                r#""seasons":[{"season_number":3,"episode_count":2,"episodes":["#,
                r#"{"episode_number":15,"name":"Phyllis' Wedding","runtime":22},"#,
                r#"{"episode_number":16,"name":"Business School","runtime":null}]},"#,
                r#"{"season_number":0,"episode_count":0,"episodes":[]}]}"#,
            )
        );
    }

    #[test]
    fn search_json_writes_movie_and_tv_results_in_their_own_fields() {
        assert_eq!(
            search_json(&[
                ("movie", 1417, "Pan's Labyrinth", "El laberinto del fauno", "2006-10-11"),
                ("tv", 2316, "The Office", "The Office", "2005-03-24"),
            ]),
            concat!(
                r#"{"page":1,"results":["#,
                r#"{"media_type":"movie","id":1417,"title":"Pan's Labyrinth","#,
                r#""original_title":"El laberinto del fauno","release_date":"2006-10-11"},"#,
                r#"{"media_type":"tv","id":2316,"name":"The Office","original_name":"The Office","#,
                r#""first_air_date":"2005-03-24"}"#,
                r#"],"total_pages":1,"total_results":2}"#,
            )
        );
        assert_eq!(search_json(&[]), r#"{"page":1,"results":[],"total_pages":1,"total_results":0}"#);
    }
}
