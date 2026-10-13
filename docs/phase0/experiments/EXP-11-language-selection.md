# Experiment 11: does a selection by language rip exactly the predicted streams?

- Date: 2026-10-06 (Disc A) and 2026-10-12 (the DVD of Step 7), mini PC's local clock
- Disc(s): pans-labyrinth-2006-uhd, UHD (meta.json format bluray), drive 1 (`D:`, HL-DT-ST BD-RE BU40N); and
  short-circuit-2-1988-dvd, DVD (meta.json format dvd), drive 2 (`D:`, Pioneer BDR-XS07U). A DVD with a flagged
  commentary (Step 7a) has not been supplied yet.
- MakeMKV version: MakeMKV v2.0.0 win(x64-release) (the `MSG:1005` line of both rips)
- Spec reference: section 17, row 11

## Question

With a profile that selects by language (wanted-language and no-language audio, English subtitles, no derived
cores, no flagged commentary), does MakeMKV rip exactly the streams predicted from the scan, and in what order?
The answer decides whether the rip-time fast path (section 16, item 1) can be built.

## What was run

- Profile: `C:\lazymkv\phase0\profiles\eng-only.mmcp.xml`, built in Task 7 by `lazymkv-capture profile` from
  MakeMKV's `default.mmcp.xml`, name `lazymkv eng only`. Its selection string, as in the plan's Step 2:

  ```text
  -sel:all,+sel:video,+sel:(audio*(eng|nolang)),+sel:(subtitle*eng),-sel:mvcvideo,-sel:core,-sel:special,=100:all
  ```

  Step 2's `findstr` output was not copied into the repository at the time. Re-read on 2026-10-06 over ssh
  (`type C:\lazymkv\phase0\profiles\eng-only.mmcp.xml`, read-only): the four `defaultSelection` attributes
  (`copy`, `lpcm`, `wavex`, `convertToSRT`) all carry exactly the string above; the file's only other selection
  string is MakeMKV's own `app_DefaultSelectionString` in a comment that the file says MakeMKV ignores. MakeMKV
  accepted the profile, so no `SELECTION_2` was needed (observation 4).
- The prediction is `compare_tracks.py --selection eng` applied to the scan: every video stream that is not 3D;
  audio whose attribute 3 is `eng`, empty, absent or `und`, except flags 256 and 1, 2, 4; every English subtitle
  stream, whatever its flags.
- Disc A, title N = 0 (`00800.mpls`, 1:59:10, 22 chapters), label `eng-only`:
  `rip-title.ps1 -Slug pans-labyrinth-2006-uhd -Title 0 -Label eng-only -Profile C:\lazymkv\phase0\profiles\eng-only.mmcp.xml`
  as job `rip-eng-only`, started 2026-10-06 10:02:49 (`--cache=1024`, `--debug`). Then
  `probe-title.ps1 -Slug pans-labyrinth-2006-uhd -Label eng-only -SkipRemux` (job `probe-eng-only`), pulled to
  `corpus/owner/pans-labyrinth-2006-uhd/eng-only/`.
- `rip-summary.json`: `ok` true, `elapsed_s` 2245, `bytes` 49,848,443,686. The all-tracks rip of the same title
  (EXP-01) is 54,346,666,920 bytes, also in 2245 s. Ratio 0.917; difference 4,498,223,234 bytes. Also: success
  `MSG:5036`, `fail_codes` empty, `profile_errors` 0, no read or write errors, saving began at 16.8 s, 21.2 MB/s
  overall (all-tracks 23.1), min free RAM 2.97 GB over 365 samples, `rip_source_file` = `scan_source_file` =
  `00800.mpls`. `probe-summary.json`: `ok` true, 1 subtitle track, evidence extraction 699 s, remux skipped.
- Where the bytes went: the three all-tracks tracks the eng-only file lacks carry, by their `NUMBER_OF_BYTES`
  tags, 3,132,770,444 (spa DTS-HD MA) + 1,348,695,912 (spa DTS core) + 14,610,116 (spa PGS) = 4,496,076,472
  bytes, 99.95 % of the difference. The other 2,146,762 bytes are in no track's tag: each file's size minus its
  tracks' tags is 4,484,039 bytes (all-tracks) and 2,337,277 bytes (eng-only), so they are container framing. The
  three kept tracks have the same byte counts in both files.
- Read on the Mac, one command at a time (compare_tracks.py as of commit f850ba0, observation 6):

  ```bash
  python3 scripts/phase0/exp11_tracks.py coverage corpus/owner/pans-labyrinth-2006-uhd@0
  python3 scripts/phase0/compare_tracks.py corpus/owner/pans-labyrinth-2006-uhd/scan.txt 0 corpus/owner/pans-labyrinth-2006-uhd/eng-only/mkvmerge.json --selection eng
  python3 scripts/phase0/compare_tracks.py corpus/owner/pans-labyrinth-2006-uhd/scan.txt 0 corpus/owner/pans-labyrinth-2006-uhd/all-tracks/mkvmerge.json
  python3 scripts/phase0/exp11_tracks.py match corpus/owner/pans-labyrinth-2006-uhd 0 eng-only
  grep -E '^MSG:' corpus/owner/pans-labyrinth-2006-uhd/eng-only/rip.txt | cut -d, -f1,4 | sort | uniq -c
  ```

