# The OCR trial

The optical character recognition (OCR) trial of the phase 2 design, part 10.9 (P17, P26, P34; pull request 5's notes B2, B3, F3 and calls 8 and 19), ran on the mini PC (Intel J4125, 4 cores) twice, its verdict below being the second run's, on 2026-11-08 on commit `164099d62bf00e1504617e3a1a6ee79473c90428` (continuous integration (CI) run 42520167404 green on ubuntu, macos and windows; CodeQL 0 open alerts), the fix of what the first run found; the first run, on 2026-11-07 with the build `9b7e45b1d1cbe9315f19e19cd895bea2c5f8378f` (CI run 42432584243 green on the three systems; CodeQL 0 open alerts), failed parity. The second run's parity was judged by `lazymkv-dev ocr-trial parity` as amended in commit `c6111d6` on the owner's ruling of 2026-11-08 (part 10.9: a count may differ by one event; everything else exact), run again over the unchanged arm outputs. The figures below are the second run's; the first run's are in this file's version at commit ebf3883 and summarised at the end. The tracks are named by their row in part 10.9's table. This file holds no subtitle text.

Verdict: PASS

The trial was reopened for ocrs 0.14.0 on 2026-11-24 and passed again: The reopened trial on ocrs 0.14.0, below. The sections between here and there are the trial on ocrs 0.13.1.

## The arms

- R, the shipped method: the release build of `lazymkv-ocr.exe` (ocrs 0.13.1, rten 0.26.0; models `text-detection.onnx` SHA-256 (Secure Hash Algorithm 256) `a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe` and `text-recognition.onnx` SHA-256 `86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5`, embedded), started by `lazymkv-dev ocr-trial read` through the Job Object guard at below-normal priority with `RTEN_NUM_THREADS=3` and the cap max(60, 2.03 x events) seconds, on a copy of each track's `evidence.mks` on C:. Constants fixed before the run (part 10.6): ink level 127, a gap of 2 rows, 3-pixel padding and a margin of at most 3 rows on each line box (16-pixel padding and no margin in the first run), factor 1.6, 3 lines, the video frame's middle row; line finder `lazymkv-rows 2`.
- F, the full ocrs method: phase 0's `ocrs-trial.exe` and `ocrs-trial.ps1`, unchanged, on the full phase 0 frames, models SHA-1 (Secure Hash Algorithm 1) `709176816006063da8155f549aa0e44735ed0e9c` and `b16f361df884a9afaa067e4d6932567f6f652a81`; row 2's F is phase 0's reading. F was read once, in the first run, and reused in the second: it does not depend on the fix.
- R', the row method on the phase 0 frames, in process, with `RTEN_NUM_THREADS=3`; reported beside R, deciding nothing.

## Speed

Row 4 (887 events) read through R in 212.7 s, 239.8 ms per event, against the bar of 600 s (about 676 ms per event): pass. Over the six tracks' 4,931 events R took 270.0 ms per event (information, not a condition).

| Row | Codec | Events | Cap s | End | Wall s | ms per event | run.json wall_ms | Distinct crops | Read by rows | Detector, triggers 1/2/3/4 | decode ms | prepare ms | lines ms | detect ms | recognize ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | PGS | 363 | 736.9 | Read | 95.2 | 262.3 | 94601 | 352 | 363 | 0/0/0/0 | 79.7 | 2232.7 | 13.5 | 0.0 | 90613.8 |
| 2 | PGS | 345 | 700.3 | Read | 99.9 | 289.6 | 99768 | 341 | 343 | 0/0/1/1 | 83.4 | 2231.4 | 12.3 | 1334.4 | 94447.8 |
| 3 | PGS | 710 | 1441.3 | Read | 212.9 | 299.9 | 212748 | 683 | 709 | 0/0/0/1 | 172.3 | 4543.3 | 34.0 | 691.9 | 203756.1 |
| 4 | PGS | 887 | 1800.6 | Read | 212.7 | 239.8 | 212555 | 859 | 887 | 0/0/0/0 | 152.9 | 5467.6 | 23.5 | 0.0 | 203027.0 |
| 5 | PGS | 884 | 1794.5 | Read | 242.1 | 273.9 | 241995 | 853 | 884 | 0/0/0/0 | 161.4 | 5367.6 | 27.3 | 0.0 | 232200.0 |
| 6 | VobSub | 1742 | 3536.3 | Read | 468.8 | 269.1 | 468705 | 1691 | 1742 | 0/0/0/0 | 134.8 | 1626.8 | 38.1 | 0.0 | 462151.2 |

| Row | F frames | F ms per frame | F wall s | R' frames | R' wall s |
|---|---|---|---|---|---|
| 1 | 616 | 791.5 | 524.5 | 616 | 123.7 |
| 2 | 600 | 933 | 560 | 600 | 126.6 |
| 3 | 1238 | 849.7 | 1126.8 | 1238 | 264.7 |
| 4 | 1732 | 806.2 | 1500.4 | 1732 | 277.9 |
| 5 | 1726 | 834.0 | 1543.6 | 1726 | 305.0 |
| 6 | 2809 | 811.8 | 2336.4 | 2809 | 501.9 |

Phase 0's reference figures on the same machine: Tesseract about 95 ms per frame (57 s for 600 frames); ocrs on the full frame 933 ms per frame (560 s for 600 frames, model load 19 ms); ocrs on the glyph-box crop 854 ms per read frame, of which detection 613 ms, recognition 240 ms, PNG (Portable Network Graphics) decode 57 ms and the box search 4 ms; the detection model takes a fixed 800 x 600 input.

## Parity

Parity passes: R matches F on every track in the counts CUE, LABEL, CHEV, M, SO, CS, ODD, NOTE and CAPS_CUE within one event (the rule as amended on 2026-11-08; equal counts before), in caps_style, the class and the readable verdict, in the side of every EXP-14 section 3 threshold for letter_events, wordlike and each stopword share, in the language verdict, in CB's band in both directions for rows 4 and 5, and in every stream's role, review and note. Rows 1, 4, 5 and 6 match on everything; row 2 differs in ODD by one event (F 1, R 0: F reads an extra line that is a lone opening bracket) and row 3 in LABEL and M by one event (F 18 and 42, R 17 and 41: R reads one speaker label's colon as a period). Under the equal-counts rule those two single events failed parity; the owner amended the rule rather than chase exact equality against a reference with its own misreads.

| Row | Arm | CUE | LABEL | CHEV | M | SO | CS | ODD | NOTE | CAPS_CUE | caps_style | class | readable | language | decision |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | F | 11 | 6 | 0 | 17 | 11 | 6 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 1 | R | 11 | 6 | 0 | 17 | 11 | 6 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 1 | R' | 11 | 6 | 0 | 17 | 11 | 6 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | F | 10 | 7 | 0 | 17 | 9 | 10 | 1 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | R | 10 | 7 | 0 | 17 | 9 | 10 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | R' | 10 | 7 | 0 | 17 | 9 | 10 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 3 | F | 24 | 18 | 0 | 42 | 20 | 18 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 3 | R | 24 | 17 | 0 | 41 | 20 | 18 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 3 | R' | 24 | 17 | 0 | 41 | 20 | 18 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 4 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 4 | R | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 4 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 5 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 5 | R | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 5 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 6 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 6 | R | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 6 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |

Row 1: letter_events 0.997 both; wordlike F 0.995, R 0.996; stopword shares en F 0.395, R 0.419, es 0.001 both, fr F 0.001, R 0.000, de 0.000 both; every share on the same side of its threshold.
Row 2: letter_events 1.000 both; wordlike F 0.993, R 0.998; stopword shares en F 0.409, R 0.426, es F 0.002, R 0.001, fr F 0.001, R 0.000, de 0.000 both; every share on the same side of its threshold.
Row 3: letter_events 1.000 both; wordlike F 0.992, R 0.996; stopword shares en F 0.398, R 0.416, es F 0.001, R 0.000, fr F 0.001, R 0.000, de 0.000 both; every share on the same side of its threshold.
Rows 4 and 5: row 4 letter_events 1.000 both, wordlike F 0.996, R 0.991, stopword shares en F 0.388, R 0.398, es, fr and de 0.000 both; row 5 letter_events 1.000 both, wordlike F 0.862, R 0.864, stopword shares en 0.012 both, es F 0.277, R 0.285, fr 0.011 both, de 0.000 both; every share on the same side of its threshold; CB between the two streams F 0.003/0.002, R 0.003/0.002, the same band in both directions. The title's other streams are not in the trial.
Row 6: letter_events F 0.995, R 0.997; wordlike 0.980 both; stopword shares en F 0.386, R 0.395, es 0.002 both, fr F 0.001, R 0.000, de 0.000 both; every share on the same side of its threshold. The title's text stream (set aside first as closed captions, a text stream) and its six non-English bitmap streams (dropped by tag) are not read by either arm and agree.

Differing measures under the amended rule: none on any row (under equal counts: row 2 ODD; row 3 LABEL and M). Between F and R, `parity` gives a character error rate (CER) and an exact-match share per row over the events it paired, computed as `ocr-compare` computes them (reported only): row 1 0.033, 0.614; row 2 0.020, 0.623; row 3 0.027, 0.600; row 4 0.021, 0.762; row 5 0.021, 0.636; row 6 0.012, 0.784.

## The owner's labels (F3, report only)

`lazymkv-dev retune --subtitles` over arm R's text against the phase 0 subtitle labels ran in the first run and exited 3: the evidence mirror holds 0 recordings with `subtitle-labels.json` and 0 labelled tracks, so no label of any kind was counted (regular 0, sdh 0, forced 0, commentary 0, trivia 0, lyrics 0, translation 0, dub-transcript 0, closed-captions 0, other 0); costly verdicts none; 0 labelled tracks placed without a review; 0 labels not judged (labelled tracks outside the six have no R text). It was not run again in the second run, since no label exists to judge. The owner's labels are to be set with `lazymkv-dev labels --subtitles set` before F3 can report anything.

## The first run

The first run (2026-11-07, commit 9b7e45b) passed speed (row 4 in 257.9 s, 290.8 ms per event; 343.6 ms per event over the six tracks) and failed parity on every title: R's ODD was 4, 7, 11, 1 and 35 against F's 0, 1, 0, 0 and 0 on rows 1, 2, 3, 4 and 6, with NOTE 22 and the class unclear on row 6 and single-event differences in LABEL, M, CUE, SO, CS and CAPS_CUE on rows 1 and 3; CER 0.037 to 0.051 and exact-match shares 0.321 to 0.471. A read-only diagnosis traced every difference to one defect: ocrs's recognizer, handed a line box with a 16-pixel white lead-in and no vertical margin, misread the start of 44% of the lines (the first glyph doubled, dropped, or replaced by `[`, `|`, `?` or `*`), while its own detector hands it rectangles expanded by 3 pixels. Commit 164099d set the padding to 3 pixels and added a margin of at most 3 rows (line finder `lazymkv-rows 2`); the second run's CER is 0.012 to 0.033 and its exact-match shares 0.600 to 0.784, its ODD and NOTE counts match F's on five of six rows, and the two remaining differences are single events.

## The reopened trial on ocrs 0.14.0

The upgrade of ocrs 0.13.1 to 0.14.0 and rten 0.26.0 to 0.28.0 reopened this trial (ruling U3 of 2026-11-14, `docs/phase2/engine.md`, "The ocrs 0.14 upgrade: the owner's decisions"; T5-R54). It ran on the mini PC on 2026-11-24 on commit `bfbc653287d5c1070bf469a542c4a41519a2f91d` (CI run 45436537397 green on ubuntu, macos and windows; the release dry run 45436537379 green; CodeQL 0 open alerts), with the engine stopped, the same scripts, constants, rows, cap and parity rule as the second run above, and Rust 1.100.0 where the second run's build used 1.99.0.

Verdict: PASS

- R: the release build of `lazymkv-ocr.exe` on that commit (ocrs 0.14.0, rten 0.28.0; the same two embedded models), started as above.
- F: not read again. It is phase 0's reading by `ocrs-trial.exe` on ocrs 0.13.1, the reference the upgrade is judged against.
- R': the row method on the phase 0 frames, in process, on ocrs 0.14.0; reported beside R, deciding nothing.

Speed: row 4 (887 events) read through R in 219.7 s, 247.7 ms per event, against the bar of 600 s: pass. Over the six tracks' 4,931 events R took 276.5 ms per event (information, not a condition; 270.0 ms on 0.13.1).

| Row | Codec | Events | Cap s | End | Wall s | ms per event | run.json wall_ms | Distinct crops | Read by rows | Detector, triggers 1/2/3/4 | decode ms | prepare ms | lines ms | detect ms | recognize ms | Wall s on 0.13.1 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | PGS | 363 | 736.9 | Read | 95.9 | 264.2 | 95248 | 352 | 363 | 0/0/0/0 | 76.8 | 2197.9 | 16.5 | 0.0 | 91298.9 | 95.2 |
| 2 | PGS | 345 | 700.3 | Read | 103.0 | 298.6 | 102903 | 341 | 343 | 0/0/1/1 | 78.4 | 2142.2 | 17.3 | 1428.4 | 97526.2 | 99.9 |
| 3 | PGS | 710 | 1441.3 | Read | 219.2 | 308.7 | 219129 | 683 | 709 | 0/0/0/1 | 163.5 | 4350.7 | 53.5 | 784.9 | 210156.6 | 212.9 |
| 4 | PGS | 887 | 1800.6 | Read | 219.7 | 247.7 | 219566 | 859 | 887 | 0/0/0/0 | 147.8 | 5279.4 | 48.0 | 0.0 | 210161.8 | 212.7 |
| 5 | PGS | 884 | 1794.5 | Read | 248.4 | 281.0 | 248334 | 853 | 884 | 0/0/0/0 | 158.1 | 5284.9 | 48.5 | 0.0 | 238589.3 | 242.1 |
| 6 | VobSub | 1742 | 3536.3 | Read | 477.1 | 273.9 | 476992 | 1691 | 1742 | 0/0/0/0 | 121.8 | 1588.1 | 42.8 | 0.0 | 470509.0 | 468.8 |

| Row | R' frames | R' blank | R' rows | R' wall s | R' wall s on 0.13.1 |
|---|---|---|---|---|---|
| 1 | 616 | 253 | 363 | 126.1 | 123.7 |
| 2 | 600 | 255 | 343 | 129.1 | 126.6 |
| 3 | 1238 | 528 | 709 | 271.0 | 264.7 |
| 4 | 1732 | 845 | 887 | 285.2 | 277.9 |
| 5 | 1726 | 842 | 884 | 317.3 | 305.0 |
| 6 | 2809 | 1069 | 1740 | 509.2 | 501.9 |

Parity passes under the rule as amended on 2026-11-08: `lazymkv-dev ocr-trial parity` printed `PARITY PASS` for each of the five titles, for F against R and for F against R'. Every count, class, verdict, share, band and decision it printed for R equals the second run's table above: rows 1, 4, 5 and 6 match F on everything, row 2 differs from F in ODD by one event (F 1, R 0) and row 3 in LABEL and M by one event (F 18 and 42, R 17 and 41). The character error rate and the exact-match share between F and R are the same as well: row 1 0.033, 0.614 (223 of 363); row 2 0.020, 0.623 (215 of 345); row 3 0.027, 0.600 (426 of 710); row 4 0.021, 0.762 (676 of 887); row 5 0.021, 0.636 (562 of 884); row 6 0.012, 0.784 (1365 of 1742).

The readings did not change with the upgrade: a read-only comparison on the share found arm R's records on 0.14.0 equal to the second run's on 0.13.1 line for line on all six rows, 4,931 lines, once each line's timings member `ms` is removed and the engine's version member is made the same; arm R's crops number the same (352, 341, 683, 859, 853 and 1691); the records of R' differ in their one summary line alone, which holds the run's timings. So what was read is the same, and arm R's wall times are 0.7 to 8.3 s longer per row (0.7% to 3.3%), on a build by another Rust version.

The owner's labels (F3, report only) were not run again: U3 names R, R', the bar and the worker check, and a report over records equal to the second run's would equal a report over those.

The second run's kept output moved from `subs-ocr-engine` to `subs-ocr-engine-trial2` on the share for the five titles before the run (356, 345, 687, 1719 and 1695 files, nothing deleted), as the first run's had moved to `subs-ocr-engine-trial1`; `subs-ocr-engine` now holds this run's arm R and R'.

The worker check made again (T6-R15, U3): `scripts/phase0/worker-check.ps1` ran over the six rows with the `lazymkv-ocr.exe` of the pull request's own release dry run (run 45436537379 on `bfbc653`; the zip's SHA-256 `536fe67130a70b1d8ff90af2262e5783179441d7d779a6c167980ebc58576fdc` equal to its `SHA256SUMS` line, the worker's `cff5ecd0ac66380cd1a2e49a984d9c511a0fb36ec455685283cd2871a37a864c`), started by the mini PC's `lazymkv-dev ocr-trial read` under the 2 GiB (gibibytes) limit. Every row printed `worker sha256 equal`, ended Read with its records equal to this run's arm R line for line (the timings member aside) and printed `RESULT ok`.

| Row | Events | Wall s | Peak committed MiB (mebibytes) | Records |
|---|---|---|---|---|
| 1 | 363 | 101.9 | 82.9 | equal, 363 lines |
| 2 | 345 | 108.8 | 193.3 | equal, 345 lines |
| 3 | 710 | 227.6 | 245.0 | equal, 710 lines |
| 4 | 887 | 226.1 | 121.7 | equal, 887 lines |
| 5 | 884 | 253.8 | 137.2 | equal, 884 lines |
| 6 | 1742 | 483.9 | 91.3 | equal, 1742 lines |

Row 4's 226.1 s is under the check's 600 s, and the largest peak, 245.0 MiB, is under its 1 GiB.
