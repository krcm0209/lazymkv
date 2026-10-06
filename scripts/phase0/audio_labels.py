"""The owner's labels for the audio tracks of one disc (decision D16): corpus/owner/<slug>/audio-labels.json.

    audio_labels.py kinds                                   the vocabulary: kinds, what each is, what the owner wants
    audio_labels.py check corpus/owner[/<slug>] ...         validate every audio-labels.json (and audio-fitted.json)
    audio_labels.py set corpus/owner/<slug> <label> <stream> <kind> --how <how> --saw-verdict yes|no
                        [--sure yes|no] [--date YYYY-MM-DD] [--film-audible yes|no|unknown] [--note TEXT]
    audio_labels.py unset corpus/owner/<slug> <label> <stream> --note TEXT
    audio_labels.py title corpus/owner/<slug> <label> --title-kind feature|episode|extra|other
                        [--dialogue-heavy yes|no|unknown] [--note TEXT]
    audio_labels.py auto corpus/owner/<slug>                label, without listening, every measured track that is an
                                                            exact duplicate (same packet MD5) or a derived core (scan
                                                            flag 2048); existing labels are never changed
    audio_labels.py fitted corpus/owner <slug> [<label> <stream>] --rule TEXT --note TEXT
                                                            record that a threshold was fitted on a disc or a track

A track is (rip label, ffprobe stream number in that rip). `set` adds or replaces one track's label; the label it
replaces is kept in the new entry's "history". `unset` moves a label to the file's "removed" list. `title` sets one
title's facts. Each command writes the whole file in one step, and only when the change adds no problem: a file that
has become invalid can be repaired with them, one entry at a time.

Whether a label counts towards experiment 13 (spec section 17 row 13) is not stored: audio_dataset.py works it out
each time from the label and from the discs and tracks the thresholds were fitted on, so a later fit never leaves a
stale flag behind. A label counts when the owner named the track from listening, the packaging, the menu, a flag or
TheDiscDB, before seeing the rule's verdict, and is sure; when it is not main, a duplicate or unknown; and when
neither its disc nor the track is one the thresholds were fitted on: the disc row 13 names, and every disc or track
in corpus/owner/audio-fitted.json. --spec <file> names the spec (default: the repository's).

The labels file:
  {"format": "lazymkv audio labels v1", "slug": "<slug>",
   "titles": {"<label>": {"title_kind": "feature|episode|extra|other", "dialogue_heavy": true|false|null, "note": ""}},
   "tracks": [{"label": "<label>", "stream": <n>, "kind": "<kind>", "how": "<how>", "sure": true|false,
               "date": "YYYY-MM-DD", "saw_verdict": false, "note": "...",
               "film_audible": true|false|null (commentary kinds only),
               "history": [<an earlier label of this track>, with "replaced": "YYYY-MM-DD"] (when it was replaced)}],
   "removed": [<a label taken away by unset, with its history if it had one>, with "removed": "YYYY-MM-DD" and
               "why": "..."] (optional)}
The fitted file, corpus/owner/audio-fitted.json:
  {"format": "lazymkv audio fitted v1",
   "fitted": [{"slug": "<slug>", "label": "<label>" or null (the whole disc), "stream": <n> or null,
               "date": "YYYY-MM-DD", "rule": "what was fitted (a commit or the audio_similarity.py SHA-1)", "note": ""}]}
"""
import argparse
import datetime
import json
import os
import pathlib
import re
import sys

from audio_similarity import read_manifest
from robot import parse

FORMAT = "lazymkv audio labels v1"
FITTED_FORMAT = "lazymkv audio fitted v1"
SLUG = re.compile(r"^[a-z0-9][a-z0-9-]*$")
DERIVED = 2048  # scan flag (attribute 22): a stream MakeMKV derived from another, such as a DTS core
SPEC = pathlib.Path(__file__).resolve().parents[2] / "docs" / "superpowers" / "specs" / "2026-10-04-lazymkv-design.md"

