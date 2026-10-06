# Experiment 3: does a whole-disc copy to the share work, and does its scan list the disc's titles?

- Date: 2026-10-06 (mini PC's local clock)
- Disc(s): pans-labyrinth-2006-uhd, UHD (meta.json format bluray), drive 1 (D:)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release) (the `MSG:1005` line of `backup.txt`)
- Spec reference: section 17, row 3

## Question

Does `backup --decrypt` to a network path work on a UHD with Java menus, and does `info file:` on the copy list the
same titles (source file, segment map, duration) as the disc scan?

## What was run

- The copy: `scripts/phase0/backup-disc.ps1 -Slug pans-labyrinth-2006-uhd` as the job `backup-copy`. It ran
  `makemkvcon64 -r --messages=-stdout --progress=-same --debug --cache=1024 backup --decrypt disc:0
  \\unas-pro\media\lazymkv-staging\pans-labyrinth-2006-uhd\backup` under `lazymkv-capture record` (`--debug` keeps
  MakeMKV's debug log, decision D16). The script had created the target folder, empty, with Assert-EmptyDir.
- The scan of the copy, by the same script straight after the copy: `info file:<copy root>`, where the copy root is
  the target folder itself (found by listing it), with Get-MkFlags (`-r --messages=-stdout --progress=-same --debug
  --minlength=120 --cache=1024 --profile=<all-tracks profile>`), the same flags as the disc-time scan `scan.txt`.
- The job ended `RESULT FAILED: the copy succeeded, but its scan lists no titles`. The copy was kept: no `-Force`
  and no repeat, so there is no `backup-failed-N` or `extract-failed-N` folder.
- Diagnosis of the failed scan, the same day, without changing the copy on the share. Each run is in
  `corpus/owner/pans-labyrinth-2006-uhd/backup/scan-<name>.txt`, with `.exit.json`, `.stderr` and, when the flags
  included `--debug`, `.makemkv-debug.txt`:
  - `scripts/phase0/scan-copy.ps1` scanned the copy on the share four more times: `repeat` (the same flags),
    `nodebug` (without `--debug`), `plain` (`-r --messages=-stdout --progress=-same` only) and `bdmv` (the same
    flags, with `<copy root>\BDMV` as the target).
  - `scripts/phase0/partial-copy-scan.ps1` copied part of the copy to a temporary folder on C: (2 GB at most) and
    scanned it with Get-MkFlags; the files it copied are listed in `scan-partial-<name>.txt.files.txt`. `t1`: every
    file except the stream clips, plus clips 27 to 60 (308 files); `t1-nojava`: the same without the 9 `BDMV/JAR`
    files and the 4 BDJO files (295 files); `all-but-6`: every file of the copy except the main feature's clip
    `BDMV/STREAM/00006.m2ts`, 55,613,952,000 bytes (333 files, 1,252,469,359 bytes); `no55mpls`: the same as
    `all-but-6` without `BDMV/PLAYLIST/00055.mpls` and `BDMV/BACKUP/PLAYLIST/00055.mpls` (331 files).
- The extraction of the main feature from the copy (`extract-from-copy.ps1`, `mkv file:<copy root> K ...`) was not
  run, and K was not chosen: see "The extraction" below.

## What was observed

### The copy

- `backup-summary.json`: `started` 2026-10-06 11:47:16.632; `elapsed_s` 2356 (39.3 min); `bytes` 56,866,421,359
  (56.87 GB) in 334 `files`; `mb_per_s` 23 (MB of 2^20 bytes, as the scripts count; 24.1 MB/s in MB of 10^6 bytes).
  The disc's `total_bytes` (`meta.json`): 56,866,420,142 in 273 files. `exit_code` 0
  (`backup.txt.exit.json`: 2,355,468 ms, 73,557 lines).
- `copy_message`: `MSG:5070,128,0,"Backup done","Backup done"`. `fail_codes` is empty: no `5069`, and no `5079`.
  `read_errors` 0, `write_errors` 0. `system-events.json` (Windows System and SMB client events during the copy) is
  an empty list.
- Every distinct message of `backup.txt` with its count (`grep -E '^MSG:' ... | cut -d, -f1,4 | sort | uniq -c`;
  the cut ends a text at its first comma):

```text
   1 MSG:1004,"Debug logging enabled
   1 MSG:1005,"MakeMKV v2.0.0 win(x64-release) started"
   1 MSG:1011,"Using LibreDrive mode (v06.3 id=866A98CB9C4E)"
   1 MSG:5070,"Backup done"
   1 MSG:5072,"Backing up disc into folder \"file://\\\\unas-pro\\media\\lazymkv-staging\\pans-labyrinth-2006-uhd\\backup\""
   1 MSG:5081,"Backup done."
   1 MSG:5085,"Loaded content hash table
```

`MSG:5085` reads in full "Loaded content hash table, will verify integrity of M2TS files."

### The copy's layout

- `top_level`: `BDMV`, `CERTIFICATE`, `MAKEMKV`. `copy_root` equals `dest`, the target folder: `backup` writes
  `BDMV` straight into the target folder, with no folder of its own, and it accepted a target folder that existed
  and was empty.
- The file comparison with the disc's `files.json` (Step 5's script):

