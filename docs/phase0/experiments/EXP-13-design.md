# Experiment 13: commentary and audio description by audio content (design)

- Date: 2026-10-05
- Status: design for the owner and the controller. Committed copy; the scratch folders it names were temporary and are not kept. Nothing in the spec, the plan or the scripts is changed by this file; section 6 gives the script contents and section 7 the plan steps.
- Built from three independent proposals (the "relation", "content" and "requirements" lenses), the adversarial attack on each, and a re-computation, by the synthesis, of every number quoted below: the two real tracks and 153 constructed runs, 155 runs in all. Scratch: `/private/tmp/claude-502/-Users-USER-code-lazymkv/d9f40c13-92fa-4abc-ab12-bdb22c67ac55/scratchpad/exp13-work/synth/` (`synth.py` the measures, `cases.py` the 127 pairs, `extra.py` 28 variants, `sweep.py` the sensitivity runs, `out-final.json`, `extra.json`, `sweep.log`, `repo/` the three proposed files).

**The owner's question: should stereo width be a requirement for calling a track non-essential? No.** Nothing in the rule below reads stereo width, and no candidate's codec, channel count or position is read to judge it. The only other uses are: the reference is the track that spec 8.6 step 3 ranks first (channels, fidelity, then position); the reference's centre channel is the dialogue curve when its layout has one; and, only if the owner approves question 10, row 3(f) refuses to judge when the reference is not the first track of its language (position used only to keep). Width fails both ways, as the owner said. Side minus mid, whole file, re-measured here:

| Track | Side minus mid |
|---|---|
| Real S03E16 commentary (owner listened) | -30.3 dB (notes.md, median of 100 ms frames: -41.8 dB) |
| Real S03E15 duplicate, front left and right | -3.8 dB (notes.md: -3.4 dB) |
| Constructed audio description over a stereo film mix | -15.1 dB |
| Constructed commentary over a stereo programme | -18.0 dB |
| Constructed lossy 2.0 companion of a mono film (must be kept) | -67.5 dB, the narrowest of all |

The track that must be kept is the narrowest, and the unwanted tracks over a stereo programme sit nearer the wide end. The content rule gives the real commentary the same verdict from its left channel alone (run R1-L) as from its mixdown (R1). Width is therefore not measured any more. The figures above remain in notes.md and in this file as observations.

**The design in one paragraph.** The candidate is compared with the top-ranked track of its language (the reference) using 20 ms speech-band loudness curves from one ffmpeg pass. A commentary does not carry the programme's dialogue: in the reference's dialogue windows its level does not follow the reference (DC, "dialogue carried", low). It also fills the reference's dialogue gaps with speech-level sound that the reference does not explain (AG, "added in the gaps", high), in nearly every tenth of the running time. DC is what protects programme audio: every lossy, compressed, delayed, remastered or rebalanced copy of the programme that the three attacks built still follows the reference's dialogue. AG and the spread across the running time are the commentary evidence. Syllabic modulation and level spread are extra necessary conditions; they protect nothing on their own. Judgeability checks refuse a verdict when the reference cannot be the programme, for example an isolated score with silence between cues. Until experiment 13 has answered yes on the owner's own labelled discs, a track with the commentary pattern is kept and flagged for review, never dropped. Audio description is never dropped by content in this version.

## 1. What the owner wants

From R3, R6 and D12 as amended, and the owner's binding constraints of 2026-10-05. "Rule today" is what the rule in section 3 does with the constructed stand-in, as measured in section 4.

| Track kind | Wanted | Rule today (stand-in) |
|---|---|---|
| The programme's main soundtrack (lossless 5.1/7.1, Atmos, DTS:X): the reference | keep | reference, never judged |
| Another mix or codec of the same programme: lossy core, 2.0 downmix, AC-3/AAC/MP2 companion, delayed or offset copy | keep | keep (17 runs: DC 0.76-1.00, AG 0.00) |
| Compressed, "night", dialogue-boosted or EQ'd mix of the programme | keep | keep (11 runs: DC 0.46-1.00, AG at most 0.18) |
| Different master: original theatrical or broadcast mix, remaster, waveform-incoherent other master, mono original beside a remix | keep | keep (14 runs: DC 0.82-0.99, AG 0.00) |
| A copy that drifts, or a different cut (scenes missing, inserted or replaced) | keep | keep (9 runs: DC 0.31-0.99, AG at most 0.28) |
| The programme with a different or replaced score | keep | keep (10 runs; one note where the gaps vanish) |
| The programme with extra music in its dialogue gaps | keep | keep or a note (8 runs: DC 0.92-0.99) |
| Programme track of a mono or 2.0 film (no centre channel) | keep | keep; the reference's mixdown serves as the dialogue curve (36 programme runs without a centre channel, none with the commentary verdict) |
| Isolated score (spec 8.6 keeps it) | keep | keep (a score-like stand-in as candidate, 3 runs besides the 155: DC 0.08-0.21, AG at most 0.08) |
| Laugh-track and no-laugh-track versions of an episode | keep (both are the programme) | keep or a note (9 runs: DC 0.84-1.00) |
| Exact duplicate of a kept track (same packets) | keep one copy, drop the other | dropped by packet MD5 before any audio analysis |
| Audio commentary, with the film absent, ducked or audible underneath | drop | commentary verdict when the film is at least 6 dB under the voices; note at 0 dB; keep when the film is 6 dB or more above the voices |
| Partial or scene-specific commentary | drop | not confirmable by content: note or keep |
| Audio description | drop | never dropped by content: note `possible_audio_description` or keep |
| Music-and-effects (M&E) track | ask the owner | keep (3 runs) |
| Same-language re-dub or alternate dialogue (an older or a censored English dub) | ask the owner | keep when the new lines sit in the original's slots; note when they do not |
| Alternate narration (documentary with a second narrator) | ask the owner | keep, note, or the commentary verdict (see open question 3) |
| Foreign-language dub tagged with a wanted language | ask the owner | keep when its lines fall in the original's slots (4 runs, which also stand for the re-dub row: c-dub, c-dub-noFC, q-c6, q-c6-noFC, DC 0.09-0.28, AG at most 0.15); a dub with its own timing is not constructed, the nearest runs (aq-A2, aq-A2s, aq-A2n, aq-A2n-noFC) gave keep, note, note and the commentary verdict |
| Unflagged trivia, interview or picture-in-picture speech | ask the owner | note or keep (an unrelated programme scores like it: note) |
| Sing-along or karaoke track | ask the owner | not constructed: judged like any candidate, so keep by row 9 or a note, and held for review if it shows the commentary pattern |

Questions for the owner:

