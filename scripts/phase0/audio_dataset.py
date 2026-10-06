"""The tuning dataset of the audio rule (decision D16): every stored measurement, the owner's labels, one command.

    audio_dataset.py [corpus/owner] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--spec FILE]
                     [--allow-unmeasured]
    audio_dataset.py args corpus/owner/<slug> <label>       the audio-levels.ps1 jobs that add a ripped title

The first form runs the current rule (audio_similarity.py, imported, never copied) over every
corpus/owner/<slug>/<label>/audio-levels-20ms-* folder, joins each candidate track with the owner's label in
corpus/owner/<slug>/audio-labels.json (see audio_labels.py), and prints:
  1. the tracks to label: measured, with no label yet. No verdict is shown: a label counts towards experiment 13 only
     when the owner gives it before seeing the verdict;
  2. kind against verdict, over the labelled tracks;
  3. experiment 13: the tracks that count, against the totals of spec section 17 row 13 (read from the spec), and
     the answer as it stands. Whether a label counts is worked out here (audio_labels.not_counted_because), never
     stored, and no track of a disc or track the thresholds were fitted on counts (row 13, audio-fitted.json);
  4. disagreements: FAIL for a programme track with a drop verdict (commentary or duplicate), for a level folder
     measured against another stream than the top-ranked one of its language, and for a reference the owner labelled
     as unwanted audio; the unwanted tracks that were kept, by kind; notes;
  5. labels without a measurement, folders that cannot be read, and the titles not yet in the dataset: every
     successful rip (rip-summary.json or extract-summary.json saying ok) and every measured title without an
     audio-proxy.json in one of its level folders (audio-levels.ps1 writes that file there last, once the title is
     complete on the share); this is the list of titles the routine still has to add;
  6. with --set or --compare, and against the previous snapshot: every labelled track whose verdict changed (an
     unlabelled one only as a count), and the tracks that are gone or new;
  7. RESULT ok, or RESULT FAILED: <reasons>.

A track measured in several level folders of one title is judged in each; a FAIL in any of them counts, and the table
and the counts use the folder with the most candidates. A track with no label of its own takes the label of the track
of another rip of the same disc with the same packets and the same place among the title's copies of those packets
(the first copy, the second, ...); when such tracks disagree, it is listed to be labelled.

It writes a JSON snapshot of the run: every track's measures, verdict and label (an unlabelled track's verdict is kept
apart, under "unlabelled_verdicts", not to be shown to the owner), the rule and constants used and the SHA-1 of
audio_similarity.py. Without --set it replaces <corpus>/audio-dataset.json (committed), which doubles as a cache:
measures whose inputs, constants and code are unchanged are taken from it. It is not replaced when a folder cannot be
read, when a track of it is no longer measured, or when no level folder is found: the snapshot then goes to the
temporary folder, and --snapshot <corpus>/audio-dataset.json replaces the baseline on purpose. With --set it goes to
the temporary folder unless --snapshot names a file, so a trial never replaces the baseline.

--set NAME=VALUE (repeatable) tries another value without editing code: a key of audio_similarity.RULE, such as
DC_MAX=0.25 (only the verdicts change), or one of audio_similarity.CONSTANTS, such as SILENT_DB=-80 (the measures are
computed again). The value must be a finite number. The report is then that of the changed rule, and ends with its
differences from the baseline.

Exit status: 0; 1 when the rule reported drops a programme track or a folder is measured against the wrong
reference; 2 for usage errors, a labels or fitted file that does not validate, or a spec row 13 that cannot be read;
3 when something could not be checked (no level folder, a folder that cannot be read, a labelled track not measured,
a track of the previous snapshot gone), unless --allow-unmeasured is given.
"""
import argparse
import array
import contextlib
import datetime
import hashlib
import json
import math
import pathlib
import sys
import tempfile

import audio_labels as labels
import audio_similarity as sim

SNAPSHOT_FORMAT = "lazymkv audio dataset snapshot v2"
VERDICTS = ("duplicate", "commentary", "reference-unclear", "ad-pattern", "note-commentary", "note-ad", "keep", "not-judged")
SHORT = ("dup", "comm", "r-unc", "ad-p", "n-com", "n-ad", "keep", "n-j")
DROP = ("duplicate", "commentary")  # the verdicts that drop a track: by MD5 (D13), by content once experiment 13 says yes
CODE = hashlib.sha1(pathlib.Path(sim.__file__).read_bytes()).hexdigest()
SPEC = labels.SPEC


def nan_to_none(m):
    return None if m is None else {k: (None if isinstance(v, float) and math.isnan(v) else v) for k, v in m.items()}


def none_to_nan(m):
    return {k: (float("nan") if v is None else v) for k, v in m.items()}


def parse_sets(items):
    """--set NAME=VALUE items as ({rule key: value}, {constant: value})."""
    rule, consts = {}, {}
    for item in items:
        name, sep, text = item.partition("=")
        if not sep:
            raise ValueError(f"--set {item}: write NAME=VALUE")
        try:
            value = float(text)
        except ValueError:
            raise ValueError(f"--set {item}: {text!r} is not a number")
        if not math.isfinite(value):
            raise ValueError(f"--set {item}: not a finite number")
        if name.lower() in sim.RULE:
            target, key, old = rule, name.lower(), sim.RULE[name.lower()]
        elif name.upper() in sim.CONSTANTS:
            target, key, old = consts, name.upper(), getattr(sim, name.upper())
        else:
            raise ValueError(f"--set {name}: neither a rule threshold ({', '.join(k.upper() for k in sim.RULE)}) "
                             f"nor a measure constant ({', '.join(sim.CONSTANTS)})")
        if isinstance(old, int):
            if value != int(value):
                raise ValueError(f"--set {item}: {key} takes a whole number")
            value = int(value)
        target[key] = value
    return rule, consts


