# Phase 0 report

Date: 2026-10-12. The design spec is `docs/superpowers/specs/2026-10-04-lazymkv-design.md`; its section 17
lists the experiments. Each experiment has its own report in `docs/phase0/experiments/`; this page
summarises them. The owner's disc recordings, which the reports cite as `corpus/owner/<slug>/...`, live in the private
repository `lazymkv-evidence`, cloned at `corpus/owner/`.

## The fourteen experiments

| # | Question | Answer | Report |
|---|---|---|---|
| 1 | Is the job profile honoured in robot mode: track order, output codecs, chapter count, forced-stream parent, Dolby Vision, statistics tags? | yes: the profile was honoured, the output track order equals the scan order, every output codec is the unconverted input codec (no attribute 41, 42 lossless throughout), and the chapter count equals attribute 8. The report adds that on a DVD the text-converted closed captions come before the bitmap subtitles, and that the Dolby Vision question is not answered (no Dolby Vision disc). | `EXP-01-profile-and-order.md` |
| 2 | Do the title numbers MakeMKV prints while a rip scans the disc equal the index ripped, and come before saving starts? | yes on this disc, which has neither angles nor duplicate titles: both titles that reached the saving message have `match=yes` and `added-before-5014=yes`. The experiment's condition, a disc with angles or duplicate titles, is not yet met. | `EXP-02-index-guard.md` |
| 3 | Does `backup --decrypt` to a network path work on a UHD, and does `info file:` on the copy list the same titles? | no: `backup --decrypt` to the network share worked on Disc A, but MakeMKV 2.0.0 crashed while scanning the copy, so `info file:` listed none of the disc scan's titles and nothing was extracted (Step 6 was skipped). The report's consequence, that 6.3 is replaced by its fallback from the start, was overridden by the owner's decision D18 (2026-10-06): 6.3 is kept, with a crash fallback to direct rips when the copy's scan fails (spec 6.3 step 6a; the report carries a dated note). | `EXP-03-whole-disc-copy.md` |
| 4 | Does a UHD rip straight to the share keep free RAM above 1 GB with no slow-write warning at `cache_mb` 1024; what are the throughput, the share-drop messages and the remux time? | not run: the owner has not done the NAS step, and no report was written. The rips of experiments 1 and 3 give the throughput and RAM figures (spec sections 14 and 18). | `EXP-04-rip-to-share.md` |
| 5 | What does MakeMKV report with its network blocked for a UHD it has never scanned, and can `Unit_Key_RO.inf` be read in both drives? | no distinct message was seen; the disc opened offline (rule 3) and listed the same two titles as online, so `NeedsNetwork` stays `Unknown`. Yes, `AACS/Unit_Key_RO.inf` was readable in both drives. | `EXP-05-offline-and-unit-key.md` |
| 6 | Does `ffprobe` tell Atmos and DTS:X apart, and do the subtitle alignment values separate dialogue from commentary? | not run, and no report was written: no disc with Atmos or DTS:X and no title with two or more full English subtitle tracks was recorded; the one UHD recorded has neither format (EXP-01's `ffprobe` profile `DTS-HD MA`; its `notes.md`). Experiment 14 keeps 8.7's timing bands, so the alignment half stays open. | `EXP-06-audio-profiles-and-subtitles.md` |
| 7 | Does programmatic eject work on both drives? | yes: on both drives the eject exited 0 with all four steps `ok`, and the disc came out: drive 1's tray opened, and drive 2, a slot-loading drive with no tray, pushed the disc out of its slot. | `EXP-07-eject.md` |
| 8 | What encoding and quote escaping does robot output use for non-ASCII titles and quotes? | not run: no disc with a non-ASCII title was recorded, and no report was written. The recordings' one non-ASCII character is valid UTF-8, and the quotes inside fields that the recordings hold (`MSG:5072`, `MSG:1003` debug lines) are backslash-escaped, never doubled (spec 9.4); a non-ASCII disc title is not yet verified. | `EXP-08-encoding.md` |
| 9 | Do the spec 8.9 file names show correctly on the owner's Plex and Jellyfin? | not run: it needs the owner's test library folder and the media servers' versions. No report was written. | `EXP-09-naming.md` |
| 10 | What share of about 20 of the owner's discs does TheDiscDB know? | not run as an experiment: of the 4 discs recorded (of about 20), TheDiscDB knows 2 by fingerprint (the-office-2005-s3d3-bluray, pans-labyrinth-2006-uhd) and not the 2 DVDs; not yet verified. No report was written. | `EXP-10-thediscdb-hit-rate.md` |
| 11 | Does a profile that selects by language rip exactly the streams predicted from the scan, in scan order? | not answered: two rips, Disc A's title 0 (UHD) and short-circuit-2-1988-dvd's title 0 (DVD), show no difference in the streams kept, but three parts of the selection were not exercised on either disc: `nolang`, `special` and `core` on its own. | `EXP-11-language-selection.md` |
| 12 | On a UHD with Java menus, does the scan report a Java runtime (`3344`) and main-feature labels? | not answered by this disc, and leaning no: Pan's Labyrinth (2019 4K) has BD-J files, but MakeMKV printed no `3344` and no labels. | `EXP-12-java-labels.md` |
| 13 | Measured over whole titles on the owner's own discs, and compared with the owner's label for every extra audio track in a wanted language, does the content rule of 8.6 give its commentary verdict to no programme track, and to at least one commentary on each of at least 3 different discs? | not answered: needs more discs. 0 of the 8 commentaries, 0 of the 20 programme tracks and 0 of the 6 separately encoded ones count so far, and there is no false verdict. Only a track whose content was checked before the verdict counts (D23): the one derived core counted before 2026-10-12 is labelled from its scan flag, an observation. | `EXP-13-design.md` |
| 14 | (a) Read by OCR and judged by the text rule of 8.7, and compared with the owner's label for every English and no-language subtitle track of the titles read, does the rule give no costly verdict and place at least half of the labelled tracks without review? (b) On the J4125, does reading one English track take at most 150 s per hour of running time? | (a) not answered: needs more discs. There was no costly verdict on the tracks read, but apart from the two fitting discs only one regular track (short-circuit-2-1988-dvd's English VobSub) is labelled. (b) no (Tesseract, canvas and changes renders: 361-487 s per hour of running time on the UHD and the Blu-ray episodes, 117 s/h on the DVD; summary.json): as row 14's (b) fallback says, the text is read only where spec 6.4 step 10 runs (two or more English subtitle streams). | `EXP-14-design.md` |

