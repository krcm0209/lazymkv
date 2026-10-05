"""Experiment 13: is an extra audio track a commentary? Judged from its content against the main track.

    audio_similarity.py <audio-levels-20ms folder> ...     the measures and the verdict of every candidate
    audio_similarity.py --titles corpus/owner/<slug>       which titles of a recorded scan have two or more audio
                                                           streams in one language (derived streams left out)

Reads a folder written by audio-levels.ps1 (audio-levels-20ms-<main>-<candidates>): manifest.txt, main-mix.txt,
main-fc.txt (only when the main track has a centre channel) and cand-<stream>.txt, each holding one speech-band
(300-3400 Hz) RMS level in dB per 20 ms frame. For every candidate it prints the measures and the verdict of the
rule in docs/phase0/experiments/EXP-13-design.md (section 3):

  DC     dialogue carried: share of the main's 2 s dialogue windows in which the candidate's level follows the main's
  AG     added in the gaps: share of the main's dialogue-gap time in which the candidate carries speech-level sound
         that the main does not explain (10 dB above the level the main predicts, in a window that does not follow)
  tenths of the tenths of the running time that hold 10 s of dialogue gaps, in how many AG reaches 0.40
  SMI    syllabic modulation of the added sound (share of envelope energy at 2.3-7.8 Hz)
  LS     the candidate's level spread, 95th minus 50th percentile (small for dense talk)
  ADL    added sound while the main's dialogue runs

Stereo width, codec and track position are not used; the main track's channel layout only decides whether its
centre channel is the dialogue curve. A folder that audio-levels.ps1 did not write in this form is refused, with a
one-line message and a non-zero exit, before any number is printed: the first measurement's folders
(audio-levels-<main>-<cand>, 100 ms frames, full band, no manifest) must be measured again with the new script.
"""
import cmath
import json
import math
import pathlib
import statistics
import sys

from robot import duration_seconds, parse

FLOOR = -100.0          # dB; digital silence
FRAME_MS = 20           # one level per 20 ms ...
FRAME_S = 0.02          # ... in seconds
MANIFEST_LINE = "# lazymkv audio-levels v2"   # first line of the manifest.txt that audio-levels.ps1 writes
DERIVED = 2048          # scan flag (attribute 22): a stream MakeMKV derived from another, such as a DTS core
WIN = 100               # 2 s windows
BLOCK = 3000            # 60 s blocks for the lag
COARSE_LAG = 100        # +-10 s, in 100 ms steps
BLOCK_LAG = 100         # +-2 s around the global lag, per block
WIN_LAG = 5             # +-100 ms per window
MATCH_R = 0.70          # a window whose levels correlate at least this much follows the main
MISMATCH_R = 0.40       # added sound must sit in a window that correlates less than this
X_ACTIVE_BELOW = 20.0   # candidate active: within 20 dB of its own 95th percentile
DLG_BELOW = 25.0        # dialogue frame: dialogue reference within 25 dB of its own 95th percentile
GAP_MIN = 25            # a dialogue gap lasts at least 0.5 s ...
GAP_TRIM = 5            # ... and loses 100 ms at each end
ADDED_DB = 10.0         # added: at least 10 dB above the level the main predicts
SILENT_DB = -90.0       # the main is digitally silent at or below this level
N_SMI = 64              # 1.28 s modulation windows

# The thresholds of the decision rule. Every one is PROVISIONAL (docs/phase0/experiments/EXP-13-design.md, section 3).
RULE = {
    "overlap_min": 0.90, "main_silent_max": 0.10, "gap_s_min": 90.0,
    "dc_max": 0.30, "ag_min": 0.50, "tenths_min": 6, "tenths_share": 0.90, "smi_min": 0.50, "ls_max": 13.0,
    "added_s_min": 60.0, "reverse_gap_s_min": 90.0, "reverse_ag_max": 0.30,
    "ad_dc_min": 0.90, "ad_adl_max": 0.02, "ad_ag_min": 0.15, "ad_ag_s_min": 45.0, "ad_tenths_share": 0.60,
    "note_comm_dc_max": 0.50, "note_comm_ag_min": 0.30,
    "note_ad_dc_min": 0.80, "note_ad_ag_min": 0.10, "note_ad_ag_s_min": 30.0,
}
_TW = [[cmath.exp(-2j * math.pi * k * n / N_SMI) for n in range(N_SMI)] for k in range(33)]


