"""The tuning dataset of the audio rule (decision D16): every stored measurement, the owner's labels, one command.

    audio_dataset.py [corpus/owner] [--set NAME=VALUE ...] [--snapshot FILE] [--compare FILE] [--spec FILE]
                     [--allow-unmeasured]
    audio_dataset.py args corpus/owner/<slug> <label>       the audio-levels.ps1 jobs that add a ripped title: the
                                                            title's programme reference against every other stream in a
                                                            wanted language (EXP-13 addendum, section 9.1)

The first form runs the current rule (audio_similarity.py, imported, never copied) over every
corpus/owner/<slug>/<label>/audio-levels-20ms-* folder, joins each candidate track with the owner's label in
corpus/owner/<slug>/audio-labels.json (see audio_labels.py), and prints:
  1. the tracks to label: measured, with no label yet. No verdict is shown: a label counts towards experiment 13 only
     when the track's content was checked (a person listened to it) before the verdict was seen (D23);
  2. kind against verdict, over the labelled tracks;
  3. experiment 13: the tracks that count, against the totals of spec section 17 row 13 (read from the spec), and
     the answer as it stands. Whether a label counts is worked out here (audio_labels.not_counted_because), never
     stored, and no track of a disc or track the thresholds were fitted on counts (row 13, audio-fitted.json);
  4. disagreements: FAIL for a programme track with a drop verdict (commentary or duplicate) or with the commentary
     pattern in another language than the reference (commentary-cross-language: kept, but the pattern fired on
     programme audio, so it counts as a false verdict), for a level folder
     measured against another stream than the title's programme reference (the stream spec 8.6 places first in the
     work's original language, else in English; tmdb.json gives the original language), for a reference the owner
     labelled as unwanted audio, and for a swapped reference: a labelled candidate against which, taken as the
     reference, the programme would get the commentary verdict (the same two runs read the other way round); the
     unwanted tracks that were kept, by kind; notes (among them every folder the position veto holds for: its
     reference is not the first stream of its language in the file, so every candidate is reference-unclear, D21);
  5. labels without a measurement, folders that cannot be read, and the titles not yet in the dataset: every
     successful rip (rip-summary.json or extract-summary.json saying ok) and every measured title without an
     audio-proxy.json in one of its level folders (audio-levels.ps1 writes that file there last, once the title is
     complete on the share); this is the list of titles the routine still has to add. A rip whose
     ffprobe-streams.json lists no audio stream has nothing to add: it is named in a note, not as missing;
  6. with --set or --compare, and against the previous snapshot: every labelled track whose verdict changed (an
     unlabelled one only as a count), and the tracks that are gone or new;
  7. RESULT ok, or RESULT FAILED: <reasons>.

A track measured in several level folders of one title is judged in each; a FAIL in any of them counts, and the table
and the counts use the folder with the most candidates. A track with no label of its own takes the label of the track
of another rip of the same disc with the same packets and the same place among the title's copies of those packets
(the first copy, the second, ...); when such tracks disagree, it is listed to be labelled.

It writes a JSON snapshot of the run: every track's measures, verdict and label (an unlabelled track's verdict is kept
apart, under "unlabelled_verdicts", not to be shown to the owner), the rule and constants used and the SHA-1 of
audio_similarity.py. Without --set it replaces <corpus>/audio-dataset.json (committed), which doubles as a cache
(a snapshot that would differ from the file it replaces only in its "created" time is not written, so a run that
changes nothing leaves the committed baseline as it is):
measures whose inputs, constants and code are unchanged are taken from it. It is not replaced when a folder cannot be
read, when a track of it is no longer measured, or when no level folder is found: the snapshot then goes to the
temporary folder, and --snapshot <corpus>/audio-dataset.json replaces the baseline on purpose. With --set it goes to
the temporary folder unless --snapshot names a file, so a trial never replaces the baseline.

--set NAME=VALUE (repeatable) tries another value without editing code: a key of audio_similarity.RULE, such as
DC_MAX=0.25 (only the verdicts change), or one of audio_similarity.CONSTANTS, such as SILENT_DB=-80 (the measures are
computed again). The value must be a finite number. The report is then that of the changed rule, and ends with its
differences from the baseline.

Exit status: 0; 1 when the rule reported drops a programme track (or gives one commentary-cross-language), a folder
is measured against the wrong reference, or a labelled candidate taken as the reference would give the programme the
commentary verdict; 2 for usage errors, a labels or fitted file that does not validate, or a spec row 13 that cannot be
read; 3 when something could not be checked (no level folder, a folder that cannot be read, a labelled track not
measured, a track of the previous snapshot gone), unless --allow-unmeasured is given.
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
VERDICTS = ("duplicate", "commentary", "commentary-cross-language", "reference-unclear", "ad-pattern", "note-commentary",
            "note-ad", "keep", "not-judged")
SHORT = ("dup", "comm", "c-x", "r-unc", "ad-p", "n-com", "n-ad", "keep", "n-j")
DROP = ("duplicate", "commentary")  # the verdicts that drop a track: by MD5 (D13), by content once experiment 13 says yes
# The commentary pattern confirmed: a drop verdict in the reference's language, keep-only in another (addendum 9.2). On a
# programme track either one is a false verdict.
PATTERN = ("commentary", "commentary-cross-language")
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


is_lossless, rank, placed_first = sim.is_lossless, sim.rank, sim.placed_first  # spec 8.6 steps 3 and 4 (the rule's own)
ffprobe_audio, read_streams = sim.ffprobe_audio, sim.read_streams  # an ffprobe stream list as the rule reads it
ISO639 = sim.ISO639  # TMDb's two-letter original_language -> three-letter stream codes


def original_language(slug_dir):
    """(codes, what): the three-letter codes of the work's original language from <slug_dir>/tmdb.json, and a phrase
    saying how it was found. codes is () when the original language is English or unknown."""
    try:
        code = json.loads((pathlib.Path(slug_dir) / "tmdb.json").read_text(encoding="utf-8-sig")).get("original_language")
    except (OSError, ValueError, AttributeError):
        return (), "original language unknown (no readable tmdb.json)"
    if not code:
        return (), "original language unknown (tmdb.json gives none)"
    if code == "en":
        return (), "original language English (TMDb en)"
    if code == sim.TMDB_NO_LANGUAGE:
        return (), f"original language none (TMDb {code}: a work without speech; English is the reference language)"
    if code not in ISO639:
        return (), f"original language unknown (TMDb {code!r} is not in audio_similarity.ISO639)"
    return ISO639[code], f"original language {'/'.join(ISO639[code])} (TMDb {code})"


def programme_reference(audio, original=()):
    """(stream, why): the title's programme reference (EXP-13 addendum, section 9.1), as an ffprobe_audio() tuple: the
    stream spec 8.6 places first in the work's original language when the title has one in it, else in English, else
    among all streams (no stream in a wanted language: measured for the dataset only)."""
    for codes, why in ((tuple(original), "the original language"), (("eng",), "English")):
        mine = [a for a in audio if a[3] in codes]
        if mine:
            return placed_first(mine), why
    return placed_first(audio), "any language (no stream is in a wanted language)"


NO_LANGUAGE = sim.NO_LANGUAGE


def wanted(a, original=()):
    """A stream measured as a candidate: in English or the original language (R4), or with no language code (the app
    does not judge those; phase 0 measures them for the dataset)."""
    return a[3] == "eng" or a[3] in original or a[3] in NO_LANGUAGE


def find_folders(corpus):
    """[(slug, label, folder)] for every audio-levels-20ms-* folder, in name order."""
    out = []
    for slug_dir in sorted(p for p in pathlib.Path(corpus).iterdir() if p.is_dir() and labels.SLUG.match(p.name)):
        for label_dir in sorted(p for p in slug_dir.iterdir() if p.is_dir()):
            out += [(slug_dir.name, label_dir.name, f) for f in sorted(label_dir.glob("audio-levels-20ms-*")) if f.is_dir()]
    return out


def audio_positions(streams):
    """{stream: (k, n, title)}: the stream is the k-th of the file's n audio streams (a player's audio track k)."""
    audio = [s for s in streams or [] if s.get("codec_type") == "audio"]
    return {s["index"]: (k + 1, len(audio), (s.get("tags") or {}).get("title", "")) for k, s in enumerate(audio)}


def reference_problem(streams, main, original=()):
    """None when there is no streams.json to check against; '' when `main` is the title's programme reference
    (programme_reference), or the stream placed first in a language that is not wanted (a folder measured for the
    dataset only); otherwise what is wrong. original: the three-letter codes of the work's original language."""
    if streams is None:
        return None
    audio = ffprobe_audio(streams)
    mine = [a for a in audio if a[0] == main]
    if not mine:
        return f"stream {main} is not an audio stream of the file (streams.json)"
    ref, why = programme_reference(audio, original)
    if ref[0] == main:
        return ""
    if not wanted(mine[0], original) and placed_first([a for a in audio if a[3] == mine[0][3]])[0] == main:
        return ""
    return (f"measured against stream {main} ({mine[0][4]}), but the top-ranked {ref[3]} stream is {ref[0]} ({ref[4]}), "
            f"the title's programme reference (placed first in {why}; EXP-13 addendum, section 9.1)")


