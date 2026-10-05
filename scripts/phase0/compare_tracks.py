"""Compare the streams MakeMKV listed for a title with the tracks in the ripped file."""
import argparse
import json
import pathlib
import sys

from robot import duration_seconds, parse

TYPE = {"Video": "video", "Audio": "audio", "Subtitles": "subtitles"}

# ISO 639-2 bibliographic codes and their terminology twins. MakeMKV's scan prints the
# terminology form (fra, deu); a Matroska file may carry either.
B_TO_T = {
    "alb": "sqi", "arm": "hye", "baq": "eus", "bur": "mya", "chi": "zho", "cze": "ces",
    "dut": "nld", "fre": "fra", "geo": "kat", "ger": "deu", "gre": "ell", "ice": "isl",
    "mac": "mkd", "mao": "mri", "may": "msa", "per": "fas", "rum": "ron", "slo": "slk",
    "tib": "bod", "wel": "cym",
}
DURATION_TOLERANCE_S = 2


def norm_lang(code):
    code = (code or "und").lower()
    return B_TO_T.get(code, code)


def expected_rows(scan, title, selection):
    rows = []
    for (t, stream), a in sorted(scan.streams.items()):
        if t != title:
            continue
        kind = TYPE.get(a.get(1), a.get(1))
        flags = int(a.get(22, "0") or 0)
        # A stream's language is attribute 3 (spec 8.6). Attribute 28 is the disc's metadata language
        # ("eng" even on the Chinese subtitles of the pinned John Wick scan), so it stands in only for
        # video, which has no attribute 3 and whose language is never compared.
        lang_raw = a.get(3) or (a.get(28) if kind == "video" else None) or "und"
        lang = norm_lang(lang_raw)
        if "MVC" in a.get(6, ""):
            continue
        if selection == "eng":
            if kind == "audio" and (lang not in ("eng", "und") or flags & 256 or flags & 7):
                continue
            if kind == "subtitles" and lang != "eng":
                continue
        rows.append(
            {"stream": stream, "type": kind, "lang": lang, "lang_raw": lang_raw, "codec": a.get(6, ""),
             "out": a.get(41, ""), "ch": a.get(14, ""), "flags": flags}
        )
    return rows


# MakeMKV's own log of the call that wrote the file, as the phase 0 scripts name it.
RIP_LOG_NAMES = ("rip.txt", "extract.txt")


def removed_streams(rip):
    """Stream numbers MakeMKV left out of the output, from the rip log's MSG:4047 lines.

    MakeMKV drops a forced-only subtitle track that turns out to be empty and says so:
    "AV sync issue in stream N at T : Forced subtitles track #N turned out to be empty and
    was removed from output file". Other MSG:4047 lines are not removals.
    """
    return {
        int(params[0])
        for code, text, params in rip.messages
        if code == 4047 and "removed from output file" in text and params and params[0].isdigit()
    }


def without_removed(rows, removed):
    """Split the predicted rows into those still expected in the file and those MakeMKV removed."""
    return [r for r in rows if r["stream"] not in removed], [r for r in rows if r["stream"] in removed]


def file_rows(ident):
    return [
        {"id": t["id"], "type": t["type"], "lang": norm_lang(t["properties"].get("language")),
         "lang_raw": t["properties"].get("language", "und"),
         "codec": t["codec"], "ch": t["properties"].get("audio_channels", ""),
         "tags": sorted(k for k in t["properties"] if k.startswith("tag_"))}
        for t in ident["tracks"]
    ]


def same_track(w, g):
    """Type must agree; language too, except on video; audio channel counts when both sides state one."""
    if w["type"] != g["type"]:
        return False
    if w["type"] != "video" and w["lang"] != g["lang"]:
        return False
    if w["type"] == "audio" and str(w["ch"]) and str(g["ch"]):
        return str(w["ch"]) == str(g["ch"])
    return True