```text
disc: 273 files, 56866420142 bytes; copy: 334 files, 56866421359 bytes
only on the disc: 18
only in the copy: 79
size differs: 0
BDMV/STREAM: 60 files on the disc, 0 missing from the copy or of another size
```

- Only on the disc: the 18 AACS files, `AACS/` and `AACS/DUPLICATE/` each holding `Content000.cer`,
  `Content001.cer`, `ContentHash000.tbl`, `ContentHash001.tbl`, `ContentRevocation.lst`, `CPSUnit00001.cci`,
  `DH_Pairing_Server.cer`, `MKB_RO.inf` and `Unit_Key_RO.inf` (the script prints the names in upper case,
  `AACS/CONTENT000.CER` and so on). This is the expected difference for a decrypted copy.
- Only in the copy: the same 18 files under `MAKEMKV/AACS/`, each with the disc's size; 60 files
  `MAKEMKV/CMAP/<clip>.cmap` of 16 bytes, one for each stream clip; and `MAKEMKV/discattd.dat` (257 bytes). These
  account for the copy's 1,217 extra bytes (60 x 16 + 257).
- No size differs, and every one of the 60 `BDMV/STREAM` files arrived complete.

### The scan comparison

The copy's scan crashed. `scan-file.txt.exit.json`: `{"exit_code":-1073741819,"elapsed_ms":8840,"lines":79}`; the
exit code is 0xC0000005, an access violation. The output has the `MSG:1005` banner, `MSG:1004`, the drive list,
`MSG:3006` "Opening files on harddrive at file:////unas-pro/media/lazymkv-staging/pans-labyrinth-2006-uhd/backup",
the progress captions up to "Processing titles", then four `MSG:3025` lines (titles below the 120 s minimum:
`00017.mpls` 8 s, `00052.mpls` 23 s, `00053.mpls` 50 s, `00054.mpls` 30 s) and two `PRGV` lines. Nothing follows:
no `MSG:3307`, no `MSG:5010` or `5011`, no `TCOUNT`. Standard error is empty. The debug log stops at the same
`00054.mpls` line and has no "Application exited" line, which the backup's and the disc scan's debug logs end with;
before that it prints `DISCID=C337194670AF8C954C320875D671653F7EF2C256`, Disc A's disc ID from `meta.json`.
`backup-summary.json` has `scan_ok` false and `scan_titles` null.

compare_scans (`scan.txt` against `backup/scan-file.txt`):

```text
titles: A=2 B=0 common=0
only in A: 2
  ('00055.mpls', '27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60', '0:05:40')
  ('00800.mpls', '6', '1:59:10')
only in B: 0
comment differs: 0
EMPTY SCAN: a scan with no titles cannot be compared
compare_scans exit=1
```

The diagnosis runs (the exit code -1073741819 is 0xC0000005 in every crash):