@contextlib.contextmanager
def constants(values):
    old = {k: getattr(sim, k) for k in values}
    try:
        for k, v in values.items():
            setattr(sim, k, v)
        yield
    finally:
        for k, v in old.items():
            setattr(sim, k, v)


class Measurer:
    """analyse() with a cache keyed by the code, the measure constants in force and the three level series."""

    def __init__(self, cache):
        self.cache, self.used, self.computed, self.reused = cache, {}, 0, 0
        self.prefix, self.key_of = None, {}

    def __call__(self, main, cand, dialogue):
        if self.prefix is None:  # the constants in force when the first track is measured
            self.prefix = CODE + json.dumps({k: getattr(sim, k) for k in sim.CONSTANTS}, sort_keys=True)
        h = hashlib.sha1(self.prefix.encode())
        for series in (main, cand, dialogue):
            h.update(b"|" if series is None else array.array("d", series).tobytes() + b"|")
        key = h.hexdigest()
        if key in self.cache:
            m = none_to_nan(self.cache[key])
            self.reused += 1
        else:
            m = sim.analyse(main, cand, dialogue)
            self.computed += 1
            self.cache[key] = nan_to_none(m)  # the same inputs again in this run (a re-ripped title) are not analysed twice
        self.used[key] = nan_to_none(m)
        self.key_of[id(m)] = key
        return m


def is_lossless(codec, profile):
    """Lossless audio as spec 8.6 ranks it. ffprobe names DTS:X tracks 'DTS-HD MA + DTS:X' (and '... IMAX')."""
    c = str(codec or "")
    return c in ("truehd", "mlp", "flac") or c.startswith("pcm") or "DTS-HD MA" in str(profile or "")


def ffprobe_audio(streams):
    """[(stream, channels, lossless, language, description)] for the audio streams of an ffprobe stream list."""
    out = []
    for s in streams:
        if s.get("codec_type") == "audio":
            tags = s.get("tags") or {}
            c = str(s.get("codec_name", ""))
            what = [c, s.get("profile"), f"{s.get('channels')} ch", s.get("channel_layout"), tags.get("title")]
            out.append((int(s["index"]), int(s.get("channels") or 0), is_lossless(c, s.get("profile")),
                        tags.get("language") or "und", " ".join(str(w) for w in what if w)))
    return out


def rank(a):
    """Task 12 Step 15's and spec 8.6's order: most channels, then lossless, then the earlier stream."""
    return (-a[1], not a[2], a[0])


def find_folders(corpus):
    """[(slug, label, folder)] for every audio-levels-20ms-* folder, in name order."""
    out = []
    for slug_dir in sorted(p for p in pathlib.Path(corpus).iterdir() if p.is_dir() and labels.SLUG.match(p.name)):
        for label_dir in sorted(p for p in slug_dir.iterdir() if p.is_dir()):
            out += [(slug_dir.name, label_dir.name, f) for f in sorted(label_dir.glob("audio-levels-20ms-*")) if f.is_dir()]
    return out


def read_streams(folder):
    """The folder's streams.json stream list; None when there is none."""
    path = folder / "streams.json"
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8-sig")).get("streams", [])


def audio_positions(streams):
    """{stream: (k, n, title)}: the stream is the k-th of the file's n audio streams (a player's audio track k)."""
    audio = [s for s in streams or [] if s.get("codec_type") == "audio"]
    return {s["index"]: (k + 1, len(audio), (s.get("tags") or {}).get("title", "")) for k, s in enumerate(audio)}


def reference_problem(streams, main):
    """None when there is no streams.json to check against; '' when `main` is the top-ranked audio stream of its
    language; otherwise what is wrong."""
    if streams is None:
        return None
    audio = ffprobe_audio(streams)
    mine = [a for a in audio if a[0] == main]
    if not mine:
        return f"stream {main} is not an audio stream of the file (streams.json)"
    top = sorted((a for a in audio if a[3] == mine[0][3]), key=rank)[0]
    if top[0] != main:
        return (f"measured against stream {main} ({mine[0][4]}), but the top-ranked {mine[0][3]} stream is {top[0]} "
                f"({top[4]})")
    return ""


