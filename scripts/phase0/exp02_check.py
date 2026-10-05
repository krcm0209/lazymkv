"""Experiment 2: do the 3307/3308 title numbers of a rip call equal the index ripped?"""
import pathlib
import re
import sys

from robot import parse


def check_run(scan, title, run):
    """The facts for one rip call of `title` that was stopped after the saving message.

    The saving message is MSG:5014, or MSG:5015 when a profile is in use (MakeMKV 2.0.0). The
    printed labels keep the name 5014 and are true for either code.
    """
    codes = [code for code, _, _ in run.messages]
    added = {}  # title number -> every file added under that number in this run
    for code, _, params in run.messages:
        if code in (3307, 3308) and len(params) >= 2 and params[-1].isdigit():
            added.setdefault(int(params[-1]), set()).add(params[0])
    added_at = [i for i, code in enumerate(codes) if code in (3307, 3308)]
    expected = scan.titles.get(title, {}).get(16)
    saving_at = [i for i, code in enumerate(codes) if code in (5014, 5015)]
    saw_5014 = bool(saving_at)
    # Nothing passes by default: a title the scan does not list, a run with no 3307/3308
    # line, and a title number added for two different files are all a "NO".
    before = saw_5014 and bool(added_at) and all(i < saving_at[0] for i in added_at)
    match = expected is not None and added.get(title) == {expected}
    return {
        "expected": expected, "added": "/".join(sorted(added.get(title, ()))) or None, "match": match, "saw_5014": saw_5014,
        "before": before, "n3308": codes.count(3308), "n3309": codes.count(3309),
    }


def main(recording):
    d = pathlib.Path(recording)
    scan = parse((d / "scan.txt").read_text(encoding="utf-8", errors="replace"))
    runs = sorted((d / "exp02").glob("title-[0-9]*.txt"), key=lambda p: int(re.search(r"\d+", p.stem).group()))
    all_ok = bool(runs)
    for path in runs:
        title = int(re.search(r"\d+", path.stem).group())
        r = check_run(scan, title, parse(path.read_text(encoding="utf-8-sig", errors="replace")))
        all_ok = all_ok and r["saw_5014"] and r["before"] and r["match"]
        print(
            f"title {title:>3}: scan source={r['expected']} rip #{title}={r['added']} "
            f"match={'yes' if r['match'] else 'NO'} 5014={'yes' if r['saw_5014'] else 'NO'} "
            f"added-before-5014={'yes' if r['before'] else 'NO'} 3308-lines={r['n3308']} 3309-lines={r['n3309']}"
        )
    print(f"\nEXPERIMENT 2: {'yes' if all_ok else 'no'} ({len(runs)} titles)")
    return all_ok


if __name__ == "__main__":
    sys.exit(0 if main(sys.argv[1]) else 1)