| Run | What MakeMKV opened | Flags | Exit, seconds | What it printed |
|---|---|---|---|---|
| `repeat` | the copy on the share | Get-MkFlags | 0xC0000005, 11.4 | the same four `MSG:3025` lines, then nothing; no `TCOUNT` |
| `nodebug` | the copy on the share | without `--debug` | 0xC0000005, 11.6 | the same |
| `plain` | the copy on the share | `-r --messages=-stdout --progress=-same` | 0xC0000005, 8.4 | the same |
| `bdmv` | `<copy root>\BDMV` | Get-MkFlags | 0xC0000005, 8.1 | the same; `MSG:3006` names the copy root, not `BDMV` |
| `t1` | C:, clips 27-60 only | Get-MkFlags | 0xC0000005, 9.9 | 114 `MSG:1003` debug lines (52 of them `Code 0`), no `MSG:3025`, no `TCOUNT` |
| `t1-nojava` | as `t1`, no JAR or BDJO | Get-MkFlags | 0xC0000005, 6.7 | 118 `MSG:1003` lines (56 `Code 0`), no `MSG:3025`, no `TCOUNT` |
| `all-but-6` | C:, all but clip 6 | Get-MkFlags | 0xC0000005, 10.2 | two `MSG:1003` lines `Code 6`, then the share's four `MSG:3025` lines; no `TCOUNT` |
| `no55mpls` | as `all-but-6`, no `00055.mpls` | Get-MkFlags | 0, 7.1 | ran to the end (below) |

- `no55mpls` printed, after the same two `Code 6` lines: six `MSG:3025` lines for playlists (`00017` 8 s, `00052`
  23 s, `00053` 50 s, `00054` 30 s, `00120` 100 s, `00140` 100 s); two `MSG:1003` lines `Code 0`; 59 `MSG:3025`
  lines, one for every clip in the partial copy (`00000.m2ts` to `00063.m2ts`, 0 to 100 s); then
  `MSG:5010,0,0,"Failed to open disc","Failed to open disc"` and `TCOUNT:0` (113 lines). There is no `MSG:5011` and
  no `MSG:3307`, and `00800.mpls`, whose only clip was left out, is not mentioned. With no title of 120 s or more,
  MakeMKV ended with `MSG:5010` here, after it had gone through the playlists and every clip without crashing. Its debug log ends "Application
  exited".
- `t1` and `t1-nojava` are inconclusive: 26 of the 60 clips are absent, and the codes of their debug lines are
  exactly the numbers of those 26 clips (the disc's clips from 00000 to 00026, and 00061 and 00063) plus 52, 53, 54,
  120 and 140, which are playlist numbers. The missing files dominate these runs, and they crash before any
  `MSG:3025`.
- Conclusion: `all-but-6` reproduces the share's crash on a local path (the same four `MSG:3025` lines, then the
  same silent end), and the only difference between `all-but-6` and `no55mpls` is the two copies of `00055.mpls`.
  So MakeMKV 2.0.0 crashes while it processes playlist `00055.mpls` from a folder source. It is not the network path
  (the crash happens on C:), not the flags (`plain`, `nodebug`), not the target folder (`bdmv`), and not the Java
  files (`no55mpls` keeps all 9 JAR and 4 BDJO files and runs to the end). In the six crashes that print `MSG:3025`
  lines, the last is `00054.mpls`, and in `no55mpls` the next playlist reported is `00120.mpls`, which fits.
- On the disc, `00055.mpls` is title 1 of `scan.txt`: 0:05:40, 34 chapters, segment map 27 to 60, one video stream
  and no audio (the rehearsal rip of title 1, `rehearsal/mkvmerge.json`, has one HEVC video track, 340.3 s, 34
  chapters). The disc scan and that rip read it from the disc without trouble. `00055.mpls` has the same size on the
  disc and in the copy (3,792 bytes); its bytes were not compared. The full copy was not scanned without
  `00055.mpls`: that would have meant changing the copy on the share or copying 55.6 GB to C:.

### Java

- `grep -ci '"BDMV/JAR/' files.json`: 9 (`BDMV/JAR/00001.jar` and eight files in `BDMV/JAR/00000/` and
  `BDMV/JAR/88888/`). `files.json` also lists `BDMV/BDJO/00000.bdjo` and `BDMV/BDJO/00001.bdjo`.
- `MSG:3344` and labels, per scan:

```text
scan.txt: titles=2 java_in_use(3344)=False labelled titles=0
backup/scan-file.txt: titles=0 java_in_use(3344)=False labelled titles=0
```

The copy's scan ended before any title, so its missing `3344` and labels say nothing.

### The extraction