## Spec sections changed in phase 0

Every section phase 0 changed: those Task 17 changed, and 2, 7 and 15, which phase 0 changed before Task 17 (2: the
owner's decisions D13 to D23; 7: the tuning evidence of D16; 15: `retune`). The final review's fixes of 2026-10-12
changed 4.4, 6.4, 7, 8.1, 8.7, 15 and 17 again.

- 1. Purpose
- 2. Owner requirements and decisions
- 3. Target environment
- 4.3 Platform interfaces
- 4.4 External programs
- 5. Identifiers and paths
- 6.1 Disc phase, common steps
- 6.2 Direct mode
- 6.3 Whole-disc copy mode
- 6.4 File phase
- 7. Storage
- 8.1 Identification
- 8.3 Movie titles
- 8.6 Audio
- 8.7 English subtitles
- 8.8 Confidence, review reasons and disposition
- 8.9 Naming
- 8.10 Track metadata in the final file
- 9.1 Versions
- 9.2 Commands
- 9.3 Job profile
- 9.4 Output parsing
- 9.5 Stream flags
- 10.1 States
- 10.2 Failure classes
- 10.3 The tray
- 10.4 Watchdogs
- 11. Review queue
- 12. Notifications
- 14. Settings
- 15. Testing
- 16. Build phases
- 17. Experiments (phase 0)
- 18. Risks
- 19. Setup prerequisites

## Discs recorded

| Slug | Format | What it covers |
|---|---|---|
| pans-labyrinth-2006-uhd | bluray | Disc A, a UHD with Java menus (2 titles), never scanned on the mini PC before phase 0: experiments 1 (`rehearsal`, `all-tracks`), 2 (`exp02`), 3 (`backup`), 5 (`scan-offline.txt`, `drives-offline.txt`), 7 and the second drive (`drive2`), 11 (`eng-only`), 12 (`scan.txt`, no Java labels), 13 (an English commentary on a Spanish-language film; its label rests on the packaging, which D23 makes an observation, so it does not count, and the thresholds were fitted on it anyway) and 14 (English and Spanish PGS dialogue tracks); a TheDiscDB hit by fingerprint (experiment 10) |
| short-circuit-1986-dvd | dvd | A Task 15 DVD (1 title) with no subtitle stream at all, no flagged stream and no closed captions; the feature ripped with all tracks (`all-tracks`); experiment 13 measured its second English audio track (a lossy companion of the 5.1 track); TheDiscDB's first miss by fingerprint (experiment 10) |
| short-circuit-2-1988-dvd | dvd | A Task 15 DVD (7 titles): the closed-caption stream (`CC`, converted to text, marked lossy) and seven unflagged VobSub tracks; a failed rip on a damaged spot (`all-tracks`: `MSG:2003` read errors, `MSG:5003`), then a successful one after the disc was cleaned (`all-tracks-2`); experiment 1's DVD observations (`all-tracks-2`: closed captions first, duration, default flag); experiment 11's second disc (`eng-only`); experiment 14's first DVD VobSub track; a TheDiscDB miss by fingerprint (experiment 10) |
| the-office-2005-s3d3-bluray | bluray | The first disc of phase 0 (11 titles), identified by its fingerprint: the rehearsals for experiments 1, 2 and 11 to 14 (`rehearsal`); two commentaries the owner named by listening (S03E16 in `e16-stereo`, S03E12-13 in `t3-all-tracks`) and an exact duplicate audio track (S03E15), on which experiment 13's thresholds were fitted; three SDH episodes (PGS), one of which (S03E16) fitted experiment 14's thresholds; a TheDiscDB hit by fingerprint (experiment 10) |

## What could not be done, and why

- Experiment 1 (Task 11): the Dolby Vision question (one video track or two) is not answered. No Dolby Vision disc was recorded; Disc A is HDR10 without Dolby Vision.
- Experiment 1, a correction to its report (2026-10-12): its consequence that subtitle event counts "can be read from `NUMBER_OF_FRAMES`" is wrong. The recordings show that `NUMBER_OF_FRAMES` is the packet count: twice the event count on PGS (1774 = 2 x 887 on Pan's English track; 690, 726 and 1420 = 2 x 345, 363 and 710 on the Office), equal to it on VobSub. Spec 6.4 step 10 therefore keeps the extraction; do not act on that bullet of `EXP-01-profile-and-order.md`.
- Experiment 2 (Task 15 Step 2, the repeat on a disc with angles or duplicate titles): not run. None of the four discs recorded has angles or duplicate titles (no `MSG:3308` or `MSG:3309` line in any scan), so the experiment's condition is not yet met.
- Experiment 3 (Task 14 Step 6, extracting the main feature from the copy): skipped. MakeMKV 2.0.0 crashed (0xC0000005) while scanning the copy, so there was nothing to extract and compare. Whether the copy's scan works on another UHD with Java menus is open, and `extract-from-copy.ps1`'s refusal of a copy whose `backup-summary.json` does not say `"ok": true` was not tested; that `mkv file:` on the copy would crash too is inferred, not tested (EXP-03).
- Experiment 4 (Task 11 Steps 8 to 10: the share-drop test, the repeats at lower cache sizes, the report): not run. The share-drop test needs the owner at the NAS and was not done. The repeats at lower cache sizes were not needed: free RAM never fell below 3.02 GB at `cache_mb` 1024 in Task 11's rip (Task 11 Step 7). No report was written. The messages a share drop produces, whether `makemkvcon` exits after one, and the working `cache_mb` are not yet verified.
- Experiment 5 (Task 9): what MakeMKV prints when a disc needs a key it does not have and cannot download is not answered. It needs a UHD whose keys are not in `keydb.cfg`, and none of the owner's discs has shown that so far.
- Experiment 6 (Task 12 Steps 1 to 12): not run, and no report was written. No disc with Atmos or DTS:X and no title with two or more full English subtitle tracks was recorded; the one UHD recorded has neither format (EXP-01). Whether `ffprobe` tells Atmos and DTS:X apart, and whether 8.7's alignment thresholds separate dialogue from commentary, are not yet verified. Experiment 14 keeps 8.7's timing bands as its guard and fallback, so the alignment half stays open.
- Experiment 7 (Task 14 Steps 8 and 9): not observed: what drive 2 does when an ejected disc is left in its slot. Spec 10.3's open-tray signal is untested on a slot-loading drive.
- Experiment 8 (Task 15 Step 4): not run, because no disc with a non-ASCII title was recorded.
- Experiment 9 (Task 16): not run. It needs the owner's test library folder and the versions of the owner's media servers (Task 16 Step 1).
- Experiment 10 (Task 15 Step 5): not run as an experiment. Only 4 discs were recorded of about 20: 2 TheDiscDB hits and 2 misses.
- Experiment 11 (Task 13): not answered. `nolang` needs a disc with a language-less audio stream, `special` a DVD whose commentary is flagged, and `core` on its own an English-only rip of an Office title; none was ripped. `-sel:mvcvideo` on a 3D title was not tested either.
- Experiment 12 (Task 15 Step 6): not answered. No later UHD was recorded to repeat the check on, and `app_Java` was not set to the installed Java 8, the one change the report names as worth trying.
- Experiment 13 (Task 12 Steps 19 and 20): not answered. The owner's discs so far do not supply the labelled tracks row 13 requires (0 of 8 commentaries, 0 of 20 programme tracks and 0 of 6 separately encoded ones count; only a track whose content was checked before the verdict counts, D23).
- Experiment 14 (Task 12 Steps 29 and 30): (a) not answered. Apart from the two fitting discs only one regular track is labelled. (b) is answered by the recordings: no (Tesseract, canvas and changes renders: 361-487 s per hour of running time on the UHD and the Blu-ray episodes, 117 s/h on the DVD; summary.json, over each title's duration in its `mkvmerge.json`), so the text is read only where spec 6.4 step 10 runs.

## Installs and other changes on the mini PC

| Package | winget ID | Version |
|---|---|---|
| MakeMKV | `GuinpinSoft.MakeMKV` | 2.0.0 (upgraded from 1.18.3) |
| MKVToolNix | `MoritzBunkus.MKVToolNix` | 100.0.0 |
| FFmpeg | `Gyan.FFmpeg` | 9.0.2 |
| MediaInfo (command line) | `MediaArea.MediaInfo` | 26.05 |
| Tesseract OCR (approved 2026-10-06, for experiment 14: reading bitmap subtitles) | `UB-Mannheim.TesseractOCR` | 5.4.0.20240606 (leptonica 1.84.1; languages `eng`, `osd`; `C:\Program Files\Tesseract-OCR\tesseract.exe`) |

- The scheduled task `lazymkv-phase0`, registered by `scripts/phase0/run-interactive.ps1` for every job, so that each job runs in the owner's desktop session.
- The firewall rule `lazymkv-exp05`, which blocked MakeMKV's outbound network access for experiment 5's offline runs: added and removed by the owner in Task 9 (after its removal `netsh` answered `No rules match the specified criteria.`).
- Smart App Control: turned off by the owner on 2026-10-06, after it had blocked `cargo` (`docs/phase0/environment.md`).
- The folders `C:\lazymkv\src` (the repository, replaced by `scripts/box.sh` on each sync), `C:\lazymkv\target` (the Cargo build output) and `C:\lazymkv\phase0`. The last holds the owner's `config.ps1` (`$Staging`, `$Drive`, `$TuningData`, and later `$MacHost`), the job profiles `profiles\all-tracks.mmcp.xml` and `profiles\eng-only.mmcp.xml`, and the folders `recordings` (one folder per disc), `logs` (the job logs), `tmp` (staging for recordings and checks), `work` (the OCR working files of `ocr-subs.ps1`) and `models` (the two ocrs model files, `text-detection.onnx` and `text-recognition.onnx`).
- The owner's TMDb token, set as `TMDB_TOKEN` on 2026-10-05 (the token is stored nowhere in this repository).
- Drive 2 (Pioneer BDR-XS07U, slot-loading) attached in place of drive 1 since Task 14 (`$Drive` in `config.ps1`).
- On the share: the staging folder `\\unas-pro\media\lazymkv-staging` (phase 0 writes only there) and the tuning dataset folder `\\unas-pro\media\lazymkv-tuning` (`$TuningData`, decision D16).
- The test library folder for experiment 9: none made yet, since experiment 9 was not run.

## What comes next

The spec's section 16 phases follow: phase 1 (the decision core, tested on recordings) and phase 2 (the direct-mode
engine on the mini PC), both with the developer (tuning) mode that keeps every rip's evidence as decision D16
describes, so that the open experiments continue as the owner's discs come through; the owner intends to start the
engine work next, with that developer mode. Unsure discs and failures (whole-disc copy mode, failure classes,
watchdogs, resume) follow in phase 3, editions and TV in phase 4, and the GPUI desktop app in phase 5.

## Closing checks, and what was left behind

- Stand-in replay (Task 17 Step 1): `fake-tool` replayed Disc A's online scan (`corpus/scenarios/disc-a-scan/`) with `LAZYMKV_FAKE_SPEED=0.001`; `fc /b` against the original recording on the mini PC: `FC: no differences encountered` (2026-10-06; checked before the machine user name in lines 2 and 47 of the scenario's `scan.txt` was scrubbed to `USER` for the public repository).
- Tests (Task 17 Step 4): Rust: `20 tests passed, 0 failed (core 8 + 1, capture 7 + 1, fake-tools 2 + 1)` (on the mini PC, 2026-10-12). Python: `Ran 202 tests in 45.370s` / `OK` (2026-10-13; 200 at Step 4 on 2026-10-12, plus 2 tests added by the final-review fix wave).
- What the experiments left on the share and on the mini PC, as `scripts/phase0/list-leftovers.ps1` listed it (job log `final-list`), and what the owner decided:

| Where | Item | Size | Decision |
|---|---|---|---|
| share | `pans-labyrinth-2006-uhd/all-tracks` | 50.6 GB | kept |
| share | `pans-labyrinth-2006-uhd/backup` | 53.0 GB | deleted by the owner |
| share | `pans-labyrinth-2006-uhd/eng-only` | 46.4 GB | deleted by the owner |
| share | `pans-labyrinth-2006-uhd/rehearsal` | 0.2 GB | deleted by the owner |
| share | `short-circuit-1986-dvd/all-tracks` | 4.0 GB | kept |
| share | `short-circuit-2-1988-dvd/all-tracks` | 0.0 GB (the failed rip) | deleted by the owner |
| share | `short-circuit-2-1988-dvd/all-tracks-2` | 3.6 GB | kept |
| share | `short-circuit-2-1988-dvd/eng-only` | 3.1 GB | deleted by the owner |
| share | `the-office-2005-s3d3-bluray/e16-stereo` | 4.4 GB | kept |
| share | `the-office-2005-s3d3-bluray/rehearsal` | 5.0 GB | kept |
| share | `the-office-2005-s3d3-bluray/t3-all-tracks` | 8.3 GB | kept |
| share | test library | (not set in `config.ps1`) | none was made (Task 16 not run) |
| share | tuning dataset `\\unas-pro\media\lazymkv-tuning` (8 title folders) | 2.4 GB | kept (never offered; decision D16) |
| mini PC | `tmp` | 0.5 MB | kept |
| mini PC | `exp09` | absent | nothing to decide |
| mini PC | `recordings` | 71.3 MB | kept |
| mini PC | `logs` | 0.1 MB | kept |

The owner deleted the five folders through the NAS's web file manager instead of the PowerShell commands of Step 6; `final-list-2` (2026-10-13) confirmed they are gone and the kept folders unchanged; the all-tracks rips are kept as phase 1 test inputs.

- Scheduled task `lazymkv-phase0`: kept (ssh check 2026-10-13: `TaskName: \lazymkv-phase0`); developer mode keeps running jobs on the mini PC, and `scripts/phase0/run-interactive.ps1` registers it again for any later job.
- Firewall rule `lazymkv-exp05`: no rule (`netsh`: "No rules match the specified criteria.").