- Second disc (Step 3). Disc A's line has `nolang=0` and `special=0` (and, of the observed-only counts,
  `sub-flagged=0` and `audio-512=0`), so rule 2 does not apply. Rule 3: the only other recording with a scan at
  the time, `the-office-2005-s3d3-bluray`, has seven titles of 20 minutes or more, each with `nolang=0` and
  `special=0` (also `audio-lang=0` and `sub-lang=0`), so no recorded disc qualified. Rule 4 applies: `special` is
  0 on every line, so Step 7 first records a DVD whose packaging lists an audio commentary. The owner has since
  supplied two DVDs, both recorded in Task 15, and neither flags a stream. `coverage` on every recording
  (2026-10-12): `short-circuit-1986-dvd` title 0 (video and two English audio streams) has every count 0;
  `short-circuit-2-1988-dvd` title 0 has `audio-lang=3 nolang=0 sub-lang=6 core=0 special=0`. With no line above
  0 for `special`, Step 7a leaves `special` "not exercised". Step 7 was run on short-circuit-2-1988-dvd's feature
  all the same (the controller's plan for this disc), T_D = 0: it puts the two language filters to the test on a
  DVD, with four audio languages and seven bitmap subtitle languages, and adds a closed-caption stream under
  `subtitle*eng`. A DVD with a flagged commentary and a disc with a language-less audio stream are still needed
  for `special` and `nolang`.
- The DVD, title T_D = 0 (the feature, 1:50:06, 28 chapters), label `eng-only`:
  `rip-title.ps1 -Slug short-circuit-2-1988-dvd -Title 0 -Label eng-only -Profile C:\lazymkv\phase0\profiles\eng-only.mmcp.xml`,
  started 2026-10-12 20:17:24 (`--cache=1024`, debug logging on: `MSG:1004`). Then
  `probe-title.ps1 -Slug short-circuit-2-1988-dvd -Label eng-only -SkipRemux`, pulled to
  `corpus/owner/short-circuit-2-1988-dvd/eng-only/`.
- `rip-summary.json`: `ok` true, `elapsed_s` 563, `bytes` 3,338,409,955. The all-tracks rip of the same title,
  label `all-tracks-2`, is 3,835,570,741 bytes in 567 s. Ratio 0.870; difference 497,160,786 bytes. Also:
  `MSG:5005` and success `MSG:5036`, `fail_codes` empty, `profile_errors` 0, `read_errors` 0, `write_errors` 0,
  saving began at 65.6 s, 5.7 MB/s overall (all-tracks-2 6.4), min free RAM 3.16 GB over 92 samples;
  `scan_source_file` and `rip_source_file` null (a DVD scan has no attribute 16, so nothing is compared).
  `probe-summary.json`: `ok` true, 2 subtitle tracks, evidence extraction 50 s, remux skipped.
- Where the bytes went: the nine all-tracks-2 tracks the eng-only file lacks carry, by their `NUMBER_OF_BYTES`
  tags, 3 x 158,966,784 (fre, spa and por AC-3 2.0) + 18,347,494 (the six other-language VobSub tracks) =
  495,247,846 bytes, 99.62 % of the difference. The other 1,912,940 bytes are in no track's tag: file size minus
  the tracks' tags is 6,010,270 bytes (all-tracks-2) and 4,097,330 bytes (eng-only), container framing. The four
  kept tracks have the same byte counts in both files.
- The DVD's first all-tracks rip (label `all-tracks`, 2026-10-06) failed on an uncorrectable sector (24 read
  errors at one offset of `VTS_01_1.VOB`, `MSG:5003`, `5004`, `5037`; MakeMKV removed its file) and is kept as
  evidence; after the owner cleaned the disc, `all-tracks-2` succeeded on 2026-10-12 with 3 read errors of the
  scrambled-sector kind the scan also logged (detail in `corpus/owner/short-circuit-2-1988-dvd/notes.md`).
- Read on the Mac for the DVD, one command at a time:

  ```bash
  python3 scripts/phase0/exp11_tracks.py coverage corpus/owner/short-circuit-2-1988-dvd@0
  python3 scripts/phase0/compare_tracks.py corpus/owner/short-circuit-2-1988-dvd/scan.txt 0 corpus/owner/short-circuit-2-1988-dvd/eng-only/mkvmerge.json --selection eng
  python3 scripts/phase0/compare_tracks.py corpus/owner/short-circuit-2-1988-dvd/scan.txt 0 corpus/owner/short-circuit-2-1988-dvd/all-tracks-2/mkvmerge.json
  python3 scripts/phase0/exp11_tracks.py match corpus/owner/short-circuit-2-1988-dvd 0 eng-only
  grep -E '^MSG:' corpus/owner/short-circuit-2-1988-dvd/eng-only/rip.txt | cut -d, -f1,4 | sort | uniq -c
  ```

  and Step 6's `bytes:` one-liner with `all-tracks-2` in place of `all-tracks`, which prints
  `bytes: all-tracks-2 3835570741 eng-only 3338409955`. `match` lists all-tracks tracks only from
  `all-tracks/mkvmerge.json`, which the failed attempt never wrote, so the all-tracks-2 tracks were listed from
  their own `mkvmerge.json` for the check by eye (section 1b).

## What was observed

### 1a. pans-labyrinth-2006-uhd, title 0

Streams and coverage (Step 3):

