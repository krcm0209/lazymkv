import contextlib
import io
import math
import pathlib
import random
import tempfile
import unittest

import audio_similarity as sim
import robot

FPS = 50  # 20 ms frames


def speech(rnd, frames, loud, quiet):
    """Syllables: loud for 4-7 frames, quiet for 4-8 frames (about 4.5 per second)."""
    out = []
    while len(out) < frames:
        out += [loud + rnd.uniform(-1, 1)] * rnd.randint(4, 7)
        out += [quiet + rnd.uniform(-1, 1)] * rnd.randint(4, 8)
    return out[:frames]


def programme(seconds=420, seed=1):
    """A dialogue-driven programme: lines of 2-4 s, then 1.5-3 s of room tone. Returns (mix, centre)."""
    rnd = random.Random(seed)
    mix, centre = [], []
    while len(mix) < seconds * FPS:
        n = rnd.randint(2 * FPS, 4 * FPS)
        line = speech(rnd, n, -20, -30)
        mix += line
        centre += [v - 1 for v in line]
        n = rnd.randint(int(1.5 * FPS), 3 * FPS)
        mix += [-60 + rnd.uniform(-1, 1) for _ in range(n)]
        centre += [-75 + rnd.uniform(-1, 1) for _ in range(n)]
    return mix[:seconds * FPS], centre[:seconds * FPS]


def power_sum(a, b):
    return 10 * math.log10(10 ** (a / 10) + 10 ** (b / 10))


def commentary(mix, seed=7):
    """Two people talking all the way through, pausing briefly, over the programme ducked by 30 dB."""
    rnd = random.Random(seed)
    voice = []
    while len(voice) < len(mix):
        voice += speech(rnd, rnd.randint(3 * FPS, 6 * FPS), -18, -27)
        voice += [-100.0] * rnd.randint(15, 30)
    return [power_sum(v, m - 30) for v, m in zip(voice, mix)]


MIX, CENTRE = programme()

# What audio-levels.ps1 writes: the manifest, and compact level files with a header line.
MANIFEST = ("# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n"
            "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc={fc} frames=1 md5={main}\n"
            "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5={cand}\n")


def level_file(path, values):
    path.write_text("# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n"
                    + "\n".join(f"{v:.1f}" for v in values) + "\n", encoding="utf-8")


def raw_file(path, values, step_s):
    """ffmpeg's ametadata output: one frame every step_s seconds."""
    path.write_text("".join(f"frame:{k}    pts:{k}    pts_time:{k * step_s:g}\n"
                            f"lavfi.astats.Overall.RMS_level={v:.6f}\n" for k, v in enumerate(values)), encoding="utf-8")


