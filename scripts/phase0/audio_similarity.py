"""Experiment 13: is an extra audio track a commentary? Judged from its content against the title's main track.

    audio_similarity.py <audio-levels-20ms folder> ...     the measures and the verdict of every candidate
    audio_similarity.py --titles corpus/owner/<slug>       which titles of a recorded scan have two or more audio
                                                           streams in one language (derived streams left out)

Reads a folder written by audio-levels.ps1 (audio-levels-20ms-<main>-<candidates>): manifest.txt, main-mix.txt,
main-fc.txt (only when the main track has a centre channel) and cand-<stream>.txt, each holding one speech-band
(300-3400 Hz) RMS level in dB per 20 ms frame. For every candidate it prints the measures and the verdict of the
rule in docs/phase0/experiments/EXP-13-design.md (section 3, rows 3 to 5 as its addendum, section 9, replaces them):

  DC     dialogue carried: share of the main's 2 s dialogue windows in which the candidate's level follows the main's
  AG     added in the gaps: share of the main's dialogue-gap time in which the candidate carries speech-level sound
         that the main does not explain (10 dB above the level the main predicts, in a window that does not follow)
  tenths of the tenths of the running time that hold 10 s of dialogue gaps, in how many AG reaches 0.40
  SMI    syllabic modulation of the added sound (share of envelope energy at 2.3-7.8 Hz)
  LS     the candidate's level spread, 95th minus 50th percentile (small for dense talk)
  ADL    added sound while the main's dialogue runs

Stereo width, codec and track position are not used to judge a candidate; the main track's channel layout only
decides whether its centre channel is the dialogue curve, and position only the keep-only veto below. A folder
that audio-levels.ps1 did not write in this form is refused, with a one-line message and a non-zero exit, before any
number is printed: the first measurement's folders
(audio-levels-<main>-<cand>, 100 ms frames, full band, no manifest) must be measured again with the new script.
A folder with a main stream and no candidate (a title measured only for the tuning dataset) has nothing to judge.

The main stream of a folder is the title's programme reference (EXP-13 addendum, section 9): the top-ranked stream of
the work's original language, else of English. A candidate in another language than the reference (an English
commentary on a Spanish film, an English dub) is judged against it like any other candidate; its report line says
"cross-language". Every candidate is also judged the other way round (the reverse run: the candidate as reference, the
main stream as candidate). When the reverse run has the commentary pattern, the reference looks like a commentary over
the candidate, so one of the two is not what its rank says: the verdict is reference-unclear (keep, review), whatever
the forward run says.

A cross-language candidate with the commentary pattern is never a plain "commentary" (addendum 9.2): level curves do
not hear language, and a lip-synced dub with loose timing on a dialogue-dense title can show the pattern against the
original. When its language has a stream that 8.6 places before it (the dub itself, for a 2.0 companion of the dub),
it is judged against that stream too (the peer run, from the same folder): if the peer run has no commentary verdict,
the candidate follows a programme track of its own language and gets only note-commentary. Otherwise its verdict is
commentary-cross-language: keep and flag for review, never dropped by content.

The position veto (row 0 of the rule, owner decision D21): when the folder's main stream is not the first stream of
its language in the file (streams.json) and that first stream is lossless or has more than two channels, the
reference choice may be inverted (a score or a commentary ranked above the programme), so no commentary is
confirmed on the title: every candidate that is not an exact duplicate is reference-unclear (keep, review),
whatever the forward and reverse runs say. The report prints a 'veto:' line for every folder. Position never
decides what a track is (D15); the veto only withholds a drop.

For other programs: results(folder, rule) gives the same measures and verdicts as data; RULE holds the decision
thresholds and CONSTANTS names the module constants of the measures, so that audio_dataset.py can try other values.
swapped(measures) gives the measures of the same pair with the roles exchanged (what the rule would say of the main
stream if the candidate were the reference), from the same two runs. placed_first() is spec 8.6's ranking (step 3)
with its guards G1 and G2 (step 4) as far as ffprobe's facts go; audio_dataset.py chooses the reference with it.
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
LAG_MIN_R = 0.5         # a lag (global or per block) is used only when the envelopes correlate at least this much
GAIN_PCT = 0.75         # the gain is this percentile of the candidate's level over the main's in matched windows
TENTH_GAP_S = 10.0      # a tenth of the running time counts when it holds this many seconds of dialogue gaps
TENTH_COMM_AG = 0.40    # a counting tenth has commentary-like added sound at this AG or more (tenths_comm)
TENTH_AD_AG = 0.05      # ... and audio-description-like added sound at this AG or more (tenths_ad)
# The measure constants above that a tuning run may change (audio_dataset.py --set NAME=VALUE); FRAME_MS, WIN, BLOCK
# and N_SMI are fixed by the level files and the precomputed transform.
CONSTANTS = ("MATCH_R", "MISMATCH_R", "X_ACTIVE_BELOW", "DLG_BELOW", "GAP_MIN", "GAP_TRIM", "ADDED_DB", "SILENT_DB",
             "COARSE_LAG", "BLOCK_LAG", "WIN_LAG", "LAG_MIN_R", "GAIN_PCT", "TENTH_GAP_S", "TENTH_COMM_AG", "TENTH_AD_AG")

# The thresholds of the decision rule. Every one is PROVISIONAL (docs/phase0/experiments/EXP-13-design.md, section 3).
RULE = {
    "overlap_min": 0.90, "main_silent_max": 0.10, "gap_s_min": 90.0,
    "dc_max": 0.30, "ag_min": 0.50, "tenths_min": 6, "tenths_share": 0.90, "smi_min": 0.50, "ls_max": 13.0,
    "added_s_min": 60.0,
    "ad_dc_min": 0.90, "ad_adl_max": 0.02, "ad_ag_min": 0.15, "ad_ag_s_min": 45.0, "ad_tenths_share": 0.60,
    "note_comm_dc_max": 0.50, "note_comm_ag_min": 0.30,
    "note_ad_dc_min": 0.80, "note_ad_ag_min": 0.10, "note_ad_ag_s_min": 30.0,
}
# The reverse run's measures kept with every candidate's measures, as "rev_<name>" (EXP-13 addendum, section 9.3).
REVERSE_KEYS = ("overlap", "main_silent", "gap_s", "dc", "ag", "ag_s", "adl", "tenths", "tenths_comm", "tenths_ad", "smi",
                "smi_windows", "ls_db", "added_s", "gain_db")
# The reverse test of EXP-13-design.md section 2 item 13 (90 s of the candidate's gaps, reverse AG 0.30). It is replaced
# by the reverse commentary pattern (addendum section 9.3) and used only for measures that carry no reverse block
# (rev_gap_s and rev_ag alone, as the code before 2026-10-06 wrote them): it can only keep.
EARLIER_REVERSE = {"gap_s_min": 90.0, "ag_min": 0.30}
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
    if not man["main"]:
        return f"{d}: manifest.txt names no main stream; {again}"
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
    if r_glob < LAG_MIN_R:
        lag = 0
    block_lag = {}
    for b in range((n + BLOCK - 1) // BLOCK):
        b0, b1 = b * BLOCK, min(n, (b + 1) * BLOCK)
        r, lb = max((corr_at(mc, xc, b0, b1, l, 4), l) for l in range(lag - BLOCK_LAG, lag + BLOCK_LAG + 1, 2))
        block_lag[b] = lb if r >= LAG_MIN_R else lag

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
        gain = percentile([xa[u] - main[u] for u in mframes], GAIN_PCT)
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
        if g * FRAME_S >= TENTH_GAP_S:
            tenths.append(sum(1 for u in range(a, b) if gap[u] and added[u]) / g)
    s = [smi(xa[t0:t0 + N_SMI]) for t0 in range(0, n - N_SMI, N_SMI // 2) if sum(added[t0:t0 + N_SMI]) >= N_SMI // 2]
    m = {
        "frames": n, "lag_ms": lag * 20, "r_global": r_glob, "overlap": len(frames) / n,
        "main_silent": sum(1 for u in frames if silent[u]) / len(frames),
        "dc": dc, "gain_db": gain, "gap_s": n_gap * FRAME_S,
        "ag": n_ag / n_gap if n_gap else float("nan"), "ag_s": n_ag * FRAME_S,
        "adl": sum(1 for u in range(n) if dlg[u] and added[u]) / n_dlg if n_dlg else float("nan"),
        "tenths": len(tenths), "tenths_comm": sum(1 for v in tenths if v >= TENTH_COMM_AG),
        "tenths_ad": sum(1 for v in tenths if v >= TENTH_AD_AG),
        "smi": statistics.median(s) if s else float("nan"), "smi_windows": len(s),
        "ls_db": x95 - percentile([xa[u] for u in frames], 0.50), "added_s": sum(added) * FRAME_S,
    }
    if reverse:
        back = analyse(cand, main, None, reverse=False)
        m["rev_error"] = back.get("error", "")
        for k in REVERSE_KEYS:
            m["rev_" + k] = back.get(k, 0.0 if k == "gap_s" else float("nan"))
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


def reverse_measures(m):
    """The reverse run (the candidate as reference, the main stream as candidate) as a measures dict; None when m
    carries no reverse block (measures written before 2026-10-06 kept only rev_gap_s and rev_ag)."""
    if "rev_tenths" not in m:
        return None
    if m.get("rev_error"):
        return {"error": m["rev_error"]}
    return {k: m["rev_" + k] for k in REVERSE_KEYS}


def reverse_pattern(m, rule=RULE):
    """True when the reverse run may be judged and has the commentary pattern: the reference looks like a commentary
    over the candidate (EXP-13 addendum, section 9.3). The same thresholds as the forward pattern; no number of its own."""
    r = reverse_measures(m)
    return r is not None and "error" not in r and not not_judged(r, rule) and commentary_pattern(r, rule)


def swapped(m):
    """The measures of the same pair with the roles exchanged: the forward run becomes the reverse one and the other way
    round. verdict(swapped(m)) is what the rule would say of the main stream if the candidate were the reference. It is
    an approximation in two ways, neither measured: the forward run used the main stream's centre channel as its
    dialogue curve, where a real swap would use its mixdown; and the reverse run used the candidate's mixdown as its
    dialogue curve, where a candidate with a centre channel (a 5.1 isolated score or M&E track) taken as the reference
    would use that channel, which carries little or no dialogue, so its gaps would be longer than those measured here.
    None when m has no reverse block or an error."""
    r = reverse_measures(m)
    if r is None or "error" in m:
        return None
    if "error" in r:
        return {"error": r["error"]}
    out = dict(r)
    out["rev_error"] = ""
    for k in REVERSE_KEYS:
        out["rev_" + k] = m.get(k, float("nan"))
    return out


def verdict(m, rule=RULE):
    """One of: commentary, reference-unclear, ad-pattern, note-commentary, note-ad, keep; with not-judged reasons.
    reference-unclear: the reverse run has the commentary pattern (the reference looks like a commentary over this
    track), whatever the forward run says; for measures without a reverse block, the earlier reverse test applies."""
    if "error" in m:
        return "not-judged"
    why = not_judged(m, rule)
    if reverse_measures(m) is not None:
        if reverse_pattern(m, rule):
            return "reference-unclear"
        if not why and commentary_pattern(m, rule):
            return "commentary"
    elif not why and commentary_pattern(m, rule):
        earlier = ok(m.get("rev_ag", float("nan"))) and m.get("rev_gap_s", 0.0) >= EARLIER_REVERSE["gap_s_min"] \
            and m["rev_ag"] >= EARLIER_REVERSE["ag_min"]
        return "reference-unclear" if earlier else "commentary"
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
    "commentary-cross-language": "commentary pattern on a track in another language than the reference: keep and flag "
                                 "for review (commentary_detected); never dropped by content until real dubs in a "
                                 "wanted language have been measured (EXP-13 addendum 9.2)",
    "reference-unclear": "the main track looks like a commentary over this one (reverse run): one of the two is not what "
                         "its rank says (a commentary or a stem ranked first, or a stem among the candidates): keep, review",
    "ad-pattern": "audio-description pattern: keep, note possible_audio_description (never dropped by content)",
    "note-commentary": "keep, note possible_commentary",
    "note-ad": "keep, note possible_audio_description",
    "keep": "keep: programme audio, nothing added",
    "not-judged": "keep: no content verdict",
}
VETO_MEANING = ("the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is "
                "confirmed on this title: keep, review (audio_reference_unclear)")


def fmt(v, digits=2):
    return "n/a" if not ok(v) else f"{v:.{digits}f}"


NO_LANGUAGE = ("", "-", "und", "zxx", "mul", "mis")  # codes that name no language (spec 8.6: "no language")


# TMDb's two-letter original_language -> the three-letter codes a stream may carry (spec 8.6 "Languages"): every ISO
# 639-1 code with its ISO 639-2 terminology and bibliographic forms, as ffmpeg 8.1.2's own table (libavformat
# avlanguage.c) pairs them, including the deprecated aliases it keeps (in, iw, ji, jw, mo, sh). ffmpeg pairs sh with
# no three-letter code, so hbs, srp, hrv and bos are added by hand, and mol is added to mo. Wider than ISO 639-2:
# no, nb, nn (a disc may tag Norwegian nob or nno) and tl (Filipino fil). TMDb's own cn (Cantonese: zho, chi, yue)
# is added; its xx (no language) is TMDB_NO_LANGUAGE, not an entry. A code missing here leaves the original
# language unknown: English is then the reference language, and args says so.
ISO639 = {
    "aa": ("aar",), "ab": ("abk",), "ae": ("ave",), "af": ("afr",), "ak": ("aka",), "am": ("amh",), "an": ("arg",),
    "ar": ("ara",), "as": ("asm",), "av": ("ava",), "ay": ("aym",), "az": ("aze",), "ba": ("bak",), "be": ("bel",),
    "bg": ("bul",), "bh": ("bih",), "bi": ("bis",), "bm": ("bam",), "bn": ("ben",), "bo": ("bod", "tib"),
    "br": ("bre",), "bs": ("bos",), "ca": ("cat",), "ce": ("che",), "ch": ("cha",), "cn": ("zho", "chi", "yue"),
    "co": ("cos",), "cr": ("cre",), "cs": ("ces", "cze"), "cu": ("chu",), "cv": ("chv",), "cy": ("cym", "wel"),
    "da": ("dan",), "de": ("deu", "ger"), "dv": ("div",), "dz": ("dzo",), "ee": ("ewe",), "el": ("ell", "gre"),
    "en": ("eng",), "eo": ("epo",), "es": ("spa",), "et": ("est",), "eu": ("eus", "baq"), "fa": ("fas", "per"),
    "ff": ("ful",), "fi": ("fin",), "fj": ("fij",), "fo": ("fao",), "fr": ("fra", "fre"), "fy": ("fry",),
    "ga": ("gle",), "gd": ("gla",), "gl": ("glg",), "gn": ("grn",), "gu": ("guj",), "gv": ("glv",), "ha": ("hau",),
    "he": ("heb",), "hi": ("hin",), "ho": ("hmo",), "hr": ("hrv",), "ht": ("hat",), "hu": ("hun",),
    "hy": ("hye", "arm"), "hz": ("her",), "ia": ("ina",), "id": ("ind",), "ie": ("ile",), "ig": ("ibo",),
    "ii": ("iii",), "ik": ("ipk",), "in": ("ind",), "io": ("ido",), "is": ("isl", "ice"), "it": ("ita",),
    "iu": ("iku",), "iw": ("heb",), "ja": ("jpn",), "ji": ("yid",), "jv": ("jav",), "jw": ("jav",),
    "ka": ("kat", "geo"), "kg": ("kon",), "ki": ("kik",), "kj": ("kua",), "kk": ("kaz",), "kl": ("kal",),
    "km": ("khm",), "kn": ("kan",), "ko": ("kor",), "kr": ("kau",), "ks": ("kas",), "ku": ("kur",), "kv": ("kom",),
    "kw": ("cor",), "ky": ("kir",), "la": ("lat",), "lb": ("ltz",), "lg": ("lug",), "li": ("lim",), "ln": ("lin",),
    "lo": ("lao",), "lt": ("lit",), "lu": ("lub",), "lv": ("lav",), "mg": ("mlg",), "mh": ("mah",),
    "mi": ("mri", "mao"), "mk": ("mkd", "mac"), "ml": ("mal",), "mn": ("mon",), "mo": ("ron", "rum", "mol"),
    "mr": ("mar",), "ms": ("msa", "may"), "mt": ("mlt",), "my": ("mya", "bur"), "na": ("nau",), "nb": ("nob", "nor"),
    "nd": ("nde",), "ne": ("nep",), "ng": ("ndo",), "nl": ("nld", "dut"), "nn": ("nno", "nor"),
    "no": ("nor", "nob", "nno"), "nr": ("nbl",), "nv": ("nav",), "ny": ("nya",), "oc": ("oci",), "oj": ("oji",),
    "om": ("orm",), "or": ("ori",), "os": ("oss",), "pa": ("pan",), "pi": ("pli",), "pl": ("pol",), "ps": ("pus",),
    "pt": ("por",), "qu": ("que",), "rm": ("roh",), "rn": ("run",), "ro": ("ron", "rum"), "ru": ("rus",),
    "rw": ("kin",), "sa": ("san",), "sc": ("srd",), "sd": ("snd",), "se": ("sme",), "sg": ("sag",),
    "sh": ("hbs", "srp", "hrv", "bos"), "si": ("sin",), "sk": ("slk", "slo"), "sl": ("slv",), "sm": ("smo",),
    "sn": ("sna",), "so": ("som",), "sq": ("sqi", "alb"), "sr": ("srp",), "ss": ("ssw",), "st": ("sot",),
    "su": ("sun",), "sv": ("swe",), "sw": ("swa",), "ta": ("tam",), "te": ("tel",), "tg": ("tgk",), "th": ("tha",),
    "ti": ("tir",), "tk": ("tuk",), "tl": ("tgl", "fil"), "tn": ("tsn",), "to": ("ton",), "tr": ("tur",),
    "ts": ("tso",), "tt": ("tat",), "tw": ("twi",), "ty": ("tah",), "ug": ("uig",), "uk": ("ukr",), "ur": ("urd",),
    "uz": ("uzb",), "ve": ("ven",), "vi": ("vie",), "vo": ("vol",), "wa": ("wln",), "wo": ("wol",), "xh": ("xho",),
    "yi": ("yid",), "yo": ("yor",), "za": ("zha",), "zh": ("zho", "chi"), "zu": ("zul",),
}

TMDB_NO_LANGUAGE = "xx"  # TMDb's original_language of a work without speech: English is then the reference language


def is_lossless(codec, profile):
    """Lossless audio as spec 8.6 ranks it. ffprobe names DTS:X tracks 'DTS-HD MA + DTS:X' (and '... IMAX'); the
    manifest of a level folder writes the profile with '_' for spaces ('DTS-HD_MA')."""
    c = str(codec or "")
    return c in ("truehd", "mlp", "flac") or c.startswith("pcm") or "DTS-HD MA" in str(profile or "").replace("_", " ")


def rank(a):
    """Spec 8.6 step 3's order of streams given as (stream, channels, lossless, language, description): most
    channels, then lossless, then the earlier stream."""
    return (-a[1], not a[2], a[0])


def placed_first(streams):
    """The stream spec 8.6 places first among the given streams of one language: rank() (step 3), then its guards
    (step 4) as far as ffprobe's facts go. G1: a lossy stream of two channels or fewer (the spec says 256 kb/s or less;
    streams.json carries no bit rate) never comes before a lossless one. G2: a lossy stream of two channels or fewer
    that is not the language's first stream and has more channels than that first stream gives way to it."""
    ranked = sorted(streams, key=rank)
    top = ranked[0]
    if not top[2] and top[1] <= 2:
        lossless = [a for a in ranked if a[2]]
        if lossless:
            return lossless[0]
        first = min(streams, key=lambda a: a[0])
        if first[0] != top[0] and first[1] < top[1]:
            return first
    return top