```text
corpus/owner/pans-labyrinth-2006-uhd title 0 (1:59:10), every stream:
  stream  0 Video     attr3=None    attr28='eng'   MpegH    ch=   flags=0     predicted: keep
  stream  1 Audio     attr3='spa'   attr28='eng'   DTS-HD MA ch=6  flags=1024  predicted: drop
  stream  2 Audio     attr3='spa'   attr28='eng'   DTS      ch=6  flags=2304  predicted: drop
  stream  3 Audio     attr3='eng'   attr28='eng'   DD       ch=2  flags=0     predicted: keep
  stream  4 Subtitles attr3='eng'   attr28='eng'   PGS      ch=   flags=0     predicted: keep
  stream  5 Subtitles attr3='eng'   attr28='eng'   PGS      ch=   flags=6144  predicted: keep
  stream  6 Subtitles attr3='spa'   attr28='eng'   PGS      ch=   flags=0     predicted: drop
  stream  7 Subtitles attr3='spa'   attr28='eng'   PGS      ch=   flags=6144  predicted: drop
COVERAGE corpus/owner/pans-labyrinth-2006-uhd title 0: audio-lang=2 nolang=0 sub-lang=2 core=1 special=0 sub-flagged=0 audio-512=0
```

`compare_tracks.py ... eng-only/mkvmerge.json --selection eng` (exit 0):

```text
scan                                                   | file
 0 video     eng  MpegH        out=         ch=   f=0  |  0 video     eng  HEVC/H.265/MPEG-H ch=
 3 audio     eng  DD           out=         ch=2  f=0  |  1 audio     eng  AC-3 ch=2
 4 subtitles eng  PGS          out=         ch=   f=0  |  2 subtitles eng  HDMV PGS ch=

RIP LOG          corpus/owner/pans-labyrinth-2006-uhd/eng-only/rip.txt
REMOVED          scan stream 5 (subtitles eng PGS f=6144): MakeMKV left it out of the file (MSG:4047), so it is not expected
TRACK COUNT      scan=3 file=3
ORDER MATCH      yes (type, language except on video, audio channels; position by position)
CHAPTERS         scan=22 file=22
DURATION         scan=7150s file=7150.1s
VIDEO TRACKS     1
STATISTICS TAGS  present
VERDICT          count=yes order=yes video=yes chapters=yes duration=yes
```

The all-tracks rip of the same title, re-read with the same script (exit 0): `REMOVED` scan streams 5 and 7 (both
`MSG:4047`), `TRACK COUNT scan=6 file=6`, and
`VERDICT count=yes order=yes video=yes chapters=yes duration=yes`, as in EXP-01. Its tracks 0 to 5 are therefore
scan streams 0, 1, 2, 3, 4 and 6.

`exp11_tracks.py match ... 0 eng-only`, as printed:

```text
title 0: 8 streams without 3D video, 4 predicted; 3 tracks in eng-only/mkvmerge.json
predicted  scan  0 video     eng  MpegH      ch=   f=0  <- file  0 HEVC/H.265/MPEG-H
predicted  scan  3 audio     eng  DD         ch=2  f=0  <- file  1 AC-3
predicted  scan  4 subtitles eng  PGS        ch=   f=0  <- file  2 HDMV PGS
MISSING    scan  5 subtitles eng  PGS        ch=   f=6144
DIFFERENCES missing=1 extra=0 unmatched=0

eng-only tracks, for checking the lines above by eye:
   0 video     eng  HEVC/H.265/MPEG-H      ch=   forced=False bytes=49661338222 name=''
   1 audio     eng  AC-3                   ch=2  forced=False bytes=171603456 name='Stereo'
   2 subtitles eng  HDMV PGS               ch=   forced=False bytes=13164731 name=''
all-tracks tracks of the same title; 'scan' is the stream at the same position, which is that
track when compare_tracks gave the all-tracks rip count=yes and order=yes:
  scan  ?   0 video     eng  HEVC/H.265/MPEG-H      ch=   forced=False bytes=49661338222 name=''
  scan  ?   1 audio     spa  DTS-HD Master Audio    ch=6  forced=False bytes=3132770444 name='Surround 5.1'
  scan  ?   2 audio     spa  DTS                    ch=6  forced=False bytes=1348695912 name='Surround 5.1'
  scan  ?   3 audio     eng  AC-3                   ch=2  forced=False bytes=171603456 name='Stereo'
  scan  ?   4 subtitles eng  HDMV PGS               ch=   forced=False bytes=13164731 name=''
  scan  ?   5 subtitles spa  HDMV PGS               ch=   forced=False bytes=14610116 name=''
```

The check by eye:

- `match` does not read the rip log, so it neither knows of `MSG:4047` removals nor can fill its `scan` column
  for the all-tracks file (6 tracks against 8 streams, hence `?`). From the all-tracks `VERDICT` above, those
  six tracks are scan streams 0, 1, 2, 3, 4, 6.
- The three `predicted` lines name the right streams. Each eng-only track has the byte count and `SOURCE_ID` of
  one all-tracks track: file 0 = all-tracks 0 = scan 0 (49,661,338,222 bytes, `001011`); file 1 = all-tracks 3 =
  scan 3 (171,603,456, `001101`); file 2 = all-tracks 4 = scan 4 (13,164,731, `0012A0`), the full English PGS
  and not its forced-only twin 5, which has the same type and language. No line names the wrong stream.