def measure(found, rule, consts, cache):
    """Every reference and candidate of every folder under one rule: (tracks, unreadable, measurer). A track is one
    stream in one folder; the same stream in several folders of a title gives several tracks."""
    meas = Measurer(cache)
    tracks, unreadable = [], []
    with constants(consts):
        for slug, label, folder in found:
            if not (folder / "manifest.txt").is_file() and (folder / "streams.json").is_file():
                unreadable.append(f"{slug}/{label}/{folder.name}: its level files are not here (git keeps only "
                                  f"streams.json): pull the recording or copy the folder from the dataset share")
                continue
            try:
                res = sim.results(folder, rule, meas)
                streams = read_streams(folder)
            except (ValueError, OSError) as e:
                unreadable.append(f"{slug}/{label}/{folder.name}: {e}")
                continue
            pos = audio_positions(streams)
            mi = res["main"]
            main = int(mi["main"])
            problem = reference_problem(streams, main)
            base = {"slug": slug, "label": label, "folder": folder.name, "main": main, "ref_fc": mi.get("fc") == "yes",
                    "ref_problem": problem, "excluded": bool(problem), "secondary": False,
                    "n_cands": len(res["candidates"])}
            order = [(main, mi.get("md5"))] + [(int(c["stream"]), c["info"].get("md5")) for c in res["candidates"]]
            ranks = {n: sum(1 for m, h in order[:k] if h and h == md5) for k, (n, md5) in enumerate(order)}
            tracks.append(dict(base, role="reference", stream=main, md5=mi.get("md5"), md5_rank=0, codec=mi.get("codec"),
                               channels=mi.get("channels"), language=mi.get("language"), position=pos.get(main),
                               verdict=None, measures=None, key=None, not_judged="", duplicate_of=None))
            for c in res["candidates"]:
                n = int(c["stream"])
                key = meas.key_of.get(id(c["measures"])) if c["measures"] is not None else None
                tracks.append(dict(base, role="candidate", stream=n, md5=c["info"].get("md5"), md5_rank=ranks[n],
                                   codec=c["info"].get("codec"), channels=c["info"].get("channels"),
                                   language=c["info"].get("language"), position=pos.get(n), verdict=c["verdict"],
                                   measures=nan_to_none(c["measures"]), key=key, not_judged=c["not_judged"],
                                   duplicate_of=c["duplicate_of"]))
    # The same stream in several valid folders of one title: each is judged; the table uses the folder with the most
    # candidates (it sees the most of the title's duplicates), the others are marked secondary.
    groups = {}
    for t in tracks:
        if not t["excluded"]:
            groups.setdefault((t["slug"], t["label"], t["role"], t["stream"]), []).append(t)
    for same in groups.values():
        primary = max(same, key=lambda t: t["n_cands"])  # max() keeps the first of equals: folder name order
        for t in same:
            t["secondary"] = t is not primary
            t["also_in"] = [o["folder"] for o in same if o is not t]
    return tracks, unreadable, meas


def join_labels(tracks, labelled, fitted):
    """Gives every track its label (its own, or inherited from a track of another rip of the same disc with the same
    packets and the same place among the copies of those packets), what the owner wants, whether it counts for
    experiment 13, and its outcome."""
    direct = {(slug, t["label"], t["stream"]): t for slug, data in labelled.items() for t in data["tracks"]}
    sources = {}
    for t in tracks:
        lab = direct.get((t["slug"], t["label"], t["stream"]))
        if lab and t["role"] == "candidate" and not t["excluded"] and t["md5"]:
            sources.setdefault((t["slug"], t["md5"], t["md5_rank"]), {})[(t["label"], t["stream"])] = lab
    # The packets of a track a threshold was fitted on never count, whichever rip of the title they are in.
    fitted_packets = {(t["slug"], t["md5"]) for t in tracks if t["md5"] and labels.is_fitted(fitted, t["slug"], t["label"], t["stream"])}
    counted = set()
    for t in sorted(tracks, key=lambda t: (t["slug"], t["label"], t["secondary"], t["folder"], t["stream"])):
        lab = direct.get((t["slug"], t["label"], t["stream"]))
        t["inherited_from"], t["ambiguous"] = None, None
        if lab is None and t["role"] == "candidate" and not t["excluded"] and t["md5"]:
            src = {k: v for k, v in sources.get((t["slug"], t["md5"], t["md5_rank"]), {}).items() if k[0] != t["label"]}
            kinds = {v["kind"] for v in src.values()}
            if len(kinds) == 1:
                (src_label, src_stream), lab = sorted(src.items(), key=lambda kv: kv[0])[0]
                t["inherited_from"] = f"{src_label} stream {src_stream}"
            elif kinds:
                t["ambiguous"] = "same packets as " + ", ".join(f"{sl} stream {ss} ({v['kind']})" for (sl, ss), v in
                                                                sorted(src.items(), key=lambda kv: kv[0]))
        t["label_entry"] = lab
        t["kind"] = lab["kind"] if lab else None
        group, wanted = (labels.KINDS[lab["kind"]][:2] if lab else (None, None))
        t["group"], t["wanted"] = group, wanted
        if t["role"] != "candidate" or t["excluded"] or t["secondary"]:
            why = "not a judged track of its title"
        elif lab is None:
            why = "no label"
        elif t["inherited_from"]:
            why = f"its label is that of {t['inherited_from']} (same packets), which counts there"
        else:
            why = labels.not_counted_because(lab, t["slug"], fitted)
        if not why and (t["slug"], t["md5"]) in fitted_packets:
            why = "the same packets as a track the thresholds were fitted on"
        if not why and t["md5"] and (t["slug"], t["md5"]) in counted:
            why = "the same packets already count on this disc"
        t["why_not_counted"], t["counts"] = why, not why
        if t["counts"] and t["md5"]:
            counted.add((t["slug"], t["md5"]))
        v, k = t["verdict"], t["kind"]
        if t["excluded"]:
            t["outcome"] = "wrong reference" if t["role"] == "reference" else "excluded"
        elif t["role"] == "reference":
            if k in (None, "main"):
                t["outcome"] = "reference"
            elif group in ("unwanted", "duplicate"):
                t["outcome"] = "reference is unwanted audio"
            else:
                t["outcome"] = f"reference labelled {k}"
        elif lab is None:
            t["outcome"] = "unlabelled"
        elif group == "programme" and v in DROP:
            t["outcome"] = "FAIL"
        elif group in ("unwanted", "duplicate") and v not in DROP:
            t["outcome"] = "kept"
        elif group == "unwanted" and v == "commentary" and k not in labels.COMMENTARY_KINDS:
            t["outcome"] = "dropped as a commentary"
        elif group == "ask" and v in DROP:
            t["outcome"] = "undecided kind, drop verdict"
        elif group == "unknown" and v in DROP:
            t["outcome"] = "unknown kind, drop verdict"
        else:
            t["outcome"] = "agrees"
    return tracks