def ffprobe_audio(streams):
    """[(stream, channels, lossless, language, description)] for the audio streams of an ffprobe stream list (a level
    folder's streams.json, a rip's ffprobe-streams.json), the form placed_first() takes."""
    out = []
    for s in streams:
        if s.get("codec_type") == "audio":
            tags = s.get("tags") or {}
            c = str(s.get("codec_name", ""))
            what = [c, s.get("profile"), f"{s.get('channels')} ch", s.get("channel_layout"), tags.get("title")]
            out.append((int(s["index"]), int(s.get("channels") or 0), is_lossless(c, s.get("profile")),
                        tags.get("language") or "und", " ".join(str(w) for w in what if w)))
    return out


def read_streams(folder):
    """The stream list of a level folder's streams.json (ffprobe's list of the file's streams); None when there is
    none."""
    path = pathlib.Path(folder) / "streams.json"
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8-sig")).get("streams", [])


def first_of_language(audio, main):
    """The first stream of `audio` (ffprobe_audio() tuples, in file order) in the language of stream `main`, when it is
    not `main` itself and is lossless or has more than two channels; None otherwise, and when `main` is not in `audio`.
    A lossy first stream of two channels or fewer is what G1 and G2 pass over on purpose, so it raises nothing. This is
    the condition of the position veto (row 0, owner decision D21)."""
    mine = [a for a in audio if a[0] == main]
    if not mine:
        return None
    first = min((a for a in audio if a[3] == mine[0][3]), key=lambda a: a[0])
    return None if first[0] == main or not (first[2] or first[1] > 2) else first


