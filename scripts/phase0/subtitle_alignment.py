"""Event counts and start-time alignment between subtitle streams (spec 8.7).

Each input is the JSON printed by
  ffprobe -v error -select_streams s:N -show_entries packet=pts_time,duration_time,size -of json FILE
"""
import argparse
import bisect
import json
import pathlib


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inputs", nargs="+", help="name=path to a packets JSON file")
    ap.add_argument("--clear-max-bytes", type=int, default=64)
    ap.add_argument("--tolerance-ms", type=int, default=300)
    args = ap.parse_args()

    tracks = {}
    for item in args.inputs:
        name, _, path = item.partition("=")
        packets = json.loads(pathlib.Path(path).read_text(encoding="utf-8-sig")).get("packets", [])
        sizes = [int(p.get("size", 0)) for p in packets]
        buckets = [sum(1 for s in sizes if s <= limit) for limit in (32, 64, 128, 256)]
        print(f"{name}: packets={len(packets)} sized<=32/64/128/256: {buckets}")
        tracks[name] = events(packets, args.clear_max_bytes)

    print()
    for name, starts in tracks.items():
        print(f"{name}: events={len(starts)}")
    print()
    names = list(tracks)
    tolerance = args.tolerance_ms / 1000
    print("aligned(row, column):")
    print(" " * 16 + "".join(f"{n[:12]:>14}" for n in names))
    for a in names:
        row = "".join(f"{aligned(tracks[a], tracks[b], tolerance):>14.2f}" for b in names)
        print(f"{a[:16]:<16}{row}")


if __name__ == "__main__":
    main()
