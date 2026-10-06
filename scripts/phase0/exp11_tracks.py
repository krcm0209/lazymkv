"""Experiment 11 (spec section 17, row 11): which streams of a title put the eng-only selection to the
test, and which of the title's streams a rip made with that selection kept.

  exp11_tracks.py coverage <recording dir or scan file>[@<title>] ...
      With @<title>: every stream of that title (attributes 3 and 28, codec, channels, flags, and whether
      compare_tracks' eng prediction keeps it), then its COVERAGE line. Without @<title>: one COVERAGE
      line for each title of 20 minutes or more. A recording dir is read as <dir>/scan.txt.
  exp11_tracks.py match <recording dir> <title> <label>
      Pairs the tracks of <dir>/<label>/mkvmerge.json, in order, with the scan streams they can be
      (type, language, audio channels, as compare_tracks does), and names each predicted stream that is
      missing and each unpredicted one that is present. Then it lists the file's tracks, and those of
      <dir>/all-tracks/mkvmerge.json when that rip is of the same title, for checking by eye.

COVERAGE counts the streams that put each part of the selection to the test: audio-lang (audio with a
language code other than English), nolang (audio with no language code, or und), sub-lang (subtitles
not in English), core (audio flag 256), special (audio flags 1, 2 or 4). Two more are only observed:
sub-flagged (subtitles with flags 1, 2 or 4) and audio-512 (secondary audio, flag 512).
Both commands exit 0 unless their input is missing or broken.
"""
import argparse
import json
import pathlib
import sys

from compare_tracks import expected_rows, file_rows, same_track
from robot import duration_seconds, parse

TOKENS = ("audio-lang", "nolang", "sub-lang", "core", "special", "sub-flagged", "audio-512")
LONG_TITLE_S = 20 * 60


def scan_path(where):
    path = pathlib.Path(where)
    return path / "scan.txt" if path.is_dir() else path


def load_scan(where):
    return parse(scan_path(where).read_text(encoding="utf-8-sig", errors="replace"))


def coverage(scan, title):
    """How many of the title's streams test each token (a stream's language is its attribute 3)."""
    n = dict.fromkeys(TOKENS, 0)
    for (t, _), a in sorted(scan.streams.items()):
        if t != title:
            continue
        kind, lang, flags = a.get(1, ""), (a.get(3) or "").lower(), int(a.get(22, "0") or 0)
        if kind == "Audio":
            if lang in ("", "und"):
                n["nolang"] += 1
            elif lang != "eng":
                n["audio-lang"] += 1
            n["core"] += bool(flags & 256)
            n["special"] += bool(flags & 7)
            n["audio-512"] += bool(flags & 512)
        elif kind == "Subtitles":
            n["sub-lang"] += lang != "eng"
            n["sub-flagged"] += bool(flags & 7)
    return n


def match(scan, title, ident):
    """(tag, scan row or None, file row or None) for every file track and every missing predicted stream.

    Each file track, in order, takes the next scan stream it can be. Tags: predicted, EXTRA (present but
    not predicted), MISSING (predicted but skipped), UNMATCHED (no later scan stream can be this track).
    """
    every = expected_rows(scan, title, "all")
    predicted = {r["stream"] for r in expected_rows(scan, title, "eng")}
    rows, i = [], 0
    for g in file_rows(ident):
        j = next((k for k in range(i, len(every)) if same_track(every[k], g)), None)
        if j is None:
            rows.append(("UNMATCHED", None, g))
            continue
        rows += [("MISSING", r, None) for r in every[i:j] if r["stream"] in predicted]
        rows.append(("predicted" if every[j]["stream"] in predicted else "EXTRA", every[j], g))
        i = j + 1
    rows += [("MISSING", r, None) for r in every[i:] if r["stream"] in predicted]
    return rows


def describe(track):
    p = track["properties"]
    return (f"{track['id']:>2} {track['type']:<9} {p.get('language', 'und'):<4} {track['codec']:<22} "
            f"ch={p.get('audio_channels', '')!s:<2} forced={p.get('forced_track')} "
            f"bytes={p.get('tag_number_of_bytes', '-')} name={p.get('track_name', '')!r}")