def position_veto(streams, main):
    """Row 0 of the rule (EXP-13 addendum 9.5; owner decision D21), for the title whose reference is stream `main`:
    (holds, why). holds is True when the reference is not the first stream of its language in the file and that first
    stream is lossless or has more than two channels: the reference choice may be inverted (a score or a commentary
    ranked above the programme), so the rule confirms no commentary on the title. False when the reference is the first
    stream of its language, or that first stream is lossy with two channels or fewer; None when it cannot be checked
    (no streams.json, or stream `main` is not in it). why names the streams. Position never decides what a track is
    (D15): the veto only withholds a drop."""
    if streams is None:
        return None, "not checked: the folder has no streams.json"
    audio = ffprobe_audio(streams)
    ref = next((a for a in audio if a[0] == main), None)
    if ref is None:
        return None, f"not checked: stream {main} is not an audio stream of streams.json"
    head = f"the reference, stream {main} ({ref[4]}),"
    first = min((a for a in audio if a[3] == ref[3]), key=lambda a: a[0])
    if first[0] == main:
        return False, f"{head} is the first {ref[3]} stream of the file"
    if first_of_language(audio, main) is None:
        return False, (f"{head} is not the first {ref[3]} stream of the file, but stream {first[0]} ({first[4]}), which "
                       f"comes first, is lossy with two channels or fewer")
    return True, (f"{head} is not the first {ref[3]} stream of the file: stream {first[0]} ({first[4]}) comes first and "
                  f"is lossless or has more than two channels, so the reference choice may be inverted")


