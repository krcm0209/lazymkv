"""Experiment 10: how many recorded discs does TheDiscDB know?"""
import json
import pathlib
import sys


def hit(path):
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return False
    if isinstance(data, list):
        return len(data) > 0
    return isinstance(data, dict) and bool(data.get("titles"))


def main(root):
    rows = []
    for disc in sorted(p for p in pathlib.Path(root).iterdir() if (p / "meta.json").exists()):
        meta = json.loads((disc / "meta.json").read_text(encoding="utf-8-sig"))
        by_hash = hit(disc / "thediscdb-hash.json")
        by_id = hit(disc / "thediscdb-discid.json")
        rows.append((disc.name, meta.get("format"), by_hash, by_id))
        print(f"{disc.name:<40} {str(meta.get('format')):<7} hash={'hit' if by_hash else 'miss':<4} discid={'hit' if by_id else 'miss'}")
    known = sum(1 for _, _, h, i in rows if h or i)
    print(f"\nTheDiscDB knows {known} of {len(rows)} discs")


if __name__ == "__main__":
    main(sys.argv[1])
