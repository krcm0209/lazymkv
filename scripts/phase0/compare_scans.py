"""Compare the title lists of two recorded scans by source file, segment map and duration."""
import pathlib
import sys

from robot import parse


def titles(path):
    scan = parse(pathlib.Path(path).read_text(encoding="utf-8", errors="replace"))
    return {(a.get(16), a.get(26), a.get(9)): a.get(49, "") for a in scan.titles.values()}


def main(path_a, path_b):
    a, b = titles(path_a), titles(path_b)
    only_a = sorted(str(k) for k in set(a) - set(b))
    only_b = sorted(str(k) for k in set(b) - set(a))
    labels_differ = sorted(str(k) for k in set(a) & set(b) if a[k] != b[k])
    print(f"titles: A={len(a)} B={len(b)} common={len(set(a) & set(b))}")
    for name, items in (("only in A", only_a), ("only in B", only_b), ("comment differs", labels_differ)):
        print(f"{name}: {len(items)}")
        for item in items:
            print(f"  {item}")
    sys.exit(0 if not only_a and not only_b else 1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
