# The OCR trial

The optical character recognition (OCR) trial of the phase 2 design, part 10.9 (P17, P26, P34; pull request 5's notes B2, B3, F3 and calls 8 and 19), ran on the mini PC (Intel J4125, 4 cores) twice: first on 2026-11-07 on commit `9b7e45b1d1cbe9315f19e19cd895bea2c5f8378f` (continuous integration (CI) run 42432584243 green on ubuntu, macos and windows; CodeQL 0 open alerts), which failed parity, and again on 2026-11-08 on commit `164099d62bf00e1504617e3a1a6ee79473c90428` (CI run 42520167404 green on the three systems; CodeQL 0 open alerts), the fix of what the first run found. The figures below are the second run's; the first run's are in this file's version at commit ebf3883 and summarised at the end. The tracks are named by their row in part 10.9's table. This file holds no subtitle text.

Verdict: FAIL (parity)

The pull request stops here and goes to the owner (B3); nothing is built for the Tesseract route of part 10.10 before the owner decides.

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

Parity fails: R does not match F on every track in the counts CUE, LABEL, CHEV, M, SO, CS, ODD, NOTE and CAPS_CUE, in caps_style, the class and the readable verdict, in the side of every EXP-14 section 3 threshold for letter_events, wordlike and each stopword share, in the language verdict, in CB's band in both directions for rows 4 and 5, and in every stream's role, review and note. Rows 1, 4, 5 and 6 match on everything; row 2 differs in ODD by one event (F 1, R 0) and row 3 in LABEL and M by one event (F 18 and 42, R 17 and 41). Every stream's role, review and note agree on all six rows.

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

Differing measures: row 1 none; row 2 ODD; row 3 LABEL and M; rows 4, 5 and 6 none. Between F and R, `parity` gives a character error rate (CER) and an exact-match share per row over the events it paired, computed as `ocr-compare` computes them (reported only): row 1 0.033, 0.614; row 2 0.020, 0.623; row 3 0.027, 0.600; row 4 0.021, 0.762; row 5 0.021, 0.636; row 6 0.012, 0.784.

## The owner's labels (F3, report only)

`lazymkv-dev retune --subtitles` over arm R's text against the phase 0 subtitle labels ran in the first run and exited 3: the evidence mirror holds 0 recordings with `subtitle-labels.json` and 0 labelled tracks, so no label of any kind was counted (regular 0, sdh 0, forced 0, commentary 0, trivia 0, lyrics 0, translation 0, dub-transcript 0, closed-captions 0, other 0); costly verdicts none; 0 labelled tracks placed without a review; 0 labels not judged (labelled tracks outside the six have no R text). It was not run again in the second run, since no label exists to judge. The owner's labels are to be set with `lazymkv-dev labels --subtitles set` before F3 can report anything.

## The first run

The first run (2026-11-07, commit 9b7e45b) passed speed (row 4 in 257.9 s, 290.8 ms per event; 343.6 ms per event over the six tracks) and failed parity on every title: R's ODD was 4, 7, 11, 1 and 35 against F's 0, 1, 0, 0 and 0 on rows 1, 2, 3, 4 and 6, with NOTE 22 and the class unclear on row 6 and single-event differences in LABEL, M, CUE, SO, CS and CAPS_CUE on rows 1 and 3; CER 0.037 to 0.051 and exact-match shares 0.321 to 0.471. A read-only diagnosis traced every difference to one defect: ocrs's recognizer, handed a line box with a 16-pixel white lead-in and no vertical margin, misread the start of 44% of the lines (the first glyph doubled, dropped, or replaced by `[`, `|`, `?` or `*`), while its own detector hands it rectangles expanded by 3 pixels. Commit 164099d set the padding to 3 pixels and added a margin of at most 3 rows (line finder `lazymkv-rows 2`); the second run's CER is 0.012 to 0.033 and its exact-match shares 0.600 to 0.784, its ODD and NOTE counts match F's on five of six rows, and the two remaining differences are single events.