class Reading(unittest.TestCase):
    def test_compact_and_raw_level_files_are_read_and_silence_is_the_floor(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            compact = pathlib.Path(tmp) / "main-mix.txt"
            compact.write_text("# lazymkv audio levels v2\n# stream=1 kind=main-mix\n-45.3\n-100.0\n-12.0\n", encoding="utf-8")
            self.assertEqual(sim.read_levels(compact), [-45.3, -100.0, -12.0])
            raw = pathlib.Path(tmp) / "raw.txt"
            raw.write_text("frame:0    pts:0       pts_time:0\nlavfi.astats.Overall.RMS_level=-inf\n"
                           "frame:1    pts:160     pts_time:0.02\nlavfi.astats.Overall.RMS_level=-123.5\n"
                           "frame:2    pts:320     pts_time:0.04\nlavfi.astats.Overall.RMS_level=-23.5\n", encoding="utf-8")
            self.assertEqual(sim.read_levels(raw), [sim.FLOOR, sim.FLOOR, -23.5])

    def test_manifest_lists_the_main_and_the_candidates(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            (pathlib.Path(tmp) / "manifest.txt").write_text(
                "# lazymkv audio-levels v2\nfile=x.mkv\nmain=1 codec=dts channels=6 layout=5.1(side) language=eng fc=yes md5=aa\n"
                "cand=3 codec=ac3 channels=2 layout=stereo language=eng md5=bb\ncand=4 codec=dts channels=6 md5=aa\n",
                encoding="utf-8")
            man = sim.read_manifest(tmp)
            self.assertEqual(man["main"]["main"], "1")
            self.assertEqual([c["cand"] for c in man["cands"]], ["3", "4"])
            self.assertEqual(man["file"], "x.mkv")


class Measures(unittest.TestCase):
    def test_a_quieter_delayed_copy_of_the_programme_is_kept(self):
        copy = [-100.0] * 3 + [v - 6 for v in MIX[:-3]]  # 60 ms late, 6 dB down
        m = sim.analyse(MIX, copy, CENTRE)
        self.assertEqual(m["lag_ms"], 60)
        self.assertGreaterEqual(m["dc"], 0.9)
        self.assertEqual(m["ag"], 0.0)
        self.assertEqual(sim.verdict(m), "keep")

    def test_a_voice_talking_through_the_programme_has_the_commentary_pattern(self):
        m = sim.analyse(MIX, commentary(MIX), CENTRE)
        self.assertEqual(sim.not_judged(m), "")
        self.assertLessEqual(m["dc"], 0.30)
        self.assertGreaterEqual(m["ag"], 0.50)
        self.assertEqual(m["tenths_comm"], m["tenths"])
        self.assertGreaterEqual(m["smi"], 0.50)
        self.assertLessEqual(m["ls_db"], 13.0)
        self.assertEqual(sim.verdict(m), "commentary")

    def test_the_same_voice_is_not_confirmed_when_the_main_is_not_continuous_programme_sound(self):
        mix, centre = list(MIX), list(CENTRE)
        for t in range(0, len(mix), 10 * FPS):  # 3 s of digital silence in every 10 s: an isolated score, say
            for u in range(t, min(len(mix), t + 3 * FPS)):
                mix[u] = centre[u] = -100.0
        m = sim.analyse(mix, commentary(MIX), centre)
        self.assertIn("silent", sim.not_judged(m))
        self.assertNotEqual(sim.verdict(m), "commentary")

    def test_steady_music_added_in_the_gaps_is_not_a_commentary(self):
        cand = [m if c > -50 else -35.0 for m, c in zip(MIX, CENTRE)]
        m = sim.analyse(MIX, cand, CENTRE)
        self.assertGreaterEqual(m["dc"], 0.8)
        self.assertNotIn(sim.verdict(m), ("commentary", "reference-unclear"))

    def test_a_candidate_covering_only_part_of_the_main_is_not_confirmed(self):
        m = sim.analyse(MIX, commentary(MIX)[:len(MIX) * 7 // 10], CENTRE)
        self.assertLess(m["overlap"], 0.9)
        self.assertNotEqual(sim.verdict(m), "commentary")

    def test_too_little_audio_gives_no_verdict(self):
        m = sim.analyse(MIX[:500], MIX[:500])
        self.assertEqual(sim.verdict(m), "not-judged")


class Rule(unittest.TestCase):
    BASE = {"overlap": 1.0, "main_silent": 0.03, "gap_s": 355.0, "dc": 0.03, "ag": 0.66, "ag_s": 234.0, "adl": 0.18,
            "tenths": 10, "tenths_comm": 10, "tenths_ad": 10, "smi": 0.58, "ls_db": 10.1, "added_s": 518.0,
            "rev_gap_s": 29.0, "rev_ag": 0.10}

    def v(self, **change):
        return sim.verdict(dict(self.BASE, **change))

    def test_the_measured_commentary_has_the_pattern(self):
        self.assertEqual(self.v(), "commentary")

    def test_each_condition_is_required(self):
        self.assertEqual(self.v(dc=0.31), "note-commentary")
        self.assertEqual(self.v(ag=0.49), "note-commentary")
        self.assertEqual(self.v(tenths_comm=8), "note-commentary")
        self.assertEqual(self.v(tenths=5, tenths_comm=5), "note-commentary")
        self.assertEqual(self.v(smi=0.49), "note-commentary")
        self.assertEqual(self.v(ls_db=13.1), "note-commentary")
        self.assertEqual(self.v(added_s=59.0), "note-commentary")
        self.assertEqual(self.v(main_silent=0.11), "note-commentary")
        self.assertEqual(self.v(gap_s=89.0), "note-commentary")
        self.assertEqual(self.v(overlap=0.89), "note-commentary")

    def test_a_main_track_that_adds_sound_to_the_candidate_makes_the_reference_unclear(self):
        self.assertEqual(self.v(rev_gap_s=150.0, rev_ag=0.35), "reference-unclear")
        self.assertEqual(self.v(rev_gap_s=60.0, rev_ag=0.35), "commentary")

    def test_audio_description_is_only_ever_a_note(self):
        ad = dict(dc=0.95, adl=0.0, ag=0.30, ag_s=100.0, tenths_comm=2, tenths_ad=9, smi=0.55, ls_db=17.0)
        self.assertEqual(self.v(**ad), "ad-pattern")
        self.assertEqual(self.v(**dict(ad, smi=0.30)), "note-ad")

    def test_a_copy_is_kept(self):
        self.assertEqual(self.v(dc=1.0, ag=0.0, ag_s=0.0, adl=0.0, tenths_comm=0, tenths_ad=0, smi=float("nan")), "keep")


class Report(unittest.TestCase):
    def folder(self, tmp, name="audio-levels-20ms-1-3"):
        d = pathlib.Path(tmp) / name
        d.mkdir()
        return d

    def test_an_identical_stream_is_reported_as_a_duplicate_without_analysis(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            for name in ("main-mix.txt", "cand-3.txt"):
                level_file(d / name, MIX[:2000])
            (d / "manifest.txt").write_text(MANIFEST.format(fc="no", main="43ae", cand="43ae"), encoding="utf-8")
            text = "\n".join(sim.report(d))
            self.assertIn("verdict: duplicate of stream 1", text)
            self.assertIn("its mono mixdown (no centre channel)", text)

    def test_a_folder_written_by_the_new_script_is_analysed(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            level_file(d / "main-mix.txt", MIX)
            level_file(d / "main-fc.txt", CENTRE)
            raw_file(d / "cand-3.txt", commentary(MIX), 0.02)  # raw ametadata output at 20 ms is read as well
            (d / "manifest.txt").write_text(MANIFEST.format(fc="yes", main="aa", cand="bb"), encoding="utf-8")
            self.assertEqual(sim.refusal(d), "")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                sim.main([str(d)])
            self.assertIn("dialogue reference: its centre channel", out.getvalue())
            self.assertIn("verdict: commentary", out.getvalue())

    def test_a_folder_of_the_first_measurement_is_refused_before_any_number(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp, "audio-levels-1-3")  # 100 ms frames, full band, no manifest
            for name in ("main-mix.txt", "main-fc.txt", "cand-mix.txt", "cand-mid.txt", "cand-side.txt"):
                raw_file(d / name, MIX[:3000:5], 0.1)
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as stop:
                sim.main([str(d)])
            self.assertIn("no manifest.txt", str(stop.exception.code))
            self.assertIn("measure the title again", str(stop.exception.code))
            self.assertEqual(out.getvalue(), "")
            self.assertRaises(ValueError, sim.report, d)

    def test_level_files_of_100_ms_frames_are_refused_even_with_a_manifest(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            for name in ("main-mix.txt", "cand-3.txt"):
                raw_file(d / name, MIX[:3000:5], 0.1)
            (d / "manifest.txt").write_text(MANIFEST.format(fc="no", main="aa", cand="bb"), encoding="utf-8")
            self.assertIn("main-mix.txt holds 100 ms frames, not 20 ms", sim.refusal(d))
            self.assertIn("measure the title again", sim.refusal(d))


class Titles(unittest.TestCase):
    SCAN = "\n".join(
        ['TINFO:0,9,0,"0:21:49"', 'TINFO:0,26,0,"7"', 'TINFO:1,9,0,"0:42:44"', 'TINFO:1,26,0,"4"',
         'TINFO:2,9,0,"1:04:33"', 'TINFO:2,26,0,"4,7"', 'TINFO:3,9,0,"0:05:10"', 'TINFO:3,26,0,"9"']
        + [f'SINFO:{t},{s},{k},0,"{v}"' for t, s, k, v in (
            (0, 0, 1, "Video"),
            (0, 1, 1, "Audio"), (0, 1, 3, "eng"), (0, 1, 22, "1024"),
            (0, 2, 1, "Audio"), (0, 2, 3, "eng"), (0, 2, 22, "2304"),  # the derived DTS core
            (0, 3, 1, "Audio"), (0, 3, 3, "eng"), (0, 3, 22, "0"),
            (1, 1, 1, "Audio"), (1, 1, 3, "eng"), (1, 1, 22, "1024"),
            (1, 2, 1, "Audio"), (1, 2, 3, "eng"), (1, 2, 22, "2304"),
            (1, 3, 1, "Audio"), (1, 3, 3, "fra"), (1, 3, 22, "0"),
            (2, 1, 1, "Audio"), (2, 1, 3, "eng"), (2, 1, 22, "0"),
            (3, 1, 1, "Audio"), (3, 1, 3, "eng"), (3, 2, 1, "Audio"), (3, 2, 3, "eng"))]) + "\n"

    def test_titles_list_audio_by_language_without_derived_streams_longest_first(self):
        info = sim.audio_by_title(robot.parse(self.SCAN))
        self.assertEqual(info[0], (1309, [7], {"eng": [1, 3]}))
        self.assertEqual(info[1][2], {"eng": [1], "fra": [3]})
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "some-disc"
            (d / "e16").mkdir(parents=True)
            (d / "scan.txt").write_text(self.SCAN, encoding="utf-8")
            (d / "e16" / "rip-summary.json").write_text('{"ok": true, "title": 0}', encoding="utf-8")
            lines = sim.titles_report(d)
        self.assertEqual([line.split()[1] for line in lines[2:]], ["2", "1", "0", "3"])
        self.assertIn("shares segments with title 0, 1", lines[2])
        self.assertNotIn("<-", lines[3])
        self.assertIn("<- eng x2; ripped as e16", lines[4])
        self.assertIn("<- eng x2", lines[5])  # no flags attribute at all: not derived


if __name__ == "__main__":
    unittest.main()