def first_of_language(streams, main):
    """The first audio stream of the file in the language of stream `main`, as (stream, description), when it is not
    `main` itself and is lossless or has more than two channels (a lossy stereo first stream is what G1 and G2 pass
    over on purpose); None otherwise, or without streams.json. It is the condition of the position veto (row 0, D21:
    audio_similarity.first_of_language): no candidate of such a folder gets a commentary verdict."""
    first = sim.first_of_language(ffprobe_audio(streams or []), main)
    return None if first is None else (first[0], first[4])


def peer_row(peer, meas):
    """A candidate's peer run (audio_similarity.results) as stored in the snapshot, or None."""
    if not peer:
        return None
    return {"stream": int(peer["stream"]), "verdict": peer["verdict"], "measures": nan_to_none(peer["measures"]),
            "key": meas.key_of.get(id(peer["measures"]))}


def measure(found, rule, consts, cache):
    """Every reference and candidate of every folder under one rule: (tracks, unreadable, measurer). A track is one
    stream in one folder; the same stream in several folders of a title gives several tracks."""
    meas = Measurer(cache)
    tracks, unreadable, originals = [], [], {}
    with constants(consts):
        for slug, label, folder in found:
            if slug not in originals:
                originals[slug] = original_language(folder.parent.parent)[0]
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
            problem = reference_problem(streams, main, originals[slug])
            base = {"slug": slug, "label": label, "folder": folder.name, "main": main, "ref_fc": mi.get("fc") == "yes",
                    "ref_problem": problem, "excluded": bool(problem), "secondary": False,
                    "n_cands": len(res["candidates"]), "ref_first": first_of_language(streams, main),
                    "veto": res["veto"]["why"] if res["veto"]["holds"] else ""}
            order = [(main, mi.get("md5"))] + [(int(c["stream"]), c["info"].get("md5")) for c in res["candidates"]]
            ranks = {n: sum(1 for m, h in order[:k] if h and h == md5) for k, (n, md5) in enumerate(order)}
            tracks.append(dict(base, role="reference", stream=main, md5=mi.get("md5"), md5_rank=0, codec=mi.get("codec"),
                               channels=mi.get("channels"), language=mi.get("language"), position=pos.get(main),
                               verdict=None, measures=None, key=None, not_judged="", duplicate_of=None,
                               cross_language="", swapped_verdict=None, peer=None, before_veto=None))
            for c in res["candidates"]:
                n = int(c["stream"])
                key = meas.key_of.get(id(c["measures"])) if c["measures"] is not None else None
                back = sim.swapped(c["measures"]) if c["measures"] is not None else None
                tracks.append(dict(base, role="candidate", stream=n, md5=c["info"].get("md5"), md5_rank=ranks[n],
                                   codec=c["info"].get("codec"), channels=c["info"].get("channels"),
                                   language=c["info"].get("language"), position=pos.get(n), verdict=c["verdict"],
                                   measures=nan_to_none(c["measures"]), key=key, not_judged=c["not_judged"],
                                   duplicate_of=c["duplicate_of"], cross_language=c.get("cross_language", ""),
                                   swapped_verdict=sim.verdict(back, rule) if back is not None else None,
                                   peer=peer_row(c.get("peer"), meas), before_veto=c.get("before_veto")))
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
        elif group == "programme" and (v in DROP or v in PATTERN):
            t["outcome"] = "FAIL"
        elif group in ("unwanted", "duplicate") and v not in DROP:
            t["outcome"] = "kept"
        elif group == "unwanted" and v == "commentary" and k not in labels.COMMENTARY_KINDS:
            t["outcome"] = "dropped as a commentary"
        elif group in ("ask", "unknown") and (v in DROP or v in PATTERN):
            t["outcome"] = ("undecided" if group == "ask" else "unknown") + " kind, " + \
                ("drop verdict" if v in DROP else "commentary pattern")
        else:
            t["outcome"] = "agrees"
    # A swapped reference: the same pair read the other way round. If this candidate were the reference, would the
    # title's reference (programme audio unless the owner labelled it otherwise) get the commentary verdict? Only for
    # labelled candidates: like every verdict, it is not shown before the owner has labelled the track.
    ref_kind = {(t["slug"], t["label"], t["folder"]): t["kind"] for t in tracks if t["role"] == "reference"}
    for t in tracks:
        k = ref_kind.get((t["slug"], t["label"], t["folder"]))
        programme_ref = k is None or labels.KINDS[k][0] == "programme"
        t["swap_fail"] = (t["role"] == "candidate" and not t["excluded"] and t["kind"] is not None
                          and t.get("swapped_verdict") == "commentary" and programme_ref)
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
        "programme_separate": sum(1 for t in prog if t["kind"] not in ("derived-core", "dub")),
        "no_centre_titles": len({(t["slug"], t["label"], t["main"]) for t in cnt if not t["ref_fc"]}),
        "dialogue_heavy_titles": len({(t["slug"], t["label"]) for t in cnt if title(t).get("dialogue_heavy") is True}),
        "others": {k: sum(1 for t in cnt if t["kind"] == k) for k in ("isolated-score", "music-effects", "same-language-dub", "alternate-narration")},
        "verdict_discs": len({t["slug"] for t in comm if t["verdict"] == "commentary"}),
        "dubs": sum(1 for t in cnt if t["kind"] == "dub"),
        "dub_discs": len({t["slug"] for t in cnt if t["kind"] == "dub"}),
        "dub_heavy": len({(t["slug"], t["label"]) for t in cnt if t["kind"] == "dub" and title(t).get("dialogue_heavy") is True}),
        "dub_pattern": sum(1 for t in cnt if t["kind"] == "dub" and t["verdict"] in PATTERN),
        "cross_comm": sum(1 for t in comm if t["cross_language"]),
        "cross_comm_pattern": sum(1 for t in comm if t["verdict"] == "commentary-cross-language"),
        "cross_prog": sum(1 for t in prog if t["cross_language"]),
        "swapped": sum(1 for t in judged(tracks) if t["role"] == "candidate" and t["kind"] and t.get("swapped_verdict")),
        "swap_fails": sum(1 for t in tracks if t.get("swap_fail")),
    }
    false = sorted({(t["slug"], t["label"], t["stream"]) for t in tracks if t["outcome"] == "FAIL" and t["verdict"] in PATTERN})
    missing = [name for name, have, need in (
        ("commentaries", c["commentaries"], req["commentaries"]), ("discs with a commentary", c["commentary_discs"], req["commentary_discs"]),
        ("commentaries on feature films", c["commentary_features"], req["commentary_features"]),
        ("commentaries with the film clearly audible", c["commentary_audible"], req["commentary_audible"]),
        ("programme tracks", c["programme"], req["programme"]),
        ("separately encoded programme tracks", c["programme_separate"], req["programme_separate"]),
        ("titles whose reference has no centre channel", c["no_centre_titles"], req["no_centre_titles"]),
        ("dialogue-heavy titles", c["dialogue_heavy_titles"], req["dialogue_heavy_titles"])) if have < need]
    if false:
        answer = f"no: {len(false)} programme track(s) with the commentary pattern"
    elif missing:
        answer = "not answered: too few " + ", ".join(missing)
    elif c["verdict_discs"] < req["verdict_discs"]:
        answer = f"no: commentary verdicts on {c['verdict_discs']} discs, {req['verdict_discs']} needed"
    else:
        answer = ("yes, if the audio-description and 'every one the owner has' kinds are covered (checked by hand); for "
                  "candidates in the reference's language only: one in another language is never dropped by content "
                  "(addendum 9.2)")
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