def judged(tracks):
    """The tracks the table and the counts use: one per stream of a title, from a folder with the right reference."""
    return [t for t in tracks if not t["excluded"] and not t["secondary"]]


def coverage(tracks, req, titles):
    """Experiment 13's counts over the tracks that count, and the answer as it stands."""
    cnt = [t for t in judged(tracks) if t["counts"]]
    title = lambda t: titles.get((t["slug"], t["label"]), {})
    comm = [t for t in cnt if t["kind"] in labels.COMMENTARY_KINDS]
    prog = [t for t in cnt if t["group"] == "programme" and t["kind"] != "main"]
    c = {
        "commentaries": len(comm), "commentary_discs": len({t["slug"] for t in comm}),
        "commentary_features": sum(1 for t in comm if title(t).get("title_kind") == "feature"),
        "commentary_audible": sum(1 for t in comm if t["label_entry"].get("film_audible") is True),
        "partial": sum(1 for t in comm if t["kind"] == "partial-commentary"),
        "ad": sum(1 for t in cnt if t["kind"] == "audio-description"),
        "programme": len(prog), "derived_cores": sum(1 for t in prog if t["kind"] == "derived-core"),
        "programme_separate": sum(1 for t in prog if t["kind"] != "derived-core"),
        "no_centre_titles": len({(t["slug"], t["label"], t["main"]) for t in cnt if not t["ref_fc"]}),
        "dialogue_heavy_titles": len({(t["slug"], t["label"]) for t in cnt if title(t).get("dialogue_heavy") is True}),
        "others": {k: sum(1 for t in cnt if t["kind"] == k) for k in ("isolated-score", "music-effects", "same-language-dub", "alternate-narration")},
        "verdict_discs": len({t["slug"] for t in comm if t["verdict"] == "commentary"}),
    }
    false = sorted({(t["slug"], t["label"], t["stream"]) for t in tracks if t["outcome"] == "FAIL" and t["verdict"] == "commentary"})
    missing = [name for name, have, need in (
        ("commentaries", c["commentaries"], req["commentaries"]), ("discs with a commentary", c["commentary_discs"], req["commentary_discs"]),
        ("commentaries on feature films", c["commentary_features"], req["commentary_features"]),
        ("commentaries with the film clearly audible", c["commentary_audible"], req["commentary_audible"]),
        ("programme tracks", c["programme"], req["programme"]),
        ("separately encoded programme tracks", c["programme_separate"], req["programme_separate"]),
        ("titles whose reference has no centre channel", c["no_centre_titles"], req["no_centre_titles"]),
        ("dialogue-heavy titles", c["dialogue_heavy_titles"], req["dialogue_heavy_titles"])) if have < need]
    if false:
        answer = f"no: {len(false)} programme track(s) with the commentary verdict"
    elif missing:
        answer = "not answered: too few " + ", ".join(missing)
    elif c["verdict_discs"] < req["verdict_discs"]:
        answer = f"no: commentary verdicts on {c['verdict_discs']} discs, {req['verdict_discs']} needed"
    else:
        answer = "yes, if the audio-description and 'every one the owner has' kinds are covered (checked by hand)"
    return c, answer, false


def table(tracks):
    cands = [t for t in judged(tracks) if t["role"] == "candidate"]
    lines = [f"  {'kind':20} {'wanted':7} " + " ".join(f"{s:>5}" for s in SHORT) + "  total"]
    for kind, (group, wanted, _) in labels.KINDS.items():
        rows = [t for t in cands if t["kind"] == kind]
        if rows:
            lines.append(f"  {kind:20} {wanted:7} " + " ".join(f"{sum(1 for t in rows if t['verdict'] == v):5}" for v in VERDICTS)
                         + f"  {len(rows):5}")
    rest = [t for t in cands if t["kind"] is None]
    lines.append(f"  {'(unlabelled)':20} {'':7} " + " ".join(f"{'-':>5}" for _ in VERDICTS) + f"  {len(rest):5}   verdicts hidden until labelled")
    lines.append("  columns: " + ", ".join(f"{s} = {v}" for s, v in zip(SHORT, VERDICTS)))
    return lines


def name(t):
    return f"{t['slug']}/{t['label']} stream {t['stream']}"


def where(t):
    """The track's name, with its folder when the stream is measured in more than one."""
    return name(t) + (f" (in {t['folder']})" if t.get("also_in") else "")


def describe(t):
    pos = t["position"]
    at = f"audio track {pos[0]} of {pos[1]} in a player" + (f', named "{pos[2]}"' if pos[2] else "") if pos else "position unknown"
    return f"{t['codec']} {t['channels']} ch {t['language']} ({at})"


def track_key(t):
    return f"{t['slug']}/{t['label']}/{t['folder']}/{t['role']}/{t['stream']}"


def verdicts_of(track_list, unlabelled=None):
    """{track key: (verdict, kind, outcome)} for the tracks of a run or of a snapshot (whose unlabelled verdicts are
    kept apart)."""
    out = {}
    for t in track_list:
        v = t.get("verdict")
        if v is None and unlabelled:
            v = unlabelled.get(track_key(t))
        out[track_key(t)] = (v, t.get("kind"), t.get("outcome"))
    return out


