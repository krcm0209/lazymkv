import contextlib
import io
import json
import math
import pathlib
import random
import tempfile
import unittest

import audio_similarity as sim
import robot

FPS = 50  # 20 ms frames
REPO = pathlib.Path(__file__).resolve().parents[2]


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

    def test_a_dvds_segments_are_not_compared_across_titles(self):
        # A DVD title's segments are the cells of its own program chain, numbered from 1 in every title.
        self.assertFalse(sim.dvd_scan(robot.parse(self.SCAN)))
        dvd = 'CINFO:1,6206,"DVD disc"\n' + self.SCAN
        self.assertTrue(sim.dvd_scan(robot.parse(dvd)))
        self.assertTrue(sim.dvd_scan(robot.parse(self.SCAN + 'SINFO:0,0,5,0,"V_MPEG2"\n')))
        self.assertFalse(sim.dvd_scan(robot.parse(dvd + 'TINFO:0,16,0,"00800.mpls"\n')))  # a source file: a Blu-ray
        firefly = REPO / "corpus" / "thediscdb" / "series" / "Firefly (2002)" / "2003-dvd-the-complete-series" / "disc01.txt"
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "firefly"
            d.mkdir()
            (d / "scan.txt").write_bytes(firefly.read_bytes())
            lines = sim.titles_report(d)
        self.assertIn("so shared segments are not looked for", lines[2])
        self.assertFalse([line for line in lines if "shares segments" in line])
        one = [line for line in lines if line.split()[:2] == ["title", "1"]]
        self.assertEqual(len(one), 1)
        self.assertIn("audio eng 1,4; fre 2; spa 3   <- eng x2", one[0])  # the episode with a commentary, not skipped

    @unittest.skipUnless((REPO / "corpus" / "owner" / "short-circuit-2-1988-dvd" / "scan.txt").is_file(),
                         "the evidence repository is not cloned at corpus/owner")
    def test_the_short_circuit_2_dvd_shares_no_segment(self):
        lines = sim.titles_report(REPO / "corpus" / "owner" / "short-circuit-2-1988-dvd")
        self.assertIn("so shared segments are not looked for", lines[2])
        self.assertFalse([line for line in lines if "shares segments" in line])

    def test_a_title_with_one_stream_in_each_wanted_language_is_marked(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "some-film"
            d.mkdir()
            (d / "scan.txt").write_text(self.SCAN.replace('SINFO:1,3,3,0,"fra"', 'SINFO:1,3,3,0,"spa"'), encoding="utf-8")
            self.assertNotIn("<-", sim.titles_report(d)[3])  # original language unknown: English alone is wanted
            (d / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
            lines = sim.titles_report(d)
        self.assertIn("TMDb original language: es", lines[0])
        self.assertEqual(lines[3].split()[1], "1")
        self.assertIn("audio eng 1; spa 3   <- wanted x2 (eng 1, spa 1)", lines[3])


def film(seconds, seed):
    """A film-like programme: lines of 2-3 s, then 2.5-4 s of quieter sound (music, effects), so that its level spread
    is a film's (about 23 dB), not that of dense talk. Returns (mix, centre)."""
    rnd = random.Random(seed)
    mix, centre = [], []
    while len(mix) < seconds * FPS:
        line = speech(rnd, rnd.randint(2 * FPS, 3 * FPS), -20, -30)
        mix += line
        centre += [v - 1 for v in line]
        n = rnd.randint(int(2.5 * FPS), 4 * FPS)
        mix += [-45 + rnd.uniform(-3, 3) for _ in range(n)]
        centre += [-75 + rnd.uniform(-1, 1) for _ in range(n)]
    return mix[:seconds * FPS], centre[:seconds * FPS]


class ReverseRun(unittest.TestCase):
    """EXP-13 addendum 9.3: the reverse run is judged with the commentary pattern itself (no number of its own)."""
    # The English commentary of Pan's Labyrinth against the Spanish main track, as measured (all-tracks, 1-2-3).
    PANS = {"overlap": 1.0, "main_silent": 0.0, "gap_s": 929.0, "dc": 0.08, "ag": 0.54, "ag_s": 505.0, "adl": 0.18,
            "tenths": 10, "tenths_comm": 9, "tenths_ad": 10, "smi": 0.51, "ls_db": 10.8, "added_s": 2016.0,
            "rev_gap_s": 470.0, "rev_ag": 0.31}
    REV = {"rev_error": "", "rev_overlap": 1.0, "rev_main_silent": 0.001, "rev_dc": 0.06, "rev_ag_s": 144.0,
           "rev_adl": 0.04, "rev_tenths": 10, "rev_tenths_comm": 1, "rev_tenths_ad": 10, "rev_smi": 0.39,
           "rev_smi_windows": 526, "rev_ls_db": 17.1, "rev_added_s": 796.0, "rev_gain_db": -6.7}

    def m(self, **change):
        return dict(dict(self.PANS, **self.REV), **change)

    def v(self, **change):
        return sim.verdict(self.m(**change))

    def test_a_commentary_whose_film_plays_in_its_pauses_keeps_its_verdict(self):
        self.assertEqual(self.v(), "commentary")
        # measures written before the reverse block existed are judged by the earlier reverse test, which blocked it
        self.assertEqual(sim.verdict(dict(self.PANS)), "reference-unclear")

    def test_a_reverse_run_with_the_commentary_pattern_makes_the_reference_unclear(self):
        pattern = dict(rev_dc=0.03, rev_ag=0.66, rev_tenths_comm=10, rev_smi=0.58, rev_ls_db=10.1, rev_added_s=517.0)
        self.assertEqual(self.v(**pattern), "reference-unclear")
        self.assertEqual(self.v(**dict(pattern, rev_gap_s=29.0)), "commentary")       # the reverse run cannot be judged
        self.assertEqual(self.v(**dict(pattern, rev_ls_db=13.1)), "commentary")       # no pattern: a condition fails
        self.assertEqual(self.v(**dict(pattern, rev_error="too short")), "commentary")
        not_judged = dict(pattern, gap_s=29.0, dc=0.02, ag=0.10, tenths=0, tenths_comm=0)
        self.assertEqual(self.v(**not_judged), "reference-unclear")                  # whatever the forward run says
        self.assertIn("90 s", sim.not_judged(self.m(**not_judged)))

    def test_swapped_exchanges_the_two_runs(self):
        m = self.m(gain_db=10.5)
        back = sim.swapped(m)
        self.assertEqual((back["dc"], back["ag"], back["gap_s"], back["ls_db"]), (0.06, 0.31, 470.0, 17.1))
        self.assertEqual((back["rev_dc"], back["rev_ag"], back["rev_tenths_comm"]), (0.08, 0.54, 9))
        self.assertEqual(sim.verdict(back), "reference-unclear")  # the programme against the commentary as reference
        self.assertIsNone(sim.swapped(dict(self.PANS)))

    def test_the_programme_against_a_commentary_taken_as_the_reference_is_kept_and_flagged(self):
        m = sim.analyse(commentary(MIX), MIX, None)
        self.assertTrue(sim.reverse_pattern(m))
        self.assertEqual(sim.verdict(m), "reference-unclear")

    def test_a_commentary_that_pauses_over_a_film_is_not_blocked_by_its_pauses(self):
        mix, centre = film(900, 3)
        rnd = random.Random(10)
        voice = []
        while len(voice) < len(mix):  # one speaker: 2.8 s of talk, 1.2 s of pause, the film 26 dB down underneath
            voice += speech(rnd, int(2.8 * FPS), -18, -27) + [-100.0] * int(1.2 * FPS)
        m = sim.analyse(mix, [power_sum(v, x - 26) for v, x in zip(voice, mix)], centre)
        self.assertTrue(m["rev_gap_s"] >= 90 and m["rev_ag"] >= 0.30)  # the earlier reverse test would block it
        self.assertFalse(sim.reverse_pattern(m))
        self.assertEqual(sim.verdict(m), "commentary")


class CrossLanguage(unittest.TestCase):
    def test_a_commentary_in_another_language_is_judged_against_the_programme_reference(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "audio-levels-20ms-1-3"
            d.mkdir()
            level_file(d / "main-mix.txt", MIX)
            level_file(d / "main-fc.txt", CENTRE)
            level_file(d / "cand-3.txt", commentary(MIX))
            (d / "manifest.txt").write_text(MANIFEST.format(fc="yes", main="aa", cand="bb").replace(
                "layout=5.1(side) language=eng", "layout=5.1(side) language=spa"), encoding="utf-8")
            res = sim.results(d)
            self.assertEqual((res["candidates"][0]["cross_language"], res["candidates"][0]["verdict"]),
                             ("eng against spa", "commentary-cross-language"))  # never a drop verdict (addendum 9.2)
            self.assertIsNone(res["candidates"][0]["peer"])  # no other English stream: nothing to compare it with
            self.assertIn("stream 3: ac3 2 ch stereo eng (cross-language: eng against spa)", "\n".join(sim.report(d)))
        self.assertEqual(sim.cross_language({"language": "spa"}, {"language": "und"}), "")

    def folder(self, tmp, cands):
        """A Spanish film's folder: cands [(stream, levels, channels[, language[, codec, profile]])], English AC-3 by
        default."""
        d = pathlib.Path(tmp) / ("audio-levels-20ms-1-" + "-".join(str(c[0]) for c in cands))
        d.mkdir()
        level_file(d / "main-mix.txt", MIX)
        level_file(d / "main-fc.txt", CENTRE)
        lines = ["# lazymkv audio-levels v2 (experiment 13)", "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400",
                 "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=spa fc=yes frames=1 md5=aa"]
        for n, values, ch, *more in cands:
            lang, codec, profile = (list(more) + ["eng", "ac3", "-"][len(more):])[:3]
            level_file(d / f"cand-{n}.txt", values)
            lines.append(f"cand={n} codec={codec} profile={profile} channels={ch} layout=x language={lang} frames=1 md5=m{n}")
        (d / "manifest.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
        return d

    def test_a_companion_of_a_dub_follows_the_dub_and_gets_only_a_note(self):
        # The dub stand-in has the commentary pattern against the original, as a loosely timed dub can (addendum 9.2);
        # its 2.0 companion is the same mix 6 dB down and 60 ms late.
        dub = commentary(MIX)
        companion = [-100.0] * 3 + [v - 6 for v in dub[:-3]]
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp, [(2, dub, 6), (3, companion, 2)])
            first, second = sim.results(d)["candidates"]
            text = "\n".join(sim.report(d))
        self.assertEqual((first["verdict"], first["peer"]), ("commentary-cross-language", None))  # the first English stream
        self.assertEqual((second["peer"]["stream"], second["peer"]["verdict"]), ("2", "keep"))
        self.assertGreater(second["peer"]["measures"]["dc"], 0.9)
        self.assertEqual(second["verdict"], "note-commentary")
        self.assertIn("    peer: against stream 2 (placed first in the candidate's language): DC 1.00", text)
        self.assertIn("keep, so not confirmed", text)

    def test_a_commentary_beside_a_dub_keeps_the_cross_language_verdict(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp, [(2, [v - 2 for v in MIX], 6), (3, commentary(MIX), 2)])  # a dub that is the programme
            dub, comm = sim.results(d)["candidates"]
        self.assertEqual(dub["verdict"], "keep")
        self.assertEqual((comm["peer"]["stream"], comm["peer"]["verdict"]), ("2", "commentary"))
        self.assertEqual(comm["verdict"], "commentary-cross-language")

    def test_the_peer_is_the_first_stream_of_the_candidates_own_language_as_8_6_ranks_it(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            # a Spanish core (6 channels) beside the English commentary: not the commentary's peer
            d = self.folder(tmp, [(2, [v - 2 for v in MIX], 6, "spa"), (3, commentary(MIX), 2)])
            core, comm = sim.results(d)["candidates"]
            self.assertEqual((core["verdict"], comm["verdict"], comm["peer"]), ("keep", "commentary-cross-language", None))
            # two English 5.1 streams: the lossless one (the manifest writes DTS-HD_MA) is placed first
            d = self.folder(tmp, [(4, commentary(MIX), 6, "eng", "ac3", "-"), (5, MIX, 6, "eng", "dts", "DTS-HD_MA")])
            comm, dub = sim.results(d)["candidates"]
        self.assertEqual(comm["peer"]["stream"], "5")

    def test_the_language_table_is_iso_639_and_tmdbs_own_codes(self):
        self.assertEqual((sim.ISO639["ml"], sim.ISO639["bo"], sim.ISO639["es"]), (("mal",), ("bod", "tib"), ("spa",)))
        self.assertIn("yue", sim.ISO639["cn"])
        self.assertIn("hrv", sim.ISO639["sh"])
        self.assertNotIn(sim.TMDB_NO_LANGUAGE, sim.ISO639)
        self.assertGreaterEqual(len(sim.ISO639), 185)
        for codes in sim.ISO639.values():
            self.assertTrue(codes and all(len(c) == 3 and c.isalpha() and c.islower() for c in codes), codes)

    def test_a_dub_is_kept_by_where_its_speech_sits_not_by_dc(self):
        rnd = random.Random(11)
        other = speech(rnd, len(MIX), -20, -30)  # other words, in the programme's dialogue windows only
        dub = [o if c > -50 else x for o, x, c in zip(other, MIX, CENTRE)]
        m = sim.analyse(MIX, dub, CENTRE)
        self.assertLess(m["dc"], 0.9)
        self.assertLess(m["ag"], 0.10)
        self.assertEqual(sim.verdict(m), "keep")


def streams_json(d, streams):
    """streams.json as audio-levels.ps1 writes it (ffprobe's stream list): [(index, codec, profile, channels, language)]
    after a video stream 0."""
    (d / "streams.json").write_text(json.dumps({"streams": [{"index": 0, "codec_type": "video"}] + [
        {"index": n, "codec_type": "audio", "codec_name": c, "profile": p, "channels": ch, "tags": {"language": lang}}
        for n, c, p, ch, lang in streams]}), encoding="utf-8")


class PositionVeto(unittest.TestCase):
    """Row 0 of the rule (owner decision D21): a reference that is not the first stream of its language, when that
    first stream is lossless or has more than two channels, confirms no commentary on the title."""

    @staticmethod
    def veto(streams, main):
        audio = [{"index": 0, "codec_type": "video"}] + [
            {"index": n, "codec_type": "audio", "codec_name": c, "profile": p, "channels": ch, "tags": {"language": lang}}
            for n, c, p, ch, lang in streams]
        return sim.position_veto(audio, main)

    def test_it_holds_when_a_lossless_first_stream_is_not_the_reference(self):
        holds, why = self.veto([(1, "truehd", None, 2, "eng"), (2, "ac3", None, 6, "eng")], 2)  # 8.6 ranks the 5.1 first
        self.assertIs(holds, True)
        self.assertEqual(why, "the reference, stream 2 (ac3 6 ch), is not the first eng stream of the file: stream 1 "
                              "(truehd 2 ch) comes first and is lossless or has more than two channels, so the reference "
                              "choice may be inverted")

    def test_it_holds_when_a_multichannel_lossy_first_stream_is_not_the_reference(self):
        holds, why = self.veto([(1, "ac3", None, 6, "eng"), (2, "dts", "DTS-HD MA", 6, "eng")], 2)
        self.assertIs(holds, True)
        self.assertIn("stream 1 (ac3 6 ch) comes first", why)

    def test_it_does_not_hold_otherwise(self):
        self.assertEqual(self.veto([(1, "ac3", None, 2, "eng"), (2, "dts", "DTS", 6, "eng")], 2),
                         (False, "the reference, stream 2 (dts DTS 6 ch), is not the first eng stream of the file, but "
                                 "stream 1 (ac3 2 ch), which comes first, is lossy with two channels or fewer"))
        self.assertEqual(self.veto([(1, "dts", "DTS-HD MA", 6, "eng"), (2, "ac3", None, 2, "eng")], 1),
                         (False, "the reference, stream 1 (dts DTS-HD MA 6 ch), is the first eng stream of the file"))
        self.assertIs(self.veto([(1, "ac3", None, 2, "eng")], 1)[0], False)  # a single stream
        self.assertIs(self.veto([(1, "truehd", None, 8, "fra"), (2, "ac3", None, 6, "spa")], 2)[0], False)  # other language
        self.assertEqual(sim.position_veto(None, 1), (None, "not checked: the folder has no streams.json"))
        self.assertIs(self.veto([(2, "ac3", None, 6, "eng")], 5)[0], None)
        self.assertIsNone(sim.first_of_language([], 1))

    def test_it_never_turns_a_keep_into_a_drop(self):
        copy = [-100.0] * 3 + [v - 6 for v in MIX[:-3]]  # a quieter, 60 ms late copy of the programme
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "audio-levels-20ms-2-1-3-4"
            d.mkdir()
            level_file(d / "main-mix.txt", MIX)
            level_file(d / "main-fc.txt", CENTRE)
            for n, values in ((1, copy), (3, commentary(MIX)), (4, MIX)):
                level_file(d / f"cand-{n}.txt", values)
            (d / "manifest.txt").write_text(
                "# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n"
                "main=2 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=1 md5=aa\n"
                "cand=1 codec=dts profile=DTS channels=6 layout=5.1(side) language=eng frames=1 md5=bb\n"
                "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5=cc\n"
                "cand=4 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng frames=1 md5=aa\n",
                encoding="utf-8")
            cache = {}
            def analyse(main, cand, dialogue):
                key = (len(main), round(sum(cand), 1), dialogue is None)
                if key not in cache:
                    cache[key] = sim.analyse(main, cand, dialogue)
                return cache[key]
            free = sim.results(d, analyse_fn=analyse)  # no streams.json: the veto is not checked
            streams_json(d, [(1, "dts", "DTS", 6, "eng"), (2, "dts", "DTS-HD MA", 6, "eng"), (3, "ac3", None, 2, "eng"),
                             (4, "dts", "DTS-HD MA", 6, "eng")])
            vetoed = sim.results(d, analyse_fn=analyse)
            text = "\n".join(sim.report(d))
        self.assertEqual(free["veto"]["holds"], None)
        self.assertEqual([c["verdict"] for c in free["candidates"]], ["keep", "commentary", "duplicate"])
        self.assertIs(vetoed["veto"]["holds"], True)
        self.assertEqual([c["verdict"] for c in vetoed["candidates"]], ["reference-unclear", "reference-unclear", "duplicate"])
        self.assertEqual([c["before_veto"] for c in vetoed["candidates"]], ["keep", "commentary", None])
        self.assertIn("  veto: holds (row 0, D21): the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng "
                      "stream of the file: stream 1 (dts DTS 6 ch) comes first", text)
        self.assertIn("verdict: reference-unclear (the position veto holds (row 0, D21): the reference may not be the "
                      "programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear); "
                      "rows 3 to 9 alone: commentary)", text)


if __name__ == "__main__":
    unittest.main()
