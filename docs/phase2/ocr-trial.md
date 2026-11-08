# The OCR trial

The optical character recognition (OCR) trial of the phase 2 design, part 10.9 (P17, P26, P34; pull request 5's notes B2, B3, F3 and calls 8 and 19), ran once, from 2026-11-07 to 2026-11-08, on the mini PC (Intel J4125, 4 cores), on commit `9b7e45b1d1cbe9315f19e19cd895bea2c5f8378f` (continuous integration (CI) run 42432584243 green on ubuntu, macos and windows; CodeQL 0 open alerts). The tracks are named by their row in part 10.9's table. This file holds no subtitle text.

Verdict: FAIL (parity)

The pull request stops here and goes to the owner (B3); nothing is built for the Tesseract route of part 10.10 before the owner decides.

## The arms

- R, the shipped method: the release build of `lazymkv-ocr.exe` (ocrs 0.13.1, rten 0.26.0; models `text-detection.onnx` SHA-256 (Secure Hash Algorithm 256) `a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe` and `text-recognition.onnx` SHA-256 `86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5`, embedded), started by `lazymkv-dev ocr-trial read` through the Job Object guard at below-normal priority with `RTEN_NUM_THREADS=3` and the cap max(60, 2.03 x events) seconds, on a copy of each track's `evidence.mks` on C:. Constants fixed before the run (part 10.6): ink level 127, a gap of 2 rows, 16-pixel padding, factor 1.6, 3 lines, the video frame's middle row.
- F, the full ocrs method: phase 0's `ocrs-trial.exe` and `ocrs-trial.ps1`, unchanged, on the full phase 0 frames, models SHA-1 (Secure Hash Algorithm 1) `709176816006063da8155f549aa0e44735ed0e9c` and `b16f361df884a9afaa067e4d6932567f6f652a81`; row 2's F is phase 0's reading.
- R', the row method on the phase 0 frames, in process, with `RTEN_NUM_THREADS=3`; reported beside R, deciding nothing.

## Speed

Row 4 (887 events) read through R in 257.9 s, 290.8 ms per event, against the bar of 600 s (about 676 ms per event): pass. Over the six tracks' 4,931 events R took 343.6 ms per event (information, not a condition).

| Row | Codec | Events | Cap s | End | Wall s | ms per event | run.json wall_ms | Distinct crops | Read by rows | Detector, triggers 1/2/3/4 | decode ms | prepare ms | lines ms | detect ms | recognize ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | PGS | 363 | 736.9 | Read | 121.4 | 334.4 | 120780 | 352 | 363 | 0/0/0/0 | 78.8 | 2544.6 | 15.2 | 0.0 | 116291.2 |
| 2 | PGS | 345 | 700.3 | Read | 126.3 | 366.1 | 126204 | 341 | 343 | 0/0/1/1 | 80.5 | 2411.7 | 13.5 | 1516.0 | 120257.2 |
| 3 | PGS | 710 | 1441.3 | Read | 260.1 | 366.3 | 259943 | 683 | 709 | 0/0/0/1 | 163.6 | 4900.5 | 33.1 | 701.7 | 250467.8 |
| 4 | PGS | 887 | 1800.6 | Read | 257.9 | 290.8 | 257743 | 859 | 887 | 0/0/0/0 | 148.3 | 5917.5 | 28.2 | 0.0 | 247613.5 |
| 5 | PGS | 884 | 1794.5 | Read | 288.3 | 326.1 | 288162 | 853 | 884 | 0/0/0/0 | 159.7 | 6004.9 | 32.0 | 0.0 | 277621.3 |
| 6 | VobSub | 1742 | 3536.3 | Read | 640.1 | 367.5 | 639963 | 1691 | 1742 | 0/0/0/0 | 122.3 | 1810.8 | 36.1 | 0.0 | 633352.5 |

| Row | F frames | F ms per frame | F wall s | R' frames | R' wall s |
|---|---|---|---|---|---|
| 1 | 616 | 791.5 | 524.5 | 616 | 140.7 |
| 2 | 600 | 933 | 560 | 600 | 144.1 |
| 3 | 1238 | 849.7 | 1126.8 | 1238 | 304.8 |
| 4 | 1732 | 806.2 | 1500.4 | 1732 | 316.4 |
| 5 | 1726 | 834.0 | 1543.6 | 1726 | 346.5 |
| 6 | 2809 | 811.8 | 2336.4 | 2809 | 672.2 |

Phase 0's reference figures on the same machine: Tesseract about 95 ms per frame (57 s for 600 frames); ocrs on the full frame 933 ms per frame (560 s for 600 frames, model load 19 ms); ocrs on the glyph-box crop 854 ms per read frame, of which detection 613 ms, recognition 240 ms, PNG (Portable Network Graphics) decode 57 ms and the box search 4 ms; the detection model takes a fixed 800 x 600 input.

## Parity

Parity fails: R does not match F on every track in the counts CUE, LABEL, CHEV, M, SO, CS, ODD, NOTE and CAPS_CUE, in caps_style, the class and the readable verdict, in the side of every EXP-14 section 3 threshold for letter_events, wordlike and each stopword share, in the language verdict, in CB's band in both directions for rows 4 and 5, and in every stream's role, review and note.

| Row | Arm | CUE | LABEL | CHEV | M | SO | CS | ODD | NOTE | CAPS_CUE | caps_style | class | readable | language | decision |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | F | 11 | 6 | 0 | 17 | 11 | 6 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 1 | R | 11 | 7 | 0 | 18 | 11 | 6 | 4 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 1 | R' | 11 | 7 | 0 | 18 | 11 | 6 | 4 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | F | 10 | 7 | 0 | 17 | 9 | 10 | 1 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | R | 10 | 7 | 0 | 17 | 9 | 10 | 7 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 2 | R' | 10 | 7 | 0 | 17 | 9 | 10 | 7 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 3 | F | 24 | 18 | 0 | 42 | 20 | 18 | 0 | 0 | 0 | no | sdh | yes | en | sdh, no review, no note |
| 3 | R | 23 | 18 | 0 | 41 | 19 | 17 | 11 | 1 | 1 | no | sdh | yes | en | sdh, no review, no note |
| 3 | R' | 23 | 18 | 0 | 41 | 19 | 17 | 11 | 1 | 1 | no | sdh | yes | en | sdh, no review, no note |
| 4 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 4 | R | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 4 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 5 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 5 | R | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 5 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | es | drop, no review, not English (tag spa) |
| 6 | F | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | no | regular | yes | en | regular, no review, no note |
| 6 | R | 0 | 0 | 0 | 0 | 0 | 0 | 35 | 22 | 0 | no | unclear | yes | en | regular, subtitle_ambiguous, 35 events with damaged brackets |
| 6 | R' | 0 | 0 | 0 | 0 | 0 | 0 | 35 | 22 | 0 | no | unclear | yes | en | regular, subtitle_ambiguous, 35 events with damaged brackets |

Row 1: letter_events F 0.997, R 0.997; wordlike F 0.995, R 0.990; stopword shares en F 0.395, R 0.361, es F 0.001, R 0.002, fr F 0.001, R 0.003, de 0.000 both; every share on the same side of its threshold.
Row 2: letter_events F 1.000, R 0.997; wordlike 0.993 both; stopword shares en F 0.409, R 0.371, es 0.002 both, fr 0.001 both, de 0.000 both; every share on the same side of its threshold.
Row 3: letter_events F 1.000, R 0.999; wordlike 0.992 both; stopword shares en F 0.398, R 0.365, es F 0.001, R 0.000, fr F 0.001, R 0.000, de 0.000 both; every share on the same side of its threshold.
Rows 4 and 5: row 4 letter_events 1.000 both, wordlike F 0.996, R 0.988, stopword shares en F 0.388, R 0.355, es, fr and de 0.000 both; row 5 letter_events F 1.000, R 0.999, wordlike F 0.862, R 0.881, stopword shares en F 0.012, R 0.010, es F 0.277, R 0.271, fr 0.011 both, de 0.000 both; every share on the same side of its threshold; CB between the two streams F 0.003/0.002, R 0.002/0.002, the same band in both directions. The title's other streams are not in the trial.
Row 6: letter_events F 0.995, R 0.996; wordlike F 0.980, R 0.974; stopword shares en F 0.386, R 0.353, es F 0.002, R 0.005, fr 0.001 both, de F 0.000, R 0.001; every share on the same side of its threshold. The title's text stream (set aside first as closed captions, a text stream) and its six non-English bitmap streams (dropped by tag) are not read by either arm and agree.

Differing measures: row 1 LABEL, M and ODD; row 2 ODD; row 3 CUE, M, SO, CS, ODD, NOTE and CAPS_CUE; row 4 ODD (row 5 none); row 6 ODD, NOTE, the class and the decision's review and note. Between F and R, `parity` gives a character error rate (CER) and an exact-match share per row over the events it paired, computed as `ocr-compare` computes them (reported only): row 1 0.051, 0.331; row 2 0.038, 0.351; row 3 0.044, 0.321; row 4 0.037, 0.471; row 5 0.045, 0.371; row 6 0.049, 0.389.

## The owner's labels (F3, report only)

`lazymkv-dev retune --subtitles` over arm R's text against the phase 0 subtitle labels exited 3: the evidence mirror holds 0 recordings with `subtitle-labels.json` and 0 labelled tracks, so no label of any kind was counted (regular 0, sdh 0, forced 0, commentary 0, trivia 0, lyrics 0, translation 0, dub-transcript 0, closed-captions 0, other 0); costly verdicts none; 0 labelled tracks placed without a review; 0 labels not judged (labelled tracks outside the six have no R text). The owner's labels are to be set with `lazymkv-dev labels --subtitles set` before F3 can report anything.
