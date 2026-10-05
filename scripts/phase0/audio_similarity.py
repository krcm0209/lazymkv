"""Compare a candidate audio track with the main track by content.

Reads the per-100-ms RMS level files written by audio-levels.ps1 (main-mix, main-fc, cand-mix,
cand-mid, cand-side) and prints the measures that tell a commentary from another copy or mix of the
same programme:

  same-programme   correlation of the two loudness curves; near 1.0 for a copy or another mix
  fills-the-gaps   share of the main track's quiet moments in which the candidate is active;
                   a commentary or an audio description talks where the programme is quiet
  active share     how much of the running time each track is active; speech tracks rarely pause
  range            spread of the loudness curve (95th minus 10th percentile); narrow for speech
  side-mid         level of (left - right) against (left + right); very low for a mono voice
"""
import math
import pathlib
import sys

FLOOR = -90.0  # dB; digital silence is reported as -inf


def levels(path):
    """One RMS level in dB per 100 ms frame."""
    out = []
    for line in pathlib.Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("lavfi.astats.Overall.RMS_level="):
            value = line.split("=", 1)[1]
            out.append(FLOOR if "inf" in value or "nan" in value else max(FLOOR, float(value)))
    return out


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(share * len(ordered)))] if ordered else FLOOR


def active(values, below_loud_db=25.0):
    """Frames within below_loud_db of the track's own loud level (its 95th percentile)."""
    threshold = percentile(values, 0.95) - below_loud_db
    return [v > threshold for v in values]


QUIET = -70.0  # dB; below this a frame counts as silent, whatever the exact figure


def correlation(a, b):
    """Pearson correlation of two loudness curves, with everything below QUIET treated alike.

    Every frame counts: the moments in which both tracks are silent together are part of what
    makes two tracks the same programme.
    """
    pairs = [(max(x, QUIET), max(y, QUIET)) for x, y in zip(a, b)]
    if len(pairs) < 2:
        return 0.0
    mean_x = sum(x for x, _ in pairs) / len(pairs)
    mean_y = sum(y for _, y in pairs) / len(pairs)
    cov = sum((x - mean_x) * (y - mean_y) for x, y in pairs)
    var_x = sum((x - mean_x) ** 2 for x, _ in pairs)
    var_y = sum((y - mean_y) ** 2 for _, y in pairs)
    return cov / math.sqrt(var_x * var_y) if var_x and var_y else 0.0


def share(flags):
    return sum(flags) / len(flags) if flags else 0.0


def measures(main_mix, main_dialogue, cand_mix, cand_mid, cand_side):
    n = min(len(main_mix), len(cand_mix))
    main_mix, cand_mix = main_mix[:n], cand_mix[:n]
    main_dialogue = (main_dialogue or main_mix)[:n]
    main_on, dialogue_on, cand_on = active(main_mix), active(main_dialogue), active(cand_mix)
    quiet = [i for i in range(n) if not main_on[i]]
    no_dialogue = [i for i in range(n) if not dialogue_on[i]]
    voiced = [i for i in range(min(n, len(cand_mid), len(cand_side))) if cand_on[i]]
    side_mid = sorted(cand_side[i] - cand_mid[i] for i in voiced)
    return {
        "frames": n,
        "same_programme": correlation(main_mix, cand_mix),
        "fills_quiet": share([cand_on[i] for i in quiet]),
        "fills_no_dialogue": share([cand_on[i] for i in no_dialogue]),
        "quiet_share_of_main": len(quiet) / n if n else 0.0,
        "main_active": share(main_on),
        "cand_active": share(cand_on),
        "main_range_db": percentile(main_mix, 0.95) - percentile(main_mix, 0.10),
        "cand_range_db": percentile(cand_mix, 0.95) - percentile(cand_mix, 0.10),
        "side_minus_mid_db": side_mid[len(side_mid) // 2] if side_mid else 0.0,
    }


def load(folder):
    d = pathlib.Path(folder)
    fc = levels(d / "main-fc.txt") if (d / "main-fc.txt").exists() else []
    if fc and max(fc) <= FLOOR:
        fc = []  # the main stream has no centre channel
    return (levels(d / "main-mix.txt"), fc, levels(d / "cand-mix.txt"),
            levels(d / "cand-mid.txt"), levels(d / "cand-side.txt"))


def main(folders):
    for folder in folders:
        m = measures(*load(folder))
        print(folder)
        print(f"  frames={m['frames']} ({m['frames'] / 600:.1f} min)")
        print(f"  same-programme correlation    {m['same_programme']:.2f}")
        print(f"  fills-the-gaps (main quiet)   {m['fills_quiet']:.2f}  (main is quiet {m['quiet_share_of_main']:.0%} of the time)")
        print(f"  fills-the-gaps (no dialogue)  {m['fills_no_dialogue']:.2f}")
        print(f"  active share                  main {m['main_active']:.2f}  candidate {m['cand_active']:.2f}")
        print(f"  range (dB)                    main {m['main_range_db']:.1f}  candidate {m['cand_range_db']:.1f}")
        print(f"  side-mid (dB)                 {m['side_minus_mid_db']:.1f}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: audio_similarity.py <audio-levels folder> ...")
    main(sys.argv[1:])
