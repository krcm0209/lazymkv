"""Experiment 10: how many recorded discs does TheDiscDB know?"""
import json
import pathlib
import sys


def statuses(disc):
    """HTTP status per lookup, from the recorder's lookup-status.txt, e.g. {"dischash": "200"}."""
    try:
        lines = (disc / "lookup-status.txt").read_text(encoding="utf-8-sig").splitlines()
    except OSError:
        return {}
    return {parts[0]: parts[-1] for parts in (line.split() for line in lines) if len(parts) >= 2}


def lookup(path, status):
    """"hit", "miss", "failed" or "n/a". Only an answer from TheDiscDB counts as a miss."""
    if status is None:
        return "n/a"  # the lookup was not made, for example the disc has no disc ID
    if status == "404":
        return "miss"
    if status != "200":
        return "failed"  # network error, rate limit or server error
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return "failed"
    if isinstance(data, list):
        return "hit" if data else "miss"
    if isinstance(data, dict) and data.get("titles"):
        return "hit"
    return "failed"  # a 200 answer that is not a disc record


def main(root):
    rows = []
    for disc in sorted(p for p in pathlib.Path(root).iterdir() if (p / "meta.json").exists()):
        meta = json.loads((disc / "meta.json").read_text(encoding="utf-8-sig"))
        status = statuses(disc)
        by_hash = lookup(disc / "thediscdb-hash.json", status.get("dischash"))
        by_id = lookup(disc / "thediscdb-discid.json", status.get("discid"))
        rows.append((by_hash, by_id))
        print(f"{disc.name:<40} {str(meta.get('format')):<7} hash={by_hash:<6} discid={by_id}")
    answered = [by_hash for by_hash, _ in rows if by_hash in ("hit", "miss")]
    unanswered = len(rows) - len(answered)
    either = sum(1 for by_hash, by_id in rows if "hit" in (by_hash, by_id))
    print(f"\nTheDiscDB knows {answered.count('hit')} of {len(answered)} discs by fingerprint")
    print(f"by fingerprint or disc ID: {either} of {len(rows)} discs")
    if unanswered:
        print(f"{unanswered} fingerprint lookups failed or were not made; they are not counted as misses")
    return 0 if rows and not unanswered else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