def read_levels(path):
    """Levels in dB, one per frame. Accepts the compact files of audio-levels.ps1 (one value per line, '#' header
    lines) and raw ffmpeg ametadata output (lavfi.astats.Overall.RMS_level=...). Values are floored at FLOOR."""
    out = []
    for line in pathlib.Path(path).read_text(encoding="utf-8-sig", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("frame:"):
            continue
        if "=" in line:
            if not line.startswith("lavfi.astats.Overall.RMS_level="):
                continue
            line = line.split("=", 1)[1]
        out.append(FLOOR if ("inf" in line or "nan" in line) else max(FLOOR, float(line)))
    return out


def read_manifest(folder):
    """manifest.txt: 'key=value' pairs; lines starting 'main=' or 'cand=' describe one stream each."""
    info = {"main": None, "cands": []}
    path = pathlib.Path(folder) / "manifest.txt"
    if not path.exists():
        return info
    for line in path.read_text(encoding="utf-8-sig", errors="replace").splitlines():
        if line.startswith("#") or "=" not in line:
            continue
        fields = dict(f.split("=", 1) for f in line.split() if "=" in f)
        if "main" in fields:
            info["main"] = fields
        elif "cand" in fields:
            info["cands"].append(fields)
        else:
            info.update(fields)
    return info


def frame_ms_of(path):
    """The frame length a level file states, in ms: 'frame_ms=' in the first line of a compact file, or the spacing
    of the pts_time lines of raw ametadata output. None when the file states neither."""
    times = []
    with open(path, encoding="utf-8-sig", errors="replace") as f:
        for k, line in enumerate(f):
            if k == 0 and line.startswith("#"):
                for word in line.split():
                    if word.startswith("frame_ms="):
                        try:
                            return float(word[len("frame_ms="):])
                        except ValueError:
                            return None
            if "pts_time:" in line:
                try:
                    times.append(float(line.split("pts_time:", 1)[1].split()[0]))
                except (ValueError, IndexError):
                    pass
            if len(times) >= 50 or k >= 200:
                break
    steps = sorted(b - a for a, b in zip(times, times[1:]))
    return round(steps[len(steps) // 2] * 1000) if steps else None


def refusal(folder):
    """'' when the folder was written by the experiment-13 audio-levels.ps1 (manifest.txt, 20 ms frames in every level
    file), otherwise the reason in one line. Numbers are never printed for a refused folder."""
    d = pathlib.Path(folder)
    again = ("this folder cannot be analysed: measure the title again with the new audio-levels.ps1, "
             "which writes audio-levels-20ms-<main>-<candidates>")
    path = d / "manifest.txt"
    if not path.is_file():
        return (f"{d}: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's "
                f"folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels); {again}")
    lines = path.read_text(encoding="utf-8-sig", errors="replace").splitlines()
    if not lines or not lines[0].startswith(MANIFEST_LINE):
        return f"{d}: manifest.txt does not begin with '{MANIFEST_LINE}'; {again}"
    man = read_manifest(d)
    if man.get("frame_ms") != str(FRAME_MS):
        return f"{d}: manifest.txt gives frame_ms={man.get('frame_ms', '(none)')}, not {FRAME_MS}; {again}"
    if not man["main"] or not man["cands"]:
        return f"{d}: manifest.txt names no main stream or no candidate stream; {again}"
    names = ["main-mix.txt"] + (["main-fc.txt"] if (d / "main-fc.txt").exists() else [])
    names += [f"cand-{c.get('cand')}.txt" for c in man["cands"]]
    for name in names:
        if not (d / name).is_file():
            return f"{d}: {name} is missing; {again}"
        ms = frame_ms_of(d / name)
        if ms != FRAME_MS:
            held = "frames of unknown length" if ms is None else f"{ms:g} ms frames"
            return f"{d}: {name} holds {held}, not {FRAME_MS} ms; {again}"
    return ""


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(share * len(ordered)))] if ordered else float("nan")


def clamp(values, below=45.0):
    floor = percentile(values, 0.95) - below
    return [max(v, floor) for v in values]


def pearson(a, b):
    n = len(a)
    if n < 2:
        return 0.0
    ma, mb = sum(a) / n, sum(b) / n
    cov = va = vb = 0.0
    for x, y in zip(a, b):
        dx, dy = x - ma, y - mb
        cov += dx * dy
        va += dx * dx
        vb += dy * dy
    return cov / math.sqrt(va * vb) if va > 0 and vb > 0 else 0.0


def corr_at(a, b, i0, i1, lag, step=1):
    """Correlation of a[i] and b[i + lag] for i in [i0, i1); -2 when fewer than 10 pairs exist."""
    lo, hi = max(i0, -lag), min(i1, len(b) - lag)
    if hi - lo < 10:
        return -2.0
    idx = range(lo, hi, step)
    return pearson([a[i] for i in idx], [b[i + lag] for i in idx])


def smi(levels_db):
    """Share of the linear envelope's modulation energy at 2.3-7.8 Hz (DFT bins 3-10 of 1-32, 64 x 20 ms)."""
    lin = [10 ** (v / 20) for v in levels_db]
    mean = sum(lin) / N_SMI
    x = [v - mean for v in lin]
    e = [abs(sum(x[n] * _TW[k][n] for n in range(N_SMI))) ** 2 for k in range(33)]
    total = sum(e[1:33])
    return sum(e[3:11]) / total if total > 0 else 0.0


def ok(v):
    return v == v  # not NaN


def analyse(main, cand, dialogue=None, reverse=True):
    """The measures of one candidate against the main track. main, cand, dialogue: levels in dB per 20 ms frame."""
    dialogue = dialogue or main
    n = len(main)
    if n < 10 * WIN or len(cand) < 10 * WIN:
        return {"frames": n, "error": "too short"}
    mc, xc = clamp(main), clamp(cand)
    # Lag: tracks of one title share a timeline. Coarse +-10 s on 100 ms means, fine +-100 ms; kept only when the
    # two envelopes follow each other (r >= 0.5), otherwise 0. Then per 60 s block +-2 s, kept when r >= 0.5.
    mk = [sum(mc[i:i + 5]) / 5 for i in range(0, n - 4, 5)]
    xk = [sum(xc[i:i + 5]) / 5 for i in range(0, len(xc) - 4, 5)]
    _, lag_k = max((corr_at(mk, xk, 0, len(mk), lag, 2), lag) for lag in range(-COARSE_LAG, COARSE_LAG + 1))
    r_glob, lag = max((corr_at(mc, xc, 0, n, l, 5), l) for l in range(5 * lag_k - 5, 5 * lag_k + 6))
    if r_glob < 0.5:
        lag = 0
    block_lag = {}
    for b in range((n + BLOCK - 1) // BLOCK):
        b0, b1 = b * BLOCK, min(n, (b + 1) * BLOCK)
        r, lb = max((corr_at(mc, xc, b0, b1, l, 4), l) for l in range(lag - BLOCK_LAG, lag + BLOCK_LAG + 1, 2))
        block_lag[b] = lb if r >= 0.5 else lag

    def lag_of(t):
        return block_lag[t // BLOCK]

    inside = [0 <= t + lag_of(t) < len(cand) for t in range(n)]
    frames = [t for t in range(n) if inside[t]]
    xa = [cand[t + lag_of(t)] if inside[t] else FLOOR for t in range(n)]
    x95 = percentile([xa[t] for t in frames], 0.95)
    m95 = percentile([main[t] for t in frames], 0.95)
    d95 = percentile([dialogue[t] for t in frames if t < len(dialogue)], 0.95)
    silent = [main[t] <= SILENT_DB for t in range(n)]
    active = [inside[t] and xa[t] >= x95 - X_ACTIVE_BELOW for t in range(n)]
    dlg = [inside[t] and t < len(dialogue) and dialogue[t] >= d95 - DLG_BELOW for t in range(n)]
    gap = [False] * n
    t = 0
    while t < n:
        if inside[t] and not dlg[t]:
            u = t
            while u < n and inside[u] and not dlg[u]:
                u += 1
            if u - t >= GAP_MIN:
                for k in range(t + GAP_TRIM, u - GAP_TRIM):
                    gap[k] = not silent[k]
            t = u
        else:
            t += 1
    wins = [w for w in range(0, n - WIN + 1, WIN) if all(inside[w:w + WIN])]
    rw = {w: max(corr_at(mc, xc, w, w + WIN, lag_of(w) + d) for d in range(-WIN_LAG, WIN_LAG + 1)) for w in wins}
    dwins = [w for w in wins if sum(dlg[w:w + WIN]) >= WIN // 2]
    dc = sum(1 for w in dwins if rw[w] >= MATCH_R) / len(dwins) if dwins else float("nan")
    matched = [w for w in wins if rw[w] >= MATCH_R]
    mframes = [u for w in matched for u in range(w, w + WIN) if main[u] >= m95 - 30 and xa[u] > FLOOR]
    if len(matched) >= max(10, len(wins) // 20) and mframes:
        gain = percentile([xa[u] - main[u] for u in mframes], 0.75)
    else:
        gain = x95 - m95
    added = [active[u] and xa[u] - gain - main[u] >= ADDED_DB and rw.get((u // WIN) * WIN, 1.0) < MISMATCH_R
             for u in range(n)]
    n_gap = sum(gap)
    n_ag = sum(1 for u in range(n) if gap[u] and added[u])
    n_dlg = sum(dlg)
    lo, hi = frames[0], frames[-1] + 1
    tenths = []
    for k in range(10):
        a, b = lo + k * (hi - lo) // 10, lo + (k + 1) * (hi - lo) // 10
        g = sum(gap[a:b])
        if g * FRAME_S >= 10:
            tenths.append(sum(1 for u in range(a, b) if gap[u] and added[u]) / g)
    s = [smi(xa[t0:t0 + N_SMI]) for t0 in range(0, n - N_SMI, N_SMI // 2) if sum(added[t0:t0 + N_SMI]) >= N_SMI // 2]
    m = {
        "frames": n, "lag_ms": lag * 20, "r_global": r_glob, "overlap": len(frames) / n,
        "main_silent": sum(1 for u in frames if silent[u]) / len(frames),
        "dc": dc, "gain_db": gain, "gap_s": n_gap * FRAME_S,
        "ag": n_ag / n_gap if n_gap else float("nan"), "ag_s": n_ag * FRAME_S,
        "adl": sum(1 for u in range(n) if dlg[u] and added[u]) / n_dlg if n_dlg else float("nan"),
        "tenths": len(tenths), "tenths_comm": sum(1 for v in tenths if v >= 0.40),
        "tenths_ad": sum(1 for v in tenths if v >= 0.05),
        "smi": statistics.median(s) if s else float("nan"), "smi_windows": len(s),
        "ls_db": x95 - percentile([xa[u] for u in frames], 0.50), "added_s": sum(added) * FRAME_S,
    }
    if reverse:
        back = analyse(cand, main, None, reverse=False)
        m["rev_gap_s"], m["rev_ag"] = back.get("gap_s", 0.0), back.get("ag", float("nan"))
    return m


def commentary_pattern(m, rule=RULE):
    return (ok(m["dc"]) and m["dc"] <= rule["dc_max"] and ok(m["ag"]) and m["ag"] >= rule["ag_min"]
            and m["tenths"] >= rule["tenths_min"] and m["tenths_comm"] >= rule["tenths_share"] * m["tenths"]
            and ok(m["smi"]) and m["smi"] >= rule["smi_min"] and m["ls_db"] <= rule["ls_max"]
            and m["added_s"] >= rule["added_s_min"])


def ad_pattern(m, rule=RULE):
    return (ok(m["dc"]) and m["dc"] >= rule["ad_dc_min"] and ok(m["adl"]) and m["adl"] <= rule["ad_adl_max"]
            and ok(m["ag"]) and m["ag"] >= rule["ad_ag_min"] and m["ag_s"] >= rule["ad_ag_s_min"]
            and m["tenths"] >= rule["tenths_min"] and m["tenths_ad"] >= rule["ad_tenths_share"] * m["tenths"]
            and ok(m["smi"]) and m["smi"] >= rule["smi_min"])


def not_judged(m, rule=RULE):
    """Why the content rule may not confirm anything for this pair, or '' when it may."""
    if "error" in m:
        return m["error"]
    if m["overlap"] < rule["overlap_min"]:
        return "the tracks overlap for less than 90% of the main track"
    if m["main_silent"] > rule["main_silent_max"]:
        return "the main track is digitally silent for more than 10% of the time (not continuous programme sound)"
    if m["gap_s"] < rule["gap_s_min"]:
        return "the main track has less than 90 s of dialogue gaps"
    if not ok(m["dc"]):
        return "the main track has no dialogue windows"
    return ""


def verdict(m, rule=RULE):
    """One of: commentary, reference-unclear, ad-pattern, note-commentary, note-ad, keep; with not-judged reasons."""
    why = not_judged(m, rule)
    if "error" in m:
        return "not-judged"
    if not why and commentary_pattern(m, rule):
        reverse = ok(m.get("rev_ag", float("nan"))) and m.get("rev_gap_s", 0.0) >= rule["reverse_gap_s_min"] \
            and m["rev_ag"] >= rule["reverse_ag_max"]
        return "reference-unclear" if reverse else "commentary"
    if not why and ad_pattern(m, rule):
        return "ad-pattern"
    if ok(m["dc"]) and ok(m["ag"]) and m["dc"] <= rule["note_comm_dc_max"] and m["ag"] >= rule["note_comm_ag_min"]:
        return "note-commentary"
    if ok(m["dc"]) and ok(m["ag"]) and m["dc"] >= rule["note_ad_dc_min"] and m["ag"] >= rule["note_ad_ag_min"] \
            and m["ag_s"] >= rule["note_ad_ag_s_min"]:
        return "note-ad"
    return "keep"


MEANING = {
    "duplicate": "exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed",
    "commentary": "commentary pattern: kept and flagged for review until experiment 13 answers yes, then dropped",
    "reference-unclear": "commentary pattern, but the main track also adds sound to the candidate: keep, review",
    "ad-pattern": "audio-description pattern: keep, note possible_audio_description (never dropped by content)",
    "note-commentary": "keep, note possible_commentary",
    "note-ad": "keep, note possible_audio_description",
    "keep": "keep: programme audio, nothing added",
    "not-judged": "keep: no content verdict",
}


def fmt(v, digits=2):
    return "n/a" if not ok(v) else f"{v:.{digits}f}"


def report(folder):
    d = pathlib.Path(folder)
    why = refusal(d)
    if why:
        raise ValueError(why)
    man = read_manifest(d)
    main = read_levels(d / "main-mix.txt")
    fc_path = d / "main-fc.txt"
    fc = read_levels(fc_path) if fc_path.exists() else []
    if fc and max(fc) <= SILENT_DB:
        fc = []
    mainfo = man["main"] or {}
    lines = [f"{d} ({man.get('file', '?')}, measured in {man.get('seconds', '?')} s)",
             f"  main stream {mainfo.get('main', '?')}: {mainfo.get('codec', '?')} {mainfo.get('channels', '?')} ch "
             f"{mainfo.get('layout', '')} {mainfo.get('language', '')}; dialogue reference: "
             f"{'its centre channel' if fc else 'its mono mixdown (no centre channel)'}; "
             f"{len(main)} frames ({len(main) * FRAME_S / 60:.1f} min)"]
    cands = man["cands"]
    seen = {mainfo["md5"]: mainfo.get("main", "?")} if mainfo.get("md5") else {}
    label = f"{d.resolve().parent.name}/{man.get('file', d.resolve().name)}"
    for c in cands:
        sid = c.get("cand")
        path = d / f"cand-{sid}.txt"
        head = f"  stream {sid}: {c.get('codec', '?')} {c.get('channels', '?')} ch {c.get('layout', '')} {c.get('language', '')}"
        if c.get("md5") in seen:
            lines += [head, f"    verdict: duplicate of stream {seen[c['md5']]} (" + MEANING["duplicate"] + ")",
                      f"    row: | {label} | {sid} | same packets as stream {seen[c['md5']]} | duplicate |"]
            continue
        if c.get("md5"):
            seen[c["md5"]] = sid
        m = analyse(main, read_levels(path), fc or None)
        v = verdict(m)
        lines.append(head)
        if "error" in m:
            lines.append(f"    verdict: not-judged ({m['error']})")
            continue
        why = not_judged(m)
        lines += [
            f"    lag {m['lag_ms']} ms (envelope r {fmt(m['r_global'])}), overlap {fmt(m['overlap'])}, "
            f"main silent {fmt(m['main_silent'])}",
            f"    DC {fmt(m['dc'])}  AG {fmt(m['ag'])} ({m['gap_s']:.0f} s of gaps, {m['ag_s']:.0f} s added)  "
            f"tenths {m['tenths_comm']}/{m['tenths']}  SMI {fmt(m['smi'])}  LS {fmt(m['ls_db'], 1)} dB  "
            f"ADL {fmt(m['adl'])}  added {m['added_s']:.0f} s",
            f"    reverse: {m['rev_gap_s']:.0f} s of gaps, AG {fmt(m['rev_ag'])}",
            f"    verdict: {v} ({MEANING[v]})" + (f"; not judged: {why}" if why else ""),
            f"    row: | {label} | {sid} | DC {fmt(m['dc'])}, AG {fmt(m['ag'])}, tenths "
            f"{m['tenths_comm']}/{m['tenths']}, SMI {fmt(m['smi'])}, LS {fmt(m['ls_db'], 1)} | {v} |",
        ]
    return lines


def segments(text):
    """A scan's segment map (title attribute 26), such as '7', '10,11' or '1-3', as a list of segment numbers."""
    out = []
    for part in text.split(","):
        lo, _, hi = part.strip().partition("-")
        if lo.isdigit() and (not hi or hi.isdigit()):
            out += list(range(int(lo), int(hi or lo) + 1))
    return out


def audio_by_title(scan):
    """{title: (seconds, [segment, ...], {language: [stream, ...]})} for a parsed scan (robot.parse). Audio streams
    that MakeMKV derived from another one (flag 2048, such as the DTS core of a DTS-HD MA track) are left out: MakeMKV
    synthesises them, they are not extra tracks on the disc. A stream without a language code counts as 'und'."""
    out = {}
    for title, a in scan.titles.items():
        try:
            seconds = duration_seconds(a.get(9, "0:00:00"))
        except ValueError:
            seconds = 0
        out[title] = (seconds, segments(a.get(26, "")), {})
    for (title, stream), a in sorted(scan.streams.items()):
        if title not in out or a.get(1) != "Audio" or int(a.get(22, "0") or 0) & DERIVED:
            continue
        out[title][2].setdefault(a.get(3) or "und", []).append(stream)
    return out


def titles_report(recording):
    """Every title of <recording>/scan.txt, longest first: its audio streams by language without derived streams, a
    mark '<- <language> x<n>' when one language has two or more, the labels it was already ripped under (from each
    <label>/rip-summary.json), and the titles it shares segments with (a play-all playlist)."""
    d = pathlib.Path(recording)
    scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
    ripped = {}
    for path in sorted(d.glob("*/rip-summary.json")):
        try:
            rip = json.loads(path.read_text(encoding="utf-8-sig"))
        except ValueError:
            continue
        if isinstance(rip.get("title"), int):
            label = path.parent.name + ("" if rip.get("ok") else " (failed)")
            ripped.setdefault(rip["title"], []).append(label)
    tmdb = d / "tmdb.json"
    original = json.loads(tmdb.read_text(encoding="utf-8-sig")).get("original_language") if tmdb.exists() else None
    info = audio_by_title(scan)
    lines = [f"{d.name}: {len(info)} titles, longest first; TMDb original language: {original or 'unknown'}",
             "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)"]
    for title in sorted(info, key=lambda t: (-info[t][0], t)):
        seconds, segs, langs = info[title]
        audio = "; ".join(f"{lang} {','.join(map(str, s))}" for lang, s in sorted(langs.items())) or "none"
        notes = [f"<- {lang} x{len(s)}" for lang, s in sorted(langs.items()) if len(s) >= 2]
        if title in ripped:
            notes.append("ripped as " + ", ".join(ripped[title]))
        shared = [str(u) for u in sorted(info) if u != title and set(segs) & set(info[u][1])]
        if shared:
            notes.append("shares segments with title " + ", ".join(shared))
        where = ",".join(map(str, segs)) or "?"
        line = f"  title {title:>3}  {scan.titles[title].get(9, '?'):>8}  segments {where:<8}  audio {audio}"
        lines.append(line + ("   " + "; ".join(notes) if notes else ""))
    return lines


def main(args):
    if args[:1] == ["--titles"]:
        if len(args) != 2:
            sys.exit("usage: audio_similarity.py --titles corpus/owner/<slug>")
        if not (pathlib.Path(args[1]) / "scan.txt").is_file():
            sys.exit(f"{args[1]}: no scan.txt (record and pull the disc first)")
        for line in titles_report(args[1]):
            print(line)
        return
    for folder in args:  # every folder is checked before anything is printed
        why = refusal(folder)
        if why:
            sys.exit(why)
    for folder in args:
        for line in report(folder):
            print(line, flush=True)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: audio_similarity.py <audio-levels-20ms folder> ... | --titles corpus/owner/<slug>")
    main(sys.argv[1:])
