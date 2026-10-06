"""Tesseract's and ocrs's text of the same subtitle frames, side by side (the ocrs trial; decisions D17 and D19).

  python3 scripts/phase0/ocr_compare.py TESS.ocr.jsonl OCRS.jsonl [--rec RECDIR] [--tag eng]

TESS.ocr.jsonl is ocr-subs.ps1's sub-<n>.ocr.jsonl and OCRS.jsonl is ocrs-trial.ps1's ocrs-s<n>.jsonl for the same
stream. Both live in the tuning dataset (decision D16); the Mac holds temporary copies and never commits them.
RECDIR is the recording folder of that reading (corpus/owner/<slug>/<label>/subs-ocr): its sub-<n>.frames.json gives
the line counts of older Tesseract records that hold only ' | '-joined text, its summary.json Tesseract's run time,
its ocrs-summary.json ocrs's.

Frames are paired by file name (by frame number when a record has no file name). Both texts are folded to ASCII and,
for an English tag, corrected the way subtitle_text.py reads them (its fold and correct), so ocrs's ASCII-only alphabet
costs nothing on accents. Prints numbers only, never subtitle text: frames compared, frames with text in each engine,
the exact-match rate after folding, the character error rate (Levenshtein distance over the longer text's length), the
marker counts of subtitle_text.measure for each engine (CUE, LABEL, CHEV, NOTE, ODD, then M, SO, CS, readability,
the stopword language), the speed of each engine, and the 10 frames that differ most (lengths and CER only).

Exit status 0 when frames were compared, 2 when an input cannot be read or no frame is in both files.
Python 3.9 standard library. ASCII only.
"""
import argparse
import json
import pathlib
import re
import statistics
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import subtitle_text as st  # noqa: E402

SHOW_WORST = 10
CER_HIGH = 0.20


def levenshtein(a, b):
    """Edit distance (insert, delete, substitute; each costs 1) between two strings."""
    if len(a) < len(b):
        a, b = b, a
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def cer(a, b):
    """Levenshtein distance over the longer text's length: 0.0 for two equal texts, 1.0 when one is empty."""
    longer = max(len(a), len(b))
    return levenshtein(a, b) / longer if longer else 0.0


def norm_lines(lines, english=True):
    """The lines as subtitle_text.py reads them: folded to ASCII, then corrected for English (Tesseract's '|' for I)
    or with '|' taken as a space otherwise; spaces collapsed, empty lines left out."""
    counts = {}
    out = []
    for line in lines:
        s = st.fold(str(line))
        s = st.correct(s, counts) if english else s.replace('|', ' ')
        s = ' '.join(s.split())
        if s:
            out.append(s)
    return out


def norm_text(lines, english=True):
    """One string per frame: the normalised lines joined with a space, so that a different line split costs nothing."""
    return ' '.join(norm_lines(lines, english))


def read_frames(path, line_counts=None):
    """(frames, summary) of a JSON-lines file: frames = [{'frame', 'file', 'lines'}] in file order; summary = the
    {'summary': {...}} line of ocrs-trial, or None. A record with a 'lines' array is taken as it is; an older Tesseract
    record with only joined 'text' is split with subtitle_text.split_joined and the frame's line count, if known."""
    frames, summary = [], None
    for raw in pathlib.Path(path).read_text(encoding='utf-8-sig').splitlines():
        if not raw.strip():
            continue
        r = json.loads(raw)
        if 'summary' in r:
            summary = r['summary']
            continue
        if isinstance(r.get('lines'), list):
            lines = [str(x).strip() for x in r['lines'] if str(x).strip()]
        else:
            text = (r.get('text') or '').strip()
            lines = st.split_joined(text, (line_counts or {}).get(r.get('frame'))) if text else []
        frames.append({'frame': r.get('frame'), 'file': r.get('file'), 'lines': lines})
    return frames, summary