def no_audio(rip_dir):
    """True when the rip's ffprobe-streams.json lists streams and none of them is audio: the title has nothing to add to
    the tuning dataset (`args` refuses it with "the file has no audio stream"). False when it lists an audio stream, or
    is missing or cannot be read."""
    try:
        streams = json.loads((pathlib.Path(rip_dir) / "ffprobe-streams.json").read_text(encoding="utf-8-sig")).get("streams")
    except (OSError, ValueError, AttributeError):
        return False
    return isinstance(streams, list) and bool(streams) and not any(
        isinstance(s, dict) and s.get("codec_type") == "audio" for s in streams)


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
    lines += ["", f"Experiment 13 (spec section 17 row 13; tracks that count: content checked by listening before the "
                  f"verdict, the owner sure, each disc's packets once, never {req['fitting_slug']} or a disc or track in "
                  f"audio-fitted.json; the packaging, the menu, a flag and TheDiscDB are observations and do not count "
                  f"on their own, D23):",
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
              f"  cross-language        commentaries {c['cross_comm']} (commentary-cross-language {c['cross_comm_pattern']}), "
              f"programme tracks {c['cross_prog']}, judged against a reference in another language; never dropped by content "
              f"(addendum 9.2)",
              f"  dubs                  {c['dubs']}" + (f" of {req['dubs']}" if req.get("dubs") is not None else "")
              + f" from {c['dub_discs']}" + (f" of {req['dub_discs']}" if req.get("dub_discs") is not None else "")
              + f" discs; on dialogue-heavy titles {c['dub_heavy']}" + (f" of {req['dub_heavy']}" if req.get("dub_heavy") is not None else "")
              + f"; with the commentary pattern {c['dub_pattern']} (" + ("what row 13 asks before content may judge a "
              "candidate in another language" if req.get("dubs") is not None else "row 13 sets no number for dubs yet")
              + "; until then such a candidate is never dropped by content)",
              f"  swapped references    {c['swapped']} labelled candidates read the other way round; the programme would get "
              f"the commentary verdict against {c['swap_fails']} of them (addendum 9.3; no number in row 13)",
              f"  false verdicts        {len(false)} (a programme track with the commentary pattern: commentary or commentary-cross-language, on any disc)",
              f"  as it stands: {answer}",
              "  not counted: " + (", ".join(f"{s} {n}" for s, n in sorted(not_counted.items())) or "none")]
    lines += ["", "Disagreements (labelled candidates):"]
    start = len(lines)
    failures = []
    fails = [t for t in tracks if t["outcome"] == "FAIL"]
    lines += [f"  FAIL {where(t)}: labelled {t['kind']} (programme audio), verdict {t['verdict']}"
              + (f" (label of {t['inherited_from']})" if t["inherited_from"] else "") for t in fails]
    if fails:
        failures.append(f"{len(fails)} programme track(s) got a drop verdict or the commentary pattern: "
                        + "; ".join(where(t) for t in fails))
    lines += [f"  FAIL {s}/{l}/{f}: wrong reference: {p}; its tracks are left out (measure the title again with the "
              f"reference that `audio_dataset.py args` prints, then delete this folder)" for s, l, f, p in wrong]
    if wrong:
        failures.append(f"{len(wrong)} folder(s) measured against the wrong reference: " + "; ".join(f"{s}/{l}/{f}" for s, l, f, _ in wrong))
    swaps = [t for t in tracks if t.get("swap_fail")]
    lines += [f"  FAIL {where(t)}: swapped reference: labelled {t['kind']}, verdict {t['verdict']}; "
              f"with this track as the reference, the programme (stream {t['main']}) would get the commentary verdict "
              f"(EXP-13 addendum, section 9.3)" for t in swaps]
    if swaps:
        failures.append(f"{len(swaps)} swapped reference(s) would drop the programme: " + "; ".join(where(t) for t in swaps))
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
        if t["outcome"] == "dropped as a commentary" or t["outcome"].startswith(("undecided kind, ", "unknown kind, ")) \
                or t["outcome"].startswith("reference labelled"):
            lines.append(f"  note: {where(t)}: {t['outcome']} ({t['kind']}, verdict {t['verdict']})")
    for t in good:
        if t["role"] == "reference" and t.get("veto"):
            lines.append(f"  note: {t['slug']}/{t['label']}/{t['folder']}: the position veto holds (row 0, D21): {t['veto']}; "
                         f"no commentary is confirmed on its candidates: each is reference-unclear")
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
    silent = [(s, l) for s, l in titles_seen if (s, l) not in measured and (s, l) not in proxies
              and no_audio(pathlib.Path(corpus) / s / l)]
    titles_seen = [x for x in titles_seen if x not in silent]
    missing = [f"{s}/{l}" + ("" if (s, l) in measured else " (not measured)") for s, l in titles_seen if (s, l) not in proxies]
    lines += ["", f"Titles whose audio is in the tuning dataset (audio-proxy.json): {len(titles_seen) - len(missing)} of "
                  f"{len(titles_seen)} (every successful rip and every measured title)" + (f"; missing: {', '.join(missing)}" if missing else "")]
    if silent:
        lines.append("  note: no audio stream (its ffprobe-streams.json lists none), so nothing to add and not missing: "
                     + ", ".join(f"{s}/{l}" for s, l in silent))
    return lines, failures, orphans