def manifest_stream(info):
    """A manifest line's stream as (stream, channels, lossless, language, description), the form placed_first() takes."""
    n = info.get("cand") or info.get("main")
    try:
        channels = int(info.get("channels") or 0)
    except ValueError:
        channels = 0
    return (int(n), channels, is_lossless(info.get("codec"), info.get("profile")), info.get("language") or "und",
            f"{info.get('codec', '?')} {channels} ch")


def cross_language(main_info, cand_info):
    """'<candidate language> against <main language>' when both streams name a language and they differ, else ''."""
    a, b = str(cand_info.get("language") or ""), str(main_info.get("language") or "")
    if a in NO_LANGUAGE or b in NO_LANGUAGE or a == b:
        return ""
    return f"{a} against {b}"


def reverse_line(m):
    """The report's reverse line: the reverse run's measures and whether it has the commentary pattern."""
    r = reverse_measures(m)
    if r is None:
        return f"    reverse: {m['rev_gap_s']:.0f} s of gaps, AG {fmt(m['rev_ag'])}"
    if "error" in r:
        return f"    reverse: not judged ({r['error']})"
    why = not_judged(r).replace("the main track", "this track (the reverse run's reference)")
    return (f"    reverse: {r['gap_s']:.0f} s of gaps, DC {fmt(r['dc'])}, AG {fmt(r['ag'])}, tenths "
            f"{r['tenths_comm']}/{r['tenths']}, SMI {fmt(r['smi'])}, LS {fmt(r['ls_db'], 1)} dB, added {r['added_s']:.0f} s: "
            + ("commentary pattern (the main track looks like a commentary over this one)" if reverse_pattern(m)
               else f"not judged ({why})" if why else "no commentary pattern"))


