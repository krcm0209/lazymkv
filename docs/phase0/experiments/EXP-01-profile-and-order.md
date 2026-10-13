# Experiment 1: is the job profile honoured, and does the file match the scan?

- Date: 2026-10-05 (mini PC's local clock)
- Disc(s): pans-labyrinth-2006-uhd, UHD (meta.json format bluray), drive D: (HL-DT-ST BD-RE BU40N)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 1

## Question

Is the job profile honoured in robot mode? Does output track order equal scan order, do output codecs match
attributes 41 and 42, and does the output chapter count equal attribute 8? Does a derived forced-only stream
directly follow its parent? On a Dolby Vision UHD, is the output one video track or two? Does MakeMKV write
statistics tags (subtitle counts) into its output?

## What was run

- Rehearsal: title K = 1 (`00055.mpls`, 0:05:40, 34 segments, video only; the only title other than the main
  feature) with `rip-title.ps1 -Label rehearsal`, then `probe-title.ps1`, then a second rip of the same label
  (refused, as the guard requires). 200 MB in 37 s; remux test 7 s, valid.
- Main feature: title N = 0 (`00800.mpls`, 1:59:10, 22 chapters, 51.7 GB in the scan) with
  `rip-title.ps1 -Slug pans-labyrinth-2006-uhd -Title 0 -Label all-tracks` (all-tracks profile, `--cache=1024`,
  `--debug`), then `probe-title.ps1 -Label all-tracks` (mkvmerge -J, ffprobe, MediaInfo, subtitle evidence
  extraction, full remux test), then `compare_tracks.py scan.txt 0 all-tracks/mkvmerge.json`.
- No other feature-length title exists on this disc (the scan lists two titles over 120 s).
- Rip: 54,346,666,920 bytes in 2245 s (23.1 MB/s overall, 23.3 MB/s while writing; saving began at 16.9 s);
  min free RAM 3.02 GB over 366 samples; success `MSG:5036`; no `MSG:1009`, no 5003/5004/5037, no read or write
  error messages. Probe: 2 subtitle tracks, evidence extraction 764 s (13.1 MB of packets), remux test 1610 s,
  exit 0, 54,332,206,575 bytes, valid, deleted.

## What was observed

`compare_tracks.py` (rip log read for `MSG:4047` removals):

```text
scan                                                   | file
 0 video     eng  MpegH        out=         ch=   f=0  |  0 video     eng  HEVC/H.265/MPEG-H ch=
 1 audio     spa  DTS-HD MA    out=         ch=6  f=1024 |  1 audio     spa  DTS-HD Master Audio ch=6
 2 audio     spa  DTS          out=         ch=6  f=2304 |  2 audio     spa  DTS ch=6
 3 audio     eng  DD           out=         ch=2  f=0  |  3 audio     eng  AC-3 ch=2
 4 subtitles eng  PGS          out=         ch=   f=0  |  4 subtitles eng  HDMV PGS ch=
 6 subtitles spa  PGS          out=         ch=   f=0  |  5 subtitles spa  HDMV PGS ch=

REMOVED          scan stream 5 (subtitles eng PGS f=6144): MakeMKV left it out of the file (MSG:4047), so it is not expected
REMOVED          scan stream 7 (subtitles spa PGS f=6144): MakeMKV left it out of the file (MSG:4047), so it is not expected
TRACK COUNT      scan=6 file=6
ORDER MATCH      yes (type, language except on video, audio channels; position by position)
CHAPTERS         scan=22 file=22
DURATION         scan=7150s file=7150.1s
VIDEO TRACKS     1
STATISTICS TAGS  present
VERDICT          count=yes order=yes video=yes chapters=yes duration=yes
```

1. **Profile honoured.** No `MSG:1009`; no message mentions the profile except `MSG:5015` ("Saving 1 titles into
   directory ... using profile 'lazymkv all tracks' from file ..."), which names it. The file has 6 tracks and the
   scan lists 8 streams: the two missing ones are the derived forced-only subtitle twins (flags 6144), which MakeMKV
   removed itself as empty (`MSG:4047` for streams 5 and 7). Every other stream, including the derived DTS core
   (flags 2304), is in the file. `TRACK COUNT scan=6 file=6` once those two are accounted for.
2. **Order.** `ORDER MATCH yes`: the file's tracks are in the scan's order.
3. **Output codecs.** The scan has attribute 42 `( Lossless conversion )` on all 8 streams and no attribute 41 on
   any (whole-scan count 0): MakeMKV 2.0.0 still gives 41 only to converted streams, and nothing was converted. By
   input codec (attributes 6/7): MpegH HEVC Main10 -> HEVC/H.265/MPEG-H; DTS-HD MA -> DTS-HD Master Audio (ffprobe
   profile `DTS-HD MA`, MediaInfo `DTS-HD Master Audio` / `XLL`); DTS core -> DTS; DD -> AC-3; PGS -> HDMV PGS. No
   LPCM and no closed-caption row on this disc. On short-circuit-2-1988-dvd (2026-10-12), in both rips
   (`all-tracks-2` and `eng-only`) MakeMKV sets the converted closed-caption SubRip track as the default subtitle
   track (`default_track` true in `mkvmerge.json`), while the scan marks the English VobSub (stream 5) default
   (attribute 38 `d`, 39 `Default`) and the closed-caption stream (12) not.
4. **Chapters.** Scan attribute 8 = 22; file 22. The default profile's `insertFirstChapter00IfMissing="true"`
   added none here.
5. **Forced-only streams.** In the scan, each `f=6144` twin directly follows the non-derived subtitle stream of
   its language (stream 5 after 4, eng; stream 7 after 6, spa). Both twins were empty on this disc and MakeMKV
   left them out of the file.
6. **Dolby Vision.** `VIDEO TRACKS 1`. The scan's video codec text is `MpegH HEVC Main10@L5.1`, 3840x2160; ffprobe
   shows `yuv420p10le`, `color_transfer smpte2084` and no Dolby Vision side data: this disc is HDR10 without Dolby
   Vision, so the two-track question is not answered by it.
7. **Statistics tags.** `STATISTICS TAGS present`: every track has `BPS`, `DURATION`, `NUMBER_OF_FRAMES`,
   `NUMBER_OF_BYTES` and `SOURCE_ID` (the stream's PID: video `001011`, audio `001100`/`001100`/`001101`, subtitles
   `0012A0`/`0012A1`), so a subtitle track's event count is in the header.
8. **Duration.** `DURATION scan=7150s file=7150.1s`, `duration=yes` (within 2 s).
9. **Language codes.** The file carries `spa` and `eng`, the same ISO 639-2 form as the scan (terminology and
   bibliographic forms coincide for these two languages; the question stays open for French or German).
10. **Title identity.** `rip_source_file` = `scan_source_file` = `00800.mpls`. `mediainfo.json` reads as UTF-8
    (with a BOM).

Also observed: the two derived cores on both discs so far are written as separate tracks by `+sel:all`; and the
remux log stayed empty until mkvmerge finished (it does not flush progress to a pipe), so a remux cannot be
watched through its output.

Later observation (2026-10-12, short-circuit-2-1988-dvd, title 0, all-tracks profile, label `all-tracks-2`): on a
DVD with a closed-caption stream, MakeMKV's output order is not the scan order. The scan lists the video, four
audio streams (1 to 4), seven VobSub streams (5 to 11) and then the closed-caption stream (12: codec `CC`,
attribute 41 `Text`, 42 `( Lossy conversion )`). The file carries that CC->Text track as SubRip/SRT at track 5,
immediately after the audio tracks and before every VobSub track, and `compare_tracks.py` gives
`count=yes order=no`. The English-only rip of the same title has the same order; EXP-11 (observation 5 and
section 1b) has the detail. Observation 2's "order matches the scan" held on pans-labyrinth-2006-uhd, a UHD
without such a stream. The Answer below stands for that disc, but the order rule needs this exception: a
text-converted subtitle track comes before the bitmap subtitle tracks. The same file gives observation 3 the
closed-caption row it lacked: attribute 41 `Text` is written as SubRip/SRT (`S_TEXT/UTF8`).

Later observation (2026-10-12, short-circuit-2-1988-dvd, title 0): the duration does not match within 2 s on this
DVD. The scan gives the title `TINFO:0,9,0,"1:50:06"` (6606 s); both files' `mkvmerge.json` give the container
`duration` 6623617000000 ns (6623.6 s), in the `all-tracks-2` rip and in the `eng-only` rip alike. Each file is
17.6 s longer than the scan, and `compare_tracks.py` gives `duration=no` on both. So the 2-second duration rule of
6.2 step 7 (observation 8) does not hold on this DVD; it is not an effect of the profile, since both rips agree.

## Answer

**Yes:** the profile was honoured, the output track order equals the scan order, every output codec is the
unconverted input codec (no attribute 41, 42 lossless throughout), and the chapter count equals attribute 8.
Forced-only twins follow their parents in the scan; this disc's were empty and were removed by MakeMKV, which
says so with `MSG:4047`. Statistics tags are written. The Dolby Vision question is not answered by this disc.

## Consequence for the spec

- 9.3 keeps the profile path (no fallback needed).
- 8.7's parent rule holds (the twin directly follows its parent); add: a twin that turns out empty is not in the
  file, announced by `MSG:4047`, and the verification must not count it as missing.
- 8.10: wording for Dolby Vision waits for a Dolby Vision disc.
- 6.4 step 10: subtitle event counts can be read from `NUMBER_OF_FRAMES` in the header without extraction (the
  alignment measure still needs the packets).
- 6.2 step 7: the 2-second duration rule held on pans-labyrinth-2006-uhd (UHD, 0.1 s) and on the Blu-ray episodes
  (0.9 s), and failed by 17.6 s on a DVD (short-circuit-2-1988-dvd, 6623.6 s against the scan's 6606 s, in both
  rips). The rule needs a DVD-sized tolerance or a different reference (the file's own duration from the probe);
  to be settled in Task 17.
- 18 (timing, on this hardware with a 54 GB UHD on the NAS): rip 37 min, subtitle evidence 13 min, remux 27 min.