def load_snapshot(path):
    try:
        data = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if data.get("format") == SNAPSHOT_FORMAT else None


def write_snapshot(path, corpus, tracks, unreadable, rule, consts, overrides, req, fitted, meas):
    """Writes the snapshot to path; False, and nothing written, when path already holds it with another "created"."""
    keep = ("slug", "label", "folder", "role", "stream", "main", "md5", "md5_rank", "codec", "channels", "language",
            "ref_fc", "ref_problem", "excluded", "secondary", "verdict", "not_judged", "duplicate_of", "measures", "key",
            "kind", "inherited_from", "counts", "why_not_counted", "outcome", "cross_language", "swapped_verdict",
            "swap_fail", "peer", "ref_first", "veto", "before_veto")
    rows, hidden = [], {}
    for t in tracks:
        row = dict({k: t[k] for k in keep}, owner_label=t["label_entry"])
        if t["role"] == "candidate" and t["kind"] is None:
            hidden[track_key(t)] = row["verdict"]
            row["verdict"] = row["swapped_verdict"] = row["before_veto"] = None
            if row["peer"]:
                row["peer"] = dict(row["peer"], verdict=None)
        rows.append(row)
    data = {"format": SNAPSHOT_FORMAT, "created": datetime.datetime.now().isoformat(timespec="seconds"),
            "corpus": str(corpus), "code": {"audio_similarity_sha1": CODE}, "rule": rule, "constants": consts,
            "overrides": overrides, "spec_row13": req, "fitted": sorted([list(f) for f in fitted], key=str),
            "tracks": rows, "unlabelled_verdicts_note": "verdicts of tracks the owner has not labelled: never show them "
                                                        "to the owner before the label is given",
            "unlabelled_verdicts": hidden, "unreadable": unreadable, "measures_cache": dict(sorted(meas.used.items()))}
    text = json.dumps(data, indent=1, sort_keys=False) + "\n"
    old = load_snapshot(path)
    timeless = lambda d: json.dumps(dict(d, created=None), indent=1, sort_keys=False)  # text, so NaN equals NaN
    if old is not None and timeless(old) == timeless(json.loads(text)):
        return False  # the same snapshot but for its time: the file (a committed baseline) is left as it is
    tmp = pathlib.Path(str(path) + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)
    return True


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
    """The audio-levels.ps1 job lines for a ripped title (EXP-13 addendum, section 9.1): one job with the title's
    programme reference (the stream spec 8.6 places first in the work's original language, from tmdb.json, else in
    English) against every other stream in a wanted language (English, the original language) or with no language,
    whatever their language: an English commentary on a Spanish film is judged against the Spanish main track. When no
    other stream is wanted, the job has the reference alone, for the dataset. A language that is not wanted and has two
    or more streams gets its own job, for the dataset only: its first-placed stream against the others.
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
    original, what = original_language(slug_dir)
    ref, why = programme_reference(audio, original)
    groups = [[ref] + sorted((a for a in audio if a[0] != ref[0] and (wanted(a, original) or a[3] == ref[3])),
                             key=lambda a: a[0])]
    others = {}
    for a in audio:
        if not wanted(a, original):
            others.setdefault(a[3], []).append(a)
    for lang, streams in sorted(others.items()):
        if len(streams) >= 2 and ref[0] not in [a[0] for a in streams]:
            top = placed_first(streams)
            groups.append([top] + sorted((a for a in streams if a[0] != top[0]), key=lambda a: a[0]))
    lines = [f"{slug}/{label}: {len(audio)} audio streams (from {source}); {what}"]
    lines += [f"  stream {a[0]}: {a[4]} lang={a[3]}" for a in audio]
    lines.append(f"  programme reference: stream {ref[0]} ({ref[4]}), placed first in {why} (spec 8.6 steps 3 and 4); "
                 f"candidates: every other stream in {', '.join(['eng'] + list(original))} or with no language, "
                 f"whatever its language")
    if original and why != "the original language":
        lines.append(f"  note: the file has no stream in the original language ({'/'.join(original)}): the reference may "
                     f"not be the programme (an English-only rip of a foreign film can hold only a commentary)")
    first = sim.first_of_language(audio, ref[0])  # not a lossy stereo first stream that G1 or G2 passed over
    if first is not None:
        lines.append(f"  note: the reference is not the first {ref[3]} stream of the file (stream {first[0]}, {first[4]}, "
                     f"comes first): the position veto holds (row 0, D21), so no candidate of this title gets a "
                     f"commentary verdict (each is reference-unclear); measure as printed and say so in the report")
    louder = [a for a in audio if a[3] in NO_LANGUAGE and a[0] != ref[0] and (a[2] or a[1] > 2)]
    if not ref[2] and ref[1] <= 2 and louder:
        lines.append(f"  note: the reference is a lossy stream of {ref[1]} channel(s), and stream "
                     f"{', '.join(str(a[0]) for a in louder)} with no language code is lossless or has more than 2 "
                     f"channels: if it is the programme, the reference may be a commentary (EXP-13 addendum 9.1); "
                     f"measure as printed and say so in the report")
    for g in groups[1:]:
        lines.append(f"  dataset only: {g[0][3]} is not a wanted language; stream {g[0][0]} against "
                     f"{','.join(str(a[0]) for a in g[1:])}")
    for g in groups:
        main, cands = g[0][0], [a[0] for a in g[1:]]
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
    written = write_snapshot(out, corpus, tracks, unreadable, dict(sim.RULE, **rule_over), effective,
                             {"rule": rule_over, "constants": const_over}, req, fitted, meas)
    lines += ["", f"snapshot: {out}{why}" + ("" if written else " (unchanged but for its time: not rewritten)")]
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