def changes(old, new, title):
    """Lines for every candidate whose verdict differs between two runs ({key: (verdict, kind, outcome)}), and for
    the tracks only one of them has. An unlabelled track's verdict is never shown."""
    out, hidden = [], 0
    for k, (v, kind, outcome) in sorted(new.items()):
        if "/candidate/" not in k or k not in old or old[k][0] == v:
            continue
        if kind is None:
            hidden += 1
            continue
        slug, label, folder, _, stream = k.split("/")
        out.append(f"  {slug}/{label} stream {stream} in {folder} ({kind}): {old[k][0]} -> {v}" + ("   FAIL" if outcome == "FAIL" else ""))
    if hidden:
        out.append(f"  {hidden} unlabelled track(s) changed verdict (not shown until labelled)")
    gone, added = sorted(set(old) - set(new)), sorted(set(new) - set(old))
    if gone:
        out.append(f"  gone: {len(gone)} track(s) not measured now: " + ", ".join(gone))
    if added:
        out.append(f"  new: {len(added)} track(s): " + ", ".join(added))
    return [title] + (out or ["  none"]), gone


def ripped(corpus):
    """{(slug, label)} of every rip that succeeded: rip-summary.json (rip-title.ps1) or extract-summary.json
    (extract-from-copy.ps1) saying "ok": true."""
    out = set()
    for fname in ("rip-summary.json", "extract-summary.json"):
        for path in pathlib.Path(corpus).glob(f"*/*/{fname}"):
            try:
                if json.loads(path.read_text(encoding="utf-8-sig")).get("ok") is True:
                    out.add((path.parent.parent.name, path.parent.name))
            except (OSError, ValueError):
                pass
    return out