def pair_frames(a, b):
    """(pairs, method, only_a, only_b): pairs of records of the same frame, in a's order. By file name when every
    record has one, else by frame number."""
    method = 'file name' if all(r.get('file') for r in a + b) else 'frame number'
    key = 'file' if method == 'file name' else 'frame'
    by_key = {r.get(key): r for r in b}
    pairs = [(r, by_key[r.get(key)]) for r in a if r.get(key) in by_key]
    keys_a = {r.get(key) for r in a}
    return pairs, method, len(a) - len(pairs), sum(1 for r in b if r.get(key) not in keys_a)


def measure_engine(frames, tag):
    """subtitle_text.measure over one engine's frames with text, each frame taken as one event."""
    events = [{'start': float(k), 'dur': 0.0, 'lines': f['lines']} for k, f in enumerate(frames) if f['lines']]
    return st.measure({'n': 0, 'tag': tag, 'events': events})


def compare(tess, ocrs, tag='eng'):
    """Every number the report prints, from two lists of frame records."""
    english = tag in ('eng', 'en')
    pairs, method, only_t, only_o = pair_frames(tess, ocrs)
    rows = []
    for t, o in pairs:
        a, b = norm_text(t['lines'], english), norm_text(o['lines'], english)
        rows.append({'frame': t.get('frame'), 'file': t.get('file') or '-', 'a': len(a), 'b': len(b),
                     'la': len(norm_lines(t['lines'], english)), 'lb': len(norm_lines(o['lines'], english)),
                     'equal': a == b, 'cer': cer(a, b)})
    either = [r for r in rows if r['a'] or r['b']]
    both = [r for r in rows if r['a'] and r['b']]
    cers = [r['cer'] for r in either]
    return {
        'pairs': len(pairs), 'method': method, 'only_tess': only_t, 'only_ocrs': only_o,
        'text_tess': sum(1 for r in rows if r['a']), 'text_ocrs': sum(1 for r in rows if r['b']),
        'both': len(both), 'either': len(either), 'neither': len(rows) - len(either),
        'exact': sum(1 for r in either if r['equal']),
        'cer_mean': statistics.mean(cers) if cers else 0.0, 'cer_median': statistics.median(cers) if cers else 0.0,
        'cer_high': sum(1 for c in cers if c > CER_HIGH),
        'same_lines': sum(1 for r in both if r['la'] == r['lb']),
        'worst': sorted((r for r in either if r['cer'] > 0),
                        key=lambda r: (-r['cer'], -abs(r['a'] - r['b'])))[:SHOW_WORST],
        'm_tess': measure_engine([t for t, _ in pairs], tag), 'm_ocrs': measure_engine([o for _, o in pairs], tag),
    }


def rate(k, n):
    return '%.3f' % (k / n) if n else 'n/a'


def marker_line(m):
    shares = ' '.join('%s %.3f' % kv for kv in m['shares'].items())
    return ('events=%d CUE=%d LABEL=%d CHEV=%d NOTE=%d ODD=%d | M=%d SO=%d CS=%d readable=%s lang=%s (%s) class=%s' % (
        m['events'], m['CUE'], m['LABEL'], m['CHEV'], m['NOTE'], m['ODD'], m['M'], m['SO'], m['CS'],
        'yes' if m['readable'] else 'no', m['lang'], shares, m['class']))


def stream_number(*paths):
    """The stream number in sub-<n>.ocr.jsonl or ocrs-s<n>.jsonl, or None."""
    for p in paths:
        m = re.match(r'(?:sub-|ocrs-s)(\d+)\.', pathlib.Path(p).name)
        if m:
            return int(m.group(1))
    return None


def stream_entry(path, n):
    """The entry of stream n in a summary file's 'streams' list (summary.json, ocrs-summary.json), or None."""
    path = pathlib.Path(path)
    if n is None or not path.is_file():
        return None
    for s in st.read_json(path).get('streams', []):
        if int(s.get('n', -1)) == n:
            return s
    return None


