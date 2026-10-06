"""Rebuild an audio-levels folder from a title's compact audio copy (the proxy), without the disc or the rip.

    proxy_levels.py <audio-proxy.mka> <new folder> --main <stream> [--cand <stream>[,<stream>...]]

The proxy is the Matroska file that audio-levels.ps1 writes to the tuning dataset on the share
($TuningData\\<slug>\\<label>\\audio-proxy.mka), with audio-proxy.json beside it. It holds every audio stream of the
ripped title as one mono track (its mixdown, series "mix"), for a stream whose layout has a centre channel besides
others a second mono track (that channel, "fc"), and for a two-channel stream a track with its side signal
0.5*L-0.5*R ("side", not used here: the level chain measures no width): FLAC, 8 kHz, 24 bit, stored gain_db (-18 dB)
below the signal the level chain measures. Each track carries the tags LAZYMKV_SOURCE_STREAM (ffprobe's stream number
in the rip) and LAZYMKV_SERIES.

Each track is read back in float, the stored gain is undone, and the samples are converted to the sample format the
source stream decodes to (sample_fmt in audio-proxy.json: s32p for DTS-HD MA, fltp for AC-3, s16 for 16-bit PCM, ...),
because ffmpeg's filters in the level chain compute in that format and 16-bit arithmetic reads quiet frames lower.
Then the exact chain of audio-levels.ps1 (aresample=8000, 300-3400 Hz, 160-sample frames, astats RMS level) runs over
the tracks in one ffmpeg pass, the levels are rounded as PowerShell does, and the same files are written:
manifest.txt (first line "# lazymkv audio-levels v2 (experiment 13)"), main-mix.txt, main-fc.txt (when the main
stream has a centre track), cand-<n>.txt, plus proxy.txt naming the proxy. The source facts and the packet MD5 of each
stream (so exact duplicates are still found) come from audio-proxy.json. audio_similarity.py reads the result like
any other folder. Measured (decision D16's design): on 48 kHz test files with 24-bit or float sources, the rebuilt
levels differ from those measured on the source by 0.1 dB in at most 0.16% of frames; on 16-bit sources (the 16 kHz
extracts of the two real cases, a 16-bit PCM 5.1 file) in up to 3.4% of frames, by up to 0.4 dB but never more than
0.1 dB above -60 dB. Silent-frame counts, verdicts and the rule's measures were the same in every case (r_global and
ADL in the fifth decimal, added sound by 0.04 s).
"""
import decimal
import json
import pathlib
import re
import subprocess
import sys

KEY = "lavfi.astats.Overall.RMS_level="
TAIL = ("aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,"
        "astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,"
        "ametadata=print:key=lavfi.astats.Overall.RMS_level:file=")
PROXY_FORMAT = "lazymkv audio proxy v1"


def word(value):
    text = "" if value is None else str(value)
    return re.sub(r"\s+", "_", text) if text else "-"


def convert_levels(raw, out, header):
    """ffmpeg's ametadata output to one value per line, 0.1 dB, floor -100, rounded half away from zero as .NET's
    ToString('0.0') does on the 6-decimal text. Returns the number of frames."""
    count = 0
    with open(raw, encoding="utf-8") as f, open(out, "w", encoding="utf-8", newline="\r\n") as w:
        w.write(header + "\n")
        for line in f:
            line = line.rstrip("\r\n")
            if not line.startswith(KEY):
                continue
            text = line[len(KEY):]
            value = decimal.Decimal("-100.0")
            if not ("inf" in text or "nan" in text):
                value = max(value, decimal.Decimal(text))
            w.write(str(value.quantize(decimal.Decimal("0.1"), rounding=decimal.ROUND_HALF_UP)) + "\n")
            count += 1
    return count


def proxy_tracks(mka):
    """{(source stream, series): proxy track number} from the proxy's own tags."""
    p = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=index,codec_type:stream_tags", "-of", "json",
                        str(mka)], capture_output=True, text=True)
    if p.returncode != 0:
        raise SystemExit(f"ffprobe cannot read {mka}: {p.stderr.strip()[:300]}")
    out = {}
    for s in json.loads(p.stdout).get("streams", []):
        tags = {k.upper(): v for k, v in (s.get("tags") or {}).items()}
        if s.get("codec_type") != "audio" or "LAZYMKV_SOURCE_STREAM" not in tags:
            continue
        out[(int(tags["LAZYMKV_SOURCE_STREAM"]), tags.get("LAZYMKV_SERIES", "mix"))] = s["index"]
    return out


def load_proxy_json(mka):
    path = pathlib.Path(mka).with_name("audio-proxy.json")
    if not path.is_file():
        raise SystemExit(f"{path} is missing: without it the proxy is incomplete (audio-levels.ps1 writes it last)")
    data = json.loads(path.read_text(encoding="utf-8-sig"))
    if data.get("format") != PROXY_FORMAT:
        raise SystemExit(f"{path}: format is {data.get('format')!r}, not {PROXY_FORMAT!r}")
    return data