def report(corpus, tracks, unreadable, labelled, req, titles, meas, rule_changes, proxies, rips=frozenset()):
    """(lines, failures, orphans): failures are the reasons for exit status 1, orphans the labels without a
    measurement."""
    good = judged(tracks)
    cands = [t for t in good if t["role"] == "candidate"]
    refs = [t for t in good if t["role"] == "reference"]
    folders = {(t["slug"], t["label"], t["folder"]) for t in tracks}
    wrong = sorted({(t["slug"], t["label"], t["folder"], t["ref_problem"]) for t in tracks if t["excluded"]})
    discs = sorted({t["slug"] for t in tracks})
    plural = lambda n, word: f"{n} {word}" + ("" if n == 1 else "s")
    lines = [f"audio dataset: {corpus}: {plural(len(discs), 'disc')}, {plural(len({(t['slug'], t['label']) for t in tracks}), 'title')}, "
             f"{plural(len(folders), 'level folder')} ({len(wrong)} measured against the wrong reference), "
             f"{plural(len(cands), 'measured track')} ({sum(1 for t in cands if t['kind'])} labelled), {plural(len(refs), 'reference')}",
             f"rule: audio_similarity.py {CODE[:12]}, " + (", ".join(f"{k.upper()} {v}" for k, v in rule_changes.items())
                                                            or "thresholds and constants as in the code"),
             f"measures: {meas.computed} analysed now, {meas.reused} taken from the cache (the snapshot, or this run's baseline)", ""]
    todo = [t for t in cands if t["kind"] is None]
    lines.append("To label (measured, no label yet; the owner answers before seeing any verdict):")
    for t in todo:
        lines += [f"  {name(t)}: {describe(t)}" + (f"; {t['ambiguous']}: they disagree" if t["ambiguous"] else ""),
                  f"    python3 scripts/phase0/audio_labels.py set corpus/owner/{t['slug']} {t['label']} {t['stream']} "
                  f"<kind> --how <how> --saw-verdict no"]
    lines += ["  none"] if not todo else []
    lines += ["", "Kind against verdict (labelled candidates; references are not judged):"] + table(tracks)
    c, answer, false = coverage(tracks, req, titles)
    o = c["others"]
    not_counted = {}
    for t in cands:
        if not t["counts"]:
            not_counted[t["slug"]] = not_counted.get(t["slug"], 0) + 1
    lines += ["", f"Experiment 13 (spec section 17 row 13; tracks that count: named before the verdict from listening, "
                  f"packaging, menu, a flag or TheDiscDB, the owner sure, each disc's packets once, never "
                  f"{req['fitting_slug']} or a disc or track in audio-fitted.json):",
              f"  commentaries          {c['commentaries']} of {req['commentaries']}, from {c['commentary_discs']} of "
              f"{req['commentary_discs']} discs; on feature films {c['commentary_features']} of {req['commentary_features']}; "
              f"film clearly audible {c['commentary_audible']} of {req['commentary_audible']} (of them partial: {c['partial']})",
              f"  audio description     {c['ad']} ({req['ad_min']} needed if the owner has a disc with one, up to {req['ad_max']}; checked by hand)",
              f"  programme tracks      {c['programme']} of {req['programme']} other than the reference; separately encoded "
              f"{c['programme_separate']} of {req['programme_separate']}; derived cores {c['derived_cores']} (row 13 does not "
              f"exclude them from the {req['programme']})",
              f"  no centre channel     {c['no_centre_titles']} of {req['no_centre_titles']} titles with a counted extra track",
              f"  dialogue-heavy        {c['dialogue_heavy_titles']} of {req['dialogue_heavy_titles']} titles with a counted extra track",
              f"  score, M&E, dub, narration  isolated-score {o['isolated-score']}, music-effects {o['music-effects']}, "
              f"same-language-dub {o['same-language-dub']}, alternate-narration {o['alternate-narration']} (every one the owner has; checked by hand)",
              f"  commentary verdicts   on {c['verdict_discs']} of {req['verdict_discs']} discs",
              f"  false verdicts        {len(false)} (a programme track with the commentary verdict, on any disc)",
              f"  as it stands: {answer}",
              "  not counted: " + (", ".join(f"{s} {n}" for s, n in sorted(not_counted.items())) or "none")]
    lines += ["", "Disagreements (labelled candidates):"]
    start = len(lines)
    failures = []
    fails = [t for t in tracks if t["outcome"] == "FAIL"]
    lines += [f"  FAIL {where(t)}: labelled {t['kind']} (programme audio), verdict {t['verdict']}"
              + (f" (label of {t['inherited_from']})" if t["inherited_from"] else "") for t in fails]
    if fails:
        failures.append(f"{len(fails)} programme track(s) got a drop verdict: " + "; ".join(where(t) for t in fails))
    lines += [f"  FAIL {s}/{l}/{f}: wrong reference: {p}; its tracks are left out (measure the title again with the "
              f"reference that `audio_dataset.py args` prints, then delete this folder)" for s, l, f, p in wrong]
    if wrong:
        failures.append(f"{len(wrong)} folder(s) measured against the wrong reference: " + "; ".join(f"{s}/{l}/{f}" for s, l, f, _ in wrong))
    bad_refs = [t for t in tracks if t["outcome"] == "reference is unwanted audio"]
    lines += [f"  FAIL {where(t)}: the reference is labelled {t['kind']} (unwanted audio): its folder measures the "
              f"title against it" for t in bad_refs]
    if bad_refs:
        failures.append(f"{len(bad_refs)} reference(s) labelled as unwanted audio: " + "; ".join(where(t) for t in bad_refs))
    kept = [t for t in cands if t["outcome"] == "kept"]
    for kind in labels.KINDS:
        rows = [t for t in kept if t["kind"] == kind]
        if rows:
            lines.append(f"  kept although unwanted, {kind}: " + "; ".join(f"{name(t)} ({t['verdict']}"
                                                                         + (f", not judged: {t['not_judged']}" if t["not_judged"] else "") + ")" for t in rows))
    for t in [t for t in tracks if not t["excluded"]]:
        if t["outcome"] in ("dropped as a commentary", "undecided kind, drop verdict", "unknown kind, drop verdict") \
                or t["outcome"].startswith("reference labelled"):
            lines.append(f"  note: {where(t)}: {t['outcome']} ({t['kind']}, verdict {t['verdict']})")
    shown = set()
    for t in good:
        if t.get("also_in") and (t["slug"], t["label"], t["stream"], t["role"]) not in shown:
            shown.add((t["slug"], t["label"], t["stream"], t["role"]))
            same = [o for o in tracks if (o["slug"], o["label"], o["stream"], o["role"]) == (t["slug"], t["label"], t["stream"], t["role"]) and not o["excluded"]]
            differ = len({o["verdict"] for o in same}) > 1
            lines.append(f"  note: {name(t)} ({t['role']}) is measured in {', '.join(o['folder'] for o in same)}; the table uses {t['folder']}"
                         + ("; their verdicts differ" + (": " + ", ".join(f"{o['folder']} {o['verdict']}" for o in same) if t["kind"] else "")
                            if differ else ""))
    for t in tracks:
        if t["ref_problem"] is None and t["role"] == "reference":
            lines.append(f"  note: {t['slug']}/{t['label']}/{t['folder']}: no streams.json, so its reference is not checked")
    lines += [f"  note: {name(t)} takes its label from {t['inherited_from']} (same packets)" for t in cands if t["inherited_from"]]
    if len(lines) == start:
        lines.append("  none")
    have = {(t["slug"], t["label"], t["stream"]) for t in good}
    orphans = [f"{slug}/{t['label']} stream {t['stream']}: {t['kind']}" for slug, data in sorted(labelled.items())
               for t in data["tracks"] if (slug, t["label"], t["stream"]) not in have]
    lines += ["", "Labels without a measurement:"] + ([f"  {o}" for o in orphans] or ["  none"])
    lines += ["", "Folders that cannot be read:"] + ([f"  {u}" for u in unreadable] or ["  none"])
    measured = {(t["slug"], t["label"]) for t in tracks} | {tuple(u.split("/")[:2]) for u in unreadable}
    titles_seen = sorted(measured | set(rips))
    missing = [f"{s}/{l}" + ("" if (s, l) in measured else " (not measured)") for s, l in titles_seen if (s, l) not in proxies]
    lines += ["", f"Titles whose audio is in the tuning dataset (audio-proxy.json): {len(titles_seen) - len(missing)} of "
                  f"{len(titles_seen)} (every successful rip and every measured title)" + (f"; missing: {', '.join(missing)}" if missing else "")]
    return lines, failures, orphans


def load_snapshot(path):
    try:
        data = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if data.get("format") == SNAPSHOT_FORMAT else None


def write_snapshot(path, corpus, tracks, unreadable, rule, consts, overrides, req, fitted, meas):
    keep = ("slug", "label", "folder", "role", "stream", "main", "md5", "md5_rank", "codec", "channels", "language",
            "ref_fc", "ref_problem", "excluded", "secondary", "verdict", "not_judged", "duplicate_of", "measures", "key",
            "kind", "inherited_from", "counts", "why_not_counted", "outcome")
    rows, hidden = [], {}
    for t in tracks:
        row = dict({k: t[k] for k in keep}, owner_label=t["label_entry"])
        if t["role"] == "candidate" and t["kind"] is None:
            hidden[track_key(t)] = row["verdict"]
            row["verdict"] = None
        rows.append(row)
    data = {"format": SNAPSHOT_FORMAT, "created": datetime.datetime.now().isoformat(timespec="seconds"),
            "corpus": str(corpus), "code": {"audio_similarity_sha1": CODE}, "rule": rule, "constants": consts,
            "overrides": overrides, "spec_row13": req, "fitted": sorted([list(f) for f in fitted], key=str),
            "tracks": rows, "unlabelled_verdicts_note": "verdicts of tracks the owner has not labelled: never show them "
                                                        "to the owner before the label is given",
            "unlabelled_verdicts": hidden, "unreadable": unreadable, "measures_cache": dict(sorted(meas.used.items()))}
    tmp = pathlib.Path(str(path) + ".tmp")
    tmp.write_text(json.dumps(data, indent=1, sort_keys=False) + "\n", encoding="utf-8")
    tmp.replace(path)