def speed_line(rec, n, ocrs_summary):
    """Each engine's time per frame: Tesseract from summary.json (one call for every frame of the stream), ocrs
    from ocrs-summary.json or, without it, from the summary line of its output."""
    parts = []
    t = stream_entry(pathlib.Path(rec) / 'summary.json', n) if rec else None
    if t and t.get('frames') and t.get('tesseract_s') is not None:
        parts.append('tesseract %.1f ms/frame (summary.json: %.1f s for %d frames, one call)' % (
            1000.0 * float(t['tesseract_s']) / int(t['frames']), float(t['tesseract_s']), int(t['frames'])))
    else:
        parts.append('tesseract n/a (no --rec summary.json entry)')
    o = stream_entry(pathlib.Path(rec) / 'ocrs-summary.json', n) if rec else None
    o = o or ocrs_summary
    if o and o.get('ms_per_frame') is not None:
        parts.append('ocrs %.1f ms/frame over %d frames (+ %.1f ms model load)' % (
            float(o['ms_per_frame']), int(o.get('frames', 0)), float(o.get('load_ms') or 0.0)))
    else:
        parts.append('ocrs n/a (no summary)')
    return 'speed: ' + '; '.join(parts)


def report(c, out=print):
    out('paired by %s: %d frames; only in tesseract %d, only in ocrs %d' % (
        c['method'], c['pairs'], c['only_tess'], c['only_ocrs']))
    out('frames with text: tesseract %d, ocrs %d, both %d, either %d, neither %d' % (
        c['text_tess'], c['text_ocrs'], c['both'], c['either'], c['neither']))
    out('exact match after folding: %d of %d frames with text in either (%s)' % (
        c['exact'], c['either'], rate(c['exact'], c['either'])))
    out('character error rate (Levenshtein / longer text, folded): mean %.3f, median %.3f over %d frames; '
        'CER > %.2f on %d frames' % (c['cer_mean'], c['cer_median'], c['either'], CER_HIGH, c['cer_high']))
    out('same line count: %d of %d frames with text in both (%s)' % (
        c['same_lines'], c['both'], rate(c['same_lines'], c['both'])))
    out('markers (subtitle_text.measure, each frame with text one event):')
    out('  tesseract: ' + marker_line(c['m_tess']))
    out('  ocrs:      ' + marker_line(c['m_ocrs']))
    out('largest differences (frame, file, characters tesseract/ocrs, lines tesseract/ocrs, CER):')
    for r in c['worst']:
        out('  frame %s %s chars %d/%d lines %d/%d CER %.3f' % (
            r['frame'], r['file'], r['a'], r['b'], r['la'], r['lb'], r['cer']))
    if not c['worst']:
        out('  none: every frame with text reads the same')


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('tesseract', help="ocr-subs.ps1's sub-<n>.ocr.jsonl")
    ap.add_argument('ocrs', help="ocrs-trial.ps1's ocrs-s<n>.jsonl for the same stream")
    ap.add_argument('--rec', default=None,
                    help='the recording folder of that reading (corpus/owner/<slug>/<label>/subs-ocr)')
    ap.add_argument('--tag', default='eng',
                    help="the stream's language tag (default eng: Tesseract's corrections apply)")
    a = ap.parse_args(argv)
    n = stream_number(a.tesseract, a.ocrs)
    try:
        counts = {}
        frames_json = pathlib.Path(a.rec) / ('sub-%d.frames.json' % n) if a.rec and n is not None else None
        if frames_json and frames_json.is_file():
            counts = {f['frame']: f.get('lines') for f in st.read_json(frames_json)}
        tess, _ = read_frames(a.tesseract, counts)
        ocrs, ocrs_summary = read_frames(a.ocrs)
        c = compare(tess, ocrs, a.tag)
        speed = speed_line(a.rec, n, ocrs_summary)
    except (OSError, ValueError, KeyError, TypeError) as e:
        print('cannot read the input: %s' % e, file=sys.stderr)
        return 2
    if not c['pairs']:
        print('no frame is in both files (%d tesseract records, %d ocrs records)' % (len(tess), len(ocrs)),
              file=sys.stderr)
        return 2
    print('tesseract: %s: %d frames; ocrs: %s: %d frames (summary line: %s)' % (
        a.tesseract, len(tess), a.ocrs, len(ocrs), 'yes' if ocrs_summary else 'no'))
    report(c)
    print(speed)
    return 0


if __name__ == '__main__':
    sys.exit(main())
