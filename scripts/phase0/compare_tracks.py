"""Compare the streams MakeMKV listed for a title with the tracks in the ripped file."""
import argparse
import json
import pathlib
import sys

from robot import duration_seconds, parse

TYPE = {"Video": "video", "Audio": "audio", "Subtitles": "subtitles"}


def expected_rows(scan, title, selection):
    rows = []
    for (t, stream), a in sorted(scan.streams.items()):
        if t != title:
            continue
        kind = TYPE.get(a.get(1), a.get(1))
        flags = int(a.get(22, "0") or 0)
        lang = a.get(3) or a.get(28) or "und"
        if "MVC" in a.get(6, ""):
            continue
        if selection == "eng":
            if kind == "audio" and (lang not in ("eng", "und") or flags & 256 or flags & 7):
                continue
            if kind == "subtitles" and lang != "eng":
                continue
        rows.append(
            {"stream": stream, "type": kind, "lang": lang, "codec": a.get(6, ""),
             "out": a.get(41, ""), "ch": a.get(14, ""), "flags": flags}
        )
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scan")
    ap.add_argument("title", type=int)
    ap.add_argument("mkvmerge_json")
    ap.add_argument("--selection", choices=["all", "eng"], default="all")
    args = ap.parse_args()

    scan = parse(pathlib.Path(args.scan).read_text(encoding="utf-8", errors="replace"))
    ident = json.loads(pathlib.Path(args.mkvmerge_json).read_text(encoding="utf-8-sig"))
    want = expected_rows(scan, args.title, args.selection)
    got = [
        {"id": t["id"], "type": t["type"], "lang": t["properties"].get("language", "und"),
         "codec": t["codec"], "ch": t["properties"].get("audio_channels", ""),
         "tags": sorted(k for k in t["properties"] if k.startswith("tag_"))}
        for t in ident["tracks"]
    ]

    print(f"{'scan':<54} | file")
    for i in range(max(len(want), len(got))):
        w = want[i] if i < len(want) else None
        g = got[i] if i < len(got) else None
        left = f"{w['stream']:>2} {w['type']:<9} {w['lang']:<4} {w['codec']:<12} out={w['out']:<8} ch={w['ch']:<2} f={w['flags']}" if w else ""
        right = f"{g['id']:>2} {g['type']:<9} {g['lang']:<4} {g['codec']} ch={g['ch']}" if g else ""
        print(f"{left:<54} | {right}")

    order_ok = [(w["type"], w["lang"]) for w in want] == [(g["type"], g["lang"]) for g in got]
    title = scan.titles[args.title]
    file_chapters = sum(c.get("num_entries", 0) for c in ident.get("chapters", []))
    file_seconds = ident["container"]["properties"].get("duration", 0) / 1e9
    print()
    print(f"TRACK COUNT      scan={len(want)} file={len(got)}")
    print(f"ORDER MATCH      {'yes' if order_ok else 'no'} (type and language, position by position)")
    print(f"CHAPTERS         scan={title.get(8, '0')} file={file_chapters}")
    print(f"DURATION         scan={duration_seconds(title.get(9, '0:00:00'))}s file={file_seconds:.1f}s")
    print(f"VIDEO TRACKS     {sum(1 for g in got if g['type'] == 'video')}")
    print(f"STATISTICS TAGS  {'present' if any(g['tags'] for g in got) else 'absent'}")
    sys.exit(0 if order_ok and len(want) == len(got) else 1)


if __name__ == "__main__":
    main()