def same_rip(slug_dir, label):
    """(another label, its ffprobe-streams.json, title, bytes) for a rip of the same title with the same size that was
    probed, or None. rip-summary.json (or extract-summary.json) gives the title and the size."""
    def summary(d):
        for fname in ("rip-summary.json", "extract-summary.json"):
            try:
                data = json.loads((d / fname).read_text(encoding="utf-8-sig"))
            except (OSError, ValueError):
                continue
            if data.get("ok") is True and data.get("title") is not None and data.get("bytes"):
                return data["title"], data["bytes"]
        return None
    slug_dir = pathlib.Path(slug_dir)
    mine = summary(slug_dir / label)
    if mine is None:
        return None
    for d in sorted(p for p in slug_dir.iterdir() if p.is_dir() and p.name != label):
        if (d / "ffprobe-streams.json").is_file() and summary(d) == mine:
            return d.name, d / "ffprobe-streams.json", mine[0], mine[1]
    return None


def levels_args(slug_dir, label):
    """The audio-levels.ps1 job lines for a ripped title: per language with two or more audio streams, the
    top-ranked stream (most channels, then lossless, then the earlier) against the others (Task 12 Step 15's
    ranking); when no language has two, one job with the top-ranked English (else any) stream alone, for the dataset.
    The streams come from the rip's own ffprobe-streams.json, else from that of a rip of the same title with the same
    size, else from its mkvmerge.json."""
    d = pathlib.Path(slug_dir) / label
    slug = pathlib.Path(slug_dir).resolve().name
    sib = None if (d / "ffprobe-streams.json").is_file() else same_rip(slug_dir, label)
    if (d / "ffprobe-streams.json").is_file():
        audio = ffprobe_audio(json.loads((d / "ffprobe-streams.json").read_text(encoding="utf-8-sig")).get("streams", []))
        source = "ffprobe-streams.json"
    elif sib:
        audio = ffprobe_audio(json.loads(sib[1].read_text(encoding="utf-8-sig")).get("streams", []))
        source = f"{sib[0]}/ffprobe-streams.json: a rip of the same title {sib[2]} with the same size, {sib[3]} bytes"
    elif (d / "mkvmerge.json").is_file():
        audio = []
        for t in json.loads((d / "mkvmerge.json").read_text(encoding="utf-8-sig")).get("tracks", []):
            if t.get("type") == "audio":
                p, c = t.get("properties") or {}, str(t.get("codec", ""))
                lossless = any(x in c for x in ("TrueHD", "DTS-HD Master Audio", "FLAC", "PCM"))
                what = [c, f"{p.get('audio_channels')} ch", p.get("track_name")]
                audio.append((t["id"], int(p.get("audio_channels") or 0), lossless, p.get("language") or "und",
                              " ".join(str(w) for w in what if w)))
        source = "mkvmerge.json (track IDs: check them against ffprobe's stream numbers in the job's stream lines)"
    else:
        raise ValueError(f"{d}: no ffprobe-streams.json or mkvmerge.json, and no other rip of the same title with the "
                         f"same size has one. Probe the rip first, then run args again:\n"
                         f"scripts/phase0/job.sh probe-title.ps1 probe-{label} -Slug {slug} -Label {label} -SkipRemux\n"
                         f"scripts/phase0/wait.sh probe-{label}")
    if not audio:
        raise ValueError(f"{d}: the file has no audio stream")
    langs = {}
    for a in audio:
        langs.setdefault(a[3], []).append(a)
    groups = [sorted(v, key=rank) for v in langs.values() if len(v) >= 2]
    if not groups:
        eng = [a for a in audio if a[3] == "eng"]
        groups = [sorted(eng or audio, key=rank)[:1]]
    lines = [f"{slug}/{label}: {len(audio)} audio streams (from {source})"]
    lines += [f"  stream {a[0]}: {a[4]} lang={a[3]}" for a in audio]
    for g in groups:
        main, cands = g[0][0], sorted(a[0] for a in g[1:])
        log = f"audio-levels-{slug}-{label}" + (f"-{main}" if len(groups) > 1 else "")
        lines += [f"scripts/phase0/job.sh audio-levels.ps1 {log} -Slug {slug} -Label {label} -Main {main}"
                  + (f" -Cand {','.join(map(str, cands))}" if cands else ""),
                  f"scripts/phase0/wait.sh {log}"]
    return lines