- The `MISSING` line names the right stream but is read differently. The rip log has one `MSG:4047`: "AV sync
  issue in stream 3 at 0:00:00 : Forced subtitles track #3 turned out to be empty and was removed from output
  file". The number counts the title's selected streams in scan order, from 0 (observation 6). The selected
  streams are the file's three tracks (scan 0, 3, 4) and then one forced-only twin, so "#3" is that twin. The
  selection took scan stream 5, and MakeMKV then removed it as empty: the same removal the all-tracks rip showed
  for both twins ("#5" and "#7"). It is not a `subtitle*eng` difference. compare_tracks, which reads the log,
  lists it as `REMOVED` and gives `count=yes`.
- What the file cannot show: the message does not name the language, and both twins (5 eng, 7 spa) are empty,
  as the all-tracks rip showed. One `MSG:4047` line means exactly one of them was selected, and the prediction
  names 5. Had the selection taken 7 in place of 5, this file would look the same. That `subtitle*eng` left out
  stream 7 rests on this count and the prediction. Stream 6 shows the filter directly.

After the check, the reading is (my own reading, not tool output):

```text
predicted  scan 0 video     eng MpegH  <- file 0  (= all-tracks 0, bytes and SOURCE_ID equal)
predicted  scan 3 audio     eng DD     <- file 1  (= all-tracks 3)
predicted  scan 4 subtitles eng PGS    <- file 2  (= all-tracks 4)
removed    scan 5 subtitles eng PGS f=6144        (selected; MSG:4047 "track #3"; empty, as on the all-tracks rip)
after the check by eye: missing=0 extra=0 unmatched=0
```

### 1b. short-circuit-2-1988-dvd, title 0

Streams and coverage (Step 7b):

```text
corpus/owner/short-circuit-2-1988-dvd title 0 (1:50:06), every stream:
  stream  0 Video     attr3=None    attr28='eng'   Mpeg2    ch=   flags=0     predicted: keep
  stream  1 Audio     attr3='eng'   attr28='eng'   DD       ch=2  flags=0     predicted: keep
  stream  2 Audio     attr3='fre'   attr28='eng'   DD       ch=2  flags=0     predicted: drop
  stream  3 Audio     attr3='spa'   attr28='eng'   DD       ch=2  flags=0     predicted: drop
  stream  4 Audio     attr3='por'   attr28='eng'   DD       ch=2  flags=0     predicted: drop
  stream  5 Subtitles attr3='eng'   attr28='eng'            ch=   flags=0     predicted: keep
  stream  6 Subtitles attr3='fre'   attr28='eng'            ch=   flags=0     predicted: drop
  stream  7 Subtitles attr3='spa'   attr28='eng'            ch=   flags=0     predicted: drop
  stream  8 Subtitles attr3='por'   attr28='eng'            ch=   flags=0     predicted: drop
  stream  9 Subtitles attr3='chi'   attr28='eng'            ch=   flags=0     predicted: drop
  stream 10 Subtitles attr3='kor'   attr28='eng'            ch=   flags=0     predicted: drop
  stream 11 Subtitles attr3='tha'   attr28='eng'            ch=   flags=0     predicted: drop
  stream 12 Subtitles attr3='eng'   attr28='eng'   CC       ch=   flags=0     predicted: keep
COVERAGE corpus/owner/short-circuit-2-1988-dvd title 0: audio-lang=3 nolang=0 sub-lang=6 core=0 special=0 sub-flagged=0 audio-512=0
```

Stream 5 is the English VobSub (attribute 5 `S_VOBSUB`, 7 `Dvd Subtitles`; its codec short name, attribute 6, is
empty, hence the blank codec column). Stream 12 is the closed-caption stream: attribute 5 `S_CC608/DVD`, 6 `CC`,
41 `Text`, 42 `( Lossy conversion )`, name (attribute 30) "CC->Text English ( Lossy conversion )". No stream of
the title carries a flag.

`compare_tracks.py ... eng-only/mkvmerge.json --selection eng` (exit 1, from `duration=no`):

```text
scan                                                   | file
 0 video     eng  Mpeg2        out=         ch=   f=0  |  0 video     eng  MPEG-1/2 ch=
 1 audio     eng  DD           out=         ch=2  f=0  |  1 audio     eng  AC-3 ch=2
 5 subtitles eng               out=         ch=   f=0  |  2 subtitles eng  SubRip/SRT ch=
12 subtitles eng  CC           out=Text     ch=   f=0  |  3 subtitles eng  VobSub ch=

RIP LOG          corpus/owner/short-circuit-2-1988-dvd/eng-only/rip.txt
TRACK COUNT      scan=4 file=4
ORDER MATCH      yes (type, language except on video, audio channels; position by position)
CHAPTERS         scan=28 file=28
DURATION         scan=6606s file=6623.6s
VIDEO TRACKS     1
STATISTICS TAGS  present
VERDICT          count=yes order=yes video=yes chapters=yes duration=no
                 codecs are not compared: read the scan's out= column against the file's codec column
```

The all-tracks-2 rip of the same title, with the same script (exit 1):

