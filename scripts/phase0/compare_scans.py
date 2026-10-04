"""Compare the title lists of two recorded scans by source file, segment map and duration."""
import pathlib
import sys
from collections import Counter

from robot import parse


def titles(path):
    """One (key, comment) pair per title; the key is (source file, segment map, duration)."""
    scan = parse(pathlib.Path(path).read_text(encoding="utf-8", errors="replace"))
    return [((a.get(16), a.get(26), a.get(9)), a.get(49, "")) for a in scan.titles.values()]


def compare(a, b):
    """Titles are compared as multisets, so a title listed twice on one side is a difference."""
    count_a, count_b = Counter(key for key, _ in a), Counter(key for key, _ in b)
    comments_a, comments_b = {}, {}
    for key, comment in a:
        comments_a.setdefault(key, []).append(comment)
    for key, comment in b:
        comments_b.setdefault(key, []).append(comment)
    return {
        "only_a": sorted(str(key) for key in (count_a - count_b).elements()),
        "only_b": sorted(str(key) for key in (count_b - count_a).elements()),
        "comment_differs": sorted(
            str(key) for key in set(comments_a) & set(comments_b)
            if sorted(comments_a[key]) != sorted(comments_b[key])
        ),
        "common": sum((count_a & count_b).values()),
    }


def main(path_a, path_b):
    a, b = titles(path_a), titles(path_b)
    result = compare(a, b)
    print(f"titles: A={len(a)} B={len(b)} common={result['common']}")
    for name, key in (("only in A", "only_a"), ("only in B", "only_b"), ("comment differs", "comment_differs")):
        print(f"{name}: {len(result[key])}")
        for item in result[key]:
            print(f"  {item}")
    if not a or not b:
        print("EMPTY SCAN: a scan with no titles cannot be compared")
        return 1
    return 0 if not result["only_a"] and not result["only_b"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