def checks(want, got, title, ident):
    """The yes/no answers and the numbers behind them. Nothing passes on empty input."""
    numbers = {
        "scan_chapters": int(title.get(8, "0") or 0),
        "file_chapters": sum(c.get("num_entries", 0) for c in ident.get("chapters", [])),
        "scan_seconds": duration_seconds(title.get(9, "0:00:00")),
        "file_seconds": ident.get("container", {}).get("properties", {}).get("duration", 0) / 1e9,
    }
    same_count = len(want) == len(got) and len(got) > 0
    verdicts = {
        "count": same_count,
        "order": same_count and all(same_track(w, g) for w, g in zip(want, got)),
        "video": any(g["type"] == "video" for g in got),
        "chapters": numbers["scan_chapters"] == numbers["file_chapters"],
        "duration": numbers["scan_seconds"] > 0
        and abs(numbers["scan_seconds"] - numbers["file_seconds"]) <= DURATION_TOLERANCE_S,
    }
    return verdicts, numbers


def yes_no(ok):
    return "yes" if ok else "no"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scan")
    ap.add_argument("title", type=int)
    ap.add_argument("mkvmerge_json")
    ap.add_argument("--selection", choices=["all", "eng"], default="all")
    ap.add_argument("--rip-log", help="MakeMKV's log of the rip; default: rip.txt or extract.txt beside the mkvmerge json")
    args = ap.parse_args()

    scan = parse(pathlib.Path(args.scan).read_text(encoding="utf-8", errors="replace"))
    ident = json.loads(pathlib.Path(args.mkvmerge_json).read_text(encoding="utf-8-sig"))
    want = expected_rows(scan, args.title, args.selection)
    got = file_rows(ident)

    folder = pathlib.Path(args.mkvmerge_json).parent
    rip_log = pathlib.Path(args.rip_log) if args.rip_log else next(
        (folder / name for name in RIP_LOG_NAMES if (folder / name).exists()), None)
    removed = set()
    if rip_log:
        removed = removed_streams(parse(rip_log.read_text(encoding="utf-8", errors="replace")))
    want, dropped = without_removed(want, removed)

    print(f"{'scan':<54} | file")
    for i in range(max(len(want), len(got))):
        w = want[i] if i < len(want) else None
        g = got[i] if i < len(got) else None
        left = f"{w['stream']:>2} {w['type']:<9} {w['lang_raw']:<4} {w['codec']:<12} out={w['out']:<8} ch={w['ch']:<2} f={w['flags']}" if w else ""
        right = f"{g['id']:>2} {g['type']:<9} {g['lang_raw']:<4} {g['codec']} ch={g['ch']}" if g else ""
        print(f"{left:<54} | {right}")

    verdicts, n = checks(want, got, scan.titles[args.title], ident)
    print()
    print(f"RIP LOG          {rip_log if rip_log else 'none found: tracks MakeMKV removed (MSG:4047) are not accounted for'}")
    for r in dropped:
        print(f"REMOVED          scan stream {r['stream']} ({r['type']} {r['lang_raw']} {r['codec']} f={r['flags']}): "
              "MakeMKV left it out of the file (MSG:4047), so it is not expected")
    for n in sorted(removed - {r["stream"] for r in dropped}):
        print(f"REMOVED          MSG:4047 names stream {n}, which the prediction does not include")
    print(f"TRACK COUNT      scan={len(want)} file={len(got)}")
    print(f"ORDER MATCH      {yes_no(verdicts['order'])} (type, language except on video, audio channels; position by position)")
    print(f"CHAPTERS         scan={n['scan_chapters']} file={n['file_chapters']}")
    print(f"DURATION         scan={n['scan_seconds']}s file={n['file_seconds']:.1f}s")
    print(f"VIDEO TRACKS     {sum(1 for g in got if g['type'] == 'video')}")
    print(f"STATISTICS TAGS  {'present' if any(g['tags'] for g in got) else 'absent'}")
    print("VERDICT          " + " ".join(f"{name}={yes_no(ok)}" for name, ok in verdicts.items()))
    print("                 codecs are not compared: read the scan's out= column against the file's codec column")
    sys.exit(0 if all(verdicts.values()) else 1)


if __name__ == "__main__":
    main()