1. Exact duplicates (identical packets): is it right to keep one copy and drop the other? This design assumes yes.
2. A music-and-effects track: keep or drop?
3. A same-language re-dub, alternate dialogue track or second narrator: keep or drop? Do any of your discs have one? This is the one kind the content rule can mistake for a commentary (section 4).
4. A foreign-language dub that the disc tags as English: keep or drop?
5. Unflagged trivia, interview or picture-in-picture speech tracks: drop?
6. A sing-along track: keep?
7. A commentary that covers only some scenes: is a note enough? The content rule cannot confirm it.
8. Until experiment 13 answers yes, should a track with the commentary pattern hold the whole job for review (this design's default, so that each one is listened to once), or only carry a note, as D12 says today?
9. Should the audio-description pattern hold the job for review, or only carry a note (this design's default; the pattern also fires on music or loud laughter in the gaps)?
10. May the content rule refuse to drop when the top-ranked track is not also the first track of its language on the disc? That reads position only to keep, never to drop (binding constraint 2 rules out position as the basis of a decision).
11. Once experiment 13 answers yes: drop a content-confirmed commentary, with a note in the `filed` notification and the raw rip kept for `retention_days` as the undo? Or only label it (commentary flag, name "Commentary (detected)", ordered last, never default) so that a wrong verdict stays reversible inside the file?
12. Which discs can you lend experiment 13: films with a commentary (at least one where the film is clearly audible under the voices), any disc with audio description, isolated scores, M&E tracks, mono or 2.0 films with a commentary, dialogue-heavy or stand-up titles, laugh-track pairs, second English dubs?
13. Can the Office disc go back in once, to rip title 3 (S03E12-13, which by its layout carries a commentary) and titles 2, 5 and 6 (by layout, duplicates), and will you listen to title 3's stereo track?

## 2. Measures

### Inputs: three loudness curves from one ffmpeg pass

For the reference stream `M` (the top-ranked track of the language) and each candidate stream `C`, one pass over the whole file computes, for every 20 ms, the RMS level of the 300-3400 Hz band in dB:

```
[0:M]asplit=2[m1][m2];
[m1]aformat=channel_layouts=mono,<tail>main-mix[o0];
[m2]pan=mono|c0=FC,<tail>main-fc[o1];           (only when M's layout has a centre channel besides others)
[0:C]aformat=channel_layouts=mono,<tail>cand-C[ok];   (one per candidate)
<tail> = aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,
         astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,
         ametadata=print:key=lavfi.astats.Overall.RMS_level:file=<name>
```

`aformat=channel_layouts=mono` uses ffmpeg's default downmix of any layout. Each value is floored at -100 dB (`-inf` and `nan` become -100) and rounded to 0.1 dB. That gives three series of one value per 20 ms frame:

- `M[t]`: the reference's mono mixdown;
- `R[t]`: the reference's dialogue curve, its centre channel when the layout has one (FC), otherwise `M` itself (mono and 2.0 films).

  Whether the layout has a centre channel is not decided from a list kept in the script: audio-levels.ps1 looks ffprobe's `channel_layout` up in the installed ffmpeg's own table (`ffmpeg -layouts`, in which 29 named layouts of ffmpeg 8.1.2 have FC besides other channels), or reads ffprobe's own spelling of an unnamed layout (`4 channels (FL+FR+FC+TFL)`). A layout it cannot resolve (`unknown`, `6 channels`, empty) counts as having no centre channel, and so does `mono`, whose only channel is the mixdown itself. The fallback is always `R = M`, the path measured in section 4 without a centre channel (the "-noFC" runs and the runs whose reference has no centre extract): 45 runs; none of the 36 programme runs among them got the commentary verdict, all 5 commentary runs did, and 2 of the 4 ask-kind runs did (open question 3). A `main-fc.txt` that is digitally silent throughout (a layout that names FC but carries nothing there) also falls back to `R = M` in audio_similarity.py;
- `X[t]`: the candidate's mono mixdown.

The same pass stream-copies the reference and every candidate into ffmpeg's `streamhash` muxer (`-c copy -f streamhash -hash md5`): one MD5 of each stream's packets, with no decoding.

`Pq(V)` below is the q-quantile `sorted(V)[floor(q * n)]`. "Pearson" is the correlation coefficient, 0 when either side has no variance. `Mc` and `Xc` are `M` and `X` clamped from below at their own `P95 - 45 dB`.

### Measures that decide or support

1. **Packet MD5 (decides: exact duplicate).** The MD5 of every packet of a stream, from `streamhash`. A candidate whose MD5 equals that of the reference or of an earlier candidate is an exact duplicate. Measured: S03E15's tracks 1 and 3 share `43ae30c17ffdd4b9b0f85ebed323aa4d` (notes.md). In the test file built here, a copy of a FLAC stream got the same MD5 and a different stream a different one. This needs no audio analysis, as the owner said.

2. **Alignment lag (supports).** Tracks of one title share a timeline, so the search is short.
   - Coarse: on 100 ms means of `Mc` and `Xc`, the lag in -10..+10 s with the highest Pearson correlation, using every second point.
   - Fine: lags within 100 ms of the coarse one at 20 ms, using every fifth point. The result, `r_global` at lag `L`, is kept only when `r_global >= 0.5`; otherwise `L = 0`.
   - Per 60 s block (3000 frames): the lag within `L +- 2 s` (2-frame steps, every fourth point) with the highest correlation, used for that block when it reaches 0.5, else `L`.

   The candidate aligned to the reference is `Xa[t] = X[t + lag(t)]`. Measured: built delays of 37 ms and 250 ms were found as 40 ms and 260 ms, built offsets of 1.5 s, 3 s and 5 s exactly, and a 0.1% drift was followed by the block lags. The lag only aligns copies of the programme; a commentary's lag is meaningless (`r_global` 0.11 for R1, so `L = 0`).

3. **Overlap (decides whether the rule may judge).** The share of the reference's frames for which `Xa` exists. Below 0.90 there is no verdict. Purpose: a candidate that covers only part of the title (a commentary that ends early, a different cut) is never confirmed. Measured: R1 cut to 85% of its length gives 0.85, so no verdict; at 95% (0.95) and at 110% (1.00) it is judged, with the commentary verdict.

4. **Reference silence (decides whether the rule may judge).** The share of overlap frames in which `M[t] <= -90 dB` (digital silence, including dither). Above 0.10 there is no verdict. Purpose: the rule assumes the reference is continuous programme sound. An isolated score or a sparse commentary that outranks the programme (spec 8.6 ranks channel count first) is silent between cues, and judged against it, the real programme would look like "speech added in the gaps". Measured: S03E16 and S03E15 references 0.031 and 0.032 (end credits and act breaks). Score-like references 0.46-0.47. A sparse commentary as reference 0.49-0.50. The threshold is set at -90 dB for now (provisional, from one episode): at -70 dB, 11.2% of S03E16's ordinary room tone counts as silence and the real commentary loses its verdict (sensitivity run `SILENT_DB=-70`).

5. **Dialogue frames, windows and gaps (support; gap time decides whether the rule may judge).**
   - A dialogue frame has `R[t] >= P95(R) - 25 dB`.
   - A 2 s window (100 frames, non-overlapping on the reference's timeline, wholly inside the overlap) is a dialogue window when at least 50 of its frames are dialogue frames.
   - A gap is a run of at least 25 consecutive non-dialogue frames (0.5 s). Its first and last 5 frames (100 ms) are dropped, and so is every frame where the reference is digitally silent (item 4).
   - `gap_s` is the number of gap frames times 0.02 s.

   Fewer than 90 s of gaps means no verdict: a dialogue-dense or speech-only reference leaves nothing to measure. Measured: 356 s in S03E16, 208 s in S03E15, 29 s for a speech-only programme.

6. **DC, dialogue carried (decides; the guard that protects programme audio).** For each 2 s window `w`, `r_w` is the largest Pearson correlation of `Mc[w..w+100)` with `Xc` over that window, shifted by the window's block lag plus -5..+5 frames (+-100 ms). DC is the share of dialogue windows with `r_w >= 0.70`.
   - Purpose: every version of the programme carries the programme's dialogue, so its level rises and falls with the reference's lines. A commentary is two people talking over the film; their voices set the level.
   - Measured: the runs with the commentary verdict 0.02-0.08. Copies, mixes, masters and companions of the programme 0.46-1.00; 69 of the 103 programme runs are at 0.70 or more.
   - Programme runs at or below 0.30: a dub in the original's slots (0.09-0.28), a score replaced at full level (0.06-0.09; 0.28 at -6 dB without a centre channel), the unrelated-programme controls and a 7-minute misalignment (0.01-0.02), a single front channel of the duplicate (0.12), and programme judged against a score-like or sparse-commentary reference (0.01-0.22). The M&E stand-in (an ask kind) scores 0.07. Every one of these fails AG or the judgeability checks instead (section 4). DC alone is not enough; the conjunction is.

7. **Gain G (supports).** The level at which the candidate carries the reference where they match. With at least `max(10, 5% of windows)` windows at `r_w >= 0.70`: `G = P75(Xa[t] - M[t])` over their frames with `M[t] >= P95(M) - 30` and `Xa[t] > -100`. Otherwise `G = P95(Xa) - P95(M)`. Purpose: "added" sound must stand above what the reference predicts, so a louder or quieter mix of the same programme adds nothing.

8. **Added sound, AG and AG_s (decide).**
   - A frame is "added" when all three hold: the candidate is active (`Xa[t] >= P95(Xa) - 20 dB`); it is at least 10 dB above the predicted level (`Xa[t] - G - M[t] >= 10`); and its 2 s window does not follow the reference (`r_w < 0.40`).
   - AG is the share of gap frames that are added. `AG_s` is the added gap frames times 0.02 s, and `added_s` all added frames times 0.02 s.
   - Purpose: a commentary or an audio description puts speech where the programme's dialogue is silent. The 10 dB excess and the mismatch condition stop compressed, louder or differently balanced copies of the programme from counting.
   - Measured: commentaries (film at least 6 dB under the voices) 0.56-0.66. Score replaced at full level 0.03; score in the dialogue gaps 0.05-0.52; programme copies at most 0.28 when judgeable.

9. **Spread over the running time (decides).** The overlap span is cut into ten equal tenths. A tenth counts when it holds at least 10 s of gaps; for each such tenth, the AG within it. The commentary pattern needs at least 6 counting tenths, with AG >= 0.40 in at least 90% of them. The audio-description pattern needs AG >= 0.05 in at least 60% of them. Purpose: a commentary talks all the way through, while a different cut, a drift or a local replacement adds sound in one place only. Measured: commentaries 10/10 (6/6 for the speech-only programme). Unrelated-programme control 8/10. Alternate narration with a centre channel 5/6.

10. **SMI, syllabic modulation of the added sound (decides, as a necessary condition only).** For each 64-frame window (1.28 s, hop 32) with at least 32 added frames, take the linear envelope `10^(Xa/20)`, remove its mean, and take the 64-point DFT power `P_k`. SMI is the median over those windows of `sum(P_3..P_10) / sum(P_1..P_32)`, the share at 2.3-7.8 Hz. The commentary pattern needs `SMI >= 0.50`. Measured: commentaries 0.56-0.58; the show's theme music added in gaps 0.24, a legato pad 0.18. But synthetic staccato piano reached 0.77 and pizzicato 0.63. **SMI does not separate speech from rhythmic music** and is never relied on to protect a programme track.

11. **LS, level spread (decides, as a necessary condition only).** `P95(Xa) - P50(Xa)` over the overlap. The commentary pattern needs `LS <= 13 dB` (dense talk). Measured: commentaries 8.8-10.1 dB. But compressed programme copies also score 4.3-12.6 dB, so LS protects nothing; it only excludes sparse speech.

12. **ADL, added during dialogue (decides the audio-description pattern only).** The share of dialogue frames that are added. Audio description talks only in the gaps (ADL 0.00-0.01). Commentaries also talk over the dialogue (0.07-0.20).

13. **Reverse test (decides, keep-only).** The same analysis with the roles swapped: the candidate as reference, its own mixdown as dialogue curve, the reference as candidate. If the swapped run has at least 90 s of gaps and AG >= 0.30, the reference also adds sound to the candidate. A commentary pattern is then not confirmed: the pair is two different programmes, misaligned, or the reference is not the programme. Measured: the real commentary has 29 s of its own gaps (swapped run not judged). Unrelated programmes and a 7-minute misalignment score swapped AG 0.37-0.52. The test has not changed a verdict in any of the 155 runs: it is a spare guard that can only keep.

### Tried and rejected

- **Stereo width (side minus mid).** Fails both ways (see the table at the top): a mono film's companion is the narrowest track, and AD or commentary over a stereo mix is wide. It is recorded as an observation only, never required, as the owner decided. The new script does not measure it.
- **Positional or slot rules** ("lossy stereo in the slot where siblings carry a duplicate"). Rejected by the owner (binding constraint 2). The only place position may enter is a keep-only veto, and only with the owner's approval (question 10).
- **Whole-title loudness correlation** (the first measurement's "same programme": 0.10 for the commentary, 1.00 for the duplicate, re-computed here on the 100 ms full-band curves). The commentary under a programme 6 dB louder scores 0.69 and a programme night mix 0.79, so the two overlap. On the 20 ms speech-band curves (`r_global`) the commentary scores 0.11, but the commentary under louder film scores 0.66-0.80. Observation only.
- **"Fills the main's quiet moments" and active share** (first measurement, re-computed here). 0.91 for the real commentary, but 0.92 for a night mix of the programme and 0.88 for the M&E stand-in. Active share: commentary 0.94, night mix 0.97, M&E 0.95. Rejected.
- **Loudness range, 4 Hz modulation of the whole track, speech-band share.** These look at the candidate alone, and each one fires on compressed or speech-only programme copies. Rejected as deciders.
- **FS_gap** (content lens): foreign speech in gaps judged by run correlation and modulation, without a gain model. In the content attack its modulation test passed rhythmic music, and its reverse guard was empty when the candidate had no gaps of its own. A legitimate track whose score differed from the main's was dropped. It is replaced here by AG (gain excess plus mismatch) together with DC. The same different-score tracks score AG 0.01-0.03 and stay `keep` here (runs ac-M0 to ac-M24).
- **Gain from the mode of the level difference** (content lens, first attempt). The content lens itself found that an M&E subset placed the mode far down and turned most of its gaps "foreign". Replaced by `G` from matched windows (M&E stand-ins AG 0.00-0.12 here).
- **Waveform measures** (relation lens): per-frame least-squares fit, level excess, deep-gap share, multi-path anchoring, residual envelope depth.
  - In the relation attack's runs (its `results.txt`, not re-computed here), the depth check let staccato, pizzicato and drum music through as "speech" (depth 23.4-49.5 dB). Compressed incoherent masters passed the gap thresholds.
  - Anchoring separated unrelated or misaligned speech (anchor 15.9-25.5) from commentaries with a bed (264-2077), but it protects no programme track: incoherent masters scored 48.3-128.1.
  - They need 8 kHz PCM, not small text files, so phase 0 cannot measure them on the mini PC. Anchoring is the only known route to telling a re-dub from a commentary (open question 3).
- **Envelope "bed follows the main"** (tried here). The correlation of `Xa` and `M` over the candidate's quietest 20% of frames. The real commentary scored 0.26 and the dense alternate narration over M&E 0.20. It does not separate them; rejected.
- **Level follows the main in the gaps** (relation attack's runs, not re-computed here). 0.78-0.99 for compressed masters and 0.21 or less for commentaries. DC already covers the same ground (masters 0.82-0.99), so it is recorded only.
- **Analysis on sampled windows.** The content attack found that scores on sampled windows move with window placement, enough to carry audio description across a drop threshold. Verdicts come from the whole title only.
- **"Confirmed audio description" by content.** Every AD stand-in is built from the commentary voice in gaps chosen by the measures' own gap definition. Laughter and music in the gaps reach the same pattern (6 programme runs did). AD is never dropped by content in this version.

## 3. Decision rule

**Scope.** The rule covers every audio stream in a wanted language that survives spec 8.6 step 1 (derived streams, flags 1, 2, 4 and 512, TheDiscDB descriptions, unwanted languages). It does not cover the top-ranked stream of its language (8.6 step 3), which is the reference. Each candidate is judged against the reference of its own language. Streams with no language are not judged; they stay as 8.6 says.

**Order.** The rows are tried in order; the first that matches decides. "Keep" always means the track goes into the final file, ranked as 8.6 step 3 says.

| # | Measured evidence | Outcome | Note or review reason |
|---|---|---|---|
| 1 | Flag 1, 2 or 4, flag 512, or an aligned TheDiscDB description (spec 8.6 step 1, unchanged) | drop (confirmed commentary) or drop (confirmed audio description) | as today |
| 2 | Packet MD5 equal to that of the reference or of a candidate ranked before it | drop this copy (exact duplicate; not a content verdict) | note `duplicate_audio_dropped`, naming the kept copy |
| 3 | Any of: (a) the analysis failed, or did not cover the whole title; (b) overlap < 0.90; (c) the reference is digitally silent (<= -90 dB) in more than 10% of frames; (d) gap time < 90 s; (e) no dialogue window; (f) only if the owner approves question 10: the reference is not also the first track of its language on the disc; (g) TheDiscDB describes the reference as score, M&E, commentary or audio description | keep (programme audio) | note `audio_content_not_judged` with the reason; also row 7's note when DC <= 0.50 and AG >= 0.30, or row 8's note when DC >= 0.80, AG >= 0.10 and AG_s >= 30 s |
| 4 | **Commentary pattern**, all of: DC <= 0.30; AG >= 0.50; at least 6 tenths with 10 s of gaps, AG >= 0.40 in at least 90% of them; SMI >= 0.50; LS <= 13 dB; added_s >= 60 s; and the reverse test is not met (row 5) | **Until experiment 13 answers yes: keep and flag for review.** After a yes, with the setting `drop_commentary_by_audio` on: **drop (confirmed commentary)** | before: review reason `commentary_detected`. After: note `commentary_dropped_by_audio`, named in the `filed` notification; the raw rip is kept for `retention_days` as the undo |
| 5 | Commentary pattern, and the reverse test has at least 90 s of gaps and AG >= 0.30 | keep and flag for review | review reason `audio_reference_unclear` |
| 6 | **Audio-description pattern**, all of: DC >= 0.90; ADL <= 0.02; AG >= 0.15; AG_s >= 45 s; at least 6 counting tenths, AG >= 0.05 in at least 60% of them; SMI >= 0.50 | keep (programme audio). Drop (confirmed audio description) is reached only through row 1 in this version | note `possible_audio_description` (question 9: review instead?) |
| 7 | DC <= 0.50 and AG >= 0.30 | keep | note `possible_commentary` |
| 8 | DC >= 0.80, AG >= 0.10 and AG_s >= 30 s | keep | note `possible_audio_description` |
| 9 | Anything else | keep (programme audio) | none |

Consequences for the spec, to be made only after experiment 13 has reported:
- **8.6 step 1** gains rows 2 and 4 (row 4's drop only after a yes).
- **8.8** gains the review reasons `commentary_detected` and `audio_reference_unclear`.
- **8.6 step 6**: the positional note (`possible_commentary` for a lossy track of two channels or fewer that is not first) is replaced by rows 7 and 8 for every track the content rule judged. It stays only for tracks the rule could not judge.
- **D12**: "a track that merely looks like a commentary is kept, with no review" becomes "is kept; with the commentary pattern it is held for review until experiment 13 answers yes".
- **Section 14** gains the setting `drop_commentary_by_audio`.

### Thresholds (every one PROVISIONAL)

Each threshold below sits between values from **one** real commentary recording (and constructions made from it) and constructed programme tracks. None was set on held-out data.

| Threshold | Value | Measured on each side | Data needed to fix it |
|---|---|---|---|
| DC, commentary | <= 0.30 | commentary verdicts 0.02-0.08; commentary under film at 0 dB 0.26-0.37, at +6 dB 0.64-0.73; programme copies and masters 0.46-1.00 | real commentaries with the film audible underneath; real lossy companions, alternate masters, dubs |
| AG, commentary | >= 0.50 | commentary verdicts 0.56-0.66; unrelated-programme control 0.52-0.53; score-reference case 0.64-0.65 (stopped by row 3); music in gaps up to 0.52 (stopped by DC 0.94) | real commentaries, especially sparse ones and feature films with score in the gaps |
| Tenths | >= 6 counting, >= 90% at AG >= 0.40 | commentary verdicts 100%; control 8/10; alternate narration with a centre channel 5/6 | feature-length commentaries with silent stretches (constructed: 1 silent stretch of 130 s gives 8/10, a note) |
| SMI | >= 0.50 | commentary verdicts 0.56-0.58; programme dialogue 0.42-0.46; staccato music 0.77, so not a guard | real isolated scores and songs in gaps |
| LS | <= 13 dB | commentary verdicts 8.8-10.1; compressed programme copies 4.3-12.6, so not a guard | real commentaries with long pauses |
| added_s | >= 60 s | commentary verdicts 364-518 s | as above |
| Gap time | >= 90 s | E16 356 s, E15 208 s, speech-only programme 29 s | real dialogue-heavy and score-heavy films |
| Overlap | >= 0.90 | R1 at 85%: no verdict; at 95%: verdict | real commentaries that stop before the end credits |
| Reference silence | <= 10% of frames at <= -90 dB | real references 3.1-3.2%; score-like references 46-47%; sparse commentary as reference 49-50% | real isolated scores and lossless commentaries that outrank the programme; films with long silent openings |
| Reverse test | gaps >= 90 s and AG >= 0.30 | never reached together with the commentary pattern in 155 runs | real pairs where the reference is not the programme |
| AD pattern | DC >= 0.90, ADL <= 0.02, AG >= 0.15, AG_s >= 45 s, 60% of tenths, SMI >= 0.50 | constructed AD 5 of 12 runs; also 6 programme runs (music or loud laughter in the gaps) | real audio-description tracks; then decide whether AD may ever be dropped by content |
| Notes | DC <= 0.50 and AG >= 0.30; DC >= 0.80, AG >= 0.10, AG_s >= 30 s | as the patterns | as the patterns |
| Measure constants | 20 ms frames, 2 s windows, follow r >= 0.70, mismatch r < 0.40, active P95 - 20 dB, dialogue P95 - 25 dB, gap >= 0.5 s trimmed 0.1 s, added >= 10 dB, gain P75, lag +-10 s / +-2 s / +-100 ms, silence -90 dB | sensitivity in section 4: one change at a time moved no programme run to the commentary verdict | the same real data |

### How the rule treats particular tracks

- **A lossy or delayed downmix.** The lag search finds the delay (built 37 ms found as 40 ms; 250 ms as 260 ms; offsets of 1.5, 3 and 5 s exactly). The window search absorbs up to 100 ms more. DC 0.76-1.00 and AG 0.00 in all 17 such runs (among them an AC-3 2.0 companion, a lossy stereo downmix with phase-shifted surrounds, and a telephone-band compressed MP2 mono copy): keep. The duplicate's front-left channel alone is also kept (DC 0.12, AG 0.00).
- **A different mix.** Rebalanced (dialogue +6 dB, centre +10 dB), compressed or squashed, night mode, waveform-incoherent remasters with or without compression, M&E raised 10 dB: DC 0.46-1.00, AG at most 0.18, keep. The squashed AAC copy has the lowest DC (0.46-0.47) and is kept because nothing in it stands 10 dB above the reference in a mismatched window.
- **A mono film** (a reference with no centre channel). The reference's own mixdown is the dialogue curve. The same path is taken for any reference whose layout the installed ffmpeg cannot resolve (section 2). None of the 36 programme runs without a centre channel got the commentary verdict. The real commentary keeps its verdict without a centre channel (R1-noFC). A mono "old master" companion of the programme: DC 0.99, keep. Risk: the one constructed false commentary verdict, a dense alternate narrator over the programme's M&E, occurs only without a centre channel (section 4, open question 3).
- **A speech-heavy programme.** Its dialogue leaves few gaps. Under 90 s of gaps means no verdict, so the track is kept (a speech-only programme had 29 s). Copies of a speech-only programme follow it (DC 0.69-1.00): keep.
- **A commentary over audible film sound.** Confirmed while the film is at least 6 dB under the voices: -6 dB, -12 dB, an incoherent film bed at -12 dB, and auto-ducking. With film and voices at the same level: note `possible_commentary`. With the film 6 dB or more above the voices: keep (it looks like the programme with additions, sometimes a `possible_audio_description` note). A missed commentary only costs space, so these are left to notes.
- **A title whose main track is itself mostly speech** (talk show, stand-up, documentary with constant narration). As above: too few gaps means no verdict, and the track is kept. When such a programme does have 90 s of gaps, a commentary over it is confirmed like any other (run c-talkcomm: DC 0.02, AG 0.65, 6/6 tenths).
- **Tracks whose length differs from the main track.** The overlap is measured after alignment. Below 90% there is no verdict (R1 cut to 85%: note only). A candidate longer than the main is judged on the main's span (R1 lengthened by 10%: same verdict). Cuts inside a track (20 s missing, 30 s inserted, 3 minutes or 2 x 90 s replaced) and drift of 0.1% to 0.3% are re-aligned by the 60 s block lags. DC 0.31-0.99, AG at most 0.28: keep.
- **A reference that is not the programme** (an isolated score or a lossless commentary that outranks the programme because 8.6 ranks channel count first). Silence in more than 10% of frames, or less than 90 s of gaps, refuses a verdict: every such run was kept, at most with a note. A continuous non-programme reference, an M&E track ranked first, gave a note at worst (DC 0.37, AG 0.42). Question 10's position veto is the backstop, and only the owner can allow it.

### Where it runs and what it costs

- **Phase 0.** One ffmpeg pass per title on the mini PC (the new audio-levels.ps1), then the Python on the Mac.
  - On this Mac, one 20 ms speech-band chain over a 21.8-minute mono stream took 2.3 s of CPU.
  - The Python takes about 3-4 s of CPU per candidate for a 21-minute episode, with the reverse test (430 s for all 127 pairs).
  - On the J4125 nothing new has been measured. The first measurement, five 100 ms chains read from the share, took 98 s and 121 s per 22-minute episode. Experiment 13 records the time of every run (`seconds=` in the log and manifest).
- **The app.**
  - The verdict is needed at 6.4 step 11 (Decide), before the remux (step 13), so it cannot ride on the remux pass. It needs one decode of the whole title for the reference and each candidate.
  - Recommendation: run it in step 10's full read, as one ffmpeg pass that also does the subtitle extraction (to be verified). Run it only when a title has an extra same-language audio track that step 1 did not decide.
  - Cost: one full read (20-40 minutes for a 50-90 GB UHD title at 25-50 MB/s, about 2-3 minutes for a Blu-ray episode), plus the decode of the lossless reference on the J4125 (TrueHD or DTS-HD MA; not measured).
  - The arithmetic is small: percentiles, Pearson correlations over 100-frame windows and 64-point DFTs over about 450,000 frames for a 2.5-hour film. That is well under a second per candidate in Rust (estimated).
  - If the full read is too slow, the answer is no verdict (keep, note), never a verdict from sampled windows.

## 4. Evidence so far

**What exists.** Two real tracks from one disc of one sitcom:
- R1, the S03E16 commentary. The owner listened to it.
- R2, S03E15's exact duplicate. Same MD5.

Everything else is constructed from the 16 kHz mono extracts of those two episodes. Every number below was computed by the synthesis, `synth.py`, on 20 ms speech-band curves made with the chain of section 2 and rounded to 0.1 dB. Two checks tie it to the proposed `audio_similarity.py`:
- It gives identical numbers on R1, R2, R3, c-talkcomm, aq-A2n-noFC, aq-D7, ac-laugh4l, c-ad16 and ar-A3a (`verify_repo.py`, `verify-final.txt`).
- An end-to-end run of the proposed ffmpeg command on a small Matroska test file worked: video, a 5.1 FLAC reference, an AC-3 2.0 candidate and a FLAC copy. The copy came out as a duplicate by MD5.
- After the review fixes (folder check, layout table), the final `audio_similarity.py` was run again on R1, R1-noFC, R1-L, R2, R2-FL, R3 and R3b, with folders written as the final audio-levels.ps1 writes them (its chain, header lines and manifest, emulated with the Mac's ffmpeg 8.1.2 on the extracts: `exp13-work/fix/emulate_ps.py`, results in `exp13-work/fix/ev/results.txt`): every number in the table below is unchanged, and R2 with its real packet MD5 comes out `duplicate of stream 1`.

Columns:
- `DC`, `AG`, `SMI`, `LS` are as in section 2.
- `Tenths` is the number of tenths with AG >= 0.40 over the number of tenths that hold 10 s of gaps.
- `Gaps` is the dialogue-gap time in seconds.
- Verdicts:
  - **commentary**: row 4 of the rule, review now, drop after experiment 13;
  - `ad-pattern (note)`: row 6;
  - `note-commentary`: row 7;
  - `note-ad`: row 8;
  - `keep`: row 9;
  - "not judged": row 3's reasons.
- "Wanted" is what the owner wants for that kind of track (section 1).

**How each group was made, and how far it stands in for the real thing.**

- **Real (R).** R1 and R2 are the owner's tracks, mixed down by the controller to 16 kHz mono (R1-L is R1's left channel; R2-FL the duplicate's front left). R3 and R3b put one episode's main track against the other's. That is not a disc case: it stands in for unrelated speech, such as an unflagged interview track, and is the closest thing to a voice-only commentary.
- **Requirements proposer (q-).** Recipes in `exp13-work/requirements/cases/cases.json`.
  - The lossy companions went through real AC-3 encodes. They are faithful for codec delay and loss, but every one is a copy of the same mix.
  - Commentary over programme is the one real commentary with the episode mixed under it at set LUFS levels. These are the same voices in the same gaps.
  - Audio description is stretches of the commentary inserted into the main's centre-channel gaps of 1.2 or 2 s or more. That is partly circular (the gaps come from the measure's own definition) and the voice is conversational, not a describer.
  - The M&E stand-in is the mix minus 0.303 of the centre channel, mostly room tone.
  - The dub is the M&E plus the other episode's dialogue gated to this episode's speaking times.
- **Content proposer (c-).** Recipes in `exp13-work/content/build.py`: an alternate-balance AC-3 mix, a night mix with hiss, a band-limited MP2 "old master", an alternate dub, a shifted copy, commentary over programme at set levels, audio description at several densities, and a speech-only "programme" (the commentary recording itself) with a copy and with a commentary laid over it. The same limits apply.
- **Relation proposer (r-).** Recipes in `exp13-work/relation/build_cases.py`. These are 8 kHz, 100-3800 Hz versions after real AC-3 or AAC encodes:
  - a lossy downmix with phase-shifted surrounds;
  - DRC + EQ;
  - an M&E;
  - "incoherent remasters" (all-pass filters and echoes, with and without compression);
  - dubs 0.4 s and 0.8 s late;
  - the show's theme in the gaps;
  - commentary at set levels relative to the film;
  - audio description.

  The remasters are synthetic stand-ins for real alternate masters.
- **Content attack (ac-).** Recipes in `exp13-work/attack-content/build.py` and `build2.py`.
  - The music is real but single: the show's own theme, 18 s, looped, and slowed 0.87x for "the other score".
  - Laughter is synthetic: pulse trains on a buzz-and-noise carrier.
  - Cuts, drift and offsets are made with real codecs.
  - The "score-heavy film" puts the theme in every channel, including the centre.
- **Relation attack (ar-).** Recipes in `exp13-work/attack-relation/attack_cases.py`.
  - Synthetic staccato, pizzicato, drum and pad loops in the gaps.
  - Synthetic crowd laughter.
  - Compressed incoherent masters, also of a programme that has a drum or staccato score.
  - Sparse commentaries (3 minutes on and 3 off; 2 on and 4 off).
  - Audio description from the other episode's dialogue phrases in half the gaps.
  - The commentary over an incoherent film bed.
  - The programme rotated by 7 minutes.
- **Requirements attack (aq-).** Recipes in `exp13-work/attack-requirements/build.sh`, `build2.sh` and `build3.sh`.
  - "Score-like" references: the M&E stand-in in cue blocks with digital silence between them.
  - Squashed programmes (dynaudnorm, 6:1, limiter, AC-3).
  - Alternate dialogue or a single dense narrator (the commentary voice) over the M&E.
  - Documentaries made of the commentary voice over the M&E.
  - A sparse commentary as reference.
  - The real commentary with 1 or 3 silent stretches of 130 s, and auto-ducked.

**None of this is a real disc track except R1 and R2.** All speech is from one sitcom and one two-person commentary. All music is one 18 s theme or synthetic. There is no real audio description, isolated score, M&E, alternate master, dub, film commentary or film soundtrack.

**Summary of 155 runs (127 pairs plus 28 variants).**

| Kind of run | Runs | commentary | ad-pattern | note-commentary | note-ad | keep (incl. not judged) |
|---|---|---|---|---|---|---|
| Programme tracks (wanted: keep) | 103 | **0** | 6 | 11 | 6 | 80 |
| Ask-the-owner kinds | 12 | 2 | 0 | 4 | 0 | 6 |
| Commentaries (wanted: drop) | 28 | 13 | 0 | 9 | 1 | 5 |
| Audio descriptions (wanted: drop) | 12 | 0 | 5 | 0 | 3 | 4 |

- No programme run got the commentary verdict.
- The two ask-kind runs that did are both a single dense narrator voice (the commentary recording) over the programme's M&E, judged without a centre channel (aq-A2n-noFC, aq-A2n640-noFC). With the centre channel, the same construction gave 5/6 tenths and only a note, and the attack showed that this margin depends on one tenth of that episode. It is the one known way to fool the rule (open question 3).
- The 13 commentary verdicts are all built from the one real recording:
  - R1 in five forms (mix, left channel, no centre, 95% and 110% length);
  - the film 6 dB under the voices (with and without a centre channel) or 12 dB under them;
  - an auto-ducked film (with and without a centre channel);
  - an incoherent film bed (with and without a centre channel);
  - a speech-only programme.

  Their margins (DC at most 0.08, AG at least 0.56, all tenths, SMI at least 0.56, LS at most 10.1 dB) are one recording's margins.
- Commentaries that pause for minutes, cover only some scenes, or sit under film as loud as the voices get notes or are kept. That is the cheap error.

**Sensitivity.** `sweep.log` re-ran all 127 pairs with one parameter changed at a time:
- match r 0.6 and 0.8; mismatch r 0.3 and 0.5;
- candidate activity 15 and 25 dB; dialogue threshold 20 and 30 dB;
- excess 7 and 13 dB; silence -70 and -80 dB; gain percentile 0.5 and 0.9;
- each decision threshold one step either way.

No programme run moved to the commentary verdict in any variant. Results:
- R1 kept its verdict in every variant except silence at -70 dB. There, 11.2% of S03E16's quiet room tone counts as "silent", and row 3 refuses a verdict. That is why the provisional silence threshold is -90 dB.
- The ask-kind alternate-narration runs move in and out of the commentary verdict with the dialogue threshold and the tenths share.
- Commentaries made from R1 move between the verdict and a note.

### All 127 pairs

Prefixes: R real; q- requirements proposer; c- content proposer; r- relation proposer; ac- content attack; ar- relation attack; aq- requirements attack. "-noFC" means the reference's mixdown served as the dialogue curve (a mono or 2.0 film).

| Run | Wanted | Made from (stands in for) | DC | AG | Tenths | SMI | LS (dB) | Gaps (s) | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| R1 | drop | S03E16 track 3: commentary (owner listened) | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 356 | **commentary** |
| R1-noFC | drop | R1 with the main's mix as dialogue reference | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 330 | **commentary** |
| R1-L | drop | R1, left channel only | 0.03 | 0.66 | 10/10 | 0.57 | 10.1 | 356 | **commentary** |
| R2 | keep | S03E15 track 3: exact duplicate (MD5) | 1.00 | 0.00 | 0/6 | n/a | 15.8 | 208 | keep |
| R2-FL | keep | R2, front-left channel only | 0.12 | 0.00 | 0/6 | n/a | 24.2 | 208 | keep |
| R3 | keep | control: E15 main as candidate against E16 main (unrelated) | 0.02 | 0.53 | 8/10 | 0.42 | 15.8 | 357 | note-commentary |
| R3b | keep | control: E16 main as candidate against E15 main (unrelated) | 0.02 | 0.31 | 2/6 | 0.46 | 22.9 | 208 | note-commentary |
| q-c1 | keep | AC-3 2.0 compressed companion +40 ms | 0.92 | 0.00 | 0/6 | n/a | 10.9 | 208 | keep |
| q-c2 | keep | remix, dialogue +6 dB, AC-3 448k | 0.96 | 0.00 | 0/6 | n/a | 17.1 | 208 | keep |
| q-c11 | keep | night-mode compressed AC-3 | 0.92 | 0.00 | 0/6 | n/a | 12.6 | 208 | keep |
| q-c3m6 | drop | commentary, programme 6 dB under it | 0.08 | 0.56 | 10/10 | 0.56 | 8.8 | 356 | **commentary** |
| q-c3p0 | drop | commentary, programme at the voices' level | 0.26 | 0.45 | 6/10 | 0.56 | 9.2 | 356 | note-commentary |
| q-c3p6 | drop | commentary, programme 6 dB above it | 0.64 | 0.28 | 1/10 | 0.56 | 11.9 | 356 | keep |
| q-c9 | drop | partial commentary, 25% of runtime | 0.71 | 0.07 | 1/6 | 0.58 | 14.2 | 208 | keep |
| q-c4d | drop | AD in 2 s gaps, ducked | 0.99 | 0.11 | 0/6 | 0.58 | 15.1 | 208 | keep |
| q-c4p | drop | AD in 2 s gaps, not ducked | 0.99 | 0.11 | 0/6 | 0.58 | 14.9 | 208 | keep |
| q-c4x | drop | AD in 1.2 s gaps | 0.98 | 0.26 | 1/6 | 0.57 | 14.3 | 208 | ad-pattern (note) |
| q-c10 | keep | theme music added in gaps | 0.97 | 0.37 | 5/6 | 0.24 | 12.7 | 208 | note-ad |
| q-c5 | ask | M&E approximation | 0.46 | 0.00 | 0/6 | n/a | 21.6 | 208 | keep |
| q-c6 | keep | dub at the original's timing | 0.09 | 0.13 | 0/6 | 0.50 | 18.0 | 208 | keep |
| q-c7 | keep | 1 s drift, AC-3 | 0.47 | 0.00 | 0/6 | n/a | 13.1 | 208 | keep |
| q-c8 | keep | 3 s offset copy | 1.00 | 0.00 | 0/6 | n/a | 15.9 | 208 | keep |
| c-altmix | keep | AC-3 alt balance mix +37 ms | 1.00 | 0.00 | 0/10 | n/a | 24.0 | 356 | keep |
| c-night | keep | AAC night mix with hiss | 0.93 | 0.00 | 0/10 | n/a | 11.5 | 356 | keep |
| c-mono | keep | band-limited MP2 mono 'old master' | 0.99 | 0.00 | 0/10 | n/a | 20.3 | 356 | keep |
| c-dub | keep | alt dub in the same slots | 0.28 | 0.01 | 0/10 | 0.63 | 20.8 | 356 | keep |
| c-me | ask | M&E approximation | 0.74 | 0.12 | 0/10 | 0.37 | 17.8 | 356 | keep |
| c-shift | keep | copy shifted 1.5 s | 1.00 | 0.00 | 0/10 | n/a | 23.2 | 356 | keep |
| c-cm12 | drop | commentary, programme 12 dB under it | 0.05 | 0.63 | 10/10 | 0.58 | 9.2 | 356 | **commentary** |
| c-cp0 | drop | commentary, programme at the voices' level | 0.37 | 0.40 | 4/10 | 0.56 | 9.8 | 356 | note-commentary |
| c-cp6 | drop | commentary, programme 6 dB above it | 0.73 | 0.24 | 0/10 | 0.56 | 13.2 | 356 | keep |
| c-ad16 | drop | AD 22% narration | 0.95 | 0.39 | 3/10 | 0.53 | 17.2 | 356 | ad-pattern (note) |
| c-ad15 | drop | AD 14% narration | 0.95 | 0.28 | 2/6 | 0.57 | 14.4 | 208 | ad-pattern (note) |
| c-ad15s | drop | AD sparse | 1.00 | 0.00 | 0/6 | 0.64 | 15.7 | 208 | keep |
| c-talkcopy | keep | speech-only programme, AAC copy | 1.00 | 0.00 | 0/0 | n/a | 10.4 | 29 | keep (not judged: gaps 29 s) |
| c-talkcomm | drop | commentary over a speech-only programme | 0.02 | 0.65 | 6/6 | 0.58 | 9.0 | 208 | **commentary** |
| r-K1 | keep | lossy stereo downmix, phase-shifted surrounds, 28.75 ms | 1.00 | 0.00 | 0/6 | n/a | 16.6 | 209 | keep |
| r-K2 | keep | DRC + EQ AAC | 0.99 | 0.00 | 0/10 | n/a | 18.0 | 359 | keep |
| r-K3 | ask | M&E | 0.07 | 0.00 | 0/10 | n/a | 20.9 | 359 | keep |
| r-K4 | keep | incoherent remaster (allpass+echo) | 0.99 | 0.00 | 0/10 | n/a | 22.4 | 359 | keep |
| r-K5 | keep | replaced in places | 0.82 | 0.02 | 0/10 | 0.36 | 22.0 | 359 | keep |
| r-K6a | keep | dub 0.4 s late | 0.98 | 0.00 | 0/10 | n/a | 22.1 | 359 | keep |
| r-K6b | keep | dub 0.8 s late | 0.96 | 0.00 | 0/10 | n/a | 22.0 | 359 | keep |
| r-K7 | keep | incoherent + compressed | 0.98 | 0.00 | 0/10 | n/a | 15.3 | 359 | keep |
| r-K8 | keep | theme music in gaps | 0.95 | 0.50 | 8/10 | 0.39 | 14.0 | 359 | note-ad |
| r-D1 | drop | commentary 12 dB under the film | 0.91 | 0.14 | 0/10 | 0.54 | 15.9 | 359 | note-ad |
| r-D3 | drop | commentary and film at the same active level | 0.26 | 0.44 | 7/10 | 0.57 | 9.2 | 359 | note-commentary |
| r-D4 | drop | AD | 0.95 | 0.23 | 0/10 | 0.43 | 18.4 | 359 | note-ad |
| r-D5 | drop | AD quiet, no ducking | 0.99 | 0.14 | 0/10 | 0.44 | 19.3 | 359 | note-ad |
| r-D6 | drop | AD on lossy downmix | 0.96 | 0.16 | 0/6 | 0.45 | 15.3 | 209 | note-ad |
| ac-tele | keep | telephone-band squashed MP2 mono +250 ms | 0.77 | 0.00 | 0/10 | n/a | 4.3 | 356 | keep |
| ac-c10 | keep | centre +10 dB AC-3 | 1.00 | 0.00 | 0/10 | n/a | 23.9 | 356 | keep |
| ac-squash | keep | dynamics squashed 36 dB AAC | 0.47 | 0.17 | 0/10 | 0.35 | 5.7 | 356 | keep |
| ac-talktele | keep | talk title, telephone copy | 0.69 | 0.00 | 0/0 | n/a | 2.7 | 29 | keep (not judged: gaps 29 s) |
| ac-cutmiss | keep | cut: 20 s missing | 0.62 | 0.14 | 0/10 | 0.38 | 23.3 | 356 | keep |
| ac-cutins | keep | cut: 30 s inserted | 0.31 | 0.28 | 1/10 | 0.43 | 22.5 | 356 | keep |
| ac-cutrep | keep | cut: 3 min replaced | 0.88 | 0.07 | 1/10 | 0.39 | 21.9 | 356 | keep |
| ac-drift1 | keep | drift 0.1% | 0.99 | 0.00 | 0/10 | n/a | 23.2 | 356 | keep |
| ac-drift3 | keep | drift 0.3% | 0.61 | 0.10 | 0/10 | 0.44 | 23.3 | 355 | keep |
| ac-shift5 | keep | offset 5 s | 1.00 | 0.00 | 0/10 | n/a | 23.2 | 356 | keep |
| ac-M12 | keep | different score in L/R, -12 dB | 0.80 | 0.02 | 0/10 | 0.51 | 12.3 | 395 | keep |
| ac-M18 | keep | different score, -18 dB | 0.96 | 0.01 | 0/10 | 0.35 | 16.8 | 395 | keep |
| ac-M24 | keep | different score, -24 dB | 0.99 | 0.00 | 0/10 | n/a | 20.6 | 395 | keep |
| ac-M6 | keep | different score, -6 dB | 0.43 | 0.03 | 0/10 | 0.57 | 7.9 | 395 | keep |
| ac-M0 | keep | different score, 0 dB | 0.09 | 0.03 | 0/10 | 0.57 | 5.5 | 395 | keep |
| ac-Mremix | keep | same score +6 dB (control) | 1.00 | 0.00 | 0/10 | n/a | 7.8 | 395 | keep |
| ac-Q | keep | different score in 60 s cues | 0.90 | 0.01 | 0/10 | 0.46 | 14.3 | 360 | keep |
| ac-B | keep | same score, different balance + ambience | 0.82 | 0.00 | 0/10 | n/a | 7.8 | 395 | keep |
| ac-subset | keep | M&E ranked first, programme as candidate | 0.62 | 0.00 | 0/1 | n/a | 23.2 | 37 | keep (not judged: gaps 37 s) |
| ac-laugh1 | keep | laugh track N=1 | 1.00 | 0.03 | 0/10 | n/a | 21.8 | 356 | keep |
| ac-laugh2 | keep | laugh track N=2 | 1.00 | 0.07 | 0/10 | 0.75 | 21.2 | 356 | keep |
| ac-laugh4 | keep | laugh track N=4 | 0.99 | 0.10 | 0/10 | 0.55 | 20.7 | 356 | keep |
| ac-laugh4l | keep | laugh track N=4 at dialogue level | 0.98 | 0.19 | 0/10 | 0.53 | 19.7 | 356 | ad-pattern (note) |
| ac-laugh30 | keep | laugh track N=30 | 0.99 | 0.14 | 0/10 | 0.29 | 20.1 | 356 | note-ad |
| ac-laughrev | keep | laugh version ranked first | 0.84 | 0.00 | 0/10 | n/a | 23.2 | 198 | keep |
| ac-sp50 | drop | commentary 60 s on / 60 s off | 0.48 | 0.34 | 3/10 | 0.57 | 17.7 | 356 | note-commentary |
| ac-sp33 | drop | commentary 40 s on / 80 s off | 0.66 | 0.25 | 0/10 | 0.56 | 20.2 | 356 | keep |
| ac-scored | drop | commentary over score-heavy film | 0.02 | 0.35 | 3/5 | 0.58 | 10.0 | 118 | note-commentary |
| ac-ADdense | drop | dense AD-shaped superset | 0.95 | 0.24 | 1/10 | 0.54 | 18.6 | 356 | ad-pattern (note) |
| ac-ADg1 | drop | AD in gaps >= 1 s | 0.96 | 0.23 | 1/10 | 0.54 | 18.9 | 356 | ad-pattern (note) |
| ar-A1a | keep | incoherent master, 3:1 compression | 0.98 | 0.00 | 0/10 | n/a | 11.6 | 359 | keep |
| ar-A1b | keep | incoherent master, 6:1 compression | 0.94 | 0.00 | 0/10 | n/a | 8.3 | 359 | keep |
| ar-A1c | keep | incoherent master, dynaudnorm | 0.99 | 0.00 | 0/10 | n/a | 18.4 | 359 | keep |
| ar-A1a-coh | keep | coherent, 3:1 compression | 1.00 | 0.00 | 0/10 | n/a | 12.3 | 359 | keep |
| ar-A2b | keep | incoherent, M&E +10 dB | 0.98 | 0.00 | 0/10 | n/a | 20.7 | 359 | keep |
| ar-A3a | keep | staccato piano in gaps | 0.94 | 0.39 | 4/10 | 0.77 | 18.0 | 359 | ad-pattern (note) |
| ar-A3b | keep | drums in gaps | 0.99 | 0.05 | 0/10 | n/a | 21.1 | 359 | keep |
| ar-A3c | keep | pizzicato in gaps | 0.94 | 0.42 | 6/10 | 0.63 | 17.6 | 359 | ad-pattern (note) |
| ar-A3d | keep | legato pad in gaps | 0.94 | 0.52 | 8/10 | 0.18 | 13.8 | 359 | note-ad |
| ar-A4a | keep | laughs crowd 25 | 0.96 | 0.26 | 1/10 | 0.26 | 18.5 | 359 | note-ad |
| ar-A4b | keep | laughs crowd 6 | 0.96 | 0.25 | 1/10 | 0.44 | 19.2 | 359 | note-ad |
| ar-M1 | drop | sparse commentary 3 min on/off | 0.50 | 0.37 | 5/10 | 0.56 | 15.8 | 359 | note-commentary |
| ar-M1b | drop | sparse commentary 2 on/4 off | 0.67 | 0.26 | 2/10 | 0.57 | 19.9 | 359 | keep |
| ar-M2 | drop | AD in half the gaps, short phrases | 0.99 | 0.10 | 0/10 | 0.33 | 21.4 | 359 | keep |
| ar-M3 | drop | commentary, remastered (incoherent) film 12 dB under it | 0.04 | 0.63 | 10/10 | 0.58 | 9.7 | 359 | **commentary** |
| ar-N1 | keep | same programme rotated 7 min | 0.01 | 0.38 | 4/10 | 0.44 | 23.3 | 359 | note-commentary |
| ar-P1a | keep | programme with staccato score; incoherent compressed master | 0.98 | 0.00 | 0/10 | n/a | 8.6 | 374 | keep |
| ar-P1b | keep | programme with drum score; incoherent compressed master | 0.98 | 0.00 | 0/10 | n/a | 10.7 | 370 | keep |
| ar-P1d | keep | drum score, heavy compression | 0.93 | 0.00 | 0/10 | n/a | 8.0 | 370 | keep |
| ar-P1e | keep | staccato score, heavy compression | 0.94 | 0.00 | 0/10 | n/a | 6.4 | 374 | keep |
| aq-A1 | keep | score-like track ranked first, programme as candidate | 0.22 | 0.00 | 0/5 | 0.44 | 15.8 | 165 | keep (not judged: main silent 0.47) |
| aq-A1s | keep | score first, squashed programme | 0.08 | 0.00 | 0/5 | 0.40 | 5.8 | 165 | keep (not judged: main silent 0.47) |
| aq-A1me | keep | M&E first, squashed programme | 0.14 | 0.01 | 0/9 | 0.23 | 5.8 | 309 | keep |
| aq-A2 | ask | alt dialogue at its own timing over M&E | 0.06 | 0.30 | 2/6 | 0.47 | 17.5 | 208 | keep |
| aq-A2s | ask | alt dialogue own timing, squashed | 0.06 | 0.44 | 4/6 | 0.41 | 5.7 | 208 | note-commentary |
| aq-A2n | ask | alt narration (one dense voice) over M&E | 0.00 | 0.56 | 5/6 | 0.59 | 9.1 | 208 | note-commentary |
| aq-A2n-noFC | ask | alt narration, main without centre | 0.01 | 0.65 | 6/6 | 0.59 | 9.1 | 154 | **commentary** |
| aq-A2n640-noFC | ask | alt narration offset 640 s, no centre | 0.01 | 0.67 | 6/6 | 0.57 | 9.1 | 154 | **commentary** |
| aq-A2n300 | ask | alt narration offset 300 s | 0.01 | 0.57 | 5/6 | 0.59 | 9.0 | 208 | note-commentary |
| aq-A3 | keep | different cut 2x90 s, AC-3 | 0.82 | 0.01 | 0/6 | 0.35 | 12.5 | 208 | keep |
| aq-A5 | keep | drift 0.3%, AC-3 | 0.85 | 0.04 | 0/6 | 0.34 | 10.9 | 207 | keep |
| aq-A6 | keep | squashed companion | 0.75 | 0.01 | 0/6 | 0.32 | 5.8 | 208 | keep |
| aq-A6-noFC | keep | squashed companion, no centre | 0.72 | 0.01 | 0/6 | 0.32 | 5.8 | 154 | keep |
| aq-B1 | drop | commentary, film auto-ducked | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 356 | **commentary** |
| aq-B2 | drop | commentary silent 3x130 s | 0.28 | 0.38 | 5/10 | 0.56 | 13.9 | 356 | note-commentary |
| aq-B3 | drop | commentary silent 1x130 s | 0.14 | 0.59 | 8/10 | 0.57 | 11.0 | 356 | note-commentary |
| aq-D1 | ask | documentary, alternate narrator | 0.00 | 0.13 | 0/10 | 0.47 | 14.6 | 479 | keep |
| aq-D2 | ask | documentary, alternate narrator, no centre | 0.00 | 0.16 | 1/9 | 0.47 | 14.6 | 239 | keep |
| aq-D3 | keep | documentary vs score ranked first | 0.34 | 0.41 | 4/5 | 0.54 | 15.8 | 165 | note-commentary (not judged: main silent 0.47) |
| aq-D3a | keep | documentary AC-3 vs score ranked first | 0.34 | 0.42 | 4/5 | 0.54 | 15.8 | 165 | note-commentary (not judged: main silent 0.47) |
| aq-D4 | keep | documentary vs M&E ranked first | 0.37 | 0.42 | 6/9 | 0.56 | 15.8 | 309 | note-commentary |
| aq-D5 | keep | programme vs sparse commentary ranked first | 0.01 | 0.09 | 0/0 | 0.44 | 23.2 | 15 | keep (not judged: main silent 0.50, gaps 15 s) |
| aq-D5s | keep | squashed programme vs sparse commentary first | 0.01 | 0.44 | 0/0 | 0.36 | 8.7 | 15 | note-commentary (not judged: main silent 0.50, gaps 15 s) |
| aq-D5d | keep | documentary vs sparse commentary first | 0.62 | 0.09 | 0/0 | 0.53 | 15.8 | 15 | keep (not judged: main silent 0.49, gaps 15 s) |
| aq-D7 | keep | documentary (bed +8) vs its score first | 0.08 | 0.65 | 5/5 | 0.56 | 9.2 | 166 | note-commentary (not judged: main silent 0.46) |
| aq-D7a | keep | documentary AC-3 vs its score first | 0.18 | 0.64 | 5/5 | 0.56 | 9.2 | 166 | note-commentary (not judged: main silent 0.46) |
| aq-D8 | keep | squashed remix vs score first | 0.08 | 0.01 | 0/5 | 0.40 | 5.5 | 165 | keep (not judged: main silent 0.47) |

### 28 variants

The hardest programme runs again with no centre channel, and the real tracks at other lengths.

| Run | Wanted | Made from (stands in for) | DC | AG | Tenths | SMI | LS (dB) | Gaps (s) | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| q-c1-noFC | keep | AC-3 2.0 compressed companion +40 ms (main without centre) | 0.91 | 0.00 | 0/6 | n/a | 10.9 | 154 | keep |
| q-c11-noFC | keep | night-mode compressed AC-3 (main without centre) | 0.92 | 0.00 | 0/6 | n/a | 12.6 | 154 | keep |
| c-night-noFC | keep | AAC night mix with hiss (main without centre) | 0.94 | 0.00 | 0/10 | n/a | 11.5 | 330 | keep |
| c-mono-noFC | keep | band-limited MP2 mono 'old master' (main without centre) | 0.99 | 0.00 | 0/10 | n/a | 20.3 | 330 | keep |
| c-dub-noFC | keep | alt dub in the same slots (main without centre) | 0.27 | 0.00 | 0/10 | 0.63 | 20.8 | 330 | keep |
| q-c6-noFC | keep | dub at the original's timing (main without centre) | 0.12 | 0.15 | 0/6 | 0.50 | 18.0 | 154 | keep |
| ar-A1b-noFC | keep | incoherent master, 6:1 compression (main without centre) | 0.93 | 0.00 | 0/10 | n/a | 8.3 | 335 | keep |
| ar-A1c-noFC | keep | incoherent master, dynaudnorm (main without centre) | 0.98 | 0.00 | 0/10 | n/a | 18.4 | 335 | keep |
| ar-P1b-noFC | keep | programme with drum score; incoherent compressed master (main without centre) | 0.98 | 0.00 | 0/8 | n/a | 10.7 | 118 | keep |
| ar-A3a-noFC | keep | staccato piano in gaps (main without centre) | 0.93 | 0.40 | 5/10 | 0.77 | 18.0 | 335 | ad-pattern (note) |
| ar-A3c-noFC | keep | pizzicato in gaps (main without centre) | 0.92 | 0.43 | 7/10 | 0.63 | 17.6 | 335 | ad-pattern (note) |
| ac-laugh4l-noFC | keep | laugh track N=4 at dialogue level (main without centre) | 0.97 | 0.21 | 0/10 | 0.53 | 19.7 | 330 | ad-pattern (note) |
| ac-M12-noFC | keep | different score in L/R, -12 dB (main without centre) | 0.55 | 0.59 | 0/0 | 0.51 | 12.3 | 7 | keep (not judged: gaps 7 s) |
| ac-M6-noFC | keep | different score, -6 dB (main without centre) | 0.28 | 0.50 | 0/0 | 0.57 | 7.9 | 1 | note-commentary (not judged: gaps 1 s) |
| ac-M0-noFC | keep | different score, 0 dB (main without centre) | 0.06 | n/a | 0/0 | 0.57 | 5.5 | 0 | keep (not judged: gaps 0 s) |
| ac-squash-noFC | keep | dynamics squashed 36 dB AAC (main without centre) | 0.46 | 0.18 | 1/10 | 0.35 | 5.7 | 330 | keep |
| ac-tele-noFC | keep | telephone-band squashed MP2 mono +250 ms (main without centre) | 0.76 | 0.00 | 0/10 | n/a | 4.3 | 330 | keep |
| q-c3m6-noFC | drop | q-c3m6, main without centre | 0.08 | 0.57 | 10/10 | 0.56 | 8.8 | 330 | **commentary** |
| aq-B1-noFC | drop | commentary, film auto-ducked (main without centre) | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 330 | **commentary** |
| ar-M3-noFC | drop | commentary over incoherent bed -12 dB (main without centre) | 0.03 | 0.64 | 10/10 | 0.58 | 9.7 | 335 | **commentary** |
| aq-A2s-noFC | ask | alt dialogue own timing, squashed (main without centre) | 0.06 | 0.56 | 5/6 | 0.41 | 5.7 | 154 | note-commentary |
| r-K1-noFC | keep | lossy stereo downmix, phase-shifted surrounds, 28.75 ms (main without centre) | 1.00 | 0.00 | 0/6 | n/a | 16.6 | 157 | keep |
| R3-noFC | keep | control: E15 main as candidate against E16 main (unrelated) (main without centre) | 0.02 | 0.52 | 8/10 | 0.42 | 15.8 | 332 | note-commentary |
| R1-cut85 | drop | R1 cut to 85% of the main's length | 0.02 | 0.65 | 10/10 | 0.59 | 10.0 | 282 | note-commentary (not judged: overlap 0.85) |
| R1-cut95 | drop | R1 cut to 95% | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 354 | **commentary** |
| R1-long110 | drop | R1 lengthened to 110% | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 356 | **commentary** |
| q-c1-cut85 | keep | q-c1 cut to 85% | 0.93 | 0.00 | 0/8 | n/a | 10.8 | 180 | keep (not judged: overlap 0.85) |
| R2-cut85 | keep | R2 cut to 85% | 1.00 | 0.00 | 0/8 | n/a | 15.5 | 180 | keep (not judged: overlap 0.85) |

## 5. Experiment 13

Row for the table in spec section 17:

| # | Question | If the answer is no |
|---|---|---|
| 13 | Measured over whole titles on the owner's own discs, and compared with the owner's label for every extra same-language audio track, does the content rule of 8.6 give its commentary verdict (row 4: DC <= 0.30; AG >= 0.50 over all the dialogue gaps; at least 6 of the ten equal tenths of the overlap span each holding at least 10 s of dialogue gaps, with AG >= 0.40 in at least 90% of those tenths; SMI >= 0.50; LS <= 13 dB; added sound >= 60 s; the judgeability checks of row 3 passed and the reverse test of row 5 not met) to **no** programme track, and to at least one commentary on each of at least 3 different discs? Before the answer may be yes, the labelled tracks must include: at least 8 commentaries from at least 4 discs, at least 2 of them on feature films and at least 1 with the film clearly audible under the voices; at least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least 20 programme tracks other than the reference, at least 6 of them separately encoded lossy companions or alternate mixes (not MakeMKV-derived cores, not exact duplicates); at least 1 title whose reference has no centre channel (mono or 2.0) and at least 1 dialogue-heavy or speech-only title, each with an extra track; and every isolated score, M&E track, second same-language dub or narration the owner has. A track counts only when the owner names it from listening, the packaging, the menu, a flag or TheDiscDB, before seeing the verdict. Every threshold was fitted on two tracks of the Office disc (`the-office-2005-s3d3-bluray`: the S03E16 commentary, title 0, and the S03E15 duplicate, title 1), so no track of that disc counts towards any of the totals above, and that disc is not one of the 4 discs or of the 3 discs with a commentary verdict; its tracks are still measured and reported, and a programme track of that disc with the commentary verdict still makes the answer no. | Content never drops a track: a track with the commentary pattern stays in the file and is flagged for review (`commentary_detected`); every other look-alike gets a note (D12). If the owner's discs cannot supply the tracks listed, the answer is "not answered": the report gives the count per kind against the list and names the missing kinds, and the same fallback applies. A single programme track with the commentary verdict makes the answer no, whatever else was measured. |

What a yes does and does not show. Zero false verdicts in 20 programme tracks bounds the false-verdict rate only below about 15% per track (95%, rule of three). So even a yes enables dropping only behind the setting `drop_commentary_by_audio`, with the dropped track named in the `filed` notification and the raw rip kept for `retention_days`, as the undo. The thresholds are not re-tuned on the experiment's own tracks: a change made after seeing them needs a fresh set of discs to be judged on.

## 6. Changes to the measurement scripts

Three files change. Complete new contents follow.

**`scripts/phase0/audio-levels.ps1`** (PowerShell 5.1, ASCII only):
- `-Cand` now takes a list (`-Cand 2,3`). One ffmpeg pass per file writes the 20 ms speech-band curves and the packet MD5s.
- The raw `ametadata` output, about 80 bytes per frame (5.3 MB for a 22-minute stream, about 35 MB for 2.5 hours), is converted to one value per line (about 0.4 MB and 3 MB) and then deleted. Each level file starts with a header line that states `frame_ms=20`, and the folder gets a `manifest.txt` whose first line is `# lazymkv audio-levels v2 (experiment 13)` and which states `frame_ms=20` and the stream numbers; audio_similarity.py reads only folders that have both.
- Width (mid and side) is no longer measured.
- The script keeps the plan's conventions:
  - `Assert-Slug` on the slug, the label and the folder name;
  - no global `ErrorActionPreference`, and `-ErrorAction Stop` on cmdlets;
  - `RESULT ok` or `RESULT FAILED: <reason>`, and a throw on failure;
  - it refuses to replace an existing folder without `-Force`, and writes nothing to the share.

  It checks every stream number against ffprobe's list before anything is decoded. It says which of three things is wrong when the rip cannot be read: `cannot reach the staging share`, `expected one .mkv in ... found 0 (the folder does not exist ...)`, or `expected one .mkv in ... found <n>`. ffprobe's messages go to `ffprobe.txt` and into the `ffprobe failed` reason.
- **Centre channel.** No list of layout names is kept. The script reads the installed ffmpeg's own table (`ffmpeg -layouts`) once per run and measures the main stream's centre channel only when the stream's layout, as ffprobe names it, has FC besides other channels (or, for a layout ffmpeg does not name, when ffprobe's spelling `<n> channels (FL+FR+FC+...)` lists FC). Any layout it cannot resolve, such as `unknown`, falls back to the mixdown as the dialogue curve, the path measured in section 4. If ffmpeg prints no table at all, the script stops (`ffmpeg -layouts printed no table of channel layouts`) rather than silently losing the centre channel of every 5.1 reference. On the Mac's ffmpeg 8.1.2 this gives exactly the 29 named layouts that have FC besides other channels, where the earlier pattern wrongly matched `3.0(back)` and missed 12 others. (On 8.1.2, `pan=mono|c0=FC` on a layout without FC does not fail; it yields digital silence, which audio_similarity.py would also have turned into the mixdown fallback.)

**`scripts/phase0/audio_similarity.py`** (Python 3.9 standard library): the measures and the rule of sections 2 and 3, with every threshold in one `RULE` table.
- **It refuses every folder that the new audio-levels.ps1 did not write.** A folder without `manifest.txt`, with a manifest that is not `audio-levels v2` or does not say `frame_ms=20`, without a main or a candidate line, or with a level file whose header (or, for raw ametadata output, whose `pts_time` spacing) is not 20 ms, is refused: one line on standard error saying that the title must be measured again, exit status 1, and no number printed for any folder of that run. The first measurement's folders (`e16-stereo/audio-levels-1-3`, `rehearsal/audio-levels-1-3`: 100 ms full-band curves, no manifest) are refused, so the two Office titles must be measured again with the new script (plan Step 16). The old folders and the table in notes.md stay as they are.
- The reader accepts raw `ametadata` files (at 20 ms) as well as the compact files.
- `--titles corpus/owner/<slug>` lists every title of a recorded scan, longest first, with its audio streams by language, leaving out the streams MakeMKV derived from another (flag 2048), and marks a title `<- <language> x<n>` when one language has two or more. It also says which titles were already ripped (from each label's `rip-summary.json`) and which share segments (a play-all playlist). Plan Step 19 chooses titles from it.
- Run time is about 3-4 s per candidate for a 22-minute episode on this Mac. The work grows with the running time, so a 2.5-hour film should take under a minute (estimated).

**`scripts/phase0/test_audio_similarity.py`**: 18 tests on synthetic curves and files: a delayed quieter copy is kept; a voice talking through the programme has the commentary pattern; the same voice is not confirmed against a reference with digital silence or when it covers 70% of the title; steady music in the gaps is not a commentary; each rule condition is required; audio description is only ever a note; an identical packet MD5 is reported as a duplicate without analysis; a folder written in the new form (compact files and raw 20 ms output) is analysed; a folder of the first measurement is refused before any number is printed; 100 ms level files are refused even beside a manifest; the title listing leaves out derived streams and orders titles longest first. They import `robot.py` (scan parsing) and run in about 4 s.

The existing `.gitignore` rule `corpus/owner/*/*/audio-levels-*/*.txt` already keeps the level files, the manifest, `ffprobe.txt`, `streamhash.txt` and `ffmpeg.txt` out of git; `streams.json` is small and is committed. The analysis output, which holds every measure and verdict, is committed as `corpus/owner/<slug>/<label>/audio-content-20ms-<main>-<cands>.txt` (plan Step 17).

### `scripts/phase0/audio-levels.ps1`

```powershell
# Experiment 13: measures a ripped title's audio streams so that audio_similarity.py can tell a commentary, an
# exact duplicate or programme audio apart by content. One ffmpeg pass over the one .mkv in
# $Staging\<Slug>\<Label>\ writes, for the main stream and every candidate stream, the speech-band (300-3400 Hz)
# RMS level of every 20 ms, and the MD5 of each of those streams' packets. Small text files only, in
# $Recordings\<Slug>\<Label>\audio-levels-20ms-<Main>-<Cand>[-<Cand>...]\ :
#   manifest.txt   first line "# lazymkv audio-levels v2 (experiment 13)"; then the file, the run time and
#                  frame_ms=20; then one line per stream: number, codec, channels, layout, language, frames,
#                  MD5 of its packets (identical MD5 = exact duplicate). audio_similarity.py refuses a folder
#                  without it, so the first measurement's folders (audio-levels-<Main>-<Cand>, 100 ms) are never read.
#   main-mix.txt   the main stream mixed down to mono: a header line "# lazymkv audio levels v2: frame_ms=20 ...",
#                  then one level in dB per line, 0.1 dB steps, -100 = silence
#   main-fc.txt    the main stream's centre channel (dialogue); only when its layout has one besides other channels
#   cand-<n>.txt   candidate stream <n> mixed down to mono
#   streams.json   ffprobe's list of the file's streams
#   ffprobe.txt    ffprobe's messages
#   streamhash.txt ffmpeg's MD5 of each stream's packets, in the order main, candidates
#   ffmpeg.txt     ffmpeg's messages
# Whether the main stream has a centre channel is read from the installed ffmpeg's own table of channel layouts
# (ffmpeg -layouts), not from a list kept here. A layout that ffmpeg does not name and ffprobe does not spell out
# (for example "unknown") counts as having none: the main stream's mixdown is then the dialogue curve.
# Stream numbers are ffprobe's stream indexes (0 is normally the video). Stereo width is not measured.
# Nothing is written to the share. An existing folder is kept unless -Force is given.
# The last line is "RESULT ok" or "RESULT FAILED: <reason>"; after a failure the script throws.
param(
    [Parameter(Mandatory)][string]$Slug,
    [Parameter(Mandatory)][string]$Label,
    [Parameter(Mandatory)][int]$Main,
    [Parameter(Mandatory)][int[]]$Cand,
    [switch]$Force
)
. C:\lazymkv\src\scripts\phase0\env.ps1

$Key = 'lavfi.astats.Overall.RMS_level='

# Turns ffmpeg's ametadata output into one value per line (0.1 dB, -100 for silence). Returns the frame count.
function Convert-Levels([string]$Raw, [string]$Out, [string]$Header) {
    $inv = [Globalization.CultureInfo]::InvariantCulture
    $writer = New-Object System.IO.StreamWriter($Out, $false, (New-Object System.Text.UTF8Encoding($false)))
    $count = 0
    try {
        $writer.WriteLine($Header)
        foreach ($line in [System.IO.File]::ReadLines($Raw)) {
            if (-not $line.StartsWith($Key)) { continue }
            $text = $line.Substring($Key.Length)
            $value = -100.0
            if (-not ($text.Contains('inf') -or $text.Contains('nan'))) {
                $value = [math]::Max(-100.0, [double]::Parse($text, $inv))
            }
            $writer.WriteLine($value.ToString('0.0', $inv))
            $count++
        }
    } finally { $writer.Close() }
    $count
}

# A manifest value without spaces.
function Get-Word($Value) {
    $text = [string]$Value
    if (-not $text) { return '-' }
    $text -replace '\s+', '_'
}

# The installed ffmpeg's own table of named channel layouts (ffmpeg -layouts, section "NAME DECOMPOSITION"):
# name -> its channels, for example 5.1(side) -> FL FR FC LFE SL SR.
function Get-LayoutTable {
    $table = @{}
    $inTable = $false
    foreach ($line in @(& $FFMPEG -hide_banner -layouts)) {
        $cols = @(([string]$line).Trim() -split '\s+')
        if ($cols.Count -ne 2) { continue }
        if ($cols[0] -eq 'NAME') { $inTable = ($cols[1] -eq 'DECOMPOSITION'); continue }
        if ($inTable) { $table[$cols[0]] = @($cols[1] -split '\+') }
    }
    if ($table.Count -eq 0) { throw "ffmpeg -layouts printed no table of channel layouts (no NAME DECOMPOSITION section): $FFMPEG" }
    $table
}

# The channels of a layout as ffprobe names it: a named layout from ffmpeg's table; an unnamed one from ffprobe's
# own spelling "<n> channels (FL+FR+...)". Anything else ("unknown", "<n> channels", empty) has no known channels.
function Get-LayoutChannels([string]$Layout, $Table) {
    if ($Layout -and $Table.ContainsKey($Layout)) { return @($Table[$Layout]) }
    if ($Layout -match '^\d+ channels \(([^()]+)\)$') { return @($Matches[1] -split '\+' | ForEach-Object { ($_ -split '@')[0] }) }
    @()
}

try {
    Assert-Slug $Slug
    Assert-Slug $Label
    if (-not $FFMPEG) { throw 'ffmpeg not found' }
    if (-not $FFPROBE) { throw 'ffprobe not found' }
    $Cand = @($Cand)
    if ($Cand -contains $Main) { throw "stream $Main is the main stream and cannot also be a candidate" }
    if (@($Cand | Select-Object -Unique).Count -ne $Cand.Count) { throw "a candidate stream is listed twice: $($Cand -join ',')" }
    $all = @($Main) + $Cand
    $name = 'audio-levels-20ms-' + ($all -join '-')
    Assert-Slug $name
    $layoutTable = Get-LayoutTable
    if (-not (Test-Path -LiteralPath $Staging)) { throw "cannot reach the staging share $Staging" }
    $dir = "$Staging\$Slug\$Label"
    if (-not (Test-Path -LiteralPath $dir)) { throw "expected one .mkv in $dir, found 0 (the folder does not exist: the rip is not in staging)" }
    $mkv = @(Get-ChildItem -LiteralPath $dir -File -ErrorAction Stop | Where-Object { $_.Extension -eq '.mkv' })
    if ($mkv.Count -ne 1) { throw "expected one .mkv in $dir, found $($mkv.Count)" }
    $file = $mkv[0].FullName
    $out = "$Recordings\$Slug\$Label\$name"
    if ((Test-Path -LiteralPath $out) -and -not $Force) { throw "$out already exists and is kept as evidence; use -Force only where the plan says so" }
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Recurse -Force -ErrorAction Stop }
    New-Item -ItemType Directory -Force -Path $out -ErrorAction Stop | Out-Null

    # Which streams the file has, and whether the main stream has a centre channel.
    # File names are relative (the tools run inside $out): a drive colon cannot be written in a filter argument.
    Push-Location -LiteralPath $out
    try {
        $probeLog = & cmd /c "`"$FFPROBE`" -v error -show_entries stream=index,codec_type,codec_name,profile,channels,channel_layout:stream_tags=language -of json -o streams.json `"$file`" 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    Set-Content -LiteralPath "$out\ffprobe.txt" -Value $probeLog -Encoding UTF8 -ErrorAction Stop
    if ($code -ne 0 -or -not (Test-Path -LiteralPath "$out\streams.json")) { throw "ffprobe failed (exit $code): $(@($probeLog) | Select-Object -First 3)" }
    $streams = @((Get-Content -LiteralPath "$out\streams.json" -Raw -Encoding UTF8 -ErrorAction Stop | ConvertFrom-Json).streams)
    $info = @{}
    foreach ($n in $all) {
        $s = @($streams | Where-Object { $_.index -eq $n }) | Select-Object -First 1
        if (-not $s) { throw "the file has no stream $n" }
        if ($s.codec_type -ne 'audio') { throw "stream $n is $($s.codec_type), not audio" }
        $info[$n] = $s
    }
    # A centre channel besides other channels: for mono the centre is the mixdown itself.
    $layout = [string]$info[$Main].channel_layout
    $channels = @(Get-LayoutChannels $layout $layoutTable)
    $hasFc = ($channels -contains 'FC') -and ($channels.Count -gt 1)

    # One pass: a level chain per series, and an MD5 of each stream's packets (stream copy, no decoding).
    $tail = 'aresample=8000,highpass=f=300,lowpass=f=3400,asetnsamples=n=160:p=0,' +
        'astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level,' +
        'ametadata=print:key=lavfi.astats.Overall.RMS_level:file='
    $series = @()
    $parts = @()
    if ($hasFc) {
        $parts += "[0:$Main]asplit=2[m1][m2]"
        $parts += "[m1]aformat=channel_layouts=mono,${tail}raw-main-mix.txt[o0]"
        $parts += "[m2]pan=mono|c0=FC,${tail}raw-main-fc.txt[o1]"
        $series += 'main-mix', 'main-fc'
    } else {
        $parts += "[0:$Main]aformat=channel_layouts=mono,${tail}raw-main-mix.txt[o0]"
        $series += 'main-mix'
    }
    foreach ($n in $Cand) {
        $parts += "[0:$n]aformat=channel_layouts=mono,${tail}raw-cand-$n.txt[o$($series.Count)]"
        $series += "cand-$n"
    }
    $graph = $parts -join ';'
    $nulls = (0..($series.Count - 1) | ForEach-Object { "-map [o$_] -f null o$_" }) -join ' '
    $copies = ($all | ForEach-Object { "-map 0:$_" }) -join ' '
    $started = Get-Date
    Push-Location -LiteralPath $out
    try {
        $log = & cmd /c "`"$FFMPEG`" -v error -nostdin -i `"$file`" -filter_complex `"$graph`" $nulls $copies -c copy -f streamhash -hash md5 streamhash.txt 2>&1"
        $code = $LASTEXITCODE
    } finally { Pop-Location }
    $seconds = [int]((Get-Date) - $started).TotalSeconds
    Set-Content -LiteralPath "$out\ffmpeg.txt" -Value $log -Encoding UTF8 -ErrorAction Stop
    if ($code -ne 0) { throw "ffmpeg failed (exit $code): $(@($log) | Select-Object -First 3)" }

    # streamhash.txt has one line per copied stream, in -map order: "<k>,a,MD5=<hash>".
    $md5 = @{}
    foreach ($line in @(Get-Content -LiteralPath "$out\streamhash.txt" -ErrorAction Stop)) {
        if ($line -match '^(\d+),a,MD5=([0-9a-f]{32})$') { $md5[$all[[int]$Matches[1]]] = $Matches[2] }
    }
    if ($md5.Count -ne $all.Count) { throw "streamhash.txt holds $($md5.Count) of $($all.Count) stream hashes" }

    # The raw ametadata files are about 80 bytes per frame; keep only the compact files.
    $frames = @{}
    foreach ($s in $series) {
        $raw = "$out\raw-$s.txt"
        if (-not (Test-Path -LiteralPath $raw)) { throw "ffmpeg wrote no raw-$s.txt" }
        $header = "# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=$s file=$(Get-Word $mkv[0].Name)"
        $frames[$s] = Convert-Levels $raw "$out\$s.txt" $header
        Remove-Item -LiteralPath $raw -Force -ErrorAction Stop
        if ($frames[$s] -eq 0) { throw "$s.txt holds no level" }
    }

    $lines = @('# lazymkv audio-levels v2 (experiment 13)',
        "file=$(Get-Word $mkv[0].Name) bytes=$($mkv[0].Length) seconds=$seconds frame_ms=20 band_hz=300-3400")
    foreach ($n in $all) {
        $s = $info[$n]
        $role = if ($n -eq $Main) { "main=$n" } else { "cand=$n" }
        $count = if ($n -eq $Main) { $frames['main-mix'] } else { $frames["cand-$n"] }
        $fc = if ($n -eq $Main) { ' fc=' + $(if ($hasFc) { 'yes' } else { 'no' }) } else { '' }
        $lines += "$role codec=$(Get-Word $s.codec_name) profile=$(Get-Word $s.profile) channels=$($s.channels) " +
            "layout=$(Get-Word $s.channel_layout) language=$(Get-Word $s.tags.language)$fc frames=$count md5=$($md5[$n])"
    }
    Set-Content -LiteralPath "$out\manifest.txt" -Value $lines -Encoding UTF8 -ErrorAction Stop

    "levels: $($mkv[0].Name) main=$Main cand=$($Cand -join ',') centre=$hasFc ffmpeg_exit=$code seconds=$seconds folder=$name"
    foreach ($n in $all) {
        $s = $info[$n]
        $count = if ($n -eq $Main) { $frames['main-mix'] } else { $frames["cand-$n"] }
        "stream ${n}: $($s.codec_name) $($s.channels) ch $($s.channel_layout) $($s.tags.language) frames=$count md5=$($md5[$n])"
    }
    'RESULT ok'
} catch {
    "RESULT FAILED: $($_.Exception.Message)"
    throw
}
```

### `scripts/phase0/audio_similarity.py`

```python
"""Experiment 13: is an extra audio track a commentary? Judged from its content against the main track.

    audio_similarity.py <audio-levels-20ms folder> ...     the measures and the verdict of every candidate
    audio_similarity.py --titles corpus/owner/<slug>       which titles of a recorded scan have two or more audio
                                                           streams in one language (derived streams left out)

Reads a folder written by audio-levels.ps1 (audio-levels-20ms-<main>-<candidates>): manifest.txt, main-mix.txt,
main-fc.txt (only when the main track has a centre channel) and cand-<stream>.txt, each holding one speech-band
(300-3400 Hz) RMS level in dB per 20 ms frame. For every candidate it prints the measures and the verdict of the
rule in docs/phase0/experiments/EXP-13-design.md (section 3):

  DC     dialogue carried: share of the main's 2 s dialogue windows in which the candidate's level follows the main's
  AG     added in the gaps: share of the main's dialogue-gap time in which the candidate carries speech-level sound
         that the main does not explain (10 dB above the level the main predicts, in a window that does not follow)
  tenths of the tenths of the running time that hold 10 s of dialogue gaps, in how many AG reaches 0.40
  SMI    syllabic modulation of the added sound (share of envelope energy at 2.3-7.8 Hz)
  LS     the candidate's level spread, 95th minus 50th percentile (small for dense talk)
  ADL    added sound while the main's dialogue runs

Stereo width, codec and track position are not used; the main track's channel layout only decides whether its
centre channel is the dialogue curve. A folder that audio-levels.ps1 did not write in this form is refused, with a
one-line message and a non-zero exit, before any number is printed: the first measurement's folders
(audio-levels-<main>-<cand>, 100 ms frames, full band, no manifest) must be measured again with the new script.
"""
import cmath
import json
import math
import pathlib
import statistics
import sys

from robot import duration_seconds, parse

FLOOR = -100.0          # dB; digital silence
FRAME_MS = 20           # one level per 20 ms ...
FRAME_S = 0.02          # ... in seconds
MANIFEST_LINE = "# lazymkv audio-levels v2"   # first line of the manifest.txt that audio-levels.ps1 writes
DERIVED = 2048          # scan flag (attribute 22): a stream MakeMKV derived from another, such as a DTS core
WIN = 100               # 2 s windows
BLOCK = 3000            # 60 s blocks for the lag
COARSE_LAG = 100        # +-10 s, in 100 ms steps
BLOCK_LAG = 100         # +-2 s around the global lag, per block
WIN_LAG = 5             # +-100 ms per window
MATCH_R = 0.70          # a window whose levels correlate at least this much follows the main
MISMATCH_R = 0.40       # added sound must sit in a window that correlates less than this
X_ACTIVE_BELOW = 20.0   # candidate active: within 20 dB of its own 95th percentile
DLG_BELOW = 25.0        # dialogue frame: dialogue reference within 25 dB of its own 95th percentile
GAP_MIN = 25            # a dialogue gap lasts at least 0.5 s ...
GAP_TRIM = 5            # ... and loses 100 ms at each end
ADDED_DB = 10.0         # added: at least 10 dB above the level the main predicts
SILENT_DB = -90.0       # the main is digitally silent at or below this level
N_SMI = 64              # 1.28 s modulation windows

# The thresholds of the decision rule. Every one is PROVISIONAL (docs/phase0/experiments/EXP-13-design.md, section 3).
RULE = {
    "overlap_min": 0.90, "main_silent_max": 0.10, "gap_s_min": 90.0,
    "dc_max": 0.30, "ag_min": 0.50, "tenths_min": 6, "tenths_share": 0.90, "smi_min": 0.50, "ls_max": 13.0,
    "added_s_min": 60.0, "reverse_gap_s_min": 90.0, "reverse_ag_max": 0.30,
    "ad_dc_min": 0.90, "ad_adl_max": 0.02, "ad_ag_min": 0.15, "ad_ag_s_min": 45.0, "ad_tenths_share": 0.60,
    "note_comm_dc_max": 0.50, "note_comm_ag_min": 0.30,
    "note_ad_dc_min": 0.80, "note_ad_ag_min": 0.10, "note_ad_ag_s_min": 30.0,
}
_TW = [[cmath.exp(-2j * math.pi * k * n / N_SMI) for n in range(N_SMI)] for k in range(33)]


def read_levels(path):
    """Levels in dB, one per frame. Accepts the compact files of audio-levels.ps1 (one value per line, '#' header
    lines) and raw ffmpeg ametadata output (lavfi.astats.Overall.RMS_level=...). Values are floored at FLOOR."""
    out = []
    for line in pathlib.Path(path).read_text(encoding="utf-8-sig", errors="replace").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("frame:"):
            continue
        if "=" in line:
            if not line.startswith("lavfi.astats.Overall.RMS_level="):
                continue
            line = line.split("=", 1)[1]
        out.append(FLOOR if ("inf" in line or "nan" in line) else max(FLOOR, float(line)))
    return out


def read_manifest(folder):
    """manifest.txt: 'key=value' pairs; lines starting 'main=' or 'cand=' describe one stream each."""
    info = {"main": None, "cands": []}
    path = pathlib.Path(folder) / "manifest.txt"
    if not path.exists():
        return info
    for line in path.read_text(encoding="utf-8-sig", errors="replace").splitlines():
        if line.startswith("#") or "=" not in line:
            continue
        fields = dict(f.split("=", 1) for f in line.split() if "=" in f)
        if "main" in fields:
            info["main"] = fields
        elif "cand" in fields:
            info["cands"].append(fields)
        else:
            info.update(fields)
    return info


def frame_ms_of(path):
    """The frame length a level file states, in ms: 'frame_ms=' in the first line of a compact file, or the spacing
    of the pts_time lines of raw ametadata output. None when the file states neither."""
    times = []
    with open(path, encoding="utf-8-sig", errors="replace") as f:
        for k, line in enumerate(f):
            if k == 0 and line.startswith("#"):
                for word in line.split():
                    if word.startswith("frame_ms="):
                        try:
                            return float(word[len("frame_ms="):])
                        except ValueError:
                            return None
            if "pts_time:" in line:
                try:
                    times.append(float(line.split("pts_time:", 1)[1].split()[0]))
                except (ValueError, IndexError):
                    pass
            if len(times) >= 50 or k >= 200:
                break
    steps = sorted(b - a for a, b in zip(times, times[1:]))
    return round(steps[len(steps) // 2] * 1000) if steps else None


def refusal(folder):
    """'' when the folder was written by the experiment-13 audio-levels.ps1 (manifest.txt, 20 ms frames in every level
    file), otherwise the reason in one line. Numbers are never printed for a refused folder."""
    d = pathlib.Path(folder)
    again = ("this folder cannot be analysed: measure the title again with the new audio-levels.ps1, "
             "which writes audio-levels-20ms-<main>-<candidates>")
    path = d / "manifest.txt"
    if not path.is_file():
        return (f"{d}: no manifest.txt, so it was not written by the new audio-levels.ps1 (the first measurement's "
                f"folders, audio-levels-<main>-<cand>, hold 100 ms full-band levels); {again}")
    lines = path.read_text(encoding="utf-8-sig", errors="replace").splitlines()
    if not lines or not lines[0].startswith(MANIFEST_LINE):
        return f"{d}: manifest.txt does not begin with '{MANIFEST_LINE}'; {again}"
    man = read_manifest(d)
    if man.get("frame_ms") != str(FRAME_MS):
        return f"{d}: manifest.txt gives frame_ms={man.get('frame_ms', '(none)')}, not {FRAME_MS}; {again}"
    if not man["main"] or not man["cands"]:
        return f"{d}: manifest.txt names no main stream or no candidate stream; {again}"
    names = ["main-mix.txt"] + (["main-fc.txt"] if (d / "main-fc.txt").exists() else [])
    names += [f"cand-{c.get('cand')}.txt" for c in man["cands"]]
    for name in names:
        if not (d / name).is_file():
            return f"{d}: {name} is missing; {again}"
        ms = frame_ms_of(d / name)
        if ms != FRAME_MS:
            held = "frames of unknown length" if ms is None else f"{ms:g} ms frames"
            return f"{d}: {name} holds {held}, not {FRAME_MS} ms; {again}"
    return ""


def percentile(values, share):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(share * len(ordered)))] if ordered else float("nan")


def clamp(values, below=45.0):
    floor = percentile(values, 0.95) - below
    return [max(v, floor) for v in values]


def pearson(a, b):
    n = len(a)
    if n < 2:
        return 0.0
    ma, mb = sum(a) / n, sum(b) / n
    cov = va = vb = 0.0
    for x, y in zip(a, b):
        dx, dy = x - ma, y - mb
        cov += dx * dy
        va += dx * dx
        vb += dy * dy
    return cov / math.sqrt(va * vb) if va > 0 and vb > 0 else 0.0


def corr_at(a, b, i0, i1, lag, step=1):
    """Correlation of a[i] and b[i + lag] for i in [i0, i1); -2 when fewer than 10 pairs exist."""
    lo, hi = max(i0, -lag), min(i1, len(b) - lag)
    if hi - lo < 10:
        return -2.0
    idx = range(lo, hi, step)
    return pearson([a[i] for i in idx], [b[i + lag] for i in idx])


def smi(levels_db):
    """Share of the linear envelope's modulation energy at 2.3-7.8 Hz (DFT bins 3-10 of 1-32, 64 x 20 ms)."""
    lin = [10 ** (v / 20) for v in levels_db]
    mean = sum(lin) / N_SMI
    x = [v - mean for v in lin]
    e = [abs(sum(x[n] * _TW[k][n] for n in range(N_SMI))) ** 2 for k in range(33)]
    total = sum(e[1:33])
    return sum(e[3:11]) / total if total > 0 else 0.0


def ok(v):
    return v == v  # not NaN


def analyse(main, cand, dialogue=None, reverse=True):
    """The measures of one candidate against the main track. main, cand, dialogue: levels in dB per 20 ms frame."""
    dialogue = dialogue or main
    n = len(main)
    if n < 10 * WIN or len(cand) < 10 * WIN:
        return {"frames": n, "error": "too short"}
    mc, xc = clamp(main), clamp(cand)
    # Lag: tracks of one title share a timeline. Coarse +-10 s on 100 ms means, fine +-100 ms; kept only when the
    # two envelopes follow each other (r >= 0.5), otherwise 0. Then per 60 s block +-2 s, kept when r >= 0.5.
    mk = [sum(mc[i:i + 5]) / 5 for i in range(0, n - 4, 5)]
    xk = [sum(xc[i:i + 5]) / 5 for i in range(0, len(xc) - 4, 5)]
    _, lag_k = max((corr_at(mk, xk, 0, len(mk), lag, 2), lag) for lag in range(-COARSE_LAG, COARSE_LAG + 1))
    r_glob, lag = max((corr_at(mc, xc, 0, n, l, 5), l) for l in range(5 * lag_k - 5, 5 * lag_k + 6))
    if r_glob < 0.5:
        lag = 0
    block_lag = {}
    for b in range((n + BLOCK - 1) // BLOCK):
        b0, b1 = b * BLOCK, min(n, (b + 1) * BLOCK)
        r, lb = max((corr_at(mc, xc, b0, b1, l, 4), l) for l in range(lag - BLOCK_LAG, lag + BLOCK_LAG + 1, 2))
        block_lag[b] = lb if r >= 0.5 else lag

    def lag_of(t):
        return block_lag[t // BLOCK]

    inside = [0 <= t + lag_of(t) < len(cand) for t in range(n)]
    frames = [t for t in range(n) if inside[t]]
    xa = [cand[t + lag_of(t)] if inside[t] else FLOOR for t in range(n)]
    x95 = percentile([xa[t] for t in frames], 0.95)
    m95 = percentile([main[t] for t in frames], 0.95)
    d95 = percentile([dialogue[t] for t in frames if t < len(dialogue)], 0.95)
    silent = [main[t] <= SILENT_DB for t in range(n)]
    active = [inside[t] and xa[t] >= x95 - X_ACTIVE_BELOW for t in range(n)]
    dlg = [inside[t] and t < len(dialogue) and dialogue[t] >= d95 - DLG_BELOW for t in range(n)]
    gap = [False] * n
    t = 0
    while t < n:
        if inside[t] and not dlg[t]:
            u = t
            while u < n and inside[u] and not dlg[u]:
                u += 1
            if u - t >= GAP_MIN:
                for k in range(t + GAP_TRIM, u - GAP_TRIM):
                    gap[k] = not silent[k]
            t = u
        else:
            t += 1
    wins = [w for w in range(0, n - WIN + 1, WIN) if all(inside[w:w + WIN])]
    rw = {w: max(corr_at(mc, xc, w, w + WIN, lag_of(w) + d) for d in range(-WIN_LAG, WIN_LAG + 1)) for w in wins}
    dwins = [w for w in wins if sum(dlg[w:w + WIN]) >= WIN // 2]
    dc = sum(1 for w in dwins if rw[w] >= MATCH_R) / len(dwins) if dwins else float("nan")
    matched = [w for w in wins if rw[w] >= MATCH_R]
    mframes = [u for w in matched for u in range(w, w + WIN) if main[u] >= m95 - 30 and xa[u] > FLOOR]
    if len(matched) >= max(10, len(wins) // 20) and mframes:
        gain = percentile([xa[u] - main[u] for u in mframes], 0.75)
    else:
        gain = x95 - m95
    added = [active[u] and xa[u] - gain - main[u] >= ADDED_DB and rw.get((u // WIN) * WIN, 1.0) < MISMATCH_R
             for u in range(n)]
    n_gap = sum(gap)
    n_ag = sum(1 for u in range(n) if gap[u] and added[u])
    n_dlg = sum(dlg)
    lo, hi = frames[0], frames[-1] + 1
    tenths = []
    for k in range(10):
        a, b = lo + k * (hi - lo) // 10, lo + (k + 1) * (hi - lo) // 10
        g = sum(gap[a:b])
        if g * FRAME_S >= 10:
            tenths.append(sum(1 for u in range(a, b) if gap[u] and added[u]) / g)
    s = [smi(xa[t0:t0 + N_SMI]) for t0 in range(0, n - N_SMI, N_SMI // 2) if sum(added[t0:t0 + N_SMI]) >= N_SMI // 2]
    m = {
        "frames": n, "lag_ms": lag * 20, "r_global": r_glob, "overlap": len(frames) / n,
        "main_silent": sum(1 for u in frames if silent[u]) / len(frames),
        "dc": dc, "gain_db": gain, "gap_s": n_gap * FRAME_S,
        "ag": n_ag / n_gap if n_gap else float("nan"), "ag_s": n_ag * FRAME_S,
        "adl": sum(1 for u in range(n) if dlg[u] and added[u]) / n_dlg if n_dlg else float("nan"),
        "tenths": len(tenths), "tenths_comm": sum(1 for v in tenths if v >= 0.40),
        "tenths_ad": sum(1 for v in tenths if v >= 0.05),
        "smi": statistics.median(s) if s else float("nan"), "smi_windows": len(s),
        "ls_db": x95 - percentile([xa[u] for u in frames], 0.50), "added_s": sum(added) * FRAME_S,
    }
    if reverse:
        back = analyse(cand, main, None, reverse=False)
        m["rev_gap_s"], m["rev_ag"] = back.get("gap_s", 0.0), back.get("ag", float("nan"))
    return m


def commentary_pattern(m, rule=RULE):
    return (ok(m["dc"]) and m["dc"] <= rule["dc_max"] and ok(m["ag"]) and m["ag"] >= rule["ag_min"]
            and m["tenths"] >= rule["tenths_min"] and m["tenths_comm"] >= rule["tenths_share"] * m["tenths"]
            and ok(m["smi"]) and m["smi"] >= rule["smi_min"] and m["ls_db"] <= rule["ls_max"]
            and m["added_s"] >= rule["added_s_min"])


def ad_pattern(m, rule=RULE):
    return (ok(m["dc"]) and m["dc"] >= rule["ad_dc_min"] and ok(m["adl"]) and m["adl"] <= rule["ad_adl_max"]
            and ok(m["ag"]) and m["ag"] >= rule["ad_ag_min"] and m["ag_s"] >= rule["ad_ag_s_min"]
            and m["tenths"] >= rule["tenths_min"] and m["tenths_ad"] >= rule["ad_tenths_share"] * m["tenths"]
            and ok(m["smi"]) and m["smi"] >= rule["smi_min"])


def not_judged(m, rule=RULE):
    """Why the content rule may not confirm anything for this pair, or '' when it may."""
    if "error" in m:
        return m["error"]
    if m["overlap"] < rule["overlap_min"]:
        return "the tracks overlap for less than 90% of the main track"
    if m["main_silent"] > rule["main_silent_max"]:
        return "the main track is digitally silent for more than 10% of the time (not continuous programme sound)"
    if m["gap_s"] < rule["gap_s_min"]:
        return "the main track has less than 90 s of dialogue gaps"
    if not ok(m["dc"]):
        return "the main track has no dialogue windows"
    return ""


def verdict(m, rule=RULE):
    """One of: commentary, reference-unclear, ad-pattern, note-commentary, note-ad, keep; with not-judged reasons."""
    why = not_judged(m, rule)
    if "error" in m:
        return "not-judged"
    if not why and commentary_pattern(m, rule):
        reverse = ok(m.get("rev_ag", float("nan"))) and m.get("rev_gap_s", 0.0) >= rule["reverse_gap_s_min"] \
            and m["rev_ag"] >= rule["reverse_ag_max"]
        return "reference-unclear" if reverse else "commentary"
    if not why and ad_pattern(m, rule):
        return "ad-pattern"
    if ok(m["dc"]) and ok(m["ag"]) and m["dc"] <= rule["note_comm_dc_max"] and m["ag"] >= rule["note_comm_ag_min"]:
        return "note-commentary"
    if ok(m["dc"]) and ok(m["ag"]) and m["dc"] >= rule["note_ad_dc_min"] and m["ag"] >= rule["note_ad_ag_min"] \
            and m["ag_s"] >= rule["note_ad_ag_s_min"]:
        return "note-ad"
    return "keep"


MEANING = {
    "duplicate": "exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed",
    "commentary": "commentary pattern: kept and flagged for review until experiment 13 answers yes, then dropped",
    "reference-unclear": "commentary pattern, but the main track also adds sound to the candidate: keep, review",
    "ad-pattern": "audio-description pattern: keep, note possible_audio_description (never dropped by content)",
    "note-commentary": "keep, note possible_commentary",
    "note-ad": "keep, note possible_audio_description",
    "keep": "keep: programme audio, nothing added",
    "not-judged": "keep: no content verdict",
}


def fmt(v, digits=2):
    return "n/a" if not ok(v) else f"{v:.{digits}f}"


def report(folder):
    d = pathlib.Path(folder)
    why = refusal(d)
    if why:
        raise ValueError(why)
    man = read_manifest(d)
    main = read_levels(d / "main-mix.txt")
    fc_path = d / "main-fc.txt"
    fc = read_levels(fc_path) if fc_path.exists() else []
    if fc and max(fc) <= SILENT_DB:
        fc = []
    mainfo = man["main"] or {}
    lines = [f"{d} ({man.get('file', '?')}, measured in {man.get('seconds', '?')} s)",
             f"  main stream {mainfo.get('main', '?')}: {mainfo.get('codec', '?')} {mainfo.get('channels', '?')} ch "
             f"{mainfo.get('layout', '')} {mainfo.get('language', '')}; dialogue reference: "
             f"{'its centre channel' if fc else 'its mono mixdown (no centre channel)'}; "
             f"{len(main)} frames ({len(main) * FRAME_S / 60:.1f} min)"]
    cands = man["cands"]
    seen = {mainfo["md5"]: mainfo.get("main", "?")} if mainfo.get("md5") else {}
    label = f"{d.resolve().parent.name}/{man.get('file', d.resolve().name)}"
    for c in cands:
        sid = c.get("cand")
        path = d / f"cand-{sid}.txt"
        head = f"  stream {sid}: {c.get('codec', '?')} {c.get('channels', '?')} ch {c.get('layout', '')} {c.get('language', '')}"
        if c.get("md5") in seen:
            lines += [head, f"    verdict: duplicate of stream {seen[c['md5']]} (" + MEANING["duplicate"] + ")",
                      f"    row: | {label} | {sid} | same packets as stream {seen[c['md5']]} | duplicate |"]
            continue
        if c.get("md5"):
            seen[c["md5"]] = sid
        m = analyse(main, read_levels(path), fc or None)
        v = verdict(m)
        lines.append(head)
        if "error" in m:
            lines.append(f"    verdict: not-judged ({m['error']})")
            continue
        why = not_judged(m)
        lines += [
            f"    lag {m['lag_ms']} ms (envelope r {fmt(m['r_global'])}), overlap {fmt(m['overlap'])}, "
            f"main silent {fmt(m['main_silent'])}",
            f"    DC {fmt(m['dc'])}  AG {fmt(m['ag'])} ({m['gap_s']:.0f} s of gaps, {m['ag_s']:.0f} s added)  "
            f"tenths {m['tenths_comm']}/{m['tenths']}  SMI {fmt(m['smi'])}  LS {fmt(m['ls_db'], 1)} dB  "
            f"ADL {fmt(m['adl'])}  added {m['added_s']:.0f} s",
            f"    reverse: {m['rev_gap_s']:.0f} s of gaps, AG {fmt(m['rev_ag'])}",
            f"    verdict: {v} ({MEANING[v]})" + (f"; not judged: {why}" if why else ""),
            f"    row: | {label} | {sid} | DC {fmt(m['dc'])}, AG {fmt(m['ag'])}, tenths "
            f"{m['tenths_comm']}/{m['tenths']}, SMI {fmt(m['smi'])}, LS {fmt(m['ls_db'], 1)} | {v} |",
        ]
    return lines


def segments(text):
    """A scan's segment map (title attribute 26), such as '7', '10,11' or '1-3', as a list of segment numbers."""
    out = []
    for part in text.split(","):
        lo, _, hi = part.strip().partition("-")
        if lo.isdigit() and (not hi or hi.isdigit()):
            out += list(range(int(lo), int(hi or lo) + 1))
    return out


def audio_by_title(scan):
    """{title: (seconds, [segment, ...], {language: [stream, ...]})} for a parsed scan (robot.parse). Audio streams
    that MakeMKV derived from another one (flag 2048, such as the DTS core of a DTS-HD MA track) are left out: MakeMKV
    synthesises them, they are not extra tracks on the disc. A stream without a language code counts as 'und'."""
    out = {}
    for title, a in scan.titles.items():
        try:
            seconds = duration_seconds(a.get(9, "0:00:00"))
        except ValueError:
            seconds = 0
        out[title] = (seconds, segments(a.get(26, "")), {})
    for (title, stream), a in sorted(scan.streams.items()):
        if title not in out or a.get(1) != "Audio" or int(a.get(22, "0") or 0) & DERIVED:
            continue
        out[title][2].setdefault(a.get(3) or "und", []).append(stream)
    return out


def titles_report(recording):
    """Every title of <recording>/scan.txt, longest first: its audio streams by language without derived streams, a
    mark '<- <language> x<n>' when one language has two or more, the labels it was already ripped under (from each
    <label>/rip-summary.json), and the titles it shares segments with (a play-all playlist)."""
    d = pathlib.Path(recording)
    scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
    ripped = {}
    for path in sorted(d.glob("*/rip-summary.json")):
        try:
            rip = json.loads(path.read_text(encoding="utf-8-sig"))
        except ValueError:
            continue
        if isinstance(rip.get("title"), int):
            label = path.parent.name + ("" if rip.get("ok") else " (failed)")
            ripped.setdefault(rip["title"], []).append(label)
    tmdb = d / "tmdb.json"
    original = json.loads(tmdb.read_text(encoding="utf-8-sig")).get("original_language") if tmdb.exists() else None
    info = audio_by_title(scan)
    lines = [f"{d.name}: {len(info)} titles, longest first; TMDb original language: {original or 'unknown'}",
             "  audio: scan stream numbers by language, leaving out streams MakeMKV derived from another (flag 2048)"]
    for title in sorted(info, key=lambda t: (-info[t][0], t)):
        seconds, segs, langs = info[title]
        audio = "; ".join(f"{lang} {','.join(map(str, s))}" for lang, s in sorted(langs.items())) or "none"
        notes = [f"<- {lang} x{len(s)}" for lang, s in sorted(langs.items()) if len(s) >= 2]
        if title in ripped:
            notes.append("ripped as " + ", ".join(ripped[title]))
        shared = [str(u) for u in sorted(info) if u != title and set(segs) & set(info[u][1])]
        if shared:
            notes.append("shares segments with title " + ", ".join(shared))
        where = ",".join(map(str, segs)) or "?"
        line = f"  title {title:>3}  {scan.titles[title].get(9, '?'):>8}  segments {where:<8}  audio {audio}"
        lines.append(line + ("   " + "; ".join(notes) if notes else ""))
    return lines


def main(args):
    if args[:1] == ["--titles"]:
        if len(args) != 2:
            sys.exit("usage: audio_similarity.py --titles corpus/owner/<slug>")
        if not (pathlib.Path(args[1]) / "scan.txt").is_file():
            sys.exit(f"{args[1]}: no scan.txt (record and pull the disc first)")
        for line in titles_report(args[1]):
            print(line)
        return
    for folder in args:  # every folder is checked before anything is printed
        why = refusal(folder)
        if why:
            sys.exit(why)
    for folder in args:
        for line in report(folder):
            print(line, flush=True)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: audio_similarity.py <audio-levels-20ms folder> ... | --titles corpus/owner/<slug>")
    main(sys.argv[1:])
```

### `scripts/phase0/test_audio_similarity.py`

```python
import contextlib
import io
import math
import pathlib
import random
import tempfile
import unittest

import audio_similarity as sim
import robot

FPS = 50  # 20 ms frames


def speech(rnd, frames, loud, quiet):
    """Syllables: loud for 4-7 frames, quiet for 4-8 frames (about 4.5 per second)."""
    out = []
    while len(out) < frames:
        out += [loud + rnd.uniform(-1, 1)] * rnd.randint(4, 7)
        out += [quiet + rnd.uniform(-1, 1)] * rnd.randint(4, 8)
    return out[:frames]


def programme(seconds=420, seed=1):
    """A dialogue-driven programme: lines of 2-4 s, then 1.5-3 s of room tone. Returns (mix, centre)."""
    rnd = random.Random(seed)
    mix, centre = [], []
    while len(mix) < seconds * FPS:
        n = rnd.randint(2 * FPS, 4 * FPS)
        line = speech(rnd, n, -20, -30)
        mix += line
        centre += [v - 1 for v in line]
        n = rnd.randint(int(1.5 * FPS), 3 * FPS)
        mix += [-60 + rnd.uniform(-1, 1) for _ in range(n)]
        centre += [-75 + rnd.uniform(-1, 1) for _ in range(n)]
    return mix[:seconds * FPS], centre[:seconds * FPS]


def power_sum(a, b):
    return 10 * math.log10(10 ** (a / 10) + 10 ** (b / 10))


def commentary(mix, seed=7):
    """Two people talking all the way through, pausing briefly, over the programme ducked by 30 dB."""
    rnd = random.Random(seed)
    voice = []
    while len(voice) < len(mix):
        voice += speech(rnd, rnd.randint(3 * FPS, 6 * FPS), -18, -27)
        voice += [-100.0] * rnd.randint(15, 30)
    return [power_sum(v, m - 30) for v, m in zip(voice, mix)]


MIX, CENTRE = programme()

# What audio-levels.ps1 writes: the manifest, and compact level files with a header line.
MANIFEST = ("# lazymkv audio-levels v2 (experiment 13)\nfile=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n"
            "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc={fc} frames=1 md5={main}\n"
            "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5={cand}\n")


def level_file(path, values):
    path.write_text("# lazymkv audio levels v2: frame_ms=20 band_hz=300-3400 unit=dB floor=-100 series=x file=x.mkv\n"
                    + "\n".join(f"{v:.1f}" for v in values) + "\n", encoding="utf-8")


def raw_file(path, values, step_s):
    """ffmpeg's ametadata output: one frame every step_s seconds."""
    path.write_text("".join(f"frame:{k}    pts:{k}    pts_time:{k * step_s:g}\n"
                            f"lavfi.astats.Overall.RMS_level={v:.6f}\n" for k, v in enumerate(values)), encoding="utf-8")


class Reading(unittest.TestCase):
    def test_compact_and_raw_level_files_are_read_and_silence_is_the_floor(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            compact = pathlib.Path(tmp) / "main-mix.txt"
            compact.write_text("# lazymkv audio levels v2\n# stream=1 kind=main-mix\n-45.3\n-100.0\n-12.0\n", encoding="utf-8")
            self.assertEqual(sim.read_levels(compact), [-45.3, -100.0, -12.0])
            raw = pathlib.Path(tmp) / "raw.txt"
            raw.write_text("frame:0    pts:0       pts_time:0\nlavfi.astats.Overall.RMS_level=-inf\n"
                           "frame:1    pts:160     pts_time:0.02\nlavfi.astats.Overall.RMS_level=-123.5\n"
                           "frame:2    pts:320     pts_time:0.04\nlavfi.astats.Overall.RMS_level=-23.5\n", encoding="utf-8")
            self.assertEqual(sim.read_levels(raw), [sim.FLOOR, sim.FLOOR, -23.5])

    def test_manifest_lists_the_main_and_the_candidates(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            (pathlib.Path(tmp) / "manifest.txt").write_text(
                "# lazymkv audio-levels v2\nfile=x.mkv\nmain=1 codec=dts channels=6 layout=5.1(side) language=eng fc=yes md5=aa\n"
                "cand=3 codec=ac3 channels=2 layout=stereo language=eng md5=bb\ncand=4 codec=dts channels=6 md5=aa\n",
                encoding="utf-8")
            man = sim.read_manifest(tmp)
            self.assertEqual(man["main"]["main"], "1")
            self.assertEqual([c["cand"] for c in man["cands"]], ["3", "4"])
            self.assertEqual(man["file"], "x.mkv")


class Measures(unittest.TestCase):
    def test_a_quieter_delayed_copy_of_the_programme_is_kept(self):
        copy = [-100.0] * 3 + [v - 6 for v in MIX[:-3]]  # 60 ms late, 6 dB down
        m = sim.analyse(MIX, copy, CENTRE)
        self.assertEqual(m["lag_ms"], 60)
        self.assertGreaterEqual(m["dc"], 0.9)
        self.assertEqual(m["ag"], 0.0)
        self.assertEqual(sim.verdict(m), "keep")

    def test_a_voice_talking_through_the_programme_has_the_commentary_pattern(self):
        m = sim.analyse(MIX, commentary(MIX), CENTRE)
        self.assertEqual(sim.not_judged(m), "")
        self.assertLessEqual(m["dc"], 0.30)
        self.assertGreaterEqual(m["ag"], 0.50)
        self.assertEqual(m["tenths_comm"], m["tenths"])
        self.assertGreaterEqual(m["smi"], 0.50)
        self.assertLessEqual(m["ls_db"], 13.0)
        self.assertEqual(sim.verdict(m), "commentary")

    def test_the_same_voice_is_not_confirmed_when_the_main_is_not_continuous_programme_sound(self):
        mix, centre = list(MIX), list(CENTRE)
        for t in range(0, len(mix), 10 * FPS):  # 3 s of digital silence in every 10 s: an isolated score, say
            for u in range(t, min(len(mix), t + 3 * FPS)):
                mix[u] = centre[u] = -100.0
        m = sim.analyse(mix, commentary(MIX), centre)
        self.assertIn("silent", sim.not_judged(m))
        self.assertNotEqual(sim.verdict(m), "commentary")

    def test_steady_music_added_in_the_gaps_is_not_a_commentary(self):
        cand = [m if c > -50 else -35.0 for m, c in zip(MIX, CENTRE)]
        m = sim.analyse(MIX, cand, CENTRE)
        self.assertGreaterEqual(m["dc"], 0.8)
        self.assertNotIn(sim.verdict(m), ("commentary", "reference-unclear"))

    def test_a_candidate_covering_only_part_of_the_main_is_not_confirmed(self):
        m = sim.analyse(MIX, commentary(MIX)[:len(MIX) * 7 // 10], CENTRE)
        self.assertLess(m["overlap"], 0.9)
        self.assertNotEqual(sim.verdict(m), "commentary")

    def test_too_little_audio_gives_no_verdict(self):
        m = sim.analyse(MIX[:500], MIX[:500])
        self.assertEqual(sim.verdict(m), "not-judged")


class Rule(unittest.TestCase):
    BASE = {"overlap": 1.0, "main_silent": 0.03, "gap_s": 355.0, "dc": 0.03, "ag": 0.66, "ag_s": 234.0, "adl": 0.18,
            "tenths": 10, "tenths_comm": 10, "tenths_ad": 10, "smi": 0.58, "ls_db": 10.1, "added_s": 518.0,
            "rev_gap_s": 29.0, "rev_ag": 0.10}

    def v(self, **change):
        return sim.verdict(dict(self.BASE, **change))

    def test_the_measured_commentary_has_the_pattern(self):
        self.assertEqual(self.v(), "commentary")

    def test_each_condition_is_required(self):
        self.assertEqual(self.v(dc=0.31), "note-commentary")
        self.assertEqual(self.v(ag=0.49), "note-commentary")
        self.assertEqual(self.v(tenths_comm=8), "note-commentary")
        self.assertEqual(self.v(tenths=5, tenths_comm=5), "note-commentary")
        self.assertEqual(self.v(smi=0.49), "note-commentary")
        self.assertEqual(self.v(ls_db=13.1), "note-commentary")
        self.assertEqual(self.v(added_s=59.0), "note-commentary")
        self.assertEqual(self.v(main_silent=0.11), "note-commentary")
        self.assertEqual(self.v(gap_s=89.0), "note-commentary")
        self.assertEqual(self.v(overlap=0.89), "note-commentary")

    def test_a_main_track_that_adds_sound_to_the_candidate_makes_the_reference_unclear(self):
        self.assertEqual(self.v(rev_gap_s=150.0, rev_ag=0.35), "reference-unclear")
        self.assertEqual(self.v(rev_gap_s=60.0, rev_ag=0.35), "commentary")

    def test_audio_description_is_only_ever_a_note(self):
        ad = dict(dc=0.95, adl=0.0, ag=0.30, ag_s=100.0, tenths_comm=2, tenths_ad=9, smi=0.55, ls_db=17.0)
        self.assertEqual(self.v(**ad), "ad-pattern")
        self.assertEqual(self.v(**dict(ad, smi=0.30)), "note-ad")

    def test_a_copy_is_kept(self):
        self.assertEqual(self.v(dc=1.0, ag=0.0, ag_s=0.0, adl=0.0, tenths_comm=0, tenths_ad=0, smi=float("nan")), "keep")


class Report(unittest.TestCase):
    def folder(self, tmp, name="audio-levels-20ms-1-3"):
        d = pathlib.Path(tmp) / name
        d.mkdir()
        return d

    def test_an_identical_stream_is_reported_as_a_duplicate_without_analysis(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            for name in ("main-mix.txt", "cand-3.txt"):
                level_file(d / name, MIX[:2000])
            (d / "manifest.txt").write_text(MANIFEST.format(fc="no", main="43ae", cand="43ae"), encoding="utf-8")
            text = "\n".join(sim.report(d))
            self.assertIn("verdict: duplicate of stream 1", text)
            self.assertIn("its mono mixdown (no centre channel)", text)

    def test_a_folder_written_by_the_new_script_is_analysed(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            level_file(d / "main-mix.txt", MIX)
            level_file(d / "main-fc.txt", CENTRE)
            raw_file(d / "cand-3.txt", commentary(MIX), 0.02)  # raw ametadata output at 20 ms is read as well
            (d / "manifest.txt").write_text(MANIFEST.format(fc="yes", main="aa", cand="bb"), encoding="utf-8")
            self.assertEqual(sim.refusal(d), "")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                sim.main([str(d)])
            self.assertIn("dialogue reference: its centre channel", out.getvalue())
            self.assertIn("verdict: commentary", out.getvalue())

    def test_a_folder_of_the_first_measurement_is_refused_before_any_number(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp, "audio-levels-1-3")  # 100 ms frames, full band, no manifest
            for name in ("main-mix.txt", "main-fc.txt", "cand-mix.txt", "cand-mid.txt", "cand-side.txt"):
                raw_file(d / name, MIX[:3000:5], 0.1)
            out = io.StringIO()
            with contextlib.redirect_stdout(out), self.assertRaises(SystemExit) as stop:
                sim.main([str(d)])
            self.assertIn("no manifest.txt", str(stop.exception.code))
            self.assertIn("measure the title again", str(stop.exception.code))
            self.assertEqual(out.getvalue(), "")
            self.assertRaises(ValueError, sim.report, d)

    def test_level_files_of_100_ms_frames_are_refused_even_with_a_manifest(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = self.folder(tmp)
            for name in ("main-mix.txt", "cand-3.txt"):
                raw_file(d / name, MIX[:3000:5], 0.1)
            (d / "manifest.txt").write_text(MANIFEST.format(fc="no", main="aa", cand="bb"), encoding="utf-8")
            self.assertIn("main-mix.txt holds 100 ms frames, not 20 ms", sim.refusal(d))
            self.assertIn("measure the title again", sim.refusal(d))


class Titles(unittest.TestCase):
    SCAN = "\n".join(
        ['TINFO:0,9,0,"0:21:49"', 'TINFO:0,26,0,"7"', 'TINFO:1,9,0,"0:42:44"', 'TINFO:1,26,0,"4"',
         'TINFO:2,9,0,"1:04:33"', 'TINFO:2,26,0,"4,7"', 'TINFO:3,9,0,"0:05:10"', 'TINFO:3,26,0,"9"']
        + [f'SINFO:{t},{s},{k},0,"{v}"' for t, s, k, v in (
            (0, 0, 1, "Video"),
            (0, 1, 1, "Audio"), (0, 1, 3, "eng"), (0, 1, 22, "1024"),
            (0, 2, 1, "Audio"), (0, 2, 3, "eng"), (0, 2, 22, "2304"),  # the derived DTS core
            (0, 3, 1, "Audio"), (0, 3, 3, "eng"), (0, 3, 22, "0"),
            (1, 1, 1, "Audio"), (1, 1, 3, "eng"), (1, 1, 22, "1024"),
            (1, 2, 1, "Audio"), (1, 2, 3, "eng"), (1, 2, 22, "2304"),
            (1, 3, 1, "Audio"), (1, 3, 3, "fra"), (1, 3, 22, "0"),
            (2, 1, 1, "Audio"), (2, 1, 3, "eng"), (2, 1, 22, "0"),
            (3, 1, 1, "Audio"), (3, 1, 3, "eng"), (3, 2, 1, "Audio"), (3, 2, 3, "eng"))]) + "\n"

    def test_titles_list_audio_by_language_without_derived_streams_longest_first(self):
        info = sim.audio_by_title(robot.parse(self.SCAN))
        self.assertEqual(info[0], (1309, [7], {"eng": [1, 3]}))
        self.assertEqual(info[1][2], {"eng": [1], "fra": [3]})
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            d = pathlib.Path(tmp) / "some-disc"
            (d / "e16").mkdir(parents=True)
            (d / "scan.txt").write_text(self.SCAN, encoding="utf-8")
            (d / "e16" / "rip-summary.json").write_text('{"ok": true, "title": 0}', encoding="utf-8")
            lines = sim.titles_report(d)
        self.assertEqual([line.split()[1] for line in lines[2:]], ["2", "1", "0", "3"])
        self.assertIn("shares segments with title 0, 1", lines[2])
        self.assertNotIn("<-", lines[3])
        self.assertIn("<- eng x2; ripped as e16", lines[4])
        self.assertIn("<- eng x2", lines[5])  # no flags attribute at all: not derived


if __name__ == "__main__":
    unittest.main()
```

## 7. Plan steps for Task 12

These steps go after Step 12 of Task 12. The old Step 13 (Commit) is replaced by Step 21 below. In them, "the design" is `docs/phase0/experiments/EXP-13-design.md` (this file, committed there by the controller before Task 12 runs). Task 12's header gains:

- **Files:**
  - Prerequisite, committed by the controller before this task: `docs/phase0/experiments/EXP-13-design.md` (the design).
  - Modify: `scripts/phase0/audio-levels.ps1`, `scripts/phase0/audio_similarity.py`, `scripts/phase0/test_audio_similarity.py` (contents in `docs/phase0/experiments/EXP-13-design.md`, section 6, "Changes to the measurement scripts").
  - Create: `docs/phase0/experiments/EXP-13-commentary-by-audio.md`.
  - Create (pulled from the mini PC): `corpus/owner/<slug>/<label>/audio-levels-20ms-<M>-<C...>/`. Its level files are ignored by git; its `streams.json` is committed.
  - Create (on the Mac): `corpus/owner/<slug>/<label>/audio-content-20ms-<M>-<C...>.txt`.
  - Append to (on the Mac only): a `## Experiment 13 (Task 12)` section in `corpus/owner/<slug>/notes.md` for every disc measured.
- **Produces:** `audio-levels.ps1 -Slug <slug> -Label <label> -Main <stream> -Cand <stream>[,<stream>...] [-Force]`, run as a job. Its log lines are `levels: <file> main=<M> cand=<C,...> centre=<True|False> ffmpeg_exit=<n> seconds=<s> folder=<folder>`, one `stream <n>: <codec> <channels> ch <layout> <language> frames=<n> md5=<hash>` line per stream, then `RESULT ok` or `RESULT FAILED: <reason>`. After a failure the script throws, so wait.sh exits 1.

Placeholders for Steps 15 to 18:
- **TITLES** is the list of (SLUG, LABEL) pairs whose ripped file has two or more audio streams in one language:
  - `the-office-2005-s3d3-bluray` with `e16-stereo` and with `rehearsal`;
  - SLUG_A with `all-tracks` (Task 11);
  - each NEW with `all-tracks` from Steps 6 to 8;
  - each pair ripped in Step 19.
- **M** is the reference stream number and **C** the candidate list, both chosen in Step 15. **CS** is C with hyphens in place of commas.
- **LOG** is `audio-levels-SLUG-LABEL`, with `-M` appended when one label is measured for two languages, and `-2`, `-3` and so on appended on a repeat.

When `-Force` is right for `audio-levels.ps1`: only to repeat a measurement that failed after its folder was created. That means a `RESULT FAILED:` reason beginning `ffprobe failed`, `ffmpeg failed`, `streamhash.txt holds`, `ffmpeg wrote no raw-` or `<series>.txt holds no level`, once the cause has been dealt with. A refusal before that (among them `ffmpeg -layouts printed no table of channel layouts`, `cannot reach the staging share` and `expected one .mkv`) wrote nothing and is repeated without `-Force`. Never over a folder whose run ended `RESULT ok`.

- [ ] **Step 13: Write and test the measurement scripts**

Which disc is in the drive: does not matter (this runs on the Mac).

Replace `scripts/phase0/audio-levels.ps1`, `scripts/phase0/audio_similarity.py` and `scripts/phase0/test_audio_similarity.py` with the contents given in `docs/phase0/experiments/EXP-13-design.md`, section 6, "Changes to the measurement scripts". Then:

```bash
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
python3 -c "open('scripts/phase0/audio-levels.ps1', 'rb').read().decode('ascii'); print('audio-levels.ps1 is ASCII')"
grep -c 'measure_overall=RMS_level' scripts/phase0/audio-levels.ps1
python3 scripts/phase0/audio_similarity.py corpus/owner/the-office-2005-s3d3-bluray/e16-stereo/audio-levels-1-3; echo "exit=$?"
```

Expected:
- the unit tests end `Ran <n> tests` and `OK`, with `<n>` 12 more than before this step (`test_audio_similarity.py` goes from 6 tests to 18);
- `audio-levels.ps1 is ASCII`;
- `1`;
- one line ending `this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>`, and `exit=1`: the first measurement's folder is refused, with no number printed.

A failing test, a `UnicodeDecodeError`, or any measure printed for the old folder is a defect in this step: fix it before going on.

- [ ] **Step 14: Check the measurement script on the mini PC**

Which disc is in the drive: does not matter. Nothing here reads the drive or the share. Run this step only when no phase 0 job is running: `box.sh` replaces `C:\lazymkv\src`, from which jobs run.

```bash
scripts/box.sh
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \"(Get-Command C:\lazymkv\src\scripts\phase0\audio-levels.ps1).Parameters.Keys\""
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\audio-levels.ps1 -Slug Bad_Slug -Label x -Main 1 -Cand 3"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -File C:\lazymkv\src\scripts\phase0\audio-levels.ps1 -Slug the-office-2005-s3d3-bluray -Label e16-stereo -Main 1 -Cand 1"; echo "exit=$?"
ssh USER@makemkv-box "powershell -NoProfile -ExecutionPolicy Bypass -Command \". C:\lazymkv\src\scripts\phase0\env.ps1; & \$FFMPEG -hide_banner -version | Select-Object -First 1; & \$FFMPEG -hide_banner -muxers | Select-String streamhash; & \$FFMPEG -hide_banner -layouts | Select-String -SimpleMatch 'DECOMPOSITION','5.1(side)'\""
```

Expected:
- `box.sh` exits 0.
- The parameter list begins `Slug`, `Label`, `Main`, `Cand`, `Force`, followed by PowerShell's common parameters. A parse error is a defect in Step 13.
- The `Bad_Slug` run prints `RESULT FAILED:` with Assert-Slug's rejection, then `exit=1`.
- The `-Main 1 -Cand 1` run prints `RESULT FAILED: stream 1 is the main stream and cannot also be a candidate`, then `exit=1`, within seconds and before the share is touched.
- The last command prints `ffmpeg version 9.0.2 ...`, a line containing `streamhash`, a line `NAME` ... `DECOMPOSITION`, and a line `5.1(side)` ... `FL+FR+FC+LFE+SL+SR`. If no `streamhash` line appears, the ffmpeg on the mini PC cannot hash streams: stop and report. If either layout line is missing, audio-levels.ps1 cannot read ffmpeg's table of channel layouts (it would stop with `ffmpeg -layouts printed no table`, or miss the centre channel): stop and report.

- [ ] **Step 15: Choose the reference and the candidates of each title**

Which disc is in the drive: does not matter (this runs on the Mac).

For each (SLUG, LABEL) in TITLES (the Office pairs and SLUG_A first; the recordings must have been pulled with `ffprobe-streams.json` in them):

```bash
python3 - SLUG LABEL <<'EOF'
import json, pathlib, sys
sys.path.insert(0, "scripts/phase0")
from robot import parse
slug, label = sys.argv[1], sys.argv[2]
d = pathlib.Path("corpus/owner") / slug
rip = json.loads((d / label / "rip-summary.json").read_text(encoding="utf-8-sig"))
probe = json.loads((d / label / "ffprobe-streams.json").read_text(encoding="utf-8-sig"))
scan = parse((d / "scan.txt").read_text(encoding="utf-8-sig", errors="replace"))
flags = [int(a.get(22, "0") or 0) for (t, s), a in sorted(scan.streams.items()) if t == rip["title"] and a.get(1) == "Audio"]
tmdb = d / "tmdb.json"
orig = json.loads(tmdb.read_text(encoding="utf-8-sig")).get("original_language") if tmdb.exists() else None
audio = [s for s in probe["streams"] if s.get("codec_type") == "audio"]
print(f"{slug}/{label}: title {rip['title']}, {len(audio)} audio streams in the file, {len(flags)} in the scan; TMDb original language: {orig}")


def lossless(s):
    c = str(s.get("codec_name", ""))
    return c in ("truehd", "mlp", "flac") or c.startswith("pcm") or s.get("profile") == "DTS-HD MA"


groups = {}
for k, s in enumerate(audio):
    tags = s.get("tags", {})
    lang = tags.get("language", "und")
    f = flags[k] if k < len(flags) else None
    marks = [m for bit, m in ((1, "director-comments"), (2, "alt-director-comments"), (4, "visually-impaired"),
                              (512, "secondary"), (2048, "derived")) if f is not None and f & bit]
    print(f"  stream {s['index']}: {s.get('codec_name')} profile={s.get('profile')} {s.get('channels')} ch "
          f"{s.get('channel_layout')} lang={lang} title={tags.get('title')!r} scan_flags={f} {' '.join(marks)}")
    groups.setdefault(lang, []).append(s)
for lang, streams in groups.items():
    if len(streams) < 2:
        continue
    ranked = sorted(streams, key=lambda s: (-int(s.get("channels") or 0), not lossless(s), s["index"]))
    main, cands = ranked[0]["index"], sorted(s["index"] for s in ranked[1:])
    print(f"  language {lang}: -Main {main} -Cand {','.join(map(str, cands))}  (folder audio-levels-20ms-{main}-{'-'.join(map(str, cands))})")
EOF
```

Expected:
- One line per audio stream: ffprobe's stream number, codec, profile, channels, layout, language, track name, the scan's flags for that stream (k-th audio stream of the file against the k-th of the scan), and the names of the flags that are set.
- One `language <xxx>: -Main M -Cand C` line per language with two or more streams. M is the top-ranked stream: most channels, then lossless, then the earlier stream, as in spec 8.6 step 3. Measure the English line and, when the TMDb original language is not English, that language's line. Keep derived cores (`derived`) and flagged streams in C: a core is a real lossy companion of the reference, and a flag is a free label.
- For the Office disc the lines are `language eng: -Main 1 -Cand 2,3` (e16-stereo) and `language eng: -Main 1 -Cand 2,3,4` (rehearsal).

If the two counts of audio streams differ, match the streams by codec, channels and language by hand, and say so in the report.

- [ ] **Step 16: Measure each title**

Which disc is in the drive: does not matter. The job reads only the ripped file on the share.

For each (SLUG, LABEL) and each language line of Step 15:

```bash
scripts/phase0/job.sh audio-levels.ps1 LOG -Slug SLUG -Label LABEL -Main M -Cand C
scripts/phase0/wait.sh LOG
```

job.sh prints `started audio-levels.ps1; log: C:\lazymkv\phase0\logs\LOG.txt`. If it exits 1 because a phase 0 job is still running, wait for that job with its own wait.sh line first.

What wait.sh's exit codes mean for this job:
- **0**: check the log against the Expected below.
- **1**: read the `RESULT FAILED:` line.
  - Assert-Slug's rejection: SLUG or LABEL is mistyped. Check with `ls corpus/owner/SLUG`, then start again with log `LOG-2`.
  - `already exists and is kept as evidence`: this title was measured before. Its folder is the evidence: go on with Step 17.
  - `expected one .mkv in ... found 0` (with or without `the folder does not exist`): the rip is no longer in staging. Ask the owner whether the disc may go back in to rip that title again with `rip-title.ps1` under a new label (for example `e16-again`). If not, write the title down as "not measured: rip deleted" for the report, and go on with the next title.
  - `the file has no stream`, `is ... not audio`, `is the main stream and cannot also be a candidate` or `a candidate stream is listed twice`: M or C is wrong. Repeat Step 15 for this title, then start again with log `LOG-2`.
  - An error reading the share (`cannot reach the staging share`, `Cannot find path` or `Access ... is denied`): run `scripts/phase0/job.sh setup-check.ps1 setup-check-LOG` and `scripts/phase0/wait.sh setup-check-LOG`, and proceed as in Step 3.
  - `ffprobe failed`, `ffmpeg failed`, `streamhash.txt holds`, `wrote no raw-` or `holds no level`: pull the recording (`scripts/phase0/pull-recording.sh SLUG`), read `corpus/owner/SLUG/LABEL/audio-levels-20ms-M-CS/ffprobe.txt` (after `ffprobe failed`) or `.../ffmpeg.txt` (otherwise; it is written only once ffmpeg has run), and stop and report. A repeat after the fix adds `-Force` (see "When -Force is right" above).
  - Any other reason (`ffmpeg not found`, `ffprobe not found`, `ffmpeg -layouts printed no table of channel layouts`, or any other error message from the script or from PowerShell): stop and report with the log, and do not repeat the job before the cause is known.
- **3**: still running. A Blu-ray episode should take a few minutes; a feature film may take up to an hour (not yet measured). Run wait.sh again, and start nothing else meanwhile. If 90 minutes pass without a DONE line, end ffmpeg (not the job) with `ssh USER@makemkv-box "taskkill /im ffmpeg.exe /f"`, run wait.sh again (it then exits 1), and stop and report.
- **4**: the job ended without a DONE line: stop and report.
- **5**: the mini PC cannot be reached. Run wait.sh again once it can.

Expected (exit 0), in the log:
- `levels: <name>.mkv main=M cand=C centre=<True or False> ffmpeg_exit=0 seconds=<n> folder=audio-levels-20ms-M-CS`. `centre` is True for 5.1 and 7.1 references and False for mono and 2.0 ones. It is also False when ffprobe cannot name the reference's layout (its `stream M:` line shows `unknown` or `<n> channels`): the mixdown is then the dialogue curve; note such a title for the report. A reference whose `stream M:` line shows a layout such as `5.1(side)` or `7.1` with `centre=False` is a defect: stop and report with the log. `seconds` is the cost on the J4125: write it down with the title's running time.
- One `stream <n>: ...` line for M and for each candidate, with `frames=<n>` (about 50 per second of running time) and `md5=<32 hex characters>`. Candidates whose frame count is more than 1% away from M's are judged on their overlap only; note them.
- `RESULT ok`, then `PHASE0-DONE status=ok exit=<n>`.
- For the Office disc: e16-stereo streams 1, 2 and 3 have three different MD5s. In rehearsal, stream 3's MD5 equals stream 1's and stream 4's equals stream 2's, as notes.md found with an MD5 of each stream's packets.

- [ ] **Step 17: Copy the measurements to the Mac and analyse them**

Which disc is in the drive: does not matter (this runs on the Mac).

```bash
scripts/phase0/pull-recording.sh SLUG
python3 scripts/phase0/audio_similarity.py corpus/owner/SLUG/LABEL/audio-levels-20ms-M-CS | tee corpus/owner/SLUG/LABEL/audio-content-20ms-M-CS.txt
```

Expected:
- `pulled corpus/owner/SLUG (...)`.
- The analysis begins with the folder, the file name and `measured in <n> s`, then `main stream M: ...; dialogue reference: its centre channel` (or `its mono mixdown (no centre channel)`).
- Then, for each candidate, either `verdict: duplicate of stream <n>` with its `row:` line, or:
  - the lag and overlap line;
  - the `DC ... AG ... tenths ... SMI ... LS ... ADL ... added` line;
  - the `reverse:` line;
  - `verdict: <commentary | reference-unclear | ad-pattern | note-commentary | note-ad | keep>` with its meaning, and `not judged: <reason>` when row 3 of the rule applies (or only `verdict: not-judged (too short)` for a track under 20 s);
  - a `row: | ... |` line for notes.md.
- Each candidate takes a few seconds for an episode, and under a minute for a feature.
- For the Office disc, the first check of the whole chain:
  - e16-stereo stream 3 must come out `commentary`, with DC at most 0.30 and AG at least 0.50. The 16 kHz extracts gave DC 0.03, AG 0.66, tenths 10/10, SMI 0.58, LS 10.1 dB. The 48 kHz decode may differ in the second decimal.
  - Stream 2 (the derived DTS core) must come out `keep` with DC of 0.90 or more.
  - In rehearsal, stream 3 must be `duplicate of stream 1`, stream 4 `duplicate of stream 2`, and stream 2 `keep`.

  Any other result means the measurement chain on the mini PC differs from the one in the design: stop and report with both outputs. Do not change a threshold.
- If audio_similarity.py prints instead one line ending `measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>` (on standard error; it exits 1 and prints no number), the folder was not written completely by the new script: check the path, delete the empty `audio-content-20ms-M-CS.txt` that `tee` created, and go back to Step 16 for this title.

- [ ] **Step 18: Ask the owner what each measured track is, and record it**

Which disc is in the drive: does not matter.

1. For every candidate that is not a duplicate, ask the owner what it is. Give the stream as ffprobe's number and as a player shows it: with one video stream (stream 0), ffprobe's audio stream `<s>` is the player's audio track `<s>` counting from 1; with two video streams (a Dolby Vision enhancement layer), it is track `<s> - 1`. Do not show the verdict first. The owner plays the file from the staging share in a player that lists the audio tracks, at the start, the middle and near the end, and answers with one of:
   - main programme, lossy companion, derived core, alternate or original mix;
   - isolated score, M&E, same-language dub, foreign dub, alternate narration;
   - commentary, partial commentary, audio description, trivia or other speech;
   - laugh-track version, sing-along, unknown;

   with how the owner knows: listened, packaging, menu, a scan flag, or TheDiscDB.
2. A derived core (scan flag 2048) and a duplicate by MD5 need no listening: their label is "derived core" or "duplicate".
3. Then append, to `corpus/owner/SLUG/notes.md` (never replacing what is in it), one section per disc, with one row per measured track:

```markdown
## Experiment 13 (Task 12)

Extra audio tracks measured against the top-ranked track of their language (`scripts/phase0/audio_similarity.py`;
full output in `<label>/audio-content-20ms-<M>-<C>.txt`). The owner named each track before seeing the measures.
Reference streams: <label> stream <M> (<codec> <channels> ch <language>), ...

| File | Track | What the measures say | What the owner says it is |
|---|---|---|---|
| <label>/<file name> | <n>: <codec> <channels> ch <language> "<track name>", scan flags <f> | DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x>: <verdict> | <kind> (<how the owner knows>, <date>) |
```

   The middle column is the `row:` line's measures and verdict. A duplicate's middle column is `same packets as stream <n>: duplicate`.
4. For the Office disc, e16-stereo stream 3's owner column is `commentary (listened, 2026-10-05)`. That is already recorded under "Owner's ground truth".

- [ ] **Step 19: Add discs until the experiment's kinds are covered**

Which disc is in the drive: the disc being added, which the owner inserts.

1. Count the labelled tracks so far against the list in the experiment-13 row (the design, section 5, "Experiment 13"). Leave out every track of the Office disc (`the-office-2005-s3d3-bluray`): the thresholds were fitted on two of its tracks, so none of its tracks counts towards any total and it is not one of the discs counted. Count:
   - commentaries (and how many discs, feature films, and over clearly audible film they come from);
   - audio descriptions;
   - programme tracks other than references, and how many of them are separately encoded lossy companions or alternate mixes;
   - a title whose reference has no centre channel;
   - a dialogue-heavy or speech-only title;
   - isolated scores, M&E tracks, second dubs or narrations.
2. Ask the owner, once per missing kind, for a disc that has it (the design's question 12). Also ask for the Office disc again (the design's question 13): its title 3 should carry a second commentary, and titles 2, 5 and 6 duplicates. Its tracks are measured and reported like any other, and a programme track of it with the commentary verdict still makes the answer no, but they count towards no total (point 1).
3. For each disc the owner provides:
   - Run Step 6 (fingerprint, record if new, pull, `summarize_scan.py`). For a disc already recorded (the Office disc), skip the recording.
   - List the disc's titles with their audio streams by language, derived streams (scan flag 2048, MakeMKV's cores) left out:

     ```bash
     python3 scripts/phase0/audio_similarity.py --titles corpus/owner/NEW
     ```

     Expected: a first line `NEW: <n> titles, longest first; TMDb original language: <code or unknown>`, a line `audio: scan stream numbers by language, ...`, then one line per title: its number, running time, segments and `audio <language> <stream numbers>; ...`, and at the end `<- <language> x<n>` when one language has two or more streams, `ripped as <label>` for a title ripped before, and `shares segments with title <t>` for overlapping playlists.
   - Choose at most 4 titles of the disc, going down the list (longest first) and taking a title only when all of these hold:
     - its `<-` mark names `eng`, or the original language of the first line (TMDb gives two letters; the scan uses the three-letter code, for example `fr` is `fra` or `fre`);
     - it is a feature or an episode, not an extra (when unsure, ask the owner);
     - it has no `ripped as` (other than a failed rip), and it shares no segment with a title already taken (a play-all playlist).

     On a film disc this is normally the feature alone. For the Office disc it is titles 3, 5, 2 and 6 (question 13): titles 0 and 1 are `ripped as` e16-stereo and rehearsal, and title 4 (two segments, no subtitles, only stereo tracks) is taken to be an extra.
   - For each chosen title T, run Step 7's rip and probe with the label `tT-all-tracks`: `scripts/phase0/job.sh rip-title.ps1 rip-NEW-tT -Slug NEW -Title T -Label tT-all-tracks`, then `probe-title.ps1 ... -Label tT-all-tracks -SkipRemux`. Use Step 7's exit-code handling and its rules for `-Force`.
   - Pull the recording.
   - Add (NEW, `tT-all-tracks`) to TITLES and run Steps 15 to 18 for it.
4. Stop asking when every kind is covered, or when the owner has no disc left of a missing kind. Write each such kind down as "not available: <kind>".

- [ ] **Step 20: Judge the experiment and write the report**

Which disc is in the drive: does not matter.

1. Build the coverage table and the track table from the notes.md sections, with no threshold changed. The coverage table and every total leave out each track of the Office disc (`the-office-2005-s3d3-bluray`), whose S03E16 commentary and S03E15 duplicate the thresholds were fitted on; the track table lists them, marked "fitting disc, not counted".
   - **False verdict:** a track with the verdict `commentary` that the owner names as programme audio (main programme, lossy companion, derived core, alternate or original mix, isolated score, laugh-track version), or as an ask-kind the owner wants kept (the design's questions 2 to 6). This includes the tracks of the Office disc.
   - **Answer yes:** every kind in the row is covered without any track of the Office disc, there is no false verdict, and at least one commentary got the verdict on each of at least 3 different discs other than the Office disc.
   - **Answer no:** any false verdict, on any disc, whatever the coverage. Also no when coverage is met but fewer than 3 discs other than the Office disc had a commentary verdict.
   - **Not answered:** coverage is not met and there is no false verdict.
2. Create `docs/phase0/experiments/EXP-13-commentary-by-audio.md` from `docs/phase0/experiments/TEMPLATE.md`. Header:
   - the date;
   - every disc measured, with slug, format and the titles;
   - the MakeMKV version;
   - `section 17, row 13 (added by this experiment)`.
3. Right after the header, put `## Coverage`:

```markdown
## Coverage

| Kind | Needed | Labelled tracks (discs), without the Office disc |
|---|---|---|
| Commentary | 8 from 4 discs; 2 on feature films; 1 with the film clearly audible | <n> (<discs>); films <n>; audible film <n> |
| Audio description | 1 if the owner has one (up to 3) | <n> or "owner has none" |
| Programme tracks other than the reference | 20, of which 6 separately encoded lossy companions or alternate mixes | <n>; separately encoded <n> |
| Reference without a centre channel, with an extra track | 1 title | <n> |
| Dialogue-heavy or speech-only title, with an extra track | 1 title | <n> |
| Isolated score, M&E, second dub or narration | every one the owner has | <n> |

- not available: <kind>, for each kind the owner could not supply.
- not counted: every track of the-office-2005-s3d3-bluray (the thresholds were fitted on its S03E16 commentary and S03E15 duplicate).
```

4. The report must state, under its template headings:
   - **What was run:**
     - for each title, the rip label, M and C, the `audio-levels.ps1` command and log name;
     - the `seconds=` of each run, with the title's running time and file size: the J4125's cost per minute of running time for an episode and for a feature;
     - the `audio_similarity.py` command.
   - **What was observed:**
     - one table row per measured track: file, track, DC, AG, tenths, SMI, LS, ADL, reverse, verdict, the owner's label, and whether the two agree;
     - every duplicate found by MD5;
     - every track the rule did not judge, with the reason;
     - for each owner label, how many tracks got each verdict;
     - stereo width is not measured; the earlier figures (commentary -41.8 dB by notes.md, -30.3 dB whole-file; duplicate -3.4 dB) are quoted as observations only.
   - **Answer:**
     - "yes", "no" or "not answered", by the rules above;
     - every false verdict, with its measures;
     - the commentaries found and missed (the recall), with the kind of each miss: film audible, sparse, partial, too few gaps;
     - what the audio-description tracks got. AD is never dropped by content in this version, whatever it got.
   - **Consequence for the spec:**
     - on yes: add row 13 to section 17; in 8.6 step 1, add the duplicate row and the content row behind `drop_commentary_by_audio`; in 8.8, add the review reasons `commentary_detected` and `audio_reference_unclear`; in section 14, add the setting; in 6.4 step 10, the whole-title decode with its measured cost; change D12 and R6 to say that content may confirm a commentary;
     - on no or not answered: the same spec text, except that the content row only flags for review (`commentary_detected`) and never drops; D12 changes accordingly;
     - in every case: the duplicate row, and the open questions still open.

- [ ] **Step 21: Commit** (replaces the old Step 13)

Which disc is in the drive: does not matter. Disc A is needed again in Task 13, which starts by asking the owner to put it back.

```bash
find corpus/owner -iname '*unit_key*'
grep -rIl 'eyJhbGciOi' corpus/owner
python3 -m unittest discover -s scripts/phase0 -p 'test_*.py'
git add scripts/phase0/probe-audio.ps1 scripts/phase0/audio-levels.ps1 scripts/phase0/audio_similarity.py scripts/phase0/test_audio_similarity.py corpus/owner docs/phase0/experiments
git status --short
git commit -m "Run experiments 6 and 13: audio profiles, subtitle alignment, commentary by audio content" -m "Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Expected:
- `find` and `grep` print nothing: no AACS unit-key file and no JSON Web Token in any recording. If either prints a path, stop and do not commit.
- The unit tests end `OK`.
- `git status --short` lists:
  - the four scripts;
  - the new files under `corpus/owner/`. These are what the old Step 13 listed, plus each `audio-content-20ms-*.txt`, each `audio-levels-20ms-*/streams.json`, the new `tT-all-tracks/` recordings and the `notes.md` changes. No `audio-levels-*/*.txt` file appears: `.gitignore` keeps them out.
  - both reports.
- The commit succeeds.

## 8. Open questions

Only more discs or the owner can answer these.

1. **Does the rule hold beyond one commentary recording?** Every threshold sits between one real two-person sitcom commentary (and constructions made from it) and constructed programme tracks. Film commentaries are untested: solo, pausing, over a scored film, partial, several on one title. Experiment 13 needs at least 8 commentaries from 4 discs before a yes.
2. **What does real audio description look like?** No real AD track has been measured. Every stand-in is the commentary voice laid into gaps found by the measures' own definition. Real describers speak short phrases, sometimes over music and over the edges of dialogue. Until real AD is measured, AD is dropped only by a flag or by TheDiscDB, and content gives at most a note.
3. **Same-language re-dubs and alternate narrators.** A single dense narrator voice over the programme's M&E, with a reference that has no centre channel, got the commentary verdict (aq-A2n-noFC, aq-A2n640-noFC). With a centre channel it got only a note, and the attack showed that this rests on one tenth of the episode. Envelopes cannot tell this from a commentary: in a real commentary, the film bed under the voices still carries the programme's dialogue, but it is too faint to see in loudness curves. The relation lens's waveform anchoring could test that, but it needs PCM rather than small text files, so it is a later Rust feature. First, the owner's policy (question 3) and whether such tracks exist on the owner's discs.
4. **Is the reference always the programme?** Spec 8.6 ranks channel count first. A 2.0 isolated score, or a lossless 2.0 commentary, outranks a 1.0 mono film; G1 only guards lossy tracks at 256 kb/s or less. The rule's silence and gap checks refused every such constructed case, and question 10's position veto is the backstop. Separately, the same ranking would make such a track the default audio. That is a defect in 8.6 to raise even without this experiment.
5. **Isolated scores, M&E tracks, laugh-track pairs, alternate masters.** All the music came from one 18-second theme or from synthetic loops. Synthetic staccato and pizzicato in the gaps got the audio-description pattern (a note), because SMI does not tell rhythmic music from speech. Real tracks may do better or worse.
6. **Films with few dialogue gaps.** A film whose centre channel carries continuous score or effects may never reach 90 s of gaps. The rule then never judges, so its commentaries stay in the file. Experiment 13's feature-film titles will show how often this happens.
7. **Cost on the J4125.** Nothing has been measured for the 20 ms chains, for the decode of TrueHD or DTS-HD MA, or for a whole UHD file over the share. Experiment 13 records `seconds=` per title. Whether 6.4 step 10 can become one ffmpeg pass that also extracts the subtitle evidence still has to be tried.
8. **Review or note.** Spec 8.8 holds the whole job for any review reason. The owner decides whether `commentary_detected` holds the job until experiment 13 is answered, or only adds a note (question 8), and the same for the audio-description pattern (question 9).
9. **What a yes is worth.** Zero false verdicts in 20 programme tracks bounds the false-verdict rate only below about 15% per track. A drop by content therefore stays behind a setting, with the raw rip kept as the undo and the dropped track named in the notification. Alternatively, a confirmed commentary is labelled rather than deleted (question 11).