```text
scan                                                   | file
 0 video     eng  Mpeg2        out=         ch=   f=0  |  0 video     eng  MPEG-1/2 ch=
 1 audio     eng  DD           out=         ch=2  f=0  |  1 audio     eng  AC-3 ch=2
 2 audio     fre  DD           out=         ch=2  f=0  |  2 audio     fre  AC-3 ch=2
 3 audio     spa  DD           out=         ch=2  f=0  |  3 audio     spa  AC-3 ch=2
 4 audio     por  DD           out=         ch=2  f=0  |  4 audio     por  AC-3 ch=2
 5 subtitles eng               out=         ch=   f=0  |  5 subtitles eng  SubRip/SRT ch=
 6 subtitles fre               out=         ch=   f=0  |  6 subtitles eng  VobSub ch=
 7 subtitles spa               out=         ch=   f=0  |  7 subtitles fre  VobSub ch=
 8 subtitles por               out=         ch=   f=0  |  8 subtitles spa  VobSub ch=
 9 subtitles chi               out=         ch=   f=0  |  9 subtitles por  VobSub ch=
10 subtitles kor               out=         ch=   f=0  | 10 subtitles chi  VobSub ch=
11 subtitles tha               out=         ch=   f=0  | 11 subtitles kor  VobSub ch=
12 subtitles eng  CC           out=Text     ch=   f=0  | 12 subtitles tha  VobSub ch=

RIP LOG          corpus/owner/short-circuit-2-1988-dvd/all-tracks-2/rip.txt
TRACK COUNT      scan=13 file=13
ORDER MATCH      no (type, language except on video, audio channels; position by position)
CHAPTERS         scan=28 file=28
DURATION         scan=6606s file=6623.6s
VIDEO TRACKS     1
STATISTICS TAGS  present
VERDICT          count=yes order=no video=yes chapters=yes duration=no
                 codecs are not compared: read the scan's out= column against the file's codec column
```

`exp11_tracks.py match ... 0 eng-only`, as printed:

```text
title 0: 13 streams without 3D video, 4 predicted; 4 tracks in eng-only/mkvmerge.json
predicted  scan  0 video     eng  Mpeg2      ch=   f=0  <- file  0 MPEG-1/2
predicted  scan  1 audio     eng  DD         ch=2  f=0  <- file  1 AC-3
predicted  scan  5 subtitles eng             ch=   f=0  <- file  2 SubRip/SRT
predicted  scan 12 subtitles eng  CC         ch=   f=0  <- file  3 VobSub
DIFFERENCES missing=0 extra=0 unmatched=0

eng-only tracks, for checking the lines above by eye:
   0 video     eng  MPEG-1/2               ch=   forced=False bytes=3171633468 name=''
   1 audio     eng  AC-3                   ch=2  forced=False bytes=158966784 name='Stereo'
   2 subtitles eng  SubRip/SRT             ch=   forced=False bytes=126769 name=''
   3 subtitles eng  VobSub                 ch=   forced=False bytes=3585604 name=''
no all-tracks rip of this title to compare with
```

The all-tracks-2 tracks, listed from `all-tracks-2/mkvmerge.json` with their `NUMBER_OF_BYTES` and `SOURCE_ID`
tags and `default_track` (my own listing, not tool output):

```text
   0 video     eng  MPEG-1/2    ch=   bytes=3171633468 src=0100E0 default=False
   1 audio     eng  AC-3        ch=2  bytes=158966784  src=0180BD default=True
   2 audio     fre  AC-3        ch=2  bytes=158966784  src=0181BD default=False
   3 audio     spa  AC-3        ch=2  bytes=158966784  src=0182BD default=False
   4 audio     por  AC-3        ch=2  bytes=158966784  src=0183BD default=False
   5 subtitles eng  SubRip/SRT  ch=   bytes=126769     src=0100E0 default=True
   6 subtitles eng  VobSub      ch=   bytes=3585604    src=0120BD default=False
   7 subtitles fre  VobSub      ch=   bytes=3223478    src=0121BD default=False
   8 subtitles spa  VobSub      ch=   bytes=3377844    src=0122BD default=False
   9 subtitles por  VobSub      ch=   bytes=2730808    src=0123BD default=False
  10 subtitles chi  VobSub      ch=   bytes=3356476    src=0124BD default=False
  11 subtitles kor  VobSub      ch=   bytes=2508672    src=0125BD default=False
  12 subtitles tha  VobSub      ch=   bytes=3150216    src=0126BD default=False
```

The check by eye:

