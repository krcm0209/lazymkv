"""Print what a recorded scan says about a disc."""
import pathlib
import sys

from robot import parse


def main(recording):
    scan = parse((pathlib.Path(recording) / "scan.txt").read_text(encoding="utf-8", errors="replace"))
    version = next((p[0] for code, _, p in scan.messages if code == 1005 and p), "?")
    java = next((p[0] for code, _, p in scan.messages if code == 3344 and p), None)
    print(f"MakeMKV: {version}")
    print(f"Java in use: {java or 'no (MSG:3344 absent)'}")
    print(f"Disc: type={scan.disc.get(1)} name={scan.disc.get(2)} volume={scan.disc.get(32)}")
    codes = sorted({code for code, _, _ in scan.messages})
    print(f"Message codes seen: {codes}")
    print()
    print(f"{'#':>3} {'source':<14} {'duration':>9} {'chap':>4} {'bytes':>13} {'segs':>4} {'aud':>3} {'sub':>3}  comment / segment map")
    for title in sorted(scan.titles):
        a = scan.titles[title]
        kinds = [s.get(1) for (t, _), s in scan.streams.items() if t == title]
        print(
            f"{title:>3} {a.get(16, ''):<14} {a.get(9, ''):>9} {a.get(8, '0'):>4} {a.get(11, ''):>13} "
            f"{a.get(25, ''):>4} {kinds.count('Audio'):>3} {kinds.count('Subtitles'):>3}  "
            f"{a.get(49, '')} [{a.get(26, '')[:60]}]"
        )


if __name__ == "__main__":
    main(sys.argv[1])
