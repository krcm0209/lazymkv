# Experiment 2: do a rip call's title numbers match the index asked for?

- Date: 2026-10-05 (mini PC clock)
- Disc(s): pans-labyrinth-2006-uhd, format bluray (a UHD), drive D: (HL-DT-ST BD-RE BU40N)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 2
- Tools: the Python this report names was deleted by the Rust port (pull request krcm0209/lazymkv#2); `exp02_check.py` is now `lazymkv-dev exp02`, with the same arguments and lines. The commands below are as they were run; the scripts as they last were are `git show d3a754d:scripts/phase0/<file>`.

## Question

On a disc with angles or duplicate titles: start a rip for every title index, stop each after the saving message,
and compare. Do the `3307`/`3308` title numbers equal the index ripped in every run, and do they precede the
saving message?

## What was run

- `scripts/phase0/exp02-index-guard.ps1 -Slug pans-labyrinth-2006-uhd` as the job `exp02-pans-labyrinth-2006-uhd`,
  after the plan run (`-Plan`) and the Step 2 checks. The `command` field of `exp02/run-20261005-212907.json`:
  `makemkvcon64 -r --messages=-stdout --progress=-same --debug --minlength=120 --cache=1024
  --profile=C:\lazymkv\phase0\profiles\all-tracks.mmcp.xml mkv disc:0 <title> \\unas-pro\media\lazymkv-staging\pans-labyrinth-2006-uhd\exp02-tmp`.
- Titles tested: 0 and 1, which is 2 of 2: every title the scan lists, so no sampling was needed (`-MaxTitles 12`
  only matters above twelve titles).
- Each call was read line by line and killed at the saving message. Limits per call: 1800 s or 512 MiB written.
  No redo; no title set aside.

## What was observed

`python3 scripts/phase0/exp02_check.py corpus/owner/pans-labyrinth-2006-uhd`:

```text
title   0: scan source=00800.mpls rip #0=00800.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0
title   1: scan source=00055.mpls rip #1=00055.mpls match=yes 5014=yes added-before-5014=yes 3308-lines=0 3309-lines=0

EXPERIMENT 2: yes (2 titles)
```

- The disc has no angle titles and no skipped duplicates: `scan.txt` has 0 `MSG:3308` and 0 `MSG:3309` lines, and
  the checker's `3308-lines` and `3309-lines` columns are 0 for both calls.
- The saving message of a profile rip is `MSG:5015` ("Saving 1 titles into directory ... using profile 'lazymkv
  all tracks' from file ..."), not `MSG:5014`; the checker and the script accept both (the plan's `5014=` labels
  keep their name). Each call printed 1005, 1004, 1011, 3007, 5085, then the `3307` and `3025` lines interleaved
  (both titles' `3307` lines precede saving in both calls), then 5011 and 5015, where it was stopped. The two
  title files are byte-identical: the `5015` line says "Saving 1 titles" and names no title, so the two
  `match=yes` results are the same observation made twice. They show that each call's own scan numbers the
  disc as `scan.txt` did; they do not show which title a call would have saved (the guard compares numbering,
  as spec 6.2 step 6 intends).
- The largest `written_bytes` of a stopped call is 0: when the script read `MSG:5015` and killed the call, 17 s
  after it started, no byte was in the output folder (measured 10 s after the kill). The saving message reached
  the script before saving produced data, which is what the guard needs. The delay between MakeMKV printing a
  line and the script reading it was not timed, so this does not show that the pipe is unbuffered; the scan's
  timing file (`scan.txt.timing.jsonl`, 275 lines arriving over 18.9 s at 87 distinct times) shows that `info`
  output arrives incrementally rather than at exit.
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
Two facts for the spec regardless of the final answer: in a call with the job profile the saving message to stop
at is `MSG:5015` (its text, "using profile ...", makes that general; 6.2 step 6 and the message table in 9.4),
and on this disc stopping at that message cost nothing written.