- The all-tracks-2 `VERDICT` has `order=no`, so its tracks are not the scan streams at the same positions. Read
  by codec and language instead: tracks 0 to 4 are scan streams 0 to 4 (the video, then the four audio streams
  in the scan's language order eng, fre, spa, por); track 5, SubRip/SRT (`S_TEXT/UTF8`), is scan stream 12, the
  only stream the scan converts to text (attribute 41 `Text`), and its `SOURCE_ID` is the video's, `0100E0` (a
  DVD's closed captions travel inside the video stream); tracks 6 to 12, VobSub, are scan streams 5 to 11, in the
  scan's language order eng, fre, spa, por, chi, kor, tha.
- Each eng-only track has the byte count and `SOURCE_ID` of one all-tracks-2 track: file 0 = all-tracks-2 0 =
  scan 0 (3,171,633,468 bytes, `0100E0`); file 1 = all-tracks-2 1 = scan 1 (158,966,784, `0180BD`; the four audio
  tracks have the same byte count, so the `SOURCE_ID` and the language decide); file 2 = all-tracks-2 5 = scan 12
  (SubRip/SRT, 126,769 bytes, 1671 frames); file 3 = all-tracks-2 6 = scan 5 (VobSub, 3,585,604 bytes, `0120BD`,
  1742 frames).
- So `match`'s two subtitle lines name the wrong streams: it paired scan 5 with file 2 (SubRip/SRT) and scan 12
  with file 3 (VobSub), because its pairing uses only type, language and audio channels, and both are English
  subtitles. Corrected below. Both predicted subtitle streams are present either way, so `missing=0 extra=0`
  stands.
- Read by codec, the two subtitle tracks are in reverse scan order (file 2 is scan 12, file 3 is scan 5). Step
  8's rules read a track out of scan order (`UNMATCHED`) as an order difference. It is recorded here as an
  observation instead (section 5): the all-tracks-2 rip, made without the language selection, has the same order,
  so it is MakeMKV's output order for a text-converted subtitle track, not an effect of the selection.
- No `MSG:4047` in this rip, and none was expected: the title has no forced-only twins.

After the check, the reading is (my own reading, not tool output):

```text
predicted  scan  0 video     eng Mpeg2   <- file 0  MPEG-1/2   (= all-tracks-2 0, bytes and SOURCE_ID equal)
predicted  scan  1 audio     eng DD      <- file 1  AC-3       (= all-tracks-2 1)
predicted  scan 12 subtitles eng CC      <- file 2  SubRip/SRT (= all-tracks-2 5)
predicted  scan  5 subtitles eng VobSub  <- file 3  VobSub     (= all-tracks-2 6)
after the check by eye: missing=0 extra=0; files 2 and 3 are out of scan order (observation 5)
```

### 2. Predicted streams missing, unpredicted streams present

pans-labyrinth-2006-uhd, title 0:

- Missing: scan stream 5, subtitles, eng, PGS, flags 6144 (the derived forced-only twin of stream 4). It was
  selected and then removed by MakeMKV as empty (`MSG:4047`), as on the all-tracks rip, so it is not a
  difference.
- Present but not predicted: none.

short-circuit-2-1988-dvd, title 0:

- Missing: none.
- Present but not predicted: none.

### 3. The parts of the selection

| Part | Tested on (slug, title, streams) | As predicted? | Evidence |
|---|---|---|---|
| audio language filter (other languages dropped) | pans-labyrinth-2006-uhd, 0, streams 1 and 2 (spa); stream 3 (eng) kept. short-circuit-2-1988-dvd, 0, streams 2, 3 and 4 (fre, spa, por); stream 1 (eng) kept | yes, on two discs | Disc A: streams 1 (DTS-HD MA, flags 1024) and 2 (DTS, flags 2304) are absent: 4,481,466,356 bytes in the all-tracks file. Stream 1 is neither a core nor flagged, so only the language filter can have dropped it. Stream 3 is present with the same bytes. DVD: streams 2 to 4 (DD 2.0, flags 0) are absent: 476,900,352 bytes in the all-tracks-2 file. None is a core or flagged, so only the language filter can have dropped them. Stream 1 is present with the same bytes and `SOURCE_ID` |
| `nolang` | none: neither title has audio without a language | not exercised | `nolang=0` on both titles |
| `subtitle*eng` | pans-labyrinth-2006-uhd, 0, streams 6 and 7 (spa); streams 4 and 5 (eng) kept. short-circuit-2-1988-dvd, 0, streams 6 to 11 (fre, spa, por, chi, kor, tha VobSub); streams 5 (eng VobSub) and 12 (eng closed captions) kept | yes, on two discs | Disc A: stream 6 is absent (14,610,116 bytes in the all-tracks file). Stream 4 is present with the same bytes. Stream 5 was selected and removed as empty (`MSG:4047` "#3"). Stream 7 is empty either way; one `MSG:4047` line says only one twin was selected (section 1a). DVD: streams 6 to 11 are absent (18,347,494 bytes in the all-tracks-2 file). Stream 5 is present with the same bytes and `SOURCE_ID`. Stream 12, the closed-caption stream, is kept and arrives as SubRip/SRT, the same track (bytes) as in the all-tracks-2 file (section 1b) |
| `core` | pans-labyrinth-2006-uhd, 0, stream 2 (spa DTS core, flags 2304). short-circuit-2-1988-dvd, 0: no derived core | not exercised | Disc A: stream 2 is absent. But it is Spanish, so the language filter drops it as well: this title does not show `-sel:core` acting alone. The Office disc's derived cores are English (its titles 0 to 3, 5 and 6 have `core=1` or `2` and `audio-lang=0`; title 4 has `core=0`). DVD: `core=0` |
| `special` | none: neither title has flagged audio | not exercised | `special=0` on both titles. Blu-ray and UHD discs carry no flags 1, 2 or 4 (spec 9.5), and this DVD flags none of its streams |

### 4. Observations

pans-labyrinth-2006-uhd, title 0:

- Flagged subtitles (`sub-flagged`): no such stream.
- Secondary audio (`audio-512`): no such stream.
- A stream with no language: no such audio or subtitle stream on this title. The only stream without attribute 3
  is the video stream: attribute 3 is absent (`None`; the scan has no `SINFO:0,0,3` line) and attribute 28 is
  `eng`, and the file gives the video track `eng`. Attribute 28 is `eng` on every stream, the Spanish ones
  included: it is the metadata language, not the stream's own (spec 8.6 and 9.4 already take a stream's language
  from attribute 3).