def run_coverage(args):
    for arg in args:
        where, _, title = arg.rpartition("@") if "@" in arg else (arg, "", "")
        if not scan_path(where).is_file():
            print(f"{where}: no scan.txt")
            continue
        scan = load_scan(where)
        if not title:
            for t, a in sorted(scan.titles.items()):
                try:
                    long_enough = duration_seconds(a.get(9, "0:00:00")) >= LONG_TITLE_S
                except ValueError:
                    long_enough = False
                if long_enough:
                    n = coverage(scan, t)
                    print(f"COVERAGE {where} title {t} ({a.get(9)}): " + " ".join(f"{k}={v}" for k, v in n.items()))
            continue
        t = int(title)
        if t not in scan.titles:
            sys.exit(f"{where}: the scan lists no title {t}")
        kept = {r["stream"] for r in expected_rows(scan, t, "eng")}
        print(f"{where} title {t} ({scan.titles[t].get(9, '?')}), every stream:")
        for (tt, s), a in sorted(scan.streams.items()):
            if tt == t:
                flags = int(a.get(22, "0") or 0)
                print(f"  stream {s:>2} {a.get(1, ''):<9} attr3={a.get(3)!r:<7} attr28={a.get(28)!r:<7} "
                      f"{a.get(6, ''):<8} ch={a.get(14, ''):<2} flags={flags:<5} "
                      f"predicted: {'keep' if s in kept else 'drop'}")
        n = coverage(scan, t)
        print(f"COVERAGE {where} title {t}: " + " ".join(f"{k}={v}" for k, v in n.items()))


def run_match(folder, title, label):
    d = pathlib.Path(folder)
    scan = load_scan(d)
    if title not in scan.titles:
        sys.exit(f"{d / 'scan.txt'} lists no title {title}")
    ident = json.loads((d / label / "mkvmerge.json").read_text(encoding="utf-8-sig"))
    every = expected_rows(scan, title, "all")
    rows = match(scan, title, ident)
    print(f"title {title}: {len(every)} streams without 3D video, {len(expected_rows(scan, title, 'eng'))} "
          f"predicted; {len(ident['tracks'])} tracks in {label}/mkvmerge.json")
    for tag, r, g in rows:
        if r:
            line = f"scan {r['stream']:>2} {r['type']:<9} {r['lang_raw']:<4} {r['codec']:<10} ch={r['ch']!s:<2} f={r['flags']}"
            if g:
                line += f"  <- file {g['id']:>2} {g['codec']}"
        else:
            line = f"file {g['id']:>2} {g['type']:<9} {g['lang_raw']:<4} {g['codec']} ch={g['ch']}"
        print(f"{tag:<10} {line}")
    counts = {t: sum(1 for row in rows if row[0] == t) for t in ("MISSING", "EXTRA", "UNMATCHED")}
    print("DIFFERENCES " + " ".join(f"{t.lower()}={n}" for t, n in counts.items()))
    print()
    print(f"{label} tracks, for checking the lines above by eye:")
    for track in ident["tracks"]:
        print("  " + describe(track))
    base, summary = d / "all-tracks" / "mkvmerge.json", d / "all-tracks" / "rip-summary.json"
    if not (base.is_file() and summary.is_file()
            and json.loads(summary.read_text(encoding="utf-8-sig")).get("title") == title):
        print("no all-tracks rip of this title to compare with")
        return
    tracks = json.loads(base.read_text(encoding="utf-8-sig"))["tracks"]
    print("all-tracks tracks of the same title; 'scan' is the stream at the same position, which is that")
    print("track when compare_tracks gave the all-tracks rip count=yes and order=yes:")
    for k, track in enumerate(tracks):
        stream = every[k]["stream"] if len(tracks) == len(every) else "?"
        print(f"  scan {stream!s:>2}  " + describe(track))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="command", required=True)
    cov = sub.add_parser("coverage")
    cov.add_argument("where", nargs="+")
    mat = sub.add_parser("match")
    mat.add_argument("folder")
    mat.add_argument("title", type=int)
    mat.add_argument("label")
    args = ap.parse_args()
    if args.command == "coverage":
        run_coverage(args.where)
    else:
        run_match(args.folder, args.title, args.label)


if __name__ == "__main__":
    main()