def build(mka, out, main, cands, info=None):
    """Write a level folder for reference stream `main` and candidate streams `cands` from the proxy `mka`.
    info: the parsed audio-proxy.json (read from beside the proxy when None)."""
    mka, out = pathlib.Path(mka).resolve(), pathlib.Path(out)
    info = info if info is not None else load_proxy_json(mka)
    source = {t["source_stream"]: t.get("source", {}) for t in info.get("tracks", [])}
    gain = float(info.get("encoding", {}).get("gain_db", 0))
    tracks = proxy_tracks(mka)
    if out.exists():
        raise SystemExit(f"{out} already exists")
    for n in [main] + cands:
        if (n, "mix") not in tracks:
            raise SystemExit(f"the proxy has no track of stream {n} (it holds {sorted(set(s for s, _ in tracks))})")
    if main in cands or len(set(cands)) != len(cands):
        raise SystemExit("the main stream is also a candidate, or a candidate is listed twice")
    out.mkdir(parents=True)
    has_fc = (main, "fc") in tracks
    series = [("main-mix", tracks[(main, "mix")])] + ([("main-fc", tracks[(main, "fc")])] if has_fc else [])
    series += [(f"cand-{n}", tracks[(n, "mix")]) for n in cands]
    stream_of = {"main-mix": main, "main-fc": main}
    stream_of.update({f"cand-{n}": n for n in cands})

    def reader(name):
        fmt = source.get(stream_of[name], {}).get("sample_fmt") or "fltp"
        if not re.fullmatch(r"[a-z0-9]+", fmt):
            raise SystemExit(f"audio-proxy.json gives stream {stream_of[name]} the sample format {fmt!r}")
        undo = f",volume={-gain:g}dB" if gain else ""
        return f"aformat=sample_fmts=fltp:channel_layouts=mono{undo},aformat=sample_fmts={fmt},"

    graph = ";".join(f"[0:{k}]{reader(name)}{TAIL}raw-{name}.txt[o{i}]" for i, (name, k) in enumerate(series))
    args = ["ffmpeg", "-v", "error", "-nostdin", "-i", str(mka), "-filter_complex", graph]
    for i in range(len(series)):
        args += ["-map", f"[o{i}]", "-f", "null", f"o{i}"]
    run = subprocess.run(args, cwd=out, capture_output=True, text=True)
    (out / "ffmpeg.txt").write_text(run.stdout + run.stderr, encoding="utf-8")
    if run.returncode != 0:
        raise SystemExit(f"ffmpeg failed (exit {run.returncode}): {run.stderr.strip()[:300]}")
    frames = {}
    for name, _ in series:
        header = (f"# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series={name} "
                  f"file={word(mka.name)}")
        frames[name] = convert_levels(out / f"raw-{name}.txt", out / f"{name}.txt", header)
        (out / f"raw-{name}.txt").unlink()
    src = info.get("source", {})
    lines = ["# lazymkv audio-levels v2 (experiment 13)",
             f"file={word(src.get('file'))} bytes={src.get('bytes', 0)} seconds=0 frame_ms=20 band_hz=300-3400 "
             f"from_proxy={word(mka.name)}"]
    for n in [main] + cands:
        s = source.get(n, {})
        role = f"main={n}" if n == main else f"cand={n}"
        fc = (" fc=" + ("yes" if has_fc else "no")) if n == main else ""
        count = frames["main-mix"] if n == main else frames[f"cand-{n}"]
        guess = f" layout_guessed={s['layout_guessed']}" if s.get("layout_guessed") else ""
        lines.append(f"{role} codec={word(s.get('codec'))} profile={word(s.get('profile'))} channels={s.get('channels', 0)} "
                     f"layout={word(s.get('layout'))}{guess} language={word(s.get('language'))}{fc} frames={count} "
                     f"md5={s.get('md5') or 'unknown-' + str(n)}")
    (out / "manifest.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (out / "proxy.txt").write_text(f"levels rebuilt from {mka}\n", encoding="utf-8")
    return frames


def main(args):
    if len(args) < 4 or args[2] != "--main":
        sys.exit("usage: proxy_levels.py <audio-proxy.mka> <new folder> --main <stream> [--cand <stream>[,<stream>...]]")
    cands = []
    if len(args) >= 6 and args[4] == "--cand":
        cands = [int(x) for x in args[5].split(",") if x]
    frames = build(args[0], args[1], int(args[3]), cands)
    print(f"levels from {args[0]}: " + ", ".join(f"{k} {v} frames" for k, v in frames.items()) + f" -> {args[1]}")


if __name__ == "__main__":
    main(sys.argv[1:])
