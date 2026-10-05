"""Event counts and start-time alignment between subtitle streams (spec 8.7).

Each input is the JSON printed by
  ffprobe -v error -select_streams s:N -show_entries packet=pts_time,duration_time,size -of json FILE

The inputs are named in one of two ways:
  name=path ...   packets files given one by one; every stream ignores packets of
                  --clear-max-bytes or less.
  --subs-dir DIR  a subs folder written by probe-title.ps1 (index.json and sub-<n>.packets.json).
                  Only English and no-language streams are compared. Image streams (PGS, VobSub)
                  ignore packets of --clear-max-bytes or less; text streams, such as the DVD
                  closed-caption stream that MakeMKV converts to SubRip, count every packet as an
                  event (spec 8.7).
"""
import argparse
import bisect
import json
import pathlib

# Words in mkvmerge's codec names of text subtitles: "SubRip/SRT", "SubStationAlpha", "WebVTT", "HDMV TextST".
TEXT_CODECS = ("SubRip", "SRT", "SubStation", "WebVTT", "Text")


def events(packets, clear_max_bytes):
    """Start times, in seconds, of packets large enough to show something."""
    return sorted(
        float(p["pts_time"])
        for p in packets
        if "pts_time" in p and int(p.get("size", 0)) > clear_max_bytes
    )


def aligned(a, b, tolerance_s):
    """Share of the events in a that start within tolerance_s of some event in b."""
    if not a:
        return 0.0
    hits = 0
    for start in a:
        i = bisect.bisect_left(b, start - tolerance_s)
        if i < len(b) and b[i] <= start + tolerance_s:
            hits += 1
    return hits / len(a)


def is_text(codec):
    """True when mkvmerge's codec name is a text subtitle format; PGS and VobSub are images."""
    return any(word in (codec or "") for word in TEXT_CODECS)


def index_streams(subs_dir):
    """The English and no-language streams of a subs folder, as (name, packets path, is text, index entry)."""
    subs = pathlib.Path(subs_dir)
    entries = json.loads((subs / "index.json").read_text(encoding="utf-8-sig", errors="replace"))
    streams = []
    for entry in entries:
        language = entry.get("language") or "und"
        if language not in ("eng", "und"):
            continue
        text = is_text(entry.get("codec"))
        name = f"s{entry['n']}-{language}-{'text' if text else 'image'}" + ("-forced" if entry.get("forced") else "")
        streams.append((name, subs / f"sub-{entry['n']}.packets.json", text, entry))
    return streams


def load_packets(path):
    return json.loads(pathlib.Path(path).read_text(encoding="utf-8-sig")).get("packets", [])


def buckets(packets):
    sizes = [int(p.get("size", 0)) for p in packets]
    return [sum(1 for s in sizes if s <= limit) for limit in (32, 64, 128, 256)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inputs", nargs="*", help="name=path to a packets JSON file")
    ap.add_argument("--subs-dir", help="a subs folder written by probe-title.ps1 (index.json, sub-<n>.packets.json)")
    ap.add_argument("--clear-max-bytes", type=int, default=64,
                    help="packets this small are ignored; with --subs-dir, on image streams only")
    ap.add_argument("--tolerance-ms", type=int, default=300)
    args = ap.parse_args()
    if bool(args.inputs) == bool(args.subs_dir):
        ap.error("give either name=path inputs or --subs-dir")

    tracks = {}
    if args.subs_dir:
        if not (pathlib.Path(args.subs_dir) / "index.json").exists():
            ap.error(f"no {args.subs_dir}/index.json: the title has no subtitle track, or the recording was not pulled")
        streams = index_streams(args.subs_dir)
        if not streams:
            ap.error(f"{args.subs_dir}/index.json lists no English or no-language subtitle stream")
        for name, path, text, entry in streams:
            packets = load_packets(path)
            print(f"{name}: codec={entry.get('codec')} track_name={entry.get('track_name')!r} "
                  f"packets={len(packets)} sized<=32/64/128/256: {buckets(packets)}")
            tracks[name] = events(packets, 0 if text else args.clear_max_bytes)
        rule = f"image streams ignore packets of {args.clear_max_bytes} bytes or less, text streams count every packet"
    else:
        for item in args.inputs:
            name, _, path = item.partition("=")
            packets = load_packets(path)
            print(f"{name}: packets={len(packets)} sized<=32/64/128/256: {buckets(packets)}")
            tracks[name] = events(packets, args.clear_max_bytes)
        rule = f"packets of {args.clear_max_bytes} bytes or less are ignored"

    print()
    for name, starts in tracks.items():
        print(f"{name}: events={len(starts)}")
    print()
    names = list(tracks)
    tolerance = args.tolerance_ms / 1000
    width = max([8] + [len(n) + 2 for n in names])
    print(f"aligned(row, column); {rule}; tolerance {args.tolerance_ms} ms:")
    print(" " * width + "".join(f"{n:>{width}}" for n in names))
    for a in names:
        row = "".join(f"{aligned(tracks[a], tracks[b], tolerance):>{width}.2f}" for b in names)
        print(f"{a:<{width}}{row}")
    print()
    print("pairs, with the larger of aligned(A, B) and aligned(B, A):")
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            larger = max(aligned(tracks[a], tracks[b], tolerance), aligned(tracks[b], tracks[a], tolerance))
            print(f"{a:<{width}}{b:<{width}}{larger:.2f}")


if __name__ == "__main__":
    main()
