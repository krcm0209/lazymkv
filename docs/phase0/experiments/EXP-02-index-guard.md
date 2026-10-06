# Experiment 2: do a rip call's title numbers match the index asked for?

- Date: 2026-10-05 (mini PC clock)
- Disc(s): pans-labyrinth-2006-uhd, format bluray (a UHD), drive D: (HL-DT-ST BD-RE BU40N)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 2

## Question

On a disc with angles or duplicate titles: start a rip for every title index, stop each after the saving message,
and compare. Do the `3307`/`3308` title numbers equal the index ripped in every run, and do they precede the
saving message?

## What was run

- `scripts/phase0/exp02-index-guard.ps1 -Slug pans-labyrinth-2006-uhd` as the job `exp02-pans-labyrinth-2006-uhd`
  (the `command` field of `exp02/run-20261005-212907.json`), after the plan run (`-Plan`) and the Step 2 checks.
- Titles tested: 0 and 1, which is 2 of 2: every title the scan lists, so no sampling was needed (`-MaxTitles 12`
  only matters above twelve titles).
- Each call: `makemkvcon64` with `Get-MkFlags` (the all-tracks profile, `--debug`) `mkv disc:0 <title>
  \\unas-pro\media\lazymkv-staging\pans-labyrinth-2006-uhd\exp02-tmp`, read line by line and killed at the saving
  message. Limits per call: 1800 s or 512 MiB written. No redo; no title set aside.

## What was observed

`python3 scripts/phase0/exp02_check.py corpus/owner/pans-labyrinth-2006-uhd`:

```text
title   0: scan source=00800.mpls rip #0=00800.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0
title   1: scan source=00055.mpls rip #1=00055.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0

EXPERIMENT 2: yes (2 titles)
```

- The disc has no angle titles and no skipped duplicates: SCAN_3308 = 0 and SCAN_3309 = 0 in the scan, and the
  checker's `3308-lines` and `3309-lines` columns are 0 for both calls.
- The saving message of a profile rip is `MSG:5015` ("Saving 1 titles into directory ... using profile 'lazymkv
  all tracks' from file ..."), not `MSG:5014`; the checker and the script accept both (the plan's `5014=` labels
  keep their name). Each call printed, in order: 1005, 1004, 1011, 3007, 5085, the `3307` lines (both titles'
  `3307` lines precede saving in both calls), 3025 lines, 5011, then 5015, where it was stopped.
- WRITTEN_MAX = 0 bytes: both calls were stopped before a single byte reached the output folder, 17 s after they
  started. MakeMKV's piped output reaches a reader as soon as it is printed; a guard in the engine would react
  within a line's time.
- No `MSG:1009` (the job profile was accepted; the `5015` line names it).
- No `MSG:3308` lines, so the `3308` parameter layout remains unseen.
- No `MSG:5010` straight after a stopped call: the second call opened the disc normally 10 s after the first was
  killed (the script waits 10 s between calls).
- Each call's `exit=-1` is the kill, as expected. Each took 17 s, about the same as a scan of this disc.

## Answer

The disc has neither angles nor duplicate titles, so the experiment's condition, "a disc with angles or duplicate
titles", is not yet met; Task 15 repeats the experiment on the decoy-playlist disc.

**Yes on this disc, which has neither angles nor duplicate titles:** both titles that reached the saving message
have `match=yes` and `added-before-5014=yes`. No title was set aside.

## Consequence for the spec

None yet. The pre-read guard of 6.2 step 6 stays; the final answer waits for Task 15's decoy-playlist disc.
Two facts for the spec regardless of the final answer: the saving message to stop at is `MSG:5015` whenever a
profile is in use (6.2 step 6 and the message table in 9.4), and MakeMKV's output is not buffered on a pipe, so
stopping at that message costs nothing written.