def main(argv):
    if argv[:1] == ["args"]:
        if len(argv) != 3:
            print("usage: audio_dataset.py args corpus/owner/<slug> <label>", file=sys.stderr)
            return 2
        try:
            print("\n".join(levels_args(argv[1], argv[2])))
        except (OSError, ValueError) as e:
            print(e, file=sys.stderr)
            return 2
        return 0
    p = argparse.ArgumentParser(prog="audio_dataset.py")
    p.add_argument("corpus", nargs="?", default="corpus/owner")
    p.add_argument("--set", dest="sets", action="append", default=[], metavar="NAME=VALUE")
    p.add_argument("--snapshot")
    p.add_argument("--compare", metavar="SNAPSHOT")
    p.add_argument("--spec", default=str(SPEC))
    p.add_argument("--allow-unmeasured", action="store_true",
                   help="end RESULT ok although something could not be checked (it is still listed)")
    args = p.parse_args(argv)
    corpus = pathlib.Path(args.corpus)
    try:
        req = labels.exp13_requirements(pathlib.Path(args.spec).read_text(encoding="utf-8-sig"))
        rule_over, const_over = parse_sets(args.sets)
    except (OSError, ValueError) as e:
        print(e, file=sys.stderr)
        return 2
    if not corpus.is_dir():
        print(f"{corpus}: no such folder", file=sys.stderr)
        return 2
    labelled, bad = {}, []
    for f in sorted(corpus.glob("*/audio-labels.json")):
        try:
            labelled[f.parent.name] = labels.load(f)
        except ValueError as e:
            bad.append(str(e))
    try:
        fitted = labels.load_fitted(corpus, req["fitting_slug"])
    except ValueError as e:
        bad.append(str(e))
    if bad:
        print("\n".join(bad + ["RESULT FAILED: a labels file does not validate (audio_labels.py check)"]))
        return 2
    titles = {(slug, label): t for slug, data in labelled.items() for label, t in data["titles"].items()}
    proxies = {(p.parent.parent.parent.name, p.parent.parent.name) for p in corpus.glob("*/*/audio-levels-20ms-*/audio-proxy.json")}
    baseline_path = corpus / "audio-dataset.json"
    previous = load_snapshot(baseline_path)
    cache = dict((previous or {}).get("measures_cache", {}))
    found = find_folders(corpus)
    tracks, unreadable, meas = measure(found, dict(sim.RULE), {}, cache)
    tracks = join_labels(tracks, labelled, fitted)
    base_tracks = tracks
    if rule_over or const_over:
        rule = dict(sim.RULE, **rule_over)  # the Measurer has put the baseline's measures into cache already
        tracks, unreadable, meas2 = measure(found, rule, const_over, cache)
        tracks = join_labels(tracks, labelled, fitted)
        meas2.computed += meas.computed
        meas = meas2
    shown = {**rule_over, **const_over}
    lines, failures, orphans = report(corpus, tracks, unreadable, labelled, req, titles, meas, shown, proxies, ripped(corpus))
    gone = []
    if shown:
        lines += [""] + changes(verdicts_of(base_tracks), verdicts_of(tracks), "Changes against the baseline rule (" +
                                ", ".join(f"{k.upper()}={v}" for k, v in shown.items()) + "):")[0]
    if args.compare:
        other = load_snapshot(args.compare)
        if other is None:
            print(f"{args.compare}: not a snapshot written by this audio_dataset.py ({SNAPSHOT_FORMAT})", file=sys.stderr)
            return 2
        lines += [""] + changes(verdicts_of(other["tracks"], other.get("unlabelled_verdicts")), verdicts_of(tracks),
                                f"Changes against {args.compare} ({other['created']}, code "
                                f"{other['code']['audio_similarity_sha1'][:12]}):")[0]
    if previous and not shown:
        text, gone = changes(verdicts_of(previous["tracks"], previous.get("unlabelled_verdicts")), verdicts_of(tracks),
                             f"Changes since the previous snapshot ({previous['created']}, code "
                             f"{previous['code']['audio_similarity_sha1'][:12]}):")
        lines += [""] + text
    unchecked = []
    if not found:
        unchecked.append(f"no level folder under {corpus}")
    if unreadable:
        unchecked.append(f"{len(unreadable)} folder(s) cannot be read")
    if orphans:
        unchecked.append(f"{len(orphans)} labelled track(s) not measured")
    if gone:
        unchecked.append(f"{len(gone)} track(s) of the previous snapshot are gone")
    with constants(const_over):
        effective = {k: getattr(sim, k) for k in sim.CONSTANTS}
    keep_baseline = [u for u in unchecked if not u.endswith("not measured")]
    if args.snapshot:
        out, why = pathlib.Path(args.snapshot), ""
    elif shown:
        out, why = pathlib.Path(tempfile.gettempdir()) / "lazymkv-audio-dataset-trial.json", " (a trial: the baseline is not replaced)"
    elif keep_baseline:
        out = pathlib.Path(tempfile.gettempdir()) / "lazymkv-audio-dataset-unchecked.json"
        why = (f" (the baseline {baseline_path} was NOT replaced: {'; '.join(keep_baseline)}; to replace it on purpose, "
               f"run again with --snapshot {baseline_path})")
    else:
        out, why = baseline_path, ""
    write_snapshot(out, corpus, tracks, unreadable, dict(sim.RULE, **rule_over), effective,
                   {"rule": rule_over, "constants": const_over}, req, fitted, meas)
    lines += ["", f"snapshot: {out}{why}"]
    hint = " (see 'Folders that cannot be read', 'Labels without a measurement' and the changes above)"
    if failures:
        lines.append("RESULT FAILED: " + "; ".join(failures + ([f"not checked: {', '.join(unchecked)}"] if unchecked else [])))
        code = 1
    elif unchecked and not args.allow_unmeasured:
        lines.append("RESULT FAILED: not everything was checked: " + ", ".join(unchecked) + hint)
        code = 3
    else:
        lines.append("RESULT ok" + (f" (not checked, allowed by --allow-unmeasured: {', '.join(unchecked)})" if unchecked else ""))
        code = 0
    print("\n".join(lines))
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