- `MSG:1009` and messages about the profile: no `MSG:1009`. One message names the profile, and it is not an
  error: `MSG:5015` "Saving 1 titles into directory ... using profile 'lazymkv eng only' from file
  'C:\lazymkv\phase0\profiles\eng-only.mmcp.xml'". The rip's message codes are those of the all-tracks rip, with
  one `MSG:4047` in place of two: 1004, 1005, 1011, 3007, 3025 (31 titles under 120 s skipped), 3307 (twice:
  `00800.mpls` as title #0, `00055.mpls` as title #1), 4007 ("AV synchronization issues were found in file ..."),
  4047 (once), 5005 ("1 titles saved"), 5011, 5015, 5036 ("Copy complete. 1 titles saved."), 5085.
- `chapters=no` or `duration=no` that the all-tracks rip did not show: none. Chapters 22 = 22 and duration 7150 s
  against 7150.1 s, as on the all-tracks rip.

short-circuit-2-1988-dvd, title 0:

- Flagged subtitles (`sub-flagged`): no such stream.
- Secondary audio (`audio-512`): no such stream.
- A stream with no language: none, as on Disc A. Only the video stream lacks attribute 3 (`None`), with attribute
  28 `eng`, and the file gives the video track `eng`. Attribute 28 is `eng` on every stream here too, the French,
  Spanish, Portuguese, Chinese, Korean and Thai ones included.
- `MSG:1009` and messages about the profile: no `MSG:1009`. One message names the profile, and it is not an
  error: `MSG:5015` "Saving 1 titles into directory ... using profile 'lazymkv eng only' from file
  'C:\lazymkv\phase0\profiles\eng-only.mmcp.xml'". The rip's message codes: 1003 (one debug line), 1004, 1005,
  1011, 3007, 3025 (title #2, 63 s, skipped), 3028 (seven lines: titles #1 and #3 to #8 added), 5005 ("1 titles
  saved"), 5011, 5015, 5036 ("Copy complete. 1 titles saved."). No `MSG:2003` read error and no `MSG:4047`. The
  all-tracks-2 rip has the same codes, plus three `MSG:2003` (scrambled sector at offset 1,048,576, the errors the
  scan also logged) and six more `MSG:1003` debug lines.
- `chapters=no` or `duration=no` that the all-tracks rip did not show: none. Chapters 28 = 28. `duration=no` in
  both rips: the scan's 6606 s against the file's 6623.6 s, in the all-tracks-2 rip as in the eng-only rip, so it
  is not an effect of the selection.
- Closed captions: stream 12 (codec `CC`) is a subtitle stream whose attribute 3 is `eng`, so `subtitle*eng`
  keeps it, as predicted. It arrives as a SubRip/SRT track (`S_TEXT/UTF8`, 1671 frames) whose `SOURCE_ID` is the
  video stream's (`0100E0`). MakeMKV made it the default subtitle track (`default_track` true), not the English
  VobSub, although the scan's attribute 38 marks stream 5 `d` ("Default") and stream 12 nothing; the all-tracks-2
  file does the same.

### 5. Order

- pans-labyrinth-2006-uhd: the eng-only rip's tracks came in scan order: file tracks 0, 1, 2 are scan streams 0,
  3, 4. No `UNMATCHED` line and `ORDER MATCH yes`.
- short-circuit-2-1988-dvd: the eng-only rip's tracks did not come in scan order. File tracks 0, 1, 2, 3 are scan
  streams 0, 1, 12, 5: the closed-caption text track (scan 12) comes before the English VobSub (scan 5). The tools
  do not see it on this rip: `compare_tracks` says `ORDER MATCH yes` and `match` prints no `UNMATCHED` line,
  because they pair by type, language and audio channels, and both tracks are English subtitles (section 1b). The
  all-tracks-2 rip shows the same rule where the languages differ: MakeMKV writes the CC->Text track (scan 12) as
  file track 5, immediately after the four audio tracks and before all seven VobSub tracks (scan 5 to 11, files 6
  to 12), and `compare_tracks` says `count=yes order=no`. So, with either profile, the text-converted subtitle
  track comes first, before the bitmap subtitle tracks. The selection did not cause it: this is recorded as an
  observation, not as a difference (section 1b). EXP-01 notes it as an exception to its order finding.

### 6. `MSG:4047` counts the selected streams, not the scan's

`MSG:4047`'s "stream N" and "track #N" count the title's selected streams in scan order, from 0. These are the
output file's tracks before the removal, not the scan's stream numbers. On this rip "#3" is scan stream 5, the
fourth selected stream (scan 0, 3, 4, 5). On the all-tracks rip every stream is selected, so the two numberings
coincide ("#5", "#7" are scan 5 and 7), which is why EXP-01 did not show the difference. Before commit f850ba0,
compare_tracks.py read N as a scan stream number. Run on this rip, the pre-fix version (`f850ba0^`) prints:

```text
REMOVED          scan stream 3 (audio eng DD f=0): MakeMKV left it out of the file (MSG:4047), so it is not expected
ORDER MATCH      no (type, language except on video, audio channels; position by position)
VERDICT          count=yes order=no video=yes chapters=yes duration=yes
```

It exits 1. The corrected script maps N through the predicted selection (section 1a above). The job profile of
spec 9.3 leaves out `mvcvideo`, so on a 3D title the two numberings would differ there too. No 3D title has been
recorded, so this is not tested.

### 7. An English-only selection on a Spanish film keeps only the commentary

This film's original language is Spanish. The English selection keeps exactly one audio track, scan stream 3,
English AC-3 2.0 "Stereo". The owner identified it from the packaging as the writer/director's audio commentary
(`corpus/owner/pans-labyrinth-2006-uhd/notes.md`; `audio-labels.json`, kind `commentary`, how `packaging`).
Nothing on the disc marks it (flags 0), so `-sel:special` cannot drop it. In the eng-only file it is the only
audio track and MakeMKV made it the default track (`default_track` true in `mkvmerge.json`). The film's own
Spanish soundtrack is gone. So a rip-time fast path needs the work's original language in its selection string,
not English alone. R4 and 8.6 already define the wanted languages as English plus the original language, and this
experiment tested only the English half of that set.

Also observed: writing 4.5 GB less did not shorten the rip; both rips took 2245 s. Two runs do not show what
sets the pace (not measured separately; experiment 4's throughput figures bear on it), but a selection that writes
less cannot be counted on to be faster.

## Answer

**Not answered.** Two rips, Disc A's title 0 (UHD) and short-circuit-2-1988-dvd's title 0 (DVD), show no
difference in the streams kept, after the check by eye: every predicted stream is in the file or was removed by
MakeMKV as an empty forced-only twin (`MSG:4047`, Disc A), and nothing unpredicted is present. But three parts of
the selection were not exercised on either disc: **`nolang`** (no audio without a language on either title),
**`special`** (no flagged audio: Blu-ray and UHD never carry these flags, and this DVD flags none of its streams)
and **`core`** on its own (Disc A's only derived core is Spanish, which the language filter drops anyway, and the
DVD has none; the plan's Step 8 counts `core` as exercised only by a core in a kept language). `special` waits for
a DVD whose commentary is flagged, `nolang` for a disc with a language-less audio stream, and `core` for an
English-only rip of an Office title, whose derived cores are English. The parts that were exercised behaved as
predicted, now on two discs:
- the audio language filter: on Disc A it dropped the Spanish DTS-HD MA (stream 1) and its core (2) and kept the
  English AC-3 (3); on the DVD it dropped the French, Spanish and Portuguese DD 2.0 (2, 3, 4) and kept the English
  DD 2.0 (1);
- `subtitle*eng`: on Disc A it dropped the Spanish PGS (6) and kept the English PGS (4) and its forced-only twin
  (5, then removed as empty); on the DVD it dropped the six other-language VobSub tracks (6 to 11) and kept the
  English VobSub (5) and the English closed-caption stream (12), which arrives as SubRip/SRT;
- the derived core on Disc A (2) is absent, consistent with `-sel:core`, but see above: the language filter
  explains it too.

Video was kept on both. Disc A's tracks are in scan order. The DVD's are not: the text-converted closed-caption
track comes before the English VobSub. The all-tracks-2 rip has the same order without the selection, so this is
recorded as an observation about MakeMKV's output order (observation 5), not as a difference of the selection.
The reading applied (controller's ruling, 2026-10-12): a track out of scan order is a difference only when the
all-tracks rip of the same title has it in scan order.

## Consequence for the spec

- Section 16, item 1 stays a later feature, marked **not yet verified**: experiment 11 has not exercised `nolang`,
  `special` or `core` on its own (Disc A and short-circuit-2-1988-dvd; a DVD with a flagged commentary, a disc
  with a language-less audio stream and an Office eng-only rip are pending). On one UHD title and one DVD title,
  the audio language filter and `subtitle*eng` behaved as predicted. Section 17, row 11 says the same.

The points below come from observations 5, 6 and 7, not from the Answer rule above (controller's ruling,
2026-10-06, for observations 6 and 7; the closed-caption order point from observation 5 added on the
controller's instruction, 2026-10-12): they are facts about verification and selection that this experiment
established, and Task 17 Step 2 applies them to the spec.

- The fast path's selection string must carry the work's original language as well as English (R4, 8.6). On
  Disc A, English alone keeps only the commentary and makes it the default audio track (observation 7). This is
  no change to R4 or 8.6. Section 16, item 1's "wanted-language" means that set, never English alone.
- 9.4, and the verification that aligns a rip with the scan (6.2 step 7, and 6.4 step 14 for a title the fast
  path does not remux): add `MSG:4047` to the messages acted on. "Forced subtitles track #N turned out to be
  empty and was removed from output file" means stream N of the title's selected streams in scan order, counted
  from 0, not scan stream N. The verification maps N through the profile's selection, and does not count that
  stream as missing (EXP-01 already found the removal). With the all-tracks job profile (9.3) the two numberings
  coincide unless a stream is left out (`-sel:mvcvideo` on a 3D title, not tested).
- 6.2 step 7, and 6.4 step 14 (the same verifications as the point above): when aligning a rip with the scan,
  a text-converted subtitle track (a DVD's closed captions: scan codec `CC`, attribute 41 `Text`, written as
  SubRip/SRT) precedes the bitmap subtitle tracks in the file, immediately after the audio tracks, although the
  scan lists it after them. Seen on short-circuit-2-1988-dvd with both the all-tracks and the eng-only profile
  (observation 5). Pairing by type and language alone hides the swap when the two tracks share a language
  (`order=yes` on the eng-only rip) and reports `order=no` when they do not (all-tracks-2). EXP-01's "order
  matches the scan" needs this exception.