Step 6 was skipped: Step 5 did not end `RESULT ok`. K cannot be chosen from a scan that lists no title, and
`extract-from-copy.ps1` refuses a copy whose `backup-summary.json` does not say `"ok": true`. Not tested, but
inferred: `mkv file:<copy root> K ...` scans the folder before it writes, so it would meet the same crash before
saving anything.

### Timing (for section 18)

- The copy: 2356 s (39.3 min) for 56,866,421,359 bytes, 23.0 MB/s (2^20-byte MB).
- The direct all-tracks rip of the main feature (Task 11, `all-tracks/rip-summary.json`): 2245 s (37.4 min) for
  54,346,666,920 bytes, `mb_per_s` 23.1 overall and `write_mb_per_s` 23.3 while writing.
- Both moved the disc's bytes to the share at the same rate; the whole-disc copy took 111 s longer than the direct
  rip of the feature. The extraction's reads and writes over the network were not measured.

## Answer

**No.** `backup --decrypt` to the network share worked on Disc A, but MakeMKV 2.0.0 crashed while scanning the
copy, so `info file:` listed none of the disc scan's titles and nothing could be extracted.

1. Disc A has Java menus: `files.json` lists 9 `BDMV/JAR/` entries (and two BDJO files), so the question's
   condition, a UHD with Java menus, is met. `MSG:3344` appeared in neither scan; it shows only whether MakeMKV
   used a Java runtime (EXP-12), not whether the disc has Java menus.
2. Of the three conditions for yes:
   - the copy completed: **held**. `MSG:5070`, no `5069`, no `5079`, no read or write errors, and every stream file
     arrived with the disc's size;
   - every disc-scan title is in the copy's scan: **failed**. compare_scans printed `only in A: 2` (both titles,
     `00800.mpls` and `00055.mpls`) and `EMPTY SCAN`, because `info file:` crashed (0xC0000005) before it listed
     any title. Nothing was only in the copy's scan (`only in B: 0`);
   - the extracted feature matches the direct rip: **not met**. Step 6 was skipped, so there is no extracted file
     and no `SAME TRACKS`, `SAME CHAPTERS` or `DURATION WITHIN 2 S` line.
3. Labels: `scan.txt` has no labelled title, so there were no labels to compare. `comment differs: 0` shows
   nothing here.

## Consequence for the spec

- 6.3 is replaced by its fallback (section 17, row 3): title-unsure discs rip every title that passes the
  candidate filter (8.2) in Direct mode, most likely first, waiting in `AwaitingSpace` if they do not fit. 6.3
  already switches to this when a copy fails twice; it now applies from the start.
- Labels: no design change. There were no labels to compare, and 6.3 already kept the disc-time scan, with its
  labels, as the reference.
- 9.2 and 9.4, this experiment's own finding: MakeMKV 2.0.0 can crash without any message (exit code -1073741819,
  0xC0000005; no `MSG:5010` or `5011`, no `TCOUNT`, empty standard error) when it opens a folder copy, within 12 s,
  before any watchdog in 10.4 would act. The engine must treat a `makemkvcon` run that exits without `MSG:5011`
  and a `TCOUNT` line as a failed scan, whatever the exit code; the exit code is still recorded with the log, since
  here it was the only direct sign of the crash. Until such a crash is mapped it is `Unknown` (10.2), with the full
  log attached. A note for 9.4's `5010` row: in `no55mpls`, `MSG:5010` ("Failed to open disc") ended a scan that
  had gone through the playlists and clips and found no title of `--minlength` or more (with the main clip absent), so
  `5010` may mean "no usable title" as well as "the source could not be opened".
- The crash follows one playlist of this disc (`00055.mpls`), so it may not hit other discs. Open question, not a
  plan: whether experiment 3 succeeds when repeated on a UHD with Java menus that has no such playlist.
- 18: the risk "Whole-disc copy is unverified on this setup until experiment 3" now has an answer: the copy to the
  share works, but MakeMKV 2.0.0 crashed scanning the copy of Disc A, and the fallback above is used. For the
  network-cost estimate: a whole-disc copy of this 56.9 GB UHD took 39 min at 23.0 MB/s, the same rate as the
  direct rip of its main feature (37 min, 23.1 MB/s).