def peer_line(peer):
    """The report's peer line: the cross-language candidate against the first stream of its own language."""
    m = peer["measures"]
    if "error" in m:
        return f"    peer: against stream {peer['stream']}: not judged ({m['error']})"
    return (f"    peer: against stream {peer['stream']} (placed first in the candidate's language): DC {fmt(m['dc'])}, "
            f"AG {fmt(m['ag'])}, tenths {m['tenths_comm']}/{m['tenths']}, SMI {fmt(m['smi'])}, LS {fmt(m['ls_db'], 1)} dB: "
            + ("commentary pattern" if peer["verdict"] == "commentary" else f"{peer['verdict']}, so not confirmed"))


def results(folder, rule=RULE, analyse_fn=None):
    """The measures and verdict of every candidate of a folder written by audio-levels.ps1, as data:
    {"folder", "manifest", "main" (its manifest fields), "frames", "dialogue" ("fc" or "mix"), "candidates": [
        {"stream", "info" (its manifest fields), "duplicate_of" (a stream number or None), "measures" (None for a
         duplicate), "verdict", "not_judged" (the reason, or ''), "cross_language" ('<cand> against <main>' or ''),
         "peer" (None, or {"stream", "measures", "verdict"}: the peer run of a cross-language candidate),
         "before_veto" (None, or the verdict rows 3 to 9 give when the veto replaced it)}],
     "veto": {"holds" (True, False, or None: not checked), "why" (the reason, naming the streams)}}.
    Raises ValueError with the refusal's reason. analyse_fn(main, cand, dialogue) replaces analyse (a cache, say).

    The position veto (row 0, owner decision D21) is read from the folder's streams.json: when it holds, every
    candidate that is not an exact duplicate gets reference-unclear (keep, review audio_reference_unclear), whatever
    rows 3 to 9 say, and "before_veto" keeps what they said. A duplicate stays a duplicate (row 2, not a content
    verdict).

    A cross-language candidate whose verdict would be commentary (addendum 9.2) is judged once more, against the stream
    8.6 places first among the folder's candidates of its own language (duplicates left out), with that stream's mixdown
    as the dialogue curve: the peer run, from the levels already in the folder. Its verdict is then note-commentary
    when the peer run has no commentary verdict (the candidate follows a programme track of its language: the 2.0
    companion of a dub), else commentary-cross-language. It is never commentary: content does not drop a candidate in
    another language than the reference."""
    d = pathlib.Path(folder)
    why = refusal(d)
    if why:
        raise ValueError(why)
    analyse_fn = analyse_fn or analyse
    man = read_manifest(d)
    main = read_levels(d / "main-mix.txt")
    fc_path = d / "main-fc.txt"
    fc = read_levels(fc_path) if fc_path.exists() else []
    if fc and max(fc) <= SILENT_DB:
        fc = []
    mainfo = man["main"] or {}
    holds, why_veto = position_veto(read_streams(d), int(mainfo["main"])) if mainfo.get("main") else (None, "not checked")
    out = {"folder": str(d), "manifest": man, "main": mainfo, "frames": len(main), "dialogue": "fc" if fc else "mix",
           "candidates": [], "veto": {"holds": holds, "why": why_veto}}
    seen = {mainfo["md5"]: mainfo.get("main", "?")} if mainfo.get("md5") else {}
    for c in man["cands"]:
        sid = c.get("cand")
        r = {"stream": sid, "info": c, "duplicate_of": None, "measures": None, "verdict": "duplicate", "not_judged": "",
             "cross_language": cross_language(mainfo, c), "peer": None, "before_veto": None}
        if c.get("md5") in seen:
            r["duplicate_of"] = seen[c["md5"]]
        else:
            if c.get("md5"):
                seen[c["md5"]] = sid
            m = analyse_fn(main, read_levels(d / f"cand-{sid}.txt"), fc or None)
            r["measures"], r["verdict"] = m, verdict(m, rule)
            r["not_judged"] = m["error"] if "error" in m else not_judged(m, rule)
        out["candidates"].append(r)
    for r in out["candidates"]:
        if r["verdict"] != "commentary" or not r["cross_language"]:
            continue
        own = [manifest_stream(x["info"]) for x in out["candidates"]
               if x["duplicate_of"] is None and x["info"].get("language") == r["info"].get("language")]
        peer = placed_first(own)[0]
        r["verdict"] = "commentary-cross-language"
        if peer != int(r["stream"]):
            m2 = analyse_fn(read_levels(d / f"cand-{peer}.txt"), read_levels(d / f"cand-{r['stream']}.txt"), None)
            v2 = verdict(m2, rule)
            r["peer"] = {"stream": str(peer), "measures": m2, "verdict": v2}
            if v2 != "commentary":
                r["verdict"] = "note-commentary"
    if holds:
        for r in out["candidates"]:
            if r["duplicate_of"] is None:
                r["before_veto"], r["verdict"] = r["verdict"], "reference-unclear"
    return out


