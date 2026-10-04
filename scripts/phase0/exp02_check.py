"""Experiment 2: do the 3307/3308 title numbers of a rip call equal the index ripped?"""
import pathlib
import re
import sys

from robot import parse


def main(recording):
    d = pathlib.Path(recording)
    scan = parse((d / "scan.txt").read_text(encoding="utf-8", errors="replace"))
    all_ok = True
    runs = sorted((d / "exp02").glob("title-*.txt"), key=lambda p: int(re.search(r"\d+", p.stem).group()))
    for path in runs:
        title = int(re.search(r"\d+", path.stem).group())
        run = parse(path.read_text(encoding="utf-8-sig", errors="replace"))
        codes = [code for code, _, _ in run.messages]
        added = {
            int(params[-1]): params[0]
            for code, _, params in run.messages
            if code in (3307, 3308) and len(params) >= 2 and params[-1].isdigit()
        }
        expected = scan.titles.get(title, {}).get(16)
        saw_5014 = 5014 in codes
        before = saw_5014 and all(
            i < codes.index(5014) for i, code in enumerate(codes) if code in (3307, 3308)
        )
        match = added.get(title) == expected
        all_ok = all_ok and saw_5014 and before and match
        print(
            f"title {title:>3}: scan source={expected} rip #{title}={added.get(title)} "
            f"match={'yes' if match else 'NO'} 5014={'yes' if saw_5014 else 'NO'} "
            f"added-before-5014={'yes' if before else 'NO'} 3308-lines={codes.count(3308)} 3309-lines={codes.count(3309)}"
        )
    print(f"\nEXPERIMENT 2: {'yes' if all_ok and runs else 'no'} ({len(runs)} titles)")


if __name__ == "__main__":
    main(sys.argv[1])
