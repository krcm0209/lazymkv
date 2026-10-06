import contextlib
import io
import json
import math
import pathlib
import random
import shutil
import tempfile
import unittest

import audio_dataset as ds
import audio_labels as labels
import audio_similarity as sim

FPS = 50  # 20 ms frames
REAL_SPEC = pathlib.Path(__file__).resolve().parents[2] / "docs" / "superpowers" / "specs" / "2026-10-04-lazymkv-design.md"
ROW13 = ("| 13 | Measured over whole titles on the owner's own discs, does the content rule give its commentary verdict to "
         "**no** programme track, and to at least one commentary on each of at least {verdict_discs} different discs? Before "
         "the answer may be yes, the labelled tracks must include: at least {comm} commentaries from at least {discs} discs, "
         "at least {features} of them on feature films and at least 1 with the film clearly audible under the voices; at "
         "least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least "
         "{prog} programme tracks other than the reference, at least {sep} of them separately encoded lossy companions or "
         "alternate mixes; at least {centre} title whose reference has no centre channel (mono or 2.0) and at least "
         "{heavy} dialogue-heavy or speech-only title, each with an extra track. Every threshold was fitted on two tracks of "
         "one disc (`fit-disc`: the S03E16 commentary), so no track of that disc counts. | fallback |")


def spec(**numbers):
    n = dict(verdict_discs=3, comm=8, discs=4, features=2, prog=20, sep=6, centre=1, heavy=1)
    n.update(numbers)
    return "# spec\n\n## 17. Experiments (phase 0)\n\n| # | Question | If no |\n|---|---|---|\n" + ROW13.format(**n) + "\n"


SPEC = spec()
DUB_SPEC = SPEC.replace("each with an extra track.", "each with an extra track; at least 3 dubs in a wanted language from "
                        "at least 2 discs, at least 1 of them on a dialogue-heavy title.")


def speech(rnd, frames, loud, quiet):
    out = []
    while len(out) < frames:
        out += [loud + rnd.uniform(-1, 1)] * rnd.randint(4, 7)
        out += [quiet + rnd.uniform(-1, 1)] * rnd.randint(4, 8)
    return out[:frames]


def programme(seconds=420, seed=1):
    rnd = random.Random(seed)
    mix, centre = [], []
    while len(mix) < seconds * FPS:
        line = speech(rnd, rnd.randint(2 * FPS, 4 * FPS), -20, -30)
        mix += line
        centre += [v - 1 for v in line]
        n = rnd.randint(int(1.5 * FPS), 3 * FPS)
        mix += [-60 + rnd.uniform(-1, 1) for _ in range(n)]
        centre += [-75 + rnd.uniform(-1, 1) for _ in range(n)]
    return mix[:seconds * FPS], centre[:seconds * FPS]


def commentary(mix, seed=7):
    rnd = random.Random(seed)
    voice = []
    while len(voice) < len(mix):
        voice += speech(rnd, rnd.randint(3 * FPS, 6 * FPS), -18, -27)
        voice += [-100.0] * rnd.randint(15, 30)
    return [10 * math.log10(10 ** (v / 10) + 10 ** ((m - 30) / 10)) for v, m in zip(voice, mix)]


MIX, CENTRE = programme()
COMMENT = commentary(MIX)
COPY = [-100.0] * 3 + [v - 6 for v in MIX[:-3]]  # a quieter, 60 ms late copy: programme audio


def level_file(path, values):
    path.write_text("# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n"
                    + "\n".join(f"{v:.1f}" for v in values) + "\n", encoding="utf-8")