def report(folder):
    res = results(folder)
    d = pathlib.Path(folder)
    man, mainfo = res["manifest"], res["main"]
    lines = [f"{d} ({man.get('file', '?')}, measured in {man.get('seconds', '?')} s)",
             f"  main stream {mainfo.get('main', '?')}: {mainfo.get('codec', '?')} {mainfo.get('channels', '?')} ch "
             f"{mainfo.get('layout', '')} {mainfo.get('language', '')}; dialogue reference: "
             f"{'its centre channel' if res['dialogue'] == 'fc' else 'its mono mixdown (no centre channel)'}; "
             f"{res['frames']} frames ({res['frames'] * FRAME_S / 60:.1f} min)"]
    veto = res["veto"]
    lines.append("  veto: " + ("holds (row 0, D21): " + veto["why"] + "; no commentary is confirmed on this title: every "
                               "candidate is reference-unclear" if veto["holds"]
                               else f"none (row 0, D21: {veto['why']})" if veto["holds"] is False
                               else f"{veto['why']} (row 0, D21)"))
    if not res["candidates"]:
        lines.append("  no candidate stream: nothing to judge (measured for the tuning dataset only)")
    label = f"{d.resolve().parent.name}/{man.get('file', d.resolve().name)}"
    for r in res["candidates"]:
        c, sid = r["info"], r["stream"]
        head = f"  stream {sid}: {c.get('codec', '?')} {c.get('channels', '?')} ch {c.get('layout', '')} {c.get('language', '')}" \
            + (f" (cross-language: {r['cross_language']})" if r["cross_language"] else "")
        if r["duplicate_of"] is not None:
            lines += [head, f"    verdict: duplicate of stream {r['duplicate_of']} (" + MEANING["duplicate"] + ")",
                      f"    row: | {label} | {sid} | same packets as stream {r['duplicate_of']} | duplicate |"]
            continue
        m, v, why = r["measures"], r["verdict"], r["not_judged"]
        lines.append(head)
        if "error" in m:
            lines.append(f"    verdict: not-judged ({m['error']})")
            continue
        lines += [
            f"    lag {m['lag_ms']} ms (envelope r {fmt(m['r_global'])}), overlap {fmt(m['overlap'])}, "
            f"main silent {fmt(m['main_silent'])}",
            f"    DC {fmt(m['dc'])}  AG {fmt(m['ag'])} ({m['gap_s']:.0f} s of gaps, {m['ag_s']:.0f} s added)  "
            f"tenths {m['tenths_comm']}/{m['tenths']}  SMI {fmt(m['smi'])}  LS {fmt(m['ls_db'], 1)} dB  "
            f"ADL {fmt(m['adl'])}  added {m['added_s']:.0f} s",
            reverse_line(m),
        ] + ([peer_line(r["peer"])] if r["peer"] else []) + [
            f"    verdict: {v} (" + (VETO_MEANING + f"; rows 3 to 9 alone: {r['before_veto']}" if r["before_veto"]
                                  else MEANING[v]) + ")" + (f"; not judged: {why}" if why else ""),
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
    mark '<- <language> x<n>' when one language has two or more, a mark '<- wanted x<n> (...)' when no wanted language
    has two but the wanted languages (English, the original language from tmdb.json) and streams with no language
    have two or more together (an English commentary on a Spanish film: EXP-13 addendum, section 9.1), the labels it
    was already ripped under (from each <label>/rip-summary.json), and the titles it shares segments with (a play-all
    playlist)."""
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
    wanted = {"eng"} | set(ISO639.get(original, ()))
    lines = [f"{d.name}: {len(info)} titles, longest first; TMDb original language: {original or 'unknown'}",
             "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)"]
    for title in sorted(info, key=lambda t: (-info[t][0], t)):
        seconds, segs, langs = info[title]
        audio = "; ".join(f"{lang} {','.join(map(str, s))}" for lang, s in sorted(langs.items())) or "none"
        notes = [f"<- {lang} x{len(s)}" for lang, s in sorted(langs.items()) if len(s) >= 2]
        mine = {lang: s for lang, s in sorted(langs.items()) if lang in wanted or lang in NO_LANGUAGE}
        if sum(len(s) for s in mine.values()) >= 2 and all(len(s) < 2 for s in mine.values()):
            notes.append(f"<- wanted x{sum(len(s) for s in mine.values())} ("
                         + ", ".join(f"{lang} {len(s)}" for lang, s in mine.items()) + ")")
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