# kind: (group, what the owner wants in the final file, meaning). Groups: programme audio (a drop verdict on it is a
# failure), a duplicate (dropped by MD5, D13), unwanted (dropped when confirmed: R6, D12, D14), ask (the owner has not
# decided: EXP-13 questions 3 to 6), unknown. When the owner answers one of those questions, the controller records the
# decision in the spec and moves the kind here in the same commit: to "programme"/"keep" (a drop verdict on it then
# fails the harness) or to "unwanted"/"drop".
KINDS = {
    "main": ("programme", "keep", "the programme's main soundtrack; as a reference it is never judged"),
    "lossy-companion": ("programme", "keep", "a separately encoded copy of the programme mix (AC-3/AAC/DTS companion, 2.0 downmix)"),
    "alternate-mix": ("programme", "keep", "another mix or master of the programme (original or broadcast mix, remaster, night mix)"),
    "laugh-track-version": ("programme", "keep", "the programme with or without its laugh track"),
    "dub": ("programme", "keep", "the programme in another language, tagged with it (an English dub of a Spanish film); kept (R3, R4); "
                                "content never drops a candidate in another language than the reference (EXP-13 addendum 9.2)"),
    "derived-core": ("programme", "keep", "a stream MakeMKV derived from another (scan flag 2048); the app drops it by that flag, never by content"),
    "duplicate": ("duplicate", "drop", "an exact copy of another track of the title (same packet MD5); one copy is kept (D13)"),
    "commentary": ("unwanted", "drop", "an audio commentary (R6, D12)"),
    "partial-commentary": ("unwanted", "drop", "a commentary over only some scenes"),
    "audio-description": ("unwanted", "drop", "audio description (R6)"),
    "isolated-score": ("unwanted", "drop", "the score alone (D14)"),
    "music-effects": ("unwanted", "drop", "music and effects without dialogue (D14)"),
    "same-language-dub": ("ask", "ask", "a second dub or alternate dialogue in the same language (EXP-13 question 3)"),
    "alternate-narration": ("ask", "ask", "a second narrator (EXP-13 question 3)"),
    "foreign-dub": ("ask", "ask", "a dub in another language than its tag says, tagged with a wanted one (EXP-13 question 4)"),
    "other-speech": ("ask", "ask", "trivia, interview or picture-in-picture speech (EXP-13 question 5)"),
    "sing-along": ("ask", "ask", "a sing-along or karaoke track (EXP-13 question 6)"),
    "unknown": ("unknown", "unknown", "the owner does not know"),
}
HOW = {
    "listened": "the owner listened to it",
    "packaging": "the packaging says so",
    "menu": "the disc's menu says so",
    "flag": "a flag in MakeMKV's scan",
    "thediscdb": "TheDiscDB's track description",
    "md5": "identical packet MD5 (an exact duplicate; no listening needed)",
    "inferred": "inferred, for example from the disc's layout; never counts",
}
COUNTING_HOW = ("listened", "packaging", "menu", "flag", "thediscdb")  # spec section 17, row 13
NOT_COUNTED = ("main", "duplicate", "unknown")
COMMENTARY_KINDS = ("commentary", "partial-commentary")
TITLE_KINDS = ("feature", "episode", "extra", "other")
TRACK_KEYS = {"label", "stream", "kind", "how", "sure", "date", "saw_verdict", "note"}
OPTIONAL_KEYS = {"film_audible", "history"}
SUBSTANCE = ("kind", "how", "sure", "saw_verdict", "film_audible", "note")
WORDS = {"one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7, "eight": 8, "nine": 9, "ten": 10}


def exp13_requirements(spec_text):
    """Experiment 13's totals and the fitting disc, read from row 13 of the spec's section 17. ValueError names the
    phrase that was not found, so a reworded row is noticed instead of silently counted against old numbers."""
    row = next((line for line in spec_text.splitlines() if line.startswith("| 13 |")), None)
    if row is None:
        raise ValueError("spec section 17 has no row starting '| 13 |'")
    num = r"(\d+|one|two|three|four|five|six|seven|eight|nine|ten)"
    phrases = {
        ("commentaries", "commentary_discs", "commentary_features", "commentary_audible"):
            rf"at least {num} commentaries from at least {num} discs, at least {num} of them on feature films and at least {num} with the film clearly audible",
        ("ad_min", "ad_max"): rf"at least {num} audio-description track if the owner has a disc with one \(all that the owner has, up to {num}\)",
        ("programme", "programme_separate"): rf"at least {num} programme tracks other than the reference, at least {num} of them separately encoded",
        ("no_centre_titles", "dialogue_heavy_titles"): rf"at least {num} title whose reference has no centre channel .{{0,40}}?and at least {num} dialogue-heavy or speech-only title",
        ("verdict_discs",): rf"at least one commentary on each of at least {num} different discs",
    }
    out = {}
    for keys, pattern in phrases.items():
        m = re.search(pattern, row)
        if not m:
            raise ValueError(f"spec section 17 row 13 no longer contains the phrase /{pattern}/; update audio_labels.py")
        for k, v in zip(keys, m.groups()):
            out[k] = int(v) if v.isdigit() else WORDS[v]
    m = re.search(r"Every threshold was fitted on .{0,40}?\(`([a-z0-9-]+)`", row)
    if not m:
        raise ValueError("spec section 17 row 13 no longer names the disc the thresholds were fitted on")
    out["fitting_slug"] = m.group(1)
    # Optional (EXP-13 addendum 9.5): the dubs a yes needs before content may judge a candidate in another language
    # than the reference. None while row 13 does not have the phrase: such a candidate then stays keep-only.
    m = re.search(rf"at least {num} dubs in a wanted language from at least {num} discs, at least {num} of them on a "
                  rf"dialogue-heavy title", row)
    for k, v in zip(("dubs", "dub_discs", "dub_heavy"), m.groups() if m else (None, None, None)):
        out[k] = None if v is None else int(v) if v.isdigit() else WORDS[v]
    return out


def fitting_slug(spec=SPEC):
    return exp13_requirements(pathlib.Path(spec).read_text(encoding="utf-8-sig"))["fitting_slug"]


def date_ok(text):
    try:
        return bool(re.fullmatch(r"\d{4}-\d\d-\d\d", str(text))) and bool(datetime.date.fromisoformat(text))
    except ValueError:
        return False


def fitted_problems(data):
    """Every problem of a parsed audio-fitted.json; [] when it is valid."""
    if not isinstance(data, dict):
        return ["the file is not a JSON object"]
    out = []
    if set(data) - {"format", "fitted"}:
        out.append(f"unknown top-level keys {sorted(set(data) - {'format', 'fitted'})}")
    if data.get("format") != FITTED_FORMAT:
        out.append(f"format is {data.get('format')!r}, not {FITTED_FORMAT!r}")
    if not isinstance(data.get("fitted"), list):
        return out + ["fitted is not a list"]
    for k, f in enumerate(data["fitted"]):
        where = f"entry {k}"
        if not isinstance(f, dict):
            out.append(f"{where}: not an object")
            continue
        keys = {"slug", "label", "stream", "date", "rule", "note"}
        if set(f) != keys:
            out.append(f"{where}: keys must be {sorted(keys)}, not {sorted(f)}")
        if not isinstance(f.get("slug"), str) or not SLUG.match(f.get("slug", "")):
            out.append(f"{where}: slug {f.get('slug')!r} is not a disc slug")
        if f.get("label") is None:
            if f.get("stream") is not None:
                out.append(f"{where}: a stream needs a label")
        elif not isinstance(f.get("label"), str) or not SLUG.match(f["label"]):
            out.append(f"{where}: label {f.get('label')!r} is not a rip label")
        elif not isinstance(f.get("stream"), int) or isinstance(f.get("stream"), bool) or f["stream"] < 0:
            out.append(f"{where}: a label needs ffprobe's stream number")
        if not date_ok(f.get("date")):
            out.append(f"{where}: date {f.get('date')!r} is not YYYY-MM-DD")
        for key in ("rule", "note"):
            if not isinstance(f.get(key), str):
                out.append(f"{where}: {key} is not text")
    return out


def load_fitted(corpus, fitting=None):
    """The discs and tracks the thresholds were fitted on: {(slug, label, stream)}, with label and stream None for a
    whole disc. Row 13's disc (fitting) is always in it. ValueError when audio-fitted.json does not validate."""
    out = {(fitting, None, None)} if fitting else set()
    path = pathlib.Path(corpus) / "audio-fitted.json"
    if not path.exists():
        return out
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except ValueError as e:
        raise ValueError(f"{path}: not valid JSON ({e})")
    found = fitted_problems(data)
    if found:
        raise ValueError(f"{path}: " + "; ".join(found))
    return out | {(f["slug"], f["label"], f["stream"]) for f in data["fitted"]}


def is_fitted(fitted, slug, label, stream):
    return (slug, None, None) in fitted or (slug, label, stream) in fitted


def not_counted_because(entry, slug, fitted):
    """'' when the label counts towards experiment 13 (spec section 17 row 13), otherwise why it does not."""
    if entry["kind"] in NOT_COUNTED:
        return f"a {entry['kind']} track never counts"
    if entry["how"] not in COUNTING_HOW:
        return f"'{entry['how']}' is not a source row 13 accepts"
    if entry["saw_verdict"] is not False:
        return "the owner saw the verdict first"
    if entry["sure"] is not True:
        return "the owner is not sure"
    if is_fitted(fitted, slug, entry["label"], entry["stream"]):
        return "the thresholds were fitted on it: spec row 13 or audio-fitted.json"
    return ""


def entry_problems(t, where, strict=True):
    """The problems of one track entry. strict=False (history and removed entries) checks only that the entry can be
    read: its kind and how may come from an older vocabulary."""
    out = []
    keys = TRACK_KEYS | (OPTIONAL_KEYS if strict else {"film_audible", "history", "replaced", "removed", "why"})
    missing, unknown = TRACK_KEYS - set(t), set(t) - keys
    if missing:
        out.append(f"{where}: missing {sorted(missing)}")
    if unknown:
        out.append(f"{where}: unknown keys {sorted(unknown)}")
    if not isinstance(t.get("label"), str) or not SLUG.match(t.get("label", "")):
        out.append(f"{where}: label is not a rip label")
    if not isinstance(t.get("stream"), int) or isinstance(t.get("stream"), bool) or t.get("stream", -1) < 0:
        out.append(f"{where}: stream is not ffprobe's stream number (an integer, 0 or more)")
    if not date_ok(t.get("date")):
        out.append(f"{where}: date {t.get('date')!r} is not YYYY-MM-DD")
    if not strict:
        return out
    if t.get("kind") not in KINDS:
        out.append(f"{where}: kind {t.get('kind')!r} is not in the vocabulary (audio_labels.py kinds)")
    if t.get("how") not in HOW:
        out.append(f"{where}: how {t.get('how')!r} is not one of {', '.join(HOW)}")
    if t.get("how") == "md5" and t.get("kind") != "duplicate":
        out.append(f"{where}: how md5 only says that a track is a duplicate")
    for key in ("saw_verdict", "sure"):
        if not isinstance(t.get(key), bool):
            out.append(f"{where}: {key} must be true or false")
    if not isinstance(t.get("note", ""), str):
        out.append(f"{where}: note is not text")
    if "film_audible" in t:
        if t.get("kind") not in COMMENTARY_KINDS:
            out.append(f"{where}: film_audible is only for {' and '.join(COMMENTARY_KINDS)}")
        elif t["film_audible"] not in (True, False, None):
            out.append(f"{where}: film_audible must be true, false or null")
    history = t.get("history", [])
    if not isinstance(history, list):
        out.append(f"{where}: history is not a list")
    else:
        for k, h in enumerate(history):
            if not isinstance(h, dict):
                out.append(f"{where}: history {k} is not an object")
                continue
            out += entry_problems(h, f"{where} history {k}", strict=False)
            if not date_ok(h.get("replaced")):
                out.append(f"{where} history {k}: replaced {h.get('replaced')!r} is not YYYY-MM-DD")
    return out


def problems(data, slug=None):
    """Every problem of a parsed labels file, one sentence each; [] when it is valid."""
    out = []
    if not isinstance(data, dict):
        return ["the file is not a JSON object"]
    extra = set(data) - {"format", "slug", "titles", "tracks", "removed"}
    if extra:
        out.append(f"unknown top-level keys {sorted(extra)}")
    if data.get("format") != FORMAT:
        out.append(f"format is {data.get('format')!r}, not {FORMAT!r}")
    if not isinstance(data.get("slug"), str) or not SLUG.match(data.get("slug", "")):
        out.append(f"slug {data.get('slug')!r} is not lowercase letters, digits and hyphens")
    elif slug is not None and data["slug"] != slug:
        out.append(f"slug is {data['slug']!r} but the file is in corpus/owner/{slug}")
    titles = data.get("titles")
    if not isinstance(titles, dict):
        out.append("titles is not an object")
        titles = {}
    for label, t in titles.items():
        where = f"title {label}"
        if not SLUG.match(label):
            out.append(f"{where}: not a rip label")
        if not isinstance(t, dict):
            out.append(f"{where}: not an object")
            continue
        if set(t) - {"title_kind", "dialogue_heavy", "note"}:
            out.append(f"{where}: unknown keys {sorted(set(t) - {'title_kind', 'dialogue_heavy', 'note'})}")
        if t.get("title_kind") not in TITLE_KINDS:
            out.append(f"{where}: title_kind {t.get('title_kind')!r} is not one of {', '.join(TITLE_KINDS)}")
        if t.get("dialogue_heavy") not in (True, False, None):
            out.append(f"{where}: dialogue_heavy must be true, false or null")
        if not isinstance(t.get("note", ""), str):
            out.append(f"{where}: note is not text")
    removed = data.get("removed", [])
    if not isinstance(removed, list):
        out.append("removed is not a list")
    else:
        for k, r in enumerate(removed):
            if not isinstance(r, dict):
                out.append(f"removed {k}: not an object")
                continue
            out += entry_problems(r, f"removed {k}", strict=False)
            if not date_ok(r.get("removed")):
                out.append(f"removed {k}: removed {r.get('removed')!r} is not YYYY-MM-DD")
    tracks = data.get("tracks")
    if not isinstance(tracks, list):
        return out + ["tracks is not a list"]
    seen = set()
    for k, t in enumerate(tracks):
        if not isinstance(t, dict):
            out.append(f"track {k}: not an object")
            continue
        where = f"track {t.get('label')} stream {t.get('stream')}"
        out += entry_problems(t, where)
        if (t.get("label"), t.get("stream")) in seen:
            out.append(f"{where}: labelled twice")
        seen.add((t.get("label"), t.get("stream")))
    return out


def load(path):
    """The parsed file at path; ValueError listing every problem."""
    path = pathlib.Path(path)
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except ValueError as e:
        raise ValueError(f"{path}: not valid JSON ({e})")
    found = problems(data, path.parent.name)
    if found:
        raise ValueError(f"{path}: " + "; ".join(found))
    return data


def write_json(path, data, indent):
    tmp = pathlib.Path(str(path) + ".tmp")
    tmp.write_text(json.dumps(data, indent=indent, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def save(path, data, before=()):
    """Write the file when the change adds no problem to those it had before (an invalid file can be repaired one
    entry at a time); returns the problems still left. ValueError names the new ones, and nothing is written."""
    found = problems(data, pathlib.Path(path).parent.name)
    new = [p for p in found if p not in set(before)]
    if new:
        raise ValueError(f"{path}: not written: " + "; ".join(new))
    data["tracks"].sort(key=lambda t: (str(t.get("label")), t.get("stream") if isinstance(t.get("stream"), int) else -1)
                        if isinstance(t, dict) else ("", -1))
    data["titles"] = dict(sorted(data["titles"].items()))
    write_json(path, data, 2)
    return found


def open_for_change(slug_dir):
    """(path, data, problems it has now): a file that does not validate can still be changed, but it must be JSON
    with the three main parts, and the change may not add a problem."""
    path = pathlib.Path(slug_dir) / "audio-labels.json"
    if not pathlib.Path(slug_dir).is_dir():
        raise ValueError(f"{slug_dir}: no such recording folder")
    if not path.exists():
        return path, {"format": FORMAT, "slug": pathlib.Path(slug_dir).resolve().name, "titles": {}, "tracks": []}, []
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except ValueError as e:
        raise ValueError(f"{path}: not valid JSON ({e}); restore it from git (git checkout -- {path})")
    if not isinstance(data, dict) or not isinstance(data.get("tracks"), list) or not isinstance(data.get("titles"), dict):
        raise ValueError(f"{path}: has no 'tracks' list or 'titles' object; restore it from git (git checkout -- {path})")
    return path, data, problems(data, path.parent.name)


def corpus_fitted(slug_dir, fitting):
    return load_fitted(pathlib.Path(slug_dir).resolve().parent, fitting)


def set_track(slug_dir, label, stream, kind, how, saw_verdict, date, sure=True, film_audible="absent", note="",
              fitting=None):
    """Add or replace the label of (label, stream); a replaced label goes into the new entry's history.
    Returns (entry written or kept, whether it changed, why it does not count or '', problems left in the file)."""
    path, data, before = open_for_change(slug_dir)
    entry = {"label": label, "stream": stream, "kind": kind, "how": how, "sure": sure, "date": date,
             "saw_verdict": saw_verdict, "note": note}
    if film_audible != "absent":
        entry["film_audible"] = film_audible
    elif kind in COMMENTARY_KINDS:
        entry["film_audible"] = None
    old = [t for t in data["tracks"] if isinstance(t, dict) and (t.get("label"), t.get("stream")) == (label, stream)]
    changed = True
    if len(old) == 1 and all(old[0].get(k, "absent") == entry.get(k, "absent") for k in SUBSTANCE):
        entry, changed = old[0], False
    elif old:
        history = []
        for o in old:
            history += o.get("history", []) if isinstance(o.get("history"), list) else []
            history.append(dict({k: v for k, v in o.items() if k != "history"}, replaced=date))
        entry["history"] = history
    left = before
    if changed:
        data["tracks"] = [t for t in data["tracks"] if not any(t is o for o in old)] + [entry]
        left = save(path, data, before)
    why = not_counted_because(entry, data["slug"], corpus_fitted(slug_dir, fitting)) if not entry_problems(entry, "") else "invalid"
    return entry, changed, why, left


def unset_track(slug_dir, label, stream, why, date):
    """Move the label of (label, stream) to the file's "removed" list. Returns the problems left in the file."""
    path, data, before = open_for_change(slug_dir)
    old = [t for t in data["tracks"] if isinstance(t, dict) and (t.get("label"), t.get("stream")) == (label, stream)]
    if not old:
        raise ValueError(f"{path}: {label} stream {stream} has no label")
    if not why:
        raise ValueError("unset needs --note: why the label is taken away")
    data["tracks"] = [t for t in data["tracks"] if not any(t is o for o in old)]
    data.setdefault("removed", []).extend(dict(o, removed=date, why=why) for o in old)
    return save(path, data, before)


def set_title(slug_dir, label, title_kind, dialogue_heavy=None, note=""):
    path, data, before = open_for_change(slug_dir)
    data["titles"][label] = {"title_kind": title_kind, "dialogue_heavy": dialogue_heavy, "note": note}
    left = save(path, data, before)
    return data["titles"][label], left


def add_fitted(corpus, slug, label, stream, rule, note, date):
    """Append one entry to corpus/audio-fitted.json (created when missing). Returns the entry."""
    path = pathlib.Path(corpus) / "audio-fitted.json"
    if not pathlib.Path(corpus).is_dir():
        raise ValueError(f"{corpus}: no such folder")
    data = json.loads(path.read_text(encoding="utf-8-sig")) if path.exists() else {"format": FITTED_FORMAT, "fitted": []}
    before = fitted_problems(data)
    if before:
        raise ValueError(f"{path}: does not validate, so nothing is added: " + "; ".join(before))
    entry = {"slug": slug, "label": label, "stream": stream, "date": date, "rule": rule, "note": note}
    data["fitted"].append(entry)
    found = fitted_problems(data)
    if found:
        raise ValueError(f"{path}: not written: " + "; ".join(found))
    write_json(path, data, 2)
    return entry


def _scan_flags(slug_dir, label):
    """{ffprobe stream number: scan flags} for the audio streams of a rip, matching the k-th audio stream of the file
    with the k-th of the scan's title (as Task 12 Step 15 does); None with the reason when they cannot be matched."""
    d = pathlib.Path(slug_dir)
    try:
        title = json.loads((d / label / "rip-summary.json").read_text(encoding="utf-8-sig"))["title"]
        scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
    except (OSError, ValueError, KeyError) as e:
        return None, f"no scan or rip summary ({e})"
    flags = [int(a.get(22, "0") or 0) for (t, s), a in sorted(scan.streams.items()) if t == title and a.get(1) == "Audio"]
    streams = None
    for folder in sorted((d / label).glob("audio-levels-20ms-*/streams.json")):
        streams = json.loads(folder.read_text(encoding="utf-8-sig")).get("streams", [])
        break
    if streams is None:
        return None, "no streams.json"
    audio = [s["index"] for s in streams if s.get("codec_type") == "audio"]
    if len(audio) != len(flags):
        return None, f"the file has {len(audio)} audio streams and the scan's title {title} has {len(flags)}"
    return dict(zip(audio, flags)), ""


def auto(slug_dir, date, fitting=None):
    """Label every measured candidate that needs no listening: an exact duplicate (its packet MD5 equals the main
    stream's or an earlier candidate's) and a derived core (scan flag 2048). Returns the lines to print."""
    d = pathlib.Path(slug_dir)
    path, data, _ = open_for_change(d)
    have = {(t.get("label"), t.get("stream")) for t in data["tracks"] if isinstance(t, dict)}
    lines = []
    for label_dir in sorted(p for p in d.iterdir() if p.is_dir()):
        folders = sorted(f for f in label_dir.glob("audio-levels-20ms-*") if (f / "manifest.txt").is_file())
        if not folders:
            continue
        flags, why = _scan_flags(d, label_dir.name)
        if flags is None:
            lines.append(f"{label_dir.name}: derived cores not labelled: {why}")
        for folder in folders:
            man = read_manifest(folder)
            if not man["main"]:
                continue
            seen = {man["main"].get("md5"): man["main"].get("main")}
            for c in man["cands"]:
                n, md5 = int(c["cand"]), c.get("md5")
                kind = how = note = None
                if md5 in seen:
                    kind, how, note = "duplicate", "md5", f"auto: same packets as stream {seen[md5]} ({md5})"
                elif flags and flags.get(n, 0) & DERIVED:
                    kind, how, note = "derived-core", "flag", f"auto: scan flags {flags[n]} (2048: derived by MakeMKV)"
                seen.setdefault(md5, c["cand"])
                if kind and (label_dir.name, n) not in have:
                    e, _, why_not, _ = set_track(d, label_dir.name, n, kind, how, False, date, note=note, fitting=fitting)
                    have.add((label_dir.name, n))
                    lines.append(f"labelled {label_dir.name} stream {n}: {e['kind']} ({e['how']}), "
                                 f"counts={not why_not}" + (f" ({why_not})" if why_not else ""))
    return lines or ["nothing to label automatically"]


def _yes(text, allow_unknown=False):
    t = text.lower()
    if t in ("yes", "no") or (allow_unknown and t == "unknown"):
        return {"yes": True, "no": False, "unknown": None}[t]
    raise argparse.ArgumentTypeError(f"{text!r}: say yes or no" + (" or unknown" if allow_unknown else ""))


def left_lines(left):
    return [f"warning: the file still has {len(left)} problem(s): " + "; ".join(left)] if left else []


def main(argv):
    p = argparse.ArgumentParser(prog="audio_labels.py", description="The owner's audio labels (corpus/owner/<slug>/audio-labels.json).")
    p.add_argument("--spec", default=str(SPEC), help="the spec whose section 17 row 13 names the fitting disc")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("kinds")
    c = sub.add_parser("check")
    c.add_argument("paths", nargs="+")
    today = datetime.date.today().isoformat()
    s = sub.add_parser("set")
    s.add_argument("slug_dir")
    s.add_argument("label")
    s.add_argument("stream", type=int)
    s.add_argument("kind", choices=list(KINDS))
    s.add_argument("--how", required=True, choices=list(HOW))
    s.add_argument("--saw-verdict", required=True, type=_yes, help="did the owner see the rule's verdict before answering")
    s.add_argument("--sure", type=_yes, default=True, help="is the owner sure (default yes); a guess never counts")
    s.add_argument("--date", default=today)
    s.add_argument("--film-audible", type=lambda x: _yes(x, True), default=argparse.SUPPRESS,
                   help="commentary kinds: is the film clearly audible under the voices")
    s.add_argument("--note", default="")
    u = sub.add_parser("unset")
    u.add_argument("slug_dir")
    u.add_argument("label")
    u.add_argument("stream", type=int)
    u.add_argument("--note", required=True, help="why the label is taken away")
    u.add_argument("--date", default=today)
    t = sub.add_parser("title")
    t.add_argument("slug_dir")
    t.add_argument("label")
    t.add_argument("--title-kind", required=True, choices=TITLE_KINDS)
    t.add_argument("--dialogue-heavy", type=lambda x: _yes(x, True), default=None)
    t.add_argument("--note", default="")
    a = sub.add_parser("auto")
    a.add_argument("slug_dir")
    a.add_argument("--date", default=today)
    f = sub.add_parser("fitted")
    f.add_argument("corpus")
    f.add_argument("slug")
    f.add_argument("label", nargs="?")
    f.add_argument("stream", nargs="?", type=int)
    f.add_argument("--rule", required=True, help="what was fitted: a commit, or the SHA-1 of audio_similarity.py")
    f.add_argument("--note", default="")
    f.add_argument("--date", default=today)
    args = p.parse_args(argv)
    if args.cmd == "kinds":
        for k, (group, wanted, meaning) in KINDS.items():
            print(f"{k:20} {group:10} {wanted:8} {meaning}")
        print("how: " + "; ".join(f"{k} = {v}" for k, v in HOW.items()))
        return 0
    try:
        fitting = fitting_slug(args.spec)
    except (OSError, ValueError) as e:
        print(f"cannot read the fitting disc from {args.spec}: {e}", file=sys.stderr)
        return 2
    try:
        if args.cmd == "check":
            bad = 0
            for root in args.paths:
                root = pathlib.Path(root)
                files = [root / "audio-labels.json"] if (root / "audio-labels.json").exists() else sorted(root.glob("*/audio-labels.json"))
                if not files:
                    print(f"{root}: no audio-labels.json")
                for path in files:
                    try:
                        data = load(path)
                        print(f"ok {path} ({len(data['tracks'])} tracks, {len(data['titles'])} titles)")
                    except ValueError as e:
                        print(e)
                        bad += 1
                fitted_path = root / "audio-fitted.json"
                if fitted_path.exists():
                    try:
                        print(f"ok {fitted_path} ({len(load_fitted(root))} entries)")
                    except ValueError as e:
                        print(e)
                        bad += 1
            return 2 if bad else 0
        if args.cmd == "set":
            e, changed, why, left = set_track(args.slug_dir, args.label, args.stream, args.kind, args.how, args.saw_verdict,
                                              args.date, args.sure, getattr(args, "film_audible", "absent"), args.note, fitting)
            print(("labelled" if changed else "unchanged") + f" {e['label']} stream {e['stream']}: {e['kind']} ({e['how']}, "
                  f"{e['date']}, saw verdict {'yes' if e['saw_verdict'] else 'no'}, sure {'yes' if e['sure'] else 'no'}), "
                  f"counts={not why}" + (f" ({why})" if why else "")
                  + (f"; replaces {len(e['history'])} earlier label(s), kept in its history" if changed and e.get("history") else ""))
            for line in left_lines(left):
                print(line)
        elif args.cmd == "unset":
            left = unset_track(args.slug_dir, args.label, args.stream, args.note, args.date)
            print(f"unset {args.label} stream {args.stream}: the label is kept under 'removed'")
            for line in left_lines(left):
                print(line)
        elif args.cmd == "title":
            e, left = set_title(args.slug_dir, args.label, args.title_kind, args.dialogue_heavy, args.note)
            print(f"title {args.label}: {e['title_kind']}, dialogue_heavy={e['dialogue_heavy']}")
            for line in left_lines(left):
                print(line)
        elif args.cmd == "auto":
            for line in auto(args.slug_dir, args.date, fitting):
                print(line)
        elif args.cmd == "fitted":
            if (args.label is None) != (args.stream is None):
                raise ValueError("fitted: give both <label> and <stream>, or neither (the whole disc)")
            e = add_fitted(args.corpus, args.slug, args.label, args.stream, args.rule, args.note, args.date)
            what = e["slug"] + (f"/{e['label']} stream {e['stream']}" if e["label"] else " (the whole disc)")
            print(f"fitted: {what}: no longer counts towards experiment 13")
    except ValueError as e:
        print(e, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