def folder(label_dir, main_md5, cands, fc=True, channels=6, main=1, streams=None, langs=None):
    """A level folder as audio-levels.ps1 writes it. cands: [(stream, levels, md5)]. streams: the file's audio
    streams for streams.json, [(index, channels, codec, profile, language)]; by default every stream up to the highest
    measured one, English, the main with `channels` and the others 2. langs: {stream: language} for the manifest
    (English when not given)."""
    lang = lambda n: (langs or {}).get(n, "eng")
    d = label_dir / ("audio-levels-20ms-" + "-".join(str(n) for n in [main] + [n for n, _, _ in cands]))
    d.mkdir(parents=True)
    level_file(d / "main-mix.txt", MIX)
    if fc:
        level_file(d / "main-fc.txt", CENTRE)
    lines = ["# lazymkv audio-levels v2 (experiment 13)", "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400",
             f"main={main} codec=dts profile=- channels={channels} layout={'5.1(side)' if fc else 'stereo'} language={lang(main)} "
             f"fc={'yes' if fc else 'no'} frames={len(MIX)} md5={main_md5}"]
    for n, values, md5 in cands:
        level_file(d / f"cand-{n}.txt", values)
        lines.append(f"cand={n} codec=ac3 profile=- channels=2 layout=stereo language={lang(n)} frames={len(values)} md5={md5}")
    (d / "manifest.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    if streams is None:
        top = max([main] + [n for n, _, _ in cands])
        streams = [(k, channels if k == main else 2, "dts" if k == main else "ac3", None, "eng") for k in range(1, top + 1)]
    (d / "streams.json").write_text(json.dumps({"streams": [{"index": 0, "codec_type": "video"}] + [
        {"index": k, "codec_type": "audio", "codec_name": c, "profile": p, "channels": ch, "tags": {"language": lang, "title": f"t{k}"}}
        for k, ch, c, p, lang in streams]}), encoding="utf-8")
    return d


def entry(label, stream, kind, how="listened", saw=False, sure=True, **more):
    return dict({"label": label, "stream": stream, "kind": kind, "how": how, "sure": sure, "date": "2026-10-05",
                 "saw_verdict": saw, "note": ""}, **more)


def title(kind="episode", heavy=False):
    return {"title_kind": kind, "dialogue_heavy": heavy, "note": ""}


def write_labels(slug_dir, tracks, titles=None):
    slug_dir.mkdir(parents=True, exist_ok=True)
    (slug_dir / "audio-labels.json").write_text(json.dumps(
        {"format": labels.FORMAT, "slug": slug_dir.name, "titles": titles or {}, "tracks": tracks}), encoding="utf-8")


def run(args):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = ds.main([str(a) for a in args])
    return code, out.getvalue() + err.getvalue()


def run_labels(args):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = labels.main([str(a) for a in args])
    return code, out.getvalue() + err.getvalue()


def section(out, heading):
    return out.split(heading, 1)[1].split("\n\n", 1)[0]


class Spec(unittest.TestCase):
    def test_row_13_gives_the_totals_and_the_fitting_disc(self):
        req = labels.exp13_requirements(SPEC)
        self.assertEqual((req["commentaries"], req["commentary_discs"], req["commentary_features"], req["commentary_audible"]), (8, 4, 2, 1))
        self.assertEqual((req["ad_min"], req["ad_max"], req["programme"], req["programme_separate"]), (1, 3, 20, 6))
        self.assertEqual((req["no_centre_titles"], req["dialogue_heavy_titles"], req["verdict_discs"]), (1, 1, 3))
        self.assertEqual(req["fitting_slug"], "fit-disc")
        decoy = SPEC.replace("**no** programme track", "**no** programme track (`decoy-disc`)")
        self.assertEqual(labels.exp13_requirements(decoy)["fitting_slug"], "fit-disc")

    def test_row_13_may_set_the_dubs_a_cross_language_verdict_needs(self):
        self.assertEqual([labels.exp13_requirements(SPEC)[k] for k in ("dubs", "dub_discs", "dub_heavy")], [None] * 3)
        self.assertEqual([labels.exp13_requirements(DUB_SPEC)[k] for k in ("dubs", "dub_discs", "dub_heavy")], [3, 2, 1])

    def test_a_reworded_row_is_refused_by_name(self):
        with self.assertRaises(ValueError) as stop:
            labels.exp13_requirements(SPEC.replace("commentaries from at least", "commentaries over"))
        self.assertIn("no longer contains the phrase", str(stop.exception))
        self.assertRaises(ValueError, labels.exp13_requirements, SPEC.replace("| 13 |", "| 14 |"))

    @unittest.skipUnless(REAL_SPEC.is_file(), "the repository's spec is not beside this folder")
    def test_the_repositorys_spec_names_the_office_disc(self):
        req = labels.exp13_requirements(REAL_SPEC.read_text(encoding="utf-8-sig"))
        self.assertEqual(req["fitting_slug"], "the-office-2005-s3d3-bluray")
        self.assertEqual(req["commentaries"], 8)


class Labels(unittest.TestCase):
    def check(self, *tracks, slug="disc-b", titles=None, **more):
        return labels.problems(dict({"format": labels.FORMAT, "slug": slug, "titles": titles or {}, "tracks": list(tracks)}, **more), slug)

    def test_a_good_file_has_no_problem(self):
        old = dict(entry("t0", 3, "lossy-companion"), replaced="2026-10-06")
        self.assertEqual(self.check(entry("t0", 3, "commentary", film_audible=False, history=[old]), entry("t0", 2, "duplicate", "md5"),
                                    titles={"t0": title("feature", None)},
                                    removed=[dict(entry("t0", 9, "commentary"), removed="2026-10-06", why="typo")]), [])

    def test_each_rule_is_enforced(self):
        cases = [
            (entry("t0", 3, "comentary"), "not in the vocabulary"),
            (entry("t0", 3, "commentary", how="told"), "how 'told' is not one of"),
            (entry("t0", 3, "commentary", how="md5"), "how md5 only says"),
            (entry("t0", 3, "lossy-companion", film_audible=True), "film_audible is only for"),
            (entry("t0", 3, "commentary", film_audible="yes"), "film_audible must be"),
            (entry("t0", 3, "commentary", sure="yes"), "sure must be true or false"),
            (entry("t0", 3, "commentary", saw=None), "saw_verdict must be true or false"),
            (dict(entry("t0", 3, "commentary"), date="5 Oct"), "is not YYYY-MM-DD"),
            (dict(entry("t0", 3, "commentary"), colour="red"), "unknown keys"),
            (dict(entry("t0", 3, "commentary"), counts=True), "unknown keys"),
            (dict(entry("t0", 3, "commentary"), stream="3"), "stream is not"),
            ({k: v for k, v in entry("t0", 3, "commentary").items() if k != "sure"}, "missing ['sure']"),
            (entry("t0", 3, "commentary", history=[entry("t0", 3, "unknown")]), "replaced None is not YYYY-MM-DD"),
        ]
        for track, words in cases:
            self.assertTrue(any(words in p for p in self.check(track)), (track, self.check(track)))
        self.assertIn("labelled twice", " ".join(self.check(entry("t0", 3, "commentary"), entry("t0", 3, "commentary"))))
        self.assertIn("title_kind", " ".join(self.check(titles={"t0": {"title_kind": "film"}})))
        self.assertIn("dialogue_heavy must be", " ".join(self.check(titles={"t0": dict(title(), dialogue_heavy="yes")})))
        self.assertIn("unknown top-level keys ['extra']", " ".join(self.check(extra=1)))
        self.assertIn("removed 0: removed", " ".join(self.check(removed=[entry("t0", 9, "commentary")])))
        self.assertIn("is in corpus/owner/disc-b", " ".join(labels.problems(
            {"format": labels.FORMAT, "slug": "disc-x", "titles": {}, "tracks": []}, "disc-b")))

    def test_whether_a_label_counts_is_worked_out_from_row_13_and_the_fitted_list(self):
        fitted = {("fit-disc", None, None), ("disc-h", "t0", 2)}
        why = lambda slug="disc-b", stream=3, **e: labels.not_counted_because(entry("t0", stream, **e), slug, fitted)
        self.assertEqual(why(kind="commentary"), "")
        self.assertEqual(why(kind="derived-core", how="flag"), "")
        self.assertIn("never counts", why(kind="main"))
        self.assertIn("never counts", why(kind="duplicate", how="md5"))
        self.assertIn("never counts", why(kind="unknown"))
        self.assertIn("not a source row 13 accepts", why(kind="commentary", how="inferred"))
        self.assertIn("saw the verdict", why(kind="commentary", saw=True))
        self.assertIn("not sure", why(kind="commentary", sure=False))
        self.assertIn("fitted on it", why(kind="commentary", slug="fit-disc"))
        self.assertIn("fitted on it", why(kind="commentary", slug="disc-h", stream=2))
        self.assertEqual(why(kind="commentary", slug="disc-h", stream=3), "")

    def test_set_adds_replaces_keeps_history_and_refuses_a_new_problem(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            root = pathlib.Path(tmp)
            (root / "spec.md").write_text(SPEC, encoding="utf-8")
            slug_dir = root / "disc-b"
            slug_dir.mkdir()
            set_ = ["--spec", root / "spec.md", "set", slug_dir, "t0", "3"]
            code, out = run_labels(set_ + ["commentary", "--how", "listened", "--saw-verdict", "no", "--film-audible", "yes", "--date", "2026-10-05"])
            self.assertEqual(code, 0, out)
            self.assertIn("labelled t0 stream 3: commentary (listened, 2026-10-05, saw verdict no, sure yes), counts=True", out)
            code, out = run_labels(set_ + ["commentary", "--how", "listened", "--saw-verdict", "no", "--film-audible", "yes", "--date", "2026-10-07"])
            self.assertIn("unchanged t0 stream 3", out)
            code, out = run_labels(set_ + ["partial-commentary", "--how", "menu", "--saw-verdict", "yes", "--date", "2026-10-06"])
            self.assertEqual(code, 0, out)
            self.assertIn("counts=False (the owner saw the verdict first); replaces 1 earlier label(s)", out)
            code, out = run_labels(set_ + ["unknown", "--how", "listened", "--saw-verdict", "no", "--sure", "no", "--date", "2026-10-07"])
            self.assertIn("counts=False (a unknown track never counts)", out)
            self.assertEqual(run_labels(["--spec", root / "spec.md", "title", slug_dir, "t0", "--title-kind", "feature", "--dialogue-heavy", "no"])[0], 0)
            data = json.loads((slug_dir / "audio-labels.json").read_text(encoding="utf-8"))
            self.assertEqual(len(data["tracks"]), 1)  # replaced, not added twice
            t = data["tracks"][0]
            self.assertEqual((t["kind"], t["sure"]), ("unknown", False))
            self.assertEqual([(h["kind"], h["saw_verdict"], h["replaced"]) for h in t["history"]],
                             [("commentary", False, "2026-10-06"), ("partial-commentary", True, "2026-10-07")])
            self.assertTrue(t["history"][0]["film_audible"])  # the blind label survives, with everything it said
            self.assertEqual(data["titles"]["t0"], {"title_kind": "feature", "dialogue_heavy": False, "note": ""})
            before = (slug_dir / "audio-labels.json").read_text(encoding="utf-8")
            code, out = run_labels(set_[:-1] + ["2", "lossy-companion", "--how", "menu", "--saw-verdict", "no", "--film-audible", "yes"])
            self.assertEqual(code, 2)
            self.assertIn("film_audible is only for", out)
            self.assertEqual((slug_dir / "audio-labels.json").read_text(encoding="utf-8"), before)

    def test_unset_keeps_the_history_of_a_label_that_was_replaced(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            root = pathlib.Path(tmp)
            (root / "spec.md").write_text(SPEC, encoding="utf-8")
            slug_dir = root / "disc-b"
            slug_dir.mkdir()
            set_ = ["--spec", root / "spec.md", "set", slug_dir, "t0", "3"]
            self.assertEqual(run_labels(set_ + ["commentary", "--how", "listened", "--saw-verdict", "no", "--sure", "no"])[0], 0)
            self.assertEqual(run_labels(set_ + ["commentary", "--how", "listened", "--saw-verdict", "no"])[0], 0)
            code, out = run_labels(["--spec", root / "spec.md", "unset", slug_dir, "t0", "3", "--note", "wrong stream"])
            self.assertEqual(code, 0, out)
            self.assertEqual(run_labels(["--spec", root / "spec.md", "check", root])[0], 0)
            data = json.loads((slug_dir / "audio-labels.json").read_text(encoding="utf-8"))
            self.assertEqual(data["tracks"], [])
            self.assertEqual(len(data["removed"][0]["history"]), 1)

    def test_a_broken_file_is_repaired_with_the_tool_one_entry_at_a_time(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            root = pathlib.Path(tmp)
            (root / "spec.md").write_text(SPEC, encoding="utf-8")
            slug_dir = root / "disc-b"
            write_labels(slug_dir, [entry("t0", 3, "comentary"), entry("t0", 9, "commentary"), entry("t0", 4, "commentary", how="told")])
            self.assertEqual(run_labels(["--spec", root / "spec.md", "check", root])[0], 2)
            code, out = run_labels(["--spec", root / "spec.md", "set", slug_dir, "t0", "3", "commentary", "--how", "listened", "--saw-verdict", "no"])
            self.assertEqual(code, 0, out)
            self.assertIn("warning: the file still has 1 problem(s): track t0 stream 4: how 'told'", out)
            code, out = run_labels(["--spec", root / "spec.md", "unset", slug_dir, "t0", "4", "--note", "wrong source"])
            self.assertEqual(code, 0, out)
            code, out = run_labels(["--spec", root / "spec.md", "unset", slug_dir, "t0", "9", "--note", "typo: stream 3"])
            self.assertEqual(code, 0, out)
            self.assertEqual(run_labels(["--spec", root / "spec.md", "unset", slug_dir, "t0", "9", "--note", "again"])[0], 2)
            code, out = run_labels(["--spec", root / "spec.md", "check", root])
            self.assertEqual(code, 0, out)
            data = json.loads((slug_dir / "audio-labels.json").read_text(encoding="utf-8"))
            self.assertEqual([t["stream"] for t in data["tracks"]], [3])
            self.assertEqual(data["tracks"][0]["history"][0]["kind"], "comentary")
            self.assertEqual([(r["stream"], r["why"]) for r in data["removed"]], [(4, "wrong source"), (9, "typo: stream 3")])

    def test_fitted_records_a_disc_or_a_track(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            root = pathlib.Path(tmp)
            (root / "spec.md").write_text(SPEC, encoding="utf-8")
            code, out = run_labels(["--spec", root / "spec.md", "fitted", root, "disc-g", "--rule", "abc123", "--note", "DC_MAX re-fit"])
            self.assertEqual(code, 0, out)
            self.assertIn("fitted: disc-g (the whole disc): no longer counts", out)
            self.assertEqual(run_labels(["--spec", root / "spec.md", "fitted", root, "disc-h", "t0", "2", "--rule", "abc123"])[0], 0)
            self.assertEqual(run_labels(["--spec", root / "spec.md", "fitted", root, "disc-h", "t0", "--rule", "x"])[0], 2)
            self.assertEqual(labels.load_fitted(root, "fit-disc"),
                             {("fit-disc", None, None), ("disc-g", None, None), ("disc-h", "t0", 2)})


def build_main_corpus(c):
    """fit-disc: the disc the thresholds were fitted on. disc-b: a programme copy, a commentary, an unlabelled exact
    duplicate, and the same title ripped again (eng-only) with the commentary's packets unchanged; a successful extract
    not measured yet and a failed share-drop rip. disc-c: a title measured for the dataset only (no candidate)."""
    folder(c / "fit-disc" / "t0", "m0", [(3, COMMENT, "c0")])
    write_labels(c / "fit-disc", [entry("t0", 3, "commentary")])
    lv = folder(c / "disc-b" / "all-tracks", "mb", [(2, COPY, "cp"), (3, COMMENT, "cb"), (4, MIX, "mb")])
    (lv / "audio-proxy.json").write_text("{}", encoding="utf-8")
    (c / "disc-b" / "all-tracks" / "rip-summary.json").write_text('{"ok": true, "title": 0, "bytes": 100}', encoding="utf-8")
    (c / "disc-b" / "extract").mkdir()
    (c / "disc-b" / "extract" / "extract-summary.json").write_text('{"ok":  true}', encoding="utf-8-sig")  # PowerShell: BOM
    (c / "disc-b" / "share-drop").mkdir()
    (c / "disc-b" / "share-drop" / "rip-summary.json").write_text('{"ok": false}', encoding="utf-8")
    folder(c / "disc-b" / "eng-only", "mb", [(3, COMMENT, "cb")])
    write_labels(c / "disc-b", [entry("all-tracks", 2, "lossy-companion"), entry("all-tracks", 3, "commentary", film_audible=False)],
                 {"all-tracks": title("feature")})
    folder(c / "disc-c" / "t1", "mc", [], fc=False, channels=2)


class Dataset(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        root = pathlib.Path(cls.tmp.name)
        (root / "spec.md").write_text(SPEC, encoding="utf-8")
        cls.root, cls.corpus, cls.spec = root, root / "corpus", root / "spec.md"
        build_main_corpus(cls.corpus)
        cls.code, cls.out = run([cls.corpus, "--spec", cls.spec])

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def copy(self, tmp):
        c = pathlib.Path(tmp) / "corpus"
        shutil.copytree(self.corpus, c)
        return c

    def test_the_baseline_run_passes_and_writes_its_snapshot(self):
        self.assertEqual(self.code, 0, self.out)
        self.assertTrue(self.out.rstrip().endswith("RESULT ok"))
        snap = json.loads((self.corpus / "audio-dataset.json").read_text(encoding="utf-8"))
        self.assertEqual(snap["format"], ds.SNAPSHOT_FORMAT)
        comm = [t for t in snap["tracks"] if (t["slug"], t["label"], t["stream"]) == ("disc-b", "all-tracks", 3)][0]
        self.assertEqual((comm["verdict"], comm["kind"], comm["counts"]), ("commentary", "commentary", True))
        self.assertEqual(comm["measures"], snap["measures_cache"][comm["key"]])
        dup = [t for t in snap["tracks"] if (t["slug"], t["label"], t["stream"]) == ("disc-b", "all-tracks", 4)][0]
        self.assertIsNone(dup["verdict"])  # unlabelled: its verdict is kept apart
        self.assertEqual(snap["unlabelled_verdicts"], {"disc-b/all-tracks/audio-levels-20ms-1-2-3-4/candidate/4": "duplicate"})

    def test_the_owners_to_do_list_shows_no_verdict(self):
        todo = section(self.out, "To label (measured, no label yet")
        self.assertIn('disc-b/all-tracks stream 4: ac3 2 ch eng (audio track 4 of 4 in a player, named "t4")', todo)
        self.assertIn("audio_labels.py set corpus/owner/disc-b all-tracks 4 <kind>", todo)
        for v in ds.VERDICTS:
            self.assertNotIn(v, todo.replace("<kind>", ""))
        self.assertNotIn("eng-only", todo)  # its stream 3 has the labelled track's packets: the label carries over

    def test_kind_against_verdict(self):
        rows = {line.split()[0]: line.split() for line in section(self.out, "Kind against verdict").splitlines()[2:]}
        self.assertEqual(rows["commentary"][2:], ["0", "3", "0", "0", "0", "0", "0", "0", "0", "3"])  # fit-disc, disc-b twice
        self.assertEqual(rows["lossy-companion"][-2:], ["0", "1"])
        self.assertIn("verdicts hidden until labelled", " ".join(rows["(unlabelled)"]))
        self.assertEqual(rows["(unlabelled)"][1:9], ["-"] * 8)

    def test_experiment_13_counts_each_track_once_and_never_the_fitting_disc(self):
        e = section(self.out, "Experiment 13 (")
        self.assertIn("commentaries          1 of 8, from 1 of 4 discs; on feature films 1 of 2; film clearly audible 0 of 1", e)
        self.assertIn("programme tracks      1 of 20 other than the reference; separately encoded 1 of 6", e)
        self.assertIn("commentary verdicts   on 1 of 3 discs", e)
        self.assertIn("false verdicts        0", e)
        self.assertIn("as it stands: not answered", e)
        self.assertIn("not counted: disc-b 2, fit-disc 1", e)  # the unlabelled duplicate and the re-ripped commentary

    def test_titles_missing_from_the_dataset(self):
        self.assertIn("Labels without a measurement:\n  none", self.out)
        self.assertIn("Folders that cannot be read:\n  none", self.out)
        self.assertIn("1 of 5 (every successful rip and every measured title); missing: disc-b/eng-only, "
                      "disc-b/extract (not measured), disc-c/t1, fit-disc/t0", self.out)  # not share-drop: it failed

    def test_a_second_run_takes_every_measure_from_the_snapshot(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            code, out = run([c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("measures: 0 analysed now, 4 taken from the cache", out)
        self.assertIn("Changes since the previous snapshot", out)
        self.assertIn(section(out, "Changes since the previous snapshot").splitlines()[1], "  none")

    def test_a_cached_measure_comes_back_with_its_nan(self):
        meas = ds.Measurer({})
        self.assertTrue(math.isnan(meas(MIX, COPY, CENTRE)["smi"]))
        again = ds.Measurer(json.loads(json.dumps(meas.cache)))
        self.assertTrue(math.isnan(again(MIX, COPY, CENTRE)["smi"]))
        self.assertEqual((again.computed, again.reused), (0, 1))

    def test_a_rule_trial_reuses_the_baselines_measures(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            (c / "audio-dataset.json").unlink()
            code, out = run([c, "--spec", self.spec, "--set", "DC_MAX=0.2", "--snapshot", pathlib.Path(tmp) / "t.json"])
        self.assertEqual(code, 0, out)
        self.assertIn("measures: 2 analysed now, 4 taken from the cache", out)  # the trial analyses nothing again

    def test_a_threshold_trial_lists_changes_and_leaves_the_baseline_alone(self):
        before = (self.corpus / "audio-dataset.json").read_text(encoding="utf-8")
        trial = self.root / "trial.json"
        code, out = run([self.corpus, "--spec", self.spec, "--set", "DC_MAX=0.0", "--snapshot", trial])
        self.assertEqual(code, 0, out)
        changes = section(out, "Changes against the baseline rule (DC_MAX=0.0)")
        self.assertIn("disc-b/all-tracks stream 3 in audio-levels-20ms-1-2-3-4 (commentary): commentary -> note-commentary", changes)
        self.assertIn("kept although unwanted, commentary", out)
        self.assertEqual((self.corpus / "audio-dataset.json").read_text(encoding="utf-8"), before)
        self.assertEqual(json.loads(trial.read_text(encoding="utf-8"))["overrides"]["rule"], {"dc_max": 0.0})

    def test_a_trial_never_shows_an_unlabelled_tracks_verdict(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            write_labels(c / "disc-b", [entry("all-tracks", 2, "lossy-companion")])
            code, out = run([c, "--spec", self.spec, "--set", "DC_MAX=0.0", "--snapshot", pathlib.Path(tmp) / "t.json"])
        self.assertEqual(code, 0, out)
        changes = section(out, "Changes against the baseline rule")
        self.assertIn("2 unlabelled track(s) changed verdict (not shown until labelled)", changes)  # fit-disc is labelled
        self.assertNotIn("note-commentary", changes.replace("fit-disc/t0 stream 3 in audio-levels-20ms-1-3 (commentary): commentary -> note-commentary", ""))

    def test_a_constant_trial_measures_again_and_bad_values_are_refused(self):
        code, out = run([self.corpus, "--spec", self.spec, "--set", "ADDED_DB=40", "--snapshot", self.root / "t2.json"])
        self.assertEqual(code, 0, out)
        self.assertRegex(out, r"measures: [1-9]\d* analysed now")
        self.assertIn("commentary -> keep", out)
        self.assertEqual(sim.ADDED_DB, 10.0)  # restored
        for bad in ("NOPE=1", "DC_MAX=nan", "SILENT_DB=inf", "DC_MAX=-inf", "TENTHS_MIN=1.5", "DC_MAX=x", "DC_MAX"):
            code, out = run([self.corpus, "--spec", self.spec, "--set", bad])
            self.assertEqual(code, 2, (bad, out))
        self.assertIn("not a finite number", run([self.corpus, "--spec", self.spec, "--set", "DC_MAX=nan"])[1])

    def test_a_programme_track_with_the_commentary_verdict_fails_loudly(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            write_labels(c / "disc-b", [entry("all-tracks", 3, "alternate-mix")])
            code, out = run([c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-b/all-tracks stream 3: labelled alternate-mix (programme audio), verdict commentary", out)
        self.assertIn("FAIL disc-b/eng-only stream 3: labelled alternate-mix (programme audio), verdict commentary (label of all-tracks stream 3)", out)
        self.assertIn("RESULT FAILED: 2 programme track(s) got a drop verdict", out)  # and its re-rip, by the same packets
        self.assertIn("false verdicts        2", out)
        self.assertIn("as it stands: no: 2 programme track(s)", out)

    def test_a_programme_track_with_the_duplicate_verdict_fails_loudly(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            write_labels(c / "disc-b", [entry("all-tracks", 4, "lossy-companion")])
            code, out = run([c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-b/all-tracks stream 4: labelled lossy-companion (programme audio), verdict duplicate", out)
        self.assertIn("false verdicts        0", out)

    def test_undecided_and_unknown_kinds_with_a_drop_verdict_are_notes(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            write_labels(c / "disc-b", [entry("all-tracks", 3, "same-language-dub"), entry("all-tracks", 4, "unknown"),
                                        entry("all-tracks", 2, "isolated-score"), entry("eng-only", 3, "isolated-score")])
            code, out = run([c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("note: disc-b/all-tracks stream 3: undecided kind, drop verdict (same-language-dub, verdict commentary)", out)
        self.assertIn("note: disc-b/all-tracks stream 4: unknown kind, drop verdict (unknown, verdict duplicate)", out)
        self.assertIn("kept although unwanted, isolated-score: disc-b/all-tracks stream 2 (keep)", out)
        self.assertIn("note: disc-b/eng-only stream 3: dropped as a commentary (isolated-score, verdict commentary)", out)

    def test_a_bad_labels_or_fitted_file_stops_the_run(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            c = self.copy(tmp)
            write_labels(c / "fit-disc", [entry("t0", 3, "comentary")])
            code, out = run([c, "--spec", self.spec])
            self.assertEqual(code, 2)
            self.assertIn("not in the vocabulary", out)
            write_labels(c / "fit-disc", [entry("t0", 3, "commentary")])
            (c / "audio-fitted.json").write_text('{"format": "lazymkv audio fitted v1", "fitted": [{"slug": "Disc"}]}', encoding="utf-8")
            code, out = run([c, "--spec", self.spec])
            self.assertEqual(code, 2)
            self.assertIn("audio-fitted.json", out)


class NotChecked(unittest.TestCase):
    """Runs that cannot check everything end with exit status 3 and never replace the baseline snapshot."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        root = pathlib.Path(cls.tmp.name)
        (root / "spec.md").write_text(SPEC, encoding="utf-8")
        cls.root, cls.corpus, cls.spec = root, root / "corpus", root / "spec.md"
        build_main_corpus(cls.corpus)
        assert run([cls.corpus, "--spec", cls.spec])[0] == 0

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def setUp(self):
        self.work = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        self.c = pathlib.Path(self.work.name) / "corpus"
        shutil.copytree(self.corpus, self.c)
        self.baseline = (self.c / "audio-dataset.json").read_text(encoding="utf-8")

    def tearDown(self):
        self.work.cleanup()

    def test_level_files_missing_as_after_a_clone(self):
        for f in self.c.glob("*/*/audio-levels-*/*.txt"):
            f.unlink()
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 3, out)
        self.assertIn("RESULT FAILED: not everything was checked: 4 folder(s) cannot be read, 3 labelled track(s) not measured, "
                      "9 track(s) of the previous snapshot are gone", out)
        self.assertIn("  gone: 9 track(s) not measured now: disc-b/all-tracks/audio-levels-20ms-1-2-3-4/candidate/2", out)
        self.assertIn("  disc-b/eng-only/audio-levels-20ms-1-3: its level files are not here (git keeps only streams.json)", out)
        self.assertIn("missing: disc-b/eng-only, disc-b/extract (not measured), disc-c/t1, fit-disc/t0", out)
        self.assertIn("was NOT replaced", out)
        self.assertEqual((self.c / "audio-dataset.json").read_text(encoding="utf-8"), self.baseline)
        code, out = run([self.c, "--spec", self.spec, "--allow-unmeasured"])
        self.assertEqual(code, 0, out)
        self.assertIn("RESULT ok (not checked, allowed by --allow-unmeasured: 4 folder(s) cannot be read", out)
        self.assertEqual((self.c / "audio-dataset.json").read_text(encoding="utf-8"), self.baseline)

    def test_no_level_folder_at_all(self):
        empty = pathlib.Path(self.work.name) / "empty"
        empty.mkdir()
        code, out = run([empty, "--spec", self.spec])
        self.assertEqual(code, 3, out)
        self.assertIn("no level folder under", out)

    def test_a_label_without_a_measurement(self):
        data = json.loads((self.c / "disc-b" / "audio-labels.json").read_text(encoding="utf-8"))
        write_labels(self.c / "disc-b", data["tracks"] + [entry("all-tracks", 7, "foreign-dub")], data["titles"])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 3, out)
        self.assertIn("Labels without a measurement:\n  disc-b/all-tracks stream 7: foreign-dub", out)
        self.assertIn("1 labelled track(s) not measured", out)
        code, out = run([self.c, "--spec", self.spec, "--allow-unmeasured"])
        self.assertEqual(code, 0, out)

    def test_a_folder_that_is_gone(self):
        shutil.rmtree(self.c / "disc-b" / "eng-only")
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 3, out)
        self.assertIn("gone: 2 track(s) not measured now: disc-b/eng-only/audio-levels-20ms-1-3/candidate/3, "
                      "disc-b/eng-only/audio-levels-20ms-1-3/reference/1", out)
        self.assertEqual((self.c / "audio-dataset.json").read_text(encoding="utf-8"), self.baseline)
        code, out = run([self.c, "--spec", self.spec, "--snapshot", self.c / "audio-dataset.json", "--allow-unmeasured"])
        self.assertEqual(code, 0, out)  # the removal accepted on purpose
        self.assertEqual(run([self.c, "--spec", self.spec])[0], 0)


class Joins(unittest.TestCase):
    """Labels carried over by identical packets, several folders of one title, and the reference check."""

    def setUp(self):
        self.work = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        root = pathlib.Path(self.work.name)
        (root / "spec.md").write_text(SPEC, encoding="utf-8")
        self.c, self.spec = root / "corpus", root / "spec.md"

    def tearDown(self):
        self.work.cleanup()

    def test_a_rerip_of_a_title_with_duplicates_takes_each_label_from_the_same_copy(self):
        for label in ("rehearsal", "rehearsal-512"):  # streams 1 and 3, 2 and 4 have the same packets
            folder(self.c / "disc-r" / label, "A", [(2, COPY, "B"), (3, MIX, "A"), (4, COPY, "B")])
        write_labels(self.c / "disc-r", [entry("rehearsal", 2, "derived-core", how="flag"), entry("rehearsal", 3, "duplicate", how="md5"),
                                         entry("rehearsal", 4, "duplicate", how="md5")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertNotIn("FAIL", out)
        for n in (2, 3, 4):
            self.assertIn(f"note: disc-r/rehearsal-512 stream {n} takes its label from rehearsal stream {n} (same packets)", out)
        self.assertNotIn("reference labelled", out)
        self.assertIn("programme tracks      1 of 20", out)  # the core counts once, not again from the re-rip

    def test_disagreeing_sources_leave_the_track_to_be_labelled(self):
        for label in ("all-tracks", "again", "third"):
            folder(self.c / "disc-r" / label, "A", [(2, COPY, "B")])
        write_labels(self.c / "disc-r", [entry("all-tracks", 2, "lossy-companion"), entry("again", 2, "alternate-mix")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("disc-r/third stream 2: ac3 2 ch eng (audio track 2 of 2 in a player, named \"t2\"); same packets as "
                      "again stream 2 (alternate-mix), all-tracks stream 2 (lossy-companion): they disagree", out)

    def test_a_reference_never_takes_a_label(self):
        folder(self.c / "disc-r" / "all-tracks", "A", [(2, COPY, "B")])
        folder(self.c / "disc-r" / "eng-only", "B", [])  # only stream B was ripped: it is the reference here
        write_labels(self.c / "disc-r", [entry("all-tracks", 2, "lossy-companion")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertNotIn("eng-only stream 1: reference labelled", out)

    def test_every_folder_of_a_title_is_judged(self):
        folder(self.c / "disc-w" / "t0", "w1", [(2, COPY, "w2"), (3, COMMENT, "w3")])
        folder(self.c / "disc-w" / "t0", "w1", [(3, COMMENT, "w3")])
        write_labels(self.c / "disc-w", [entry("t0", 3, "alternate-mix")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-w/t0 stream 3 (in audio-levels-20ms-1-2-3): labelled alternate-mix", out)
        self.assertIn("FAIL disc-w/t0 stream 3 (in audio-levels-20ms-1-3): labelled alternate-mix", out)
        self.assertIn("note: disc-w/t0 stream 3 (candidate) is measured in audio-levels-20ms-1-2-3, audio-levels-20ms-1-3; "
                      "the table uses audio-levels-20ms-1-2-3", out)
        rows = {line.split()[0]: line.split() for line in section(out, "Kind against verdict").splitlines()[2:]}
        self.assertEqual(rows["alternate-mix"][-1], "1")  # one track, judged twice

    def test_a_folder_measured_against_the_wrong_reference_fails_and_is_left_out(self):
        streams = [(1, 6, "dts", "DTS-HD MA", "eng"), (2, 6, "dts", "DTS", "eng"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-w" / "t0", "w1", [(2, COPY, "w2"), (3, COMMENT, "w3")], streams=streams)
        folder(self.c / "disc-w" / "t0", "w3", [(1, MIX, "w1"), (2, COPY, "w2")], main=3, streams=streams)
        write_labels(self.c / "disc-w", [entry("t0", 2, "derived-core", how="flag"), entry("t0", 3, "commentary")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-w/t0/audio-levels-20ms-3-1-2: wrong reference: measured against stream 3 (ac3 2 ch t3), but the "
                      "top-ranked eng stream is 1 (dts DTS-HD MA 6 ch t1)", out)
        self.assertIn("RESULT FAILED: 1 folder(s) measured against the wrong reference", out)
        rows = {line.split()[0]: line.split() for line in section(out, "Kind against verdict").splitlines()[2:]}
        self.assertEqual((rows["commentary"][-1], rows["derived-core"][-1]), ("1", "1"))
        self.assertIn("(1 measured against the wrong reference)", out)

    def test_a_reference_labelled_as_unwanted_audio_fails(self):
        folder(self.c / "disc-w" / "t0", "w1", [(2, COPY, "w2")])
        write_labels(self.c / "disc-w", [entry("t0", 1, "commentary"), entry("t0", 2, "lossy-companion")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-w/t0 stream 1: the reference is labelled commentary (unwanted audio)", out)

    def test_a_commentary_in_another_language_is_judged_against_the_original_language(self):
        streams = [(1, 6, "dts", "DTS-HD MA", "spa"), (2, 6, "dts", "DTS", "spa"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-s" / "t0", "s1", [(2, COPY, "s2"), (3, COMMENT, "s3")], streams=streams,
               langs={1: "spa", 2: "spa"})
        (self.c / "disc-s" / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
        write_labels(self.c / "disc-s", [entry("t0", 2, "derived-core", how="flag"), entry("t0", 3, "commentary")],
                     {"t0": title("feature")})
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("(0 measured against the wrong reference)", out)
        self.assertIn("cross-language        commentaries 1 (commentary-cross-language 1), programme tracks 0", out)
        rows = {line.split()[0]: line.split() for line in section(out, "Kind against verdict").splitlines()[2:]}
        self.assertEqual(rows["commentary"][2:5], ["0", "0", "1"])  # the pattern, in another language: keep-only
        self.assertIn("commentary verdicts   on 0 of 3 discs", out)  # only a verdict that may drop shows the recall
        self.assertIn("kept although unwanted, commentary: disc-s/t0 stream 3 (commentary-cross-language)", out)
        self.assertIn("swapped references    2 labelled candidates read the other way round; the programme would get "
                      "the commentary verdict against 0 of them", out)

    def test_a_folder_against_the_english_track_of_a_spanish_film_is_the_wrong_reference(self):
        streams = [(1, 6, "dts", "DTS-HD MA", "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-s" / "t0", "s2", [(3, COMMENT, "s3")], main=2, streams=streams)
        (self.c / "disc-s" / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("wrong reference: measured against stream 2 (ac3 2 ch t2), but the top-ranked spa stream is 1 "
                      "(dts DTS-HD MA 6 ch t1), the title's programme reference (placed first in the original language", out)

    def test_a_dub_is_programme_audio(self):
        folder(self.c / "disc-u" / "t0", "u1", [(2, COMMENT, "u2")], langs={1: "spa"},
               streams=[(1, 6, "dts", "DTS-HD MA", "spa"), (2, 2, "ac3", None, "eng")])
        (self.c / "disc-u" / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
        write_labels(self.c / "disc-u", [entry("t0", 2, "dub")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-u/t0 stream 2: labelled dub (programme audio), verdict commentary-cross-language", out)
        self.assertIn("cross-language        commentaries 0 (commentary-cross-language 0), programme tracks 1", out)
        self.assertIn("dubs                  1 from 1 discs; on dialogue-heavy titles 0; with the commentary pattern 1 "
                      "(row 13 sets no number for dubs yet", out)
        self.assertIn("false verdicts        1", out)  # kept, but the pattern fired on programme audio
        (self.c.parent / "dub-spec.md").write_text(DUB_SPEC, encoding="utf-8")
        code, out = run([self.c, "--spec", self.c.parent / "dub-spec.md"])
        self.assertIn("dubs                  1 of 3 from 1 of 2 discs; on dialogue-heavy titles 0 of 1; with the commentary "
                      "pattern 1 (what row 13 asks before content may judge a candidate in another language", out)

    def test_a_swapped_reference_that_would_drop_the_programme_fails(self):
        rnd = random.Random(5)  # a stem: music cues of 12 s at -30 dB in every 30 s, room tone between them
        stem = [(-30.0 if (t // FPS) % 30 < 12 else -62.0) + rnd.uniform(-1, 1) for t in range(len(MIX))]
        folder(self.c / "disc-m" / "t0", "m1", [(2, stem, "m2")])
        write_labels(self.c / "disc-m", [entry("t0", 2, "music-effects")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-m/t0 stream 2: swapped reference: labelled music-effects, verdict reference-unclear; with "
                      "this track as the reference, the programme (stream 1) would get the commentary verdict", out)
        self.assertIn("RESULT FAILED: 1 swapped reference(s) would drop the programme", out)
        write_labels(self.c / "disc-m", [])  # unlabelled: neither the verdict nor the swapped one is shown
        code, out = run([self.c, "--spec", self.spec, "--snapshot", self.c / "t.json"])
        self.assertEqual(code, 0, out)
        self.assertNotIn("swapped reference:", out)

    def test_the_companion_of_a_dub_is_judged_against_the_dub_too(self):
        companion = [-100.0] * 3 + [v - 6 for v in COMMENT[:-3]]  # the dub stand-in has the pattern (addendum 9.2)
        streams = [(1, 6, "dts", "DTS-HD MA", "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-x" / "t0", "x1", [(2, COMMENT, "x2"), (3, companion, "x3")], streams=streams, langs={1: "spa"})
        (self.c / "disc-x" / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
        write_labels(self.c / "disc-x", [entry("t0", 2, "dub"), entry("t0", 3, "lossy-companion")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-x/t0 stream 2: labelled dub (programme audio), verdict commentary-cross-language", out)
        self.assertNotIn("FAIL disc-x/t0 stream 3", out)
        rows = {t["stream"]: t for t in json.loads((self.c / "audio-dataset.json").read_text(encoding="utf-8"))["tracks"]}
        self.assertEqual((rows[3]["verdict"], rows[3]["peer"]["stream"], rows[3]["peer"]["verdict"]), ("note-commentary", 2, "keep"))
        self.assertIsNone(rows[2]["peer"])

    def test_the_snapshot_hides_every_verdict_of_an_unlabelled_track(self):
        companion = [-100.0] * 3 + [v - 6 for v in COMMENT[:-3]]
        streams = [(1, 6, "dts", "DTS-HD MA", "spa"), (2, 2, "ac3", None, "eng"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-x" / "t0", "x1", [(2, COMMENT, "x2"), (3, companion, "x3")], streams=streams, langs={1: "spa"})
        (self.c / "disc-x" / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        snap = json.loads((self.c / "audio-dataset.json").read_text(encoding="utf-8"))
        cands = [t for t in snap["tracks"] if t["role"] == "candidate"]
        self.assertEqual([(t["verdict"], t["swapped_verdict"]) for t in cands], [(None, None)] * 2)
        self.assertEqual((cands[1]["peer"]["stream"], cands[1]["peer"]["verdict"]), (2, None))
        self.assertEqual(sorted(snap["unlabelled_verdicts"].values()), ["commentary-cross-language", "note-commentary"])

    def test_a_folder_for_a_language_that_is_not_wanted_is_dataset_only(self):
        streams = [(1, 8, "truehd", None, "eng"), (2, 6, "ac3", None, "fra"), (3, 2, "ac3", None, "fra")]
        folder(self.c / "disc-f" / "t0", "f1", [], streams=streams, channels=8)
        folder(self.c / "disc-f" / "t0", "f2", [(3, COPY, "f3")], main=2, streams=streams, langs={2: "fra", 3: "fra"})
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("(0 measured against the wrong reference)", out)

    def test_a_reference_labelled_unwanted_is_its_own_failure_not_a_swapped_one(self):
        d = folder(self.c / "disc-v" / "t0", "v1", [(2, MIX, "v2")], fc=False)
        level_file(d / "main-mix.txt", COMMENT)  # a commentary ranked first: the programme is the candidate
        write_labels(self.c / "disc-v", [entry("t0", 1, "commentary"), entry("t0", 2, "lossy-companion")])
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL disc-v/t0 stream 1: the reference is labelled commentary (unwanted audio)", out)
        self.assertNotIn("swapped reference:", out)
        rows = {t["stream"]: t for t in json.loads((self.c / "audio-dataset.json").read_text(encoding="utf-8"))["tracks"]
                if t["role"] == "candidate"}
        self.assertEqual((rows[2]["verdict"], rows[2]["swapped_verdict"]), ("reference-unclear", "commentary"))

    def test_a_reference_that_is_not_the_first_stream_of_its_language_is_noted(self):
        streams = [(1, 6, "dts", "DTS", "eng"), (2, 6, "dts", "DTS-HD MA", "eng"), (3, 2, "ac3", None, "eng")]
        folder(self.c / "disc-q" / "t0", "q2", [(1, COPY, "q1"), (3, COMMENT, "q3")], main=2, streams=streams)
        code, out = run([self.c, "--spec", self.spec])
        self.assertEqual(code, 0, out)
        self.assertIn("note: disc-q/t0/audio-levels-20ms-2-1-3: the reference, stream 2, is not the first eng stream of the "
                      "file (stream 1, dts DTS 6 ch t1, comes first): question 10's position veto would refuse", out)


def build_count_corpus(c):
    """Every counting rule of experiment 13 decides one of the numbers this corpus prints."""
    folder(c / "disc-d" / "t0", "d1", [(2, COMMENT, "d2"), (3, COMMENT, "d3"), (4, COPY, "d4"), (5, COPY, "d5"), (6, COPY, "d6")])
    folder(c / "disc-d" / "t1", "d1", [(2, COMMENT, "d2")])  # the same commentary again, labelled again: counts once
    write_labels(c / "disc-d", [entry("t0", 2, "commentary", film_audible=True), entry("t0", 3, "commentary", film_audible=None),
                                entry("t0", 4, "lossy-companion"), entry("t0", 5, "derived-core", how="flag"),
                                entry("t0", 6, "main"), entry("t1", 2, "commentary")],
                 {"t0": title("feature", True), "t1": title("feature", True)})
    folder(c / "disc-e" / "t0", "e1", [(2, COMMENT, "e2"), (3, COMMENT, "e3"), (4, COMMENT, "e4"), (5, COMMENT, "e5")], fc=False, channels=2)
    write_labels(c / "disc-e", [entry("t0", 2, "commentary", film_audible=False), entry("t0", 3, "commentary", saw=True),
                                entry("t0", 4, "commentary", sure=False), entry("t0", 5, "commentary", how="inferred")],
                 {"t0": title("episode", False)})
    folder(c / "disc-g" / "t0", "g1", [(2, COMMENT, "g2")])
    write_labels(c / "disc-g", [entry("t0", 2, "commentary")], {"t0": title("feature", True)})
    folder(c / "disc-h" / "t0", "h1", [(2, COMMENT, "h2"), (3, COMMENT, "h3")], fc=False, channels=2)
    folder(c / "disc-h" / "t0b", "h1", [(2, COMMENT, "h2")], fc=False, channels=2)  # the fitted track again, labelled again
    write_labels(c / "disc-h", [entry("t0", 2, "commentary"), entry("t0", 3, "commentary"), entry("t0b", 2, "commentary")],
                 {"t0": title("episode", True)})
    folder(c / "disc-k" / "t0", "k1", [(2, COPY, "k2")])  # a commentary the rule misses: its disc has no commentary verdict
    write_labels(c / "disc-k", [entry("t0", 2, "commentary")], {"t0": title("episode", False)})
    labels.add_fitted(c, "disc-g", None, None, "test", "", "2026-10-05")
    labels.add_fitted(c, "disc-h", "t0", 2, "test", "", "2026-10-05")


class Counts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        root = pathlib.Path(cls.tmp.name)
        cls.root, cls.corpus = root, root / "corpus"
        build_count_corpus(cls.corpus)
        for name, text in (("spec.md", SPEC), ("short.md", spec(comm=4, discs=3, prog=2, sep=1, centre=2, heavy=2, verdict_discs=4)),
                           ("met.md", spec(comm=4, discs=3, prog=2, sep=1, centre=2, heavy=2, verdict_discs=3))):
            (root / name).write_text(text, encoding="utf-8")
        cls.code, cls.out = run([cls.corpus, "--spec", root / "spec.md"])  # its snapshot is the cache of the runs below

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_each_rule_decides_its_number(self):
        self.assertEqual(self.code, 0, self.out)
        e = section(self.out, "Experiment 13 (")
        self.assertIn("commentaries          5 of 8, from 4 of 4 discs; on feature films 2 of 2; film clearly audible 1 of 1", e)
        self.assertIn("programme tracks      2 of 20 other than the reference; separately encoded 1 of 6; derived cores 1", e)
        self.assertIn("no centre channel     2 of 1 titles", e)
        self.assertIn("dialogue-heavy        2 of 1 titles", e)
        self.assertIn("commentary verdicts   on 3 of 3 discs", e)
        self.assertIn("not counted: disc-d 2, disc-e 3, disc-g 1, disc-h 2", e)

    def test_the_answer_follows_the_rules_of_row_13(self):
        self.assertIn("as it stands: not answered: too few commentaries, programme tracks, separately encoded programme tracks", self.out)
        code, out = run([self.corpus, "--spec", self.root / "short.md"])
        self.assertIn("as it stands: no: commentary verdicts on 3 discs, 4 needed", out)
        code, out = run([self.corpus, "--spec", self.root / "met.md"])
        self.assertIn("as it stands: yes, if the audio-description", out)


class Similarity(unittest.TestCase):
    def test_results_are_the_report_as_data_and_a_reference_only_folder_is_valid(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = folder(pathlib.Path(tmp) / "t", "m", [(3, COMMENT, "c"), (4, MIX, "m"), (5, COMMENT[:600], "s")])
            res = sim.results(d)
            self.assertEqual([(r["stream"], r["verdict"], r["duplicate_of"], r["not_judged"]) for r in res["candidates"]],
                             [("3", "commentary", None, ""), ("4", "duplicate", "1", ""), ("5", "not-judged", None, "too short")])
            self.assertEqual(res["dialogue"], "fc")
            self.assertIn("verdict: commentary", "\n".join(sim.report(d)))
            self.assertEqual(sim.results(d, dict(sim.RULE, dc_max=0.0))["candidates"][0]["verdict"], "note-commentary")
            r = folder(pathlib.Path(tmp) / "r", "m", [], fc=False)
            self.assertEqual(sim.refusal(r), "")
            self.assertEqual(sim.results(r)["candidates"], [])
            self.assertIn("no candidate stream: nothing to judge", "\n".join(sim.report(r)))
        for name in sim.CONSTANTS:
            self.assertTrue(hasattr(sim, name), name)


def probe_file(path, streams):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"streams": [{"index": 0, "codec_type": "video"}] + [
        {"index": k + 1, "codec_type": "audio", "codec_name": c, "profile": p, "channels": ch, "tags": {"language": lang}}
        for k, (c, p, ch, lang) in enumerate(streams)]}), encoding="utf-8")


class Routine(unittest.TestCase):
    def jobs(self, slug, label):
        return [line for line in ds.levels_args(slug, label) if line.startswith("scripts/phase0/job.sh")]

    def test_args_rank_by_channels_then_lossless_then_order_and_prefer_english(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "some-film"
            for label, streams in (
                    ("all-tracks", [("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 6, "fra"), ("dts", "DTS", 6, "eng")]),
                    ("one-each", [("ac3", None, 6, "fra"), ("ac3", None, 2, "eng")]),
                    ("no-english", [("ac3", None, 2, "fra"), ("ac3", None, 6, "deu")]),
                    ("lossless-second", [("dts", "DTS", 6, "eng"), ("dts", "DTS-HD MA", 6, "eng")]),
                    ("dtsx", [("dts", "DTS", 8, "eng"), ("dts", "DTS-HD MA + DTS:X", 8, "eng")]),
                    ("two-languages", [("ac3", None, 6, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")])):
                probe_file(slug / label / "ffprobe-streams.json", streams)
            self.assertEqual(self.jobs(slug, "all-tracks"), ["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-all-tracks "
                                                             "-Slug some-film -Label all-tracks -Main 1 -Cand 2,4"])
            self.assertEqual(self.jobs(slug, "one-each"), ["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-one-each "
                                                           "-Slug some-film -Label one-each -Main 2"])
            self.assertTrue(self.jobs(slug, "no-english")[0].endswith("-Main 2"))
            self.assertTrue(self.jobs(slug, "lossless-second")[0].endswith("-Main 2 -Cand 1"))
            self.assertTrue(self.jobs(slug, "dtsx")[0].endswith("-Main 2 -Cand 1"))
            self.assertEqual([j.split()[2] + j.split(" -Main ")[1] for j in self.jobs(slug, "two-languages")],
                             ["audio-levels-some-film-two-languages-11 -Cand 2", "audio-levels-some-film-two-languages-33 -Cand 4"])

    def test_args_take_the_programme_reference_from_the_original_language(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "some-film"
            for label, streams in (
                    ("commentary", [("dts", "DTS-HD MA", 6, "spa"), ("dts", "DTS", 6, "spa"), ("ac3", None, 2, "eng")]),
                    ("dub-first", [("dts", "DTS-HD MA", 6, "spa"), ("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"),
                                   ("ac3", None, 6, "fra"), ("ac3", None, 2, "fra")]),
                    ("no-spanish", [("truehd", None, 8, "eng"), ("ac3", None, 2, "eng"), ("ac3", None, 2, "und")])):
                probe_file(slug / label / "ffprobe-streams.json", streams)
            (slug / "tmdb.json").write_text('{"original_language": "es"}', encoding="utf-8")
            lines = ds.levels_args(slug, "commentary")
            self.assertIn("original language spa (TMDb es)", lines[0])
            self.assertIn("programme reference: stream 1 (dts DTS-HD MA 6 ch), placed first in the original language", lines[4])
            self.assertEqual(self.jobs(slug, "commentary"), ["scripts/phase0/job.sh audio-levels.ps1 audio-levels-some-film-"
                                                             "commentary -Slug some-film -Label commentary -Main 1 -Cand 2,3"])
            self.assertEqual([j.split(" -Main ")[1] for j in self.jobs(slug, "dub-first")], ["1 -Cand 2,3", "4 -Cand 5"])
            self.assertIn("  dataset only: fra is not a wanted language; stream 4 against 5", ds.levels_args(slug, "dub-first"))
            self.assertTrue(self.jobs(slug, "no-spanish")[0].endswith("-Main 1 -Cand 2,3"))
            self.assertIn("placed first in English", "\n".join(ds.levels_args(slug, "no-spanish")))
            self.assertIn("note: the file has no stream in the original language (spa)", "\n".join(ds.levels_args(slug, "no-spanish")))
            self.assertNotIn("note:", "\n".join(ds.levels_args(slug, "commentary")))
            (slug / "tmdb.json").write_text('{"original_language": "xx"}', encoding="utf-8")
            self.assertIn("original language none (TMDb xx: a work without speech", ds.levels_args(slug, "commentary")[0])
            self.assertTrue(self.jobs(slug, "commentary")[0].endswith("-Main 3"))  # English then: the stereo track alone
            (slug / "tmdb.json").write_text('{"original_language": "qq"}', encoding="utf-8")
            self.assertIn("original language unknown (TMDb 'qq'", ds.levels_args(slug, "commentary")[0])

    def test_args_keep_a_lossy_stereo_track_from_becoming_the_reference(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "old-film"
            for label, streams in (
                    ("mono-lossless", [("pcm_s24le", None, 1, "eng"), ("ac3", None, 2, "eng")]),   # spec 8.6 G1
                    ("mono-lossy", [("ac3", None, 1, "eng"), ("ac3", None, 2, "eng")]),            # G2
                    ("stereo-later", [("ac3", None, 2, "eng"), ("ac3", None, 1, "eng"), ("ac3", None, 2, "eng")])):
                probe_file(slug / label / "ffprobe-streams.json", streams)
            self.assertTrue(self.jobs(slug, "mono-lossless")[0].endswith("-Main 1 -Cand 2"))
            self.assertTrue(self.jobs(slug, "mono-lossy")[0].endswith("-Main 1 -Cand 2"))
            self.assertTrue(self.jobs(slug, "stereo-later")[0].endswith("-Main 1 -Cand 2,3"))

    def test_args_g1_alone_and_the_notes_on_a_reference_that_may_not_be_the_programme(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "old-film"
            for label, streams in (
                    ("g1-alone", [("ac3", None, 2, "eng"), ("pcm_s24le", None, 1, "eng")]),  # G2 would keep stream 1
                    ("score-first", [("pcm_s24le", None, 1, "eng"), ("dts", "DTS-HD MA", 2, "eng"), ("ac3", None, 2, "eng")]),
                    ("und-main", [("dts", "DTS-HD MA", 6, "und"), ("ac3", None, 2, "eng")]),
                    ("malayalam", [("dts", "DTS-HD MA", 6, "mal"), ("ac3", None, 2, "eng")])):
                probe_file(slug / label / "ffprobe-streams.json", streams)
            self.assertTrue(self.jobs(slug, "g1-alone")[0].endswith("-Main 2 -Cand 1"))
            self.assertNotIn("note:", "\n".join(ds.levels_args(slug, "g1-alone")))  # G1 passed over a lossy stereo track
            score = "\n".join(ds.levels_args(slug, "score-first"))
            self.assertTrue(self.jobs(slug, "score-first")[0].endswith("-Main 2 -Cand 1,3"))
            self.assertIn("note: the reference is not the first eng stream of the file (stream 1, pcm_s24le 1 ch, comes first)", score)
            und = "\n".join(ds.levels_args(slug, "und-main"))
            self.assertTrue(self.jobs(slug, "und-main")[0].endswith("-Main 2 -Cand 1"))
            self.assertIn("note: the reference is a lossy stream of 2 channel(s), and stream 1 with no language code is "
                          "lossless or has more than 2 channels", und)
            (slug / "tmdb.json").write_text('{"original_language": "en"}', encoding="utf-8")
            self.assertIn("original language English (TMDb en)", ds.levels_args(slug, "und-main")[0])
            (slug / "tmdb.json").write_text('{"original_language": "ml"}', encoding="utf-8")
            self.assertIn("original language mal (TMDb ml)", ds.levels_args(slug, "malayalam")[0])
            self.assertTrue(self.jobs(slug, "malayalam")[0].endswith("-Main 1 -Cand 2"))

    def test_args_without_a_probe_take_a_rip_of_the_same_title_and_size_or_say_how_to_probe(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "some-film"
            probe_file(slug / "all-tracks" / "ffprobe-streams.json", [("truehd", None, 8, "eng"), ("ac3", None, 2, "eng")])
            for label, title, size in (("all-tracks", 3, 900), ("all-tracks-512", 3, 900), ("share-drop", 5, 300)):
                (slug / label).mkdir(parents=True, exist_ok=True)
                (slug / label / "rip-summary.json").write_text(json.dumps({"ok": True, "title": title, "bytes": size}), encoding="utf-8-sig")
            lines = ds.levels_args(slug, "all-tracks-512")
            self.assertIn("(from all-tracks/ffprobe-streams.json: a rip of the same title 3 with the same size, 900 bytes)", lines[0])
            self.assertTrue(lines[-2].endswith("-Label all-tracks-512 -Main 1 -Cand 2"))
            with self.assertRaises(ValueError) as stop:
                ds.levels_args(slug, "share-drop")
            self.assertIn("scripts/phase0/job.sh probe-title.ps1 probe-share-drop -Slug some-film -Label share-drop -SkipRemux", str(stop.exception))
            (slug / "extract").mkdir()
            (slug / "extract" / "mkvmerge.json").write_text(json.dumps({"tracks": [
                {"id": 0, "type": "video"}, {"id": 1, "type": "audio", "codec": "DTS-HD Master Audio", "properties": {"audio_channels": 6, "language": "eng"}},
                {"id": 2, "type": "audio", "codec": "DTS", "properties": {"audio_channels": 6, "language": "eng"}}]}), encoding="utf-8")
            self.assertTrue(self.jobs(slug, "extract")[0].endswith("-Main 1 -Cand 2"))

    def test_auto_labels_duplicates_and_derived_cores_and_keeps_existing_labels(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            slug = pathlib.Path(tmp) / "disc-d"
            d = folder(slug / "t0", "aa", [(2, COPY, "bb"), (3, MIX, "aa"), (4, COMMENT, "cc"), (5, COPY, "bb"), (6, COPY, "dd"), (7, MIX, "aa")])
            (slug / "t0" / "rip-summary.json").write_text('{"ok": true, "title": 0}', encoding="utf-8")
            folder(slug / "t1", "ee", [(2, COPY, "ff")])
            (slug / "t1" / "rip-summary.json").write_text('{"ok": true, "title": 1}', encoding="utf-8")
            flags = {1: 0, 2: 0, 3: 0, 4: 0, 5: 0, 6: 2304, 7: 0}
            (slug / "scan.txt").write_text("\n".join(
                ['TINFO:0,9,0,"0:07:00"', 'SINFO:0,0,1,0,"Video"'] + [f'SINFO:0,{s},1,0,"Audio"' for s in flags]
                + [f'SINFO:0,{s},22,0,"{f}"' for s, f in flags.items()]
                + ['SINFO:1,0,1,0,"Video"'] + [f'SINFO:1,{s},1,0,"Audio"' for s in (1, 2, 3)]) + "\n", encoding="utf-8")
            write_labels(slug, [entry("t0", 3, "duplicate", note="the owner's")])
            lines = labels.auto(slug, "2026-10-05", "fit-disc")
            data = json.loads((slug / "audio-labels.json").read_text(encoding="utf-8"))
        self.assertEqual(lines, ["labelled t0 stream 5: duplicate (md5), counts=False (a duplicate track never counts)",
                                 "labelled t0 stream 6: derived-core (flag), counts=True",
                                 "labelled t0 stream 7: duplicate (md5), counts=False (a duplicate track never counts)",
                                 "t1: derived cores not labelled: the file has 2 audio streams and the scan's title 1 has 3"])
        got = {(t["stream"], t["kind"], t["how"]) for t in data["tracks"]}
        self.assertEqual(got, {(3, "duplicate", "listened"), (5, "duplicate", "md5"), (6, "derived-core", "flag"), (7, "duplicate", "md5")})


if __name__ == "__main__":
    unittest.main()
