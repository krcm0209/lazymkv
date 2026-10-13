# Experiment 13: commentary and audio description by audio content (design)

- Date: 2026-10-05
- Status: design for the owner and the controller. Committed copy; the scratch folders it names were temporary and are not kept. Nothing in the spec, the plan or the scripts is changed by this file; section 6 gives the script contents and section 7 the plan steps.
- Built from three independent proposals (the "relation", "content" and "requirements" lenses), the adversarial attack on each, and a re-computation, by the synthesis, of every number quoted below: the two real tracks and 153 constructed runs, 155 runs in all. Scratch (a session folder, not kept; no reader can open it): `/private/tmp/claude-502/-Users-USER-code-lazymkv/d9f40c13-92fa-4abc-ab12-bdb22c67ac55/scratchpad/exp13-work/synth/` (`synth.py` the measures, `cases.py` the 127 pairs, `extra.py` 28 variants, `sweep.py` the sensitivity runs, `out-final.json`, `extra.json`, `sweep.log`, `repo/` the three proposed files).

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

13. **Reverse test (decides, keep-only).** The same analysis with the roles swapped: the candidate as reference, its own mixdown as dialogue curve, the reference as candidate. If the swapped run has at least 90 s of gaps and AG >= 0.30, the reference also adds sound to the candidate. A commentary pattern is then not confirmed: the pair is two different programmes, misaligned, or the reference is not the programme. Measured: the real commentary has 29 s of its own gaps (swapped run not judged). Unrelated programmes and a 7-minute misalignment score swapped AG 0.37-0.52. The test has not changed a verdict in any of the 155 runs: it is a spare guard that can only keep. Replaced by section 9.5's item 13 (the reverse commentary pattern) and item 14 (the peer run).

### Tried and rejected

- **Stereo width (side minus mid).** Fails both ways (see the table at the top): a mono film's companion is the narrowest track, and AD or commentary over a stereo mix is wide. It is recorded as an observation only, never required, as the owner decided. The new script does not measure it.
- **Positional or slot rules** ("lossy stereo in the slot where siblings carry a duplicate"). Rejected by the owner (binding constraint 2). The only place position may enter is a keep-only veto, and only with the owner's approval (question 10). (Allowed by the owner on 2026-10-06, decision D21: it is row 0 of section 9.5.)
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

> Replaced in part on 2026-10-06: the Scope paragraph, rows 3 to 5, the new rows 0 (the position veto, owner decision D21) and 4b and the "Reverse test" threshold row are in section 9.5.

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
- **A reference that is not the programme** (an isolated score or a lossless commentary that outranks the programme because 8.6 ranks channel count first). Silence in more than 10% of frames, or less than 90 s of gaps, refuses a verdict: every such run was kept, at most with a note. A continuous non-programme reference, an M&E track ranked first, gave a note at worst (DC 0.37, AG 0.42). Question 10's position veto is the backstop, and only the owner can allow it. (Allowed by the owner on 2026-10-06, decision D21: it is row 0 of section 9.5.)

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
git commit -m "Run experiments 6 and 13: audio profiles, subtitle alignment, commentary by audio content"
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

## 9. Addendum (2026-10-06): which track is the reference, and what replaces the reverse test

- Date: 2026-10-06
- Status: amendment to this design for the owner and the controller, revised the same day after an adversarial review (9.10 lists every finding and what was done with it). It changes rows 3, 4 and 5 of the decision rule (section 3) and adds a verdict for a candidate in another language than the reference (row 4b), the measure "reverse test" (section 2, item 13) and adds the peer run (item 14), the choice of the reference (section 2 and the plan's Task 12 Step 15) and the harness. No threshold of the commentary pattern changes. The scripts it describes (`final/` below) were installed in `scripts/phase0/`, and the plan changes of 9.8 applied, in the commit that added this section. The review-fix commit after it gave the spec's section 17 row 13 the wording of 9.5; D15 waits for the owner (9.9, question 7), and 8.8 lists neither `commentary_detected` nor `audio_reference_unclear` yet (Task 12 Step 20 adds them).
- Built from: real level folders rebuilt from the three proxies of the tuning dataset (`proxy_levels.py`), the two 16 kHz extracts of the first measurement, the designer's runs (89 tabled in 9.4), and the review's 232 level folders (260 runs: harder dub stand-ins, programme copies in another language's slot, and references that are not the programme). Every run was analysed again with the final rule. Scratch (a session folder, not kept; no reader can open it), under `/private/tmp/claude-502/-Users-USER-code-lazymkv/d9f40c13-92fa-4abc-ab12-bdb22c67ac55/scratchpad/exp13-ref-work/`:
  - the designer's: `tools/cases.py` the constructions, `tools/build.py` the level chain on constructed signals, `tools/runrule.py`, `tools/inspect_rev.py`, `tools/coef.py`, `tools/mdtables.py`, `levels/` the 73 folders, `results-orig.jsonl` (committed rule) and `results-new.jsonl` (the rule as first delivered);
  - the review's: `attack/tools/acases.py` (dub stand-ins), `attack/tools/wcases.py` (references that are not the programme), `attack/argscases/` (11 stream layouts for `args`), `attack/levels/`, `attack/out/`;
  - the revision's: `fix/repo/` (the repository's scripts with `final/` laid over, corpus and docs linked), `fix/tools/frun.py` (a rule over folders), `fix/tools/tables.py` and `etable.py` (9.4), `fix/tools/mutate.py` (40 mutations), `fix/c2/` (the corpus copy the harness ran on), `fix/levels-x/` (the review's English-reference dub folders retagged as dubs of a Japanese original), `fix/out/` (`now.jsonl` and `now-retag.jsonl`: the final rule over every folder, each row also carrying `base_verdict`, the delivered rule's verdict, which differs from the final one only by row 4b and the peer run, since the measures are the same; `etable.txt`, `tables.md`, `mutate.txt`, `suite.txt`, the harness, report and `args` outputs).

**What the real discs showed.** Three real commentaries, each labelled by the owner, none marked by a flag, a name or TheDiscDB:

| Track | DC | AG | Tenths | SMI | LS (dB) | Reverse (old test) | Verdict of the rule as designed | Final rule |
|---|---|---|---|---|---|---|---|---|
| The Office S03E16 (Blu-ray), AC-3 2.0 English over English DTS-HD MA 5.1 (listened; the thresholds were fitted on it) | 0.03 | 0.66 | 10/10 | 0.58 | 10.1 | 29 s of gaps: not applied | commentary | commentary |
| The Office S03E12-13, same kind of pair (listened) | 0.17 | 0.46 | 6/10 | 0.59 | 12.4 | 124 s, AG 0.20 | note-commentary (a miss: AG < 0.50, 6/10 tenths) | note-commentary (the same miss) |
| Pan's Labyrinth (UHD), AC-3 2.0 **English** commentary (del Toro, packaging) over the **Spanish** DTS-HD MA 5.1 | 0.08 | 0.54 | 9/10 | 0.51 | 10.8 | 470 s, AG 0.31 | reference-unclear (blocked by the reverse test); and with the reference chosen per language, as designed, never judged at all | commentary-cross-language (kept and flagged for review; never dropped by content: 9.2) |

They expose two parts of the design:
- **A. The reference.** A candidate was compared with the top-ranked track of its own language. An English commentary on a Spanish film has no English reference, so it was never judged and was kept silently. Commentaries and audio descriptions do not belong to a language; the programme does.
- **B. The reverse test.** It was meant to refuse a verdict when the reference is not the programme (an M&E track or an isolated score ranked first). On Pan's Labyrinth it blocked a true commentary. Section 9.3 shows why it fires on single-speaker commentaries over a ducked film, and that it never fired on any non-programme reference built here.

Fixing A creates a new risk, which the review showed is real: a dub in a wanted language (an English dub of a foreign film) becomes a candidate against the original, and level curves cannot tell a loosely timed dub from a commentary (9.2). The final rule therefore judges such a candidate but never drops it by content.

### 9.1 The reference (problem A)

**The rule.** For every title, one programme reference:
1. the track that spec 8.6 places first (step 3's ranking: channels, then lossless, then position; with step 4's guards G1 and G2) among the streams of the work's **original language** (TMDb), when the title has a stream in it;
2. otherwise the track 8.6 places first among the **English** streams (original language English, unknown, none, or absent from the disc);
3. otherwise (no stream in a wanted language) the top-ranked stream, for the dataset only: the app has nothing to judge (`no_wanted_audio`).

When the reference so chosen is not the first stream of its language on the disc and that first stream is lossless or has more than two channels, the choice may be inverted (a score or a commentary ranked above the programme): the position veto, row 0 of 9.5 (owner decision D21, 2026-10-06), then keeps every candidate of the title with the review reason `audio_reference_unclear`.

**The candidates.** Every other stream in a wanted language (English and the original language, R4), whatever its language. An English commentary on a Spanish film is judged against the Spanish main track; so is an English dub, an English 2.0 companion of that dub, and English audio description. A candidate in another language than the reference is judged but never dropped by content (row 4b, 9.2). In the app the candidates are those that survive 8.6 step 1 (derived cores, flags, TheDiscDB descriptions and unwanted languages are dropped first, as today), and streams with no language code are not judged (they stay as 8.6 says). Phase 0 keeps derived cores, flagged streams and streams with no language code in the candidate list, because they are free labels and free data for the dataset.

Phase 0 also measures, for the dataset only, each language that is **not** wanted and has two or more streams: its first-placed stream against its other streams (the per-language rule of before, where it costs only one more chain). The app never judges these.

**How the harness applies it** (`audio_dataset.py`, `final/`):
- `original_language(slug_dir)` reads `tmdb.json` and maps TMDb's two-letter code through `audio_similarity.ISO639`: every ISO 639-1 code with its ISO 639-2 terminology and bibliographic forms, as ffmpeg 8.1.2's own table pairs them (190 codes, among them the deprecated aliases `in`, `iw`, `ji`, `jw`, `mo` and `sh`; ffmpeg pairs `sh` with no three-letter code, so `hbs`, `srp`, `hrv`, `bos` are added by hand, and `mol` is added to `mo`), widened for `no`, `nb`, `nn` (Norwegian `nob`/`nno`) and `tl` (`fil`), plus TMDb's own `cn` (Cantonese: `zho`, `chi`, `yue`): 191 entries. TMDb's `xx` (no language) makes English the reference language, and `args` says `original language none (TMDb xx: ...)`. A code still missing, or a missing `tmdb.json`, also makes English the reference language, and `args` says `original language unknown (...)` on its first line.
- `placed_first(streams)` is spec 8.6's `rank()` plus G1 and G2 as far as ffprobe's facts go. It now lives in `audio_similarity.py` (the rule's own ranking, which the peer run below also uses); `audio_dataset.py` imports it. `streams.json` carries no bit rate, so G1 ("a lossy track at 256 kb/s or less never before a lossless track of its language") is approximated as "a lossy track of two channels or fewer": AC-3/AAC stereo commentaries are 192-256 kb/s, and a lossy 5.1 track on DVD or Blu-ray is normally 384 kb/s or more. G2 is applied without its "no later criterion" clause: a lossy stereo top-ranked track that is not its language's first stream gives way to that first stream when the first has fewer channels. Both are keep-safe: they can only make the first-on-disc or lossless track the reference.
- `levels_args` prints one job for the programme reference with every wanted or no-language stream as `-Cand` (or the reference alone), then one dataset-only job per unwanted language with two or more streams. It adds a note line in three layouts where the reference may not be the programme, and Step 15 then says so in the report:
  - `note: the file has no stream in the original language (...)`: Disc A's `eng-only` rip, whose only audio stream is the English commentary, which `args` measures alone, for the dataset, with nothing to judge;
  - `note: the reference is not the first <lang> stream of the file (stream <k>, ..., comes first)`, when that first stream is lossless or has more than two channels: a 2.0 isolated score ranked over a mono film (9.3), or a Mandarin 5.1 dub ranked over a lossless or multichannel Cantonese original when both are tagged `chi` (a lossy 2.0 original gets no note);
  - `note: the reference is a lossy stream of 2 channel(s), and stream <k> with no language code is lossless or has more than 2 channels`: an `und`-tagged main beside an English commentary.
- `reference_problem` accepts a folder whose main is the programme reference, or the first-placed stream of an unwanted language (a dataset-only folder). A folder measured against the top English stream of a Spanish film is now `wrong reference`. None of the six committed level folders (five titles) changes: all six are measured against the programme reference.
- `audio_similarity.py --titles` adds the mark `<- wanted x<n> (eng 1, spa 1)` when no wanted language has two streams but the wanted languages have two or more together. Pan's Labyrinth's feature had no mark before (spa 1, eng 1 after the derived core) and would have been skipped by Step 19.

**Why the original language rather than the top-ranked track overall.** Both normally pick a programme track. The original-language track is the programme by definition; a commentary or an audio description is never the reference of a title that has an original-language track, whatever its language and whatever spec 8.6's channel ranking says. It is also the track that 8.6 step 5 makes the first and default track under D9's default (`original_language_first` on). A lossless 7.1 English dub that outranks a Spanish 5.1 original is then a candidate, judged like a dub (9.2), instead of the reference.

**The peer run (adopted after the review).** The first version rejected a second, per-language opinion because it "needs a second decode of the title" and "a combined verdict across folders". Both were wrong: `args` already puts every wanted-language stream into the one pass, so each candidate's `cand-<n>.txt` is its mixdown curve, and a second opinion is one more pure-Python analysis inside the same folder, on the path already validated without a centre channel. The final `results()` does it: a cross-language candidate with the commentary pattern is analysed once more against the stream that `placed_first` puts first among the folder's candidates of its own language (duplicates left out), with that stream's mixdown as the dialogue curve. If that peer run has no commentary verdict, the candidate follows a programme track of its own language (the 2.0 companion of a dub follows the dub: DC 1.00 against it), and its verdict is only `note-commentary`. The report prints a `peer:` line, and the harness keeps the peer run in its snapshot. It protects companions, not the lone dub: the stream placed first in its language has no peer. Measured: the review's folder `S-T-dub` (a dub stand-in and its compressed, 40 ms late companion): the dub stand-in (stream 11: DC 0.01, AG 0.60, 10/10 tenths, SMI 0.57 against a Japanese-tagged original) is placed first among the English streams, has no peer and gets `commentary-cross-language`; its companion (stream 12: DC 0.01, AG 0.64, 10/10, SMI 0.55, LS 8.4) gets `note-commentary`, because its peer run against the dub has DC 1.00 and AG 0.00 (`keep`). The delivered rule gave both `commentary`.

**Declined after the review: ranking English and no-language streams together.** The review found that an `und`-tagged lossless main loses the reference to a lossy stereo English stream (a commentary), and proposed ranking English and no-language streams together when the original language is English or unknown. That would also let a `zxx`-tagged isolated score (ISO 639-2's code for "no linguistic content", the correct tag for a score) with more channels become the reference over the English main: the failure of 9.3, made more likely. It is not adopted. In the app, no-language streams are never judged, so the `und` main is never at risk of a content drop. In phase 0, `args` prints the note above, and the programme judged against a commentary as reference gets `reference-unclear` or a note (9.4 table B; the review's `W-*-commref` folders), never `commentary`; once the owner labels the reference, the harness fails loudly with `the reference is labelled commentary (unwanted audio)`.

**Two languages under one tag.** Cantonese and Mandarin both tagged `chi` (TMDb `cn`) are one language to this rule: a Mandarin 5.1 dub outranks a Cantonese 2.0 original, becomes the reference, and the original is judged against it as a same-language candidate (no cross-language mark). `args` prints the first-of-language note for that layout only when the first `chi` stream (the Cantonese original) is lossless or has more than two channels; with a lossy 2.0 original it prints none. Such a track is the `same-language-dub` or `foreign-dub` kind (ask the owner) until labelled.

### 9.2 What protects a dub in a wanted language

A dub in a wanted language is programme audio (R3, R4). The labels vocabulary gains the kind `dub` (group programme, wanted keep): a drop verdict on it, or the commentary pattern in any form, is a `FAIL`, like any programme track. `foreign-dub` keeps its meaning: a dub whose tag names a wanted language it is not in (section 1, question 4).

**What the first version claimed, and what the review showed.** The first version measured 29 dub stand-ins (9.4 table C: the reference's own M&E plus its centre channel pitched or frequency-shifted, or other words gated to the original's windows) and found DC useless (same words: 0.93-1.00; other words: 0.02-0.27) but AG (at most 0.42), the tenths (at most 7/10) and LS (15.3-23.7 dB) far from the pattern. It concluded that tuned numbers protect a dub. The review built harder stand-ins (`attack/tools/acases.py`), and they break that conclusion:

| Dub stand-in (the review's; P Pan's Labyrinth, E S03E16, T S03E12-13; each with and without the centre curve) | Runs | Commentary pattern under the delivered rule | The runs with the pattern |
|---|---|---|---|
| dry voice, lines 150 ms late, 300 ms release | 20 | none (10 notes, 10 keep; AG 0.11-0.51) | - |
| dry voice, lines 250 ms late, 800 ms release | 20 | 13 (T 8 of 8, E 4 of 8, P 1 of 4) | together: DC 0.01-0.05, AG 0.53-0.77, 9 or 10 of 10 tenths, SMI 0.52-0.57, LS 6.1-10.7 dB, ADL 0.21-0.29 |
| dry voice, lines 250 ms late, 1200 ms release | 20 | 15 (T 8 of 8, E 4 of 8, P 3 of 4) | (as above) |
| film-dialogue voice (the same title's centre further on), the same grid | 60 | none (37 notes; AG at most 0.53) | - |
| loud at its own timing, lines running 1.2 s long, added off-screen lines, the same words at -7 or +7 semitones, jittered timing, its own score mix | 52 | none (34 notes) | - |

Of all these runs, 13 that miss the pattern miss it by one condition only: SMI 0.38-0.45 (the loud voice at its own timing, L2), LS 13.1-13.3 dB, 7 or 8 of 10 tenths, or AG 0.49. The same words at -7 or +7 semitones keep DC 0.72-0.99 (keep), and the programme placed in another language's slot (dynaudnorm, a dialogue-boosted AC-3 copy 120 ms late, a band-limited MP2 mono copy 300 ms late) keeps DC 0.94-1.00 and AG 0.00 (keep, 18 runs).

The stand-ins that get the pattern use a dry, conversational voice (another title's commentary track) gated into the original's dialogue windows 250 ms late, with an 800 or 1200 ms release, so that each line runs about 0.5-1 s past the original's. On a dialogue-dense title most dialogue gaps are short, and that run-on fills them: AG rises to 0.53-0.77, in 9 or 10 of 10 tenths. These stand-ins sit further inside the pattern than Pan's real commentary does (AG 0.544, 9/10 tenths, SMI 0.511), so no value of AG_MIN, the tenths share or SMI separates them without also losing Pan's. Nor does any other measure: ADL is higher on these dubs (0.21-0.29) than on the three real commentaries (0.09-0.18), and measuring only the interiors of gaps (`attack/tools/trim_sweep.py`) costs Pan's a tenth while the T dub with 1.2 s of run-on keeps the pattern. The old reverse test would not have caught them either (reverse gaps 43-142 s, reverse AG 0.02-0.28).

**The rule that follows: a candidate in another language is never dropped by content.** It does not rest on a tuned number. A candidate whose language is not the reference's (both tags name a language, and they differ) and that has the commentary pattern gets the verdict `commentary-cross-language`: keep and flag for review (`commentary_detected`), never `commentary`, so the setting `drop_commentary_by_audio` does not apply to it, before or after experiment 13's yes. The peer run (9.1) turns it into `note-commentary` when the candidate follows the first stream of its own language. On a programme track (`dub`, a lossy companion of a dub) this verdict is still a false verdict in the harness: the pattern fired on programme audio, and that is the evidence that keeps the restriction in place.

What it costs: a commentary in another language than the programme (Pan's Labyrinth's, and any English commentary on a foreign film) is never dropped by content; it is flagged for review and kept. That is the cheap error (R6/D12: a missed commentary only wastes space). The owner can still drop it by hand after the review, and a flag or a TheDiscDB description still drops it at 8.6 step 1.

What would lift it: real dubs. The row-13 proposal (9.5) asks for at least 3 dubs in a wanted language from at least 2 discs, at least 1 of them on a dialogue-heavy title, measured with no commentary pattern, and then the owner's decision. Three clean dubs bound little (rule of three: a rate under about 63%); they are a floor before the question may be asked, not evidence that it is safe. `audio_labels.exp13_requirements` reads those numbers when row 13 has the phrase, and the harness prints the dub count against them.

**What is still not protected: a dub tagged with the reference's language.** The rule can only see tags. A dub that carries the original's tag (Cantonese and Mandarin both `chi`), a mis-tagged dub (`foreign-dub`), or a second dub in the same language (`same-language-dub`) is a same-language candidate and can get the plain `commentary` verdict. Run as the review built them (the S03E16 and S03E12-13 stand-ins carry the English reference's own tag), the final rule gives the plain `commentary` verdict to 24 of the 48 S03E16 and S03E12-13 dry-voice runs, the same 24 that get `commentary-cross-language` once retagged as dubs of a Japanese original; Pan's four carry a Spanish reference and get `commentary-cross-language` as built. These are the ask kinds of section 1's questions 3 and 4, which row 13 already requires the owner to bring ("every ... second same-language dub ... the owner has"); a yes cannot be reached while one of them has the commentary verdict and the owner wants it kept.

**How far the stand-ins go.** They show mechanisms and directions, not margins on real dubs:
- the music and effects are exactly the reference's own (mixdown minus 0.2929 times its centre; 0.2929 is ffmpeg's weight of FC in a 5.1-to-mono downmix, measured on a sine and on the proxies). This also removes centre-panned music and effects. The Office's 5.1 mix is almost all centre, so its "M&E" is room tone and its dubs are new dialogue over near-silence, the hardest case for AG;
- the review's dry voice is another title's commentary track, conversational and close-miked, gated into the original's windows: an upper bound on how dry a real dub voice is; the film-dialogue voice (the same title's centre further on) is the lower bound;
- the gate follows the original centre channel's level, a crude stand-in for lip sync that also opens on centre music and effects;
- nothing models a dub's own mix, loudness or compression beyond the review's compressor, a booth recording with reverb added, line lengths that differ from the original's, added lines, breaths and walla in the original's pauses, broadcast compression, or the syllable rates of other languages. A real dub may sit nearer to or further from the pattern than any stand-in.

### 9.3 The reverse test, and what guards against a non-programme reference (problem B)

**What the old test measures.** The swapped run (the candidate as reference, its mixdown as dialogue curve, the reference as candidate) asks whether the reference adds sound in the candidate's own gaps (90 s of them, AG at least 0.30). A commentary's own gaps are its pauses. In Pan's Labyrinth (`tools/inspect_rev.py`) they are 834 breath pauses of 0.5 s median (P90 0.9 s, longest 2.8 s), 470 s in all. In them the programme is louder than the commentary track by 6.3 dB at the median and by 16.1 dB at the 90th percentile (track levels: median -55.0 dB against -60.1 dB): the film is lowered or quiet under the voice. In 31% of those frames the programme counts as added (10 dB above what the gain model predicts, in a window that does not follow). That is what any single speaker over a lowered film does once the pauses add up to 90 s, which a feature-length solo commentary does easily. The dense two-person Office commentaries pause too little (29 s and 124 s).

A construction shows that this is not Pan's alone: the S03E16 commentary track, looped over Pan's mixdown, with the voice stopping 0.6 s in every 4 s (P-breath, 9.4 table D). The forward run keeps the pattern throughout (AG 0.52-0.54, 9/10). The reverse AG rises with how far the film is lowered: 0.00 at -6 and -12 dB, 0.28 at -20 dB, 0.40 at -26 dB, 0.48 at -32 dB. From about -26 dB the old test blocks a commentary.

**And it never caught what it was for.** Against every stem-like reference built here (an M&E stand-in, a front-left channel), the old test's reverse AG is 0.00: the programme taken as reference finds nothing added by them, because they are inside the programme (table B: P-ME>1, P-ME>2, E16-ME>1, E16-ME>2, T3-ME>1, T3-ME>2, R2-FL-swap). On the review's 30 false commentary verdicts (28 dub stand-ins, 2 programme runs against a score) the old test would have fired on none either (reverse gaps 43-243 s, reverse AG 0.00-0.28). "The reference is a stem and the candidate is the programme" and "the reference is the programme and the candidate is a commentary with the film underneath" have the same shape in level curves: the candidate is the reference plus speech. No test of the pair alone can separate them; only the nature of the added sound (spread, modulation, level spread) and the choice of the reference can.

**The replacement: the reverse commentary pattern.** The reverse run is kept whole (every measure, as `rev_<name>`), and it is judged with the forward rule itself: if the reverse run passes the judgeability checks on its own measures (overlap, silence, 90 s of gaps, a dialogue window) and meets the commentary pattern (DC <= 0.30, AG >= 0.50, 6 tenths with 90% at AG >= 0.40, SMI >= 0.50, LS <= 13 dB, added >= 60 s), then the reference looks like a commentary over the candidate: one of the two is not what its rank says. The verdict is `reference-unclear` (keep, review `audio_reference_unclear`), whatever the forward run says, including when the forward run cannot be judged.
- It has **no number of its own**: it uses the pattern's thresholds, so it moves only when they move, and it cannot be tuned to a case.
- It is **symmetric**: a pair can get the commentary verdict in at most one direction, and a pair that would get it in both (two unrelated programmes, two commentaries, a misaligned copy that looks like speech both ways) gets neither.
- On the three real commentaries the reverse runs miss the pattern by wide margins: Pan's AG 0.31, 1/10 tenths, SMI 0.39, LS 17.1; S03E16 has 29 s of gaps (not judged); S03E12-13 AG 0.20, 0/8.
- It **catches a commentary taken as the reference**: the programme judged against its own commentary gets `reference-unclear` on Pan's Labyrinth (forward a note, reverse the pattern) and on S03E16 (forward not judged with 29 s of gaps, reverse the pattern), against the main track and against the core. On S03E12-13 it does not, because the forward pattern itself misses that commentary; the programme is kept there by AG 0.20 and 0/8 tenths. In the review's 260 runs it fired 5 times, each with a commentary as the reference (`W-E-commref`, `W-P-commref`), never on a pair with the programme as reference.
- **What it gives up**: the old test would also have blocked an unrelated programme with the pattern (R3: reverse AG 0.37). R3 has no pattern (8/10 tenths, SMI 0.42, LS 15.8), and an unrelated programme on one title is an authoring error, not a kind of track.
- Measures stored without the reverse block (by the code before this change, for example in an old snapshot's rows) are still judged by the old test, which can only keep. The harness's cache is keyed by the code's SHA-1, so it never mixes the two.

**What now guards against a non-programme reference.**
1. **The choice of the reference (structural).** The original-language track placed first by 8.6 with G1 and G2. A commentary or audio description in another language is never the reference; a lossy stereo commentary does not outrank a lossless main (G1) or a mono main that comes first on the disc (G2), the realistic way a commentary became the reference before. What remains: an isolated score or M&E track with more channels than the programme (a lossless 2.0 or 5.1 score beside a mono film, a common boutique-label layout), a commentary in the original language with more channels than a lossy main, and two languages under one tag.
2. **The judgeability checks (unchanged).** Digital silence over 10% (a score with silence between cues) and under 90 s of gaps. The Office M&E stand-ins (room tone) give 35 s and 52 s of gaps: not judged. A score with noise between its cues instead of digital silence passes row 3(c) (the review's `W-P-scoreref`).
3. **The reverse commentary pattern (row 5).** It flags a commentary taken as the reference when the programme is not compressed. Against a hard-compressed programme the reverse run has too few gaps (34 s, `W-P-commref` 3>4) or too few tenths (7/8, 3>3) to be judged or to match.
4. **The pattern's own conditions.** These are tuned numbers. In three of the review's constructions only SMI, which the design says protects nothing (section 2, item 10), stands between the programme and the commentary verdict, and in two nothing does, with the committed rule as with this one (not a regression, but not covered by items 1 to 3 either):

   | Construction (`attack/levels/`) | Reference | Candidate | DC | AG | Tenths | SMI | LS (dB) | Reverse run | Verdict (delivered and final) |
   |---|---|---|---|---|---|---|---|---|---|
   | W-P-scoreref-dryprog 21>1 | Pan's M&E in 40 s cues every 60 s over a -80 dB noise floor (a score with noise between its cues) | the programme with dry dialogue | 0.09 | 0.71 | 10/10 | 0.54 | 9.4 | 243 s of gaps, 0/8 tenths: no pattern | **commentary** |
   | W-P-scoreref-dryprog 21>2 | the same | the same, compressed | 0.07 | 0.75 | 10/10 | 0.53 | 7.6 | 201 s, 0/8: no pattern | **commentary** |
   | W-P-scoreref 21>4 | the same score | the real programme, hard-compressed | 0.28 | 0.70 | 10/10 | 0.33 | 6.8 | 34 s: not judged | note-commentary (SMI alone) |
   | W-P-commref 3>4 | Pan's English commentary | the Spanish programme, hard-compressed | 0.04 | 0.65 | 10/10 | 0.34 | 6.8 | 34 s: not judged | note-commentary (SMI alone; row 4b would also keep it: the pair is cross-language) |
   | W-e15-flref 31>4 | S03E15's front-left channel | the programme, hard-compressed | 0.06 | 0.64 | 9/9 | 0.38 | 5.3 | 2 s: not judged | note-commentary (SMI alone) |

   With a dry, uncompressed programme over the score with noise between cues, nothing stands in the way: the programme gets the commentary verdict, and only the ranking of the score above it made that possible.
5. **Question 10 (position veto, keep-only) stays the structural backstop, and only the owner can allow it.** The review asked to make it part of the rule now. This revision does not: the design reserves position to a keep-only veto that only the owner may allow (section 2, "Tried and rejected"; question 10), and D15 calls positional rules too brittle. Instead the layout is made visible wherever it occurs: `args` prints `note: the reference is not the first <lang> stream of the file ...` before anything is measured, and the harness prints `note: <folder>: the reference, stream M, is not the first <lang> stream of the file (stream K, ...): question 10's position veto would refuse to judge its candidates` for every such folder. Both fire only when that first stream is lossless or has more than two channels (a lossy stereo first stream is what G1 and G2 pass over on purpose). Question 1 of 9.9 recommends allowing the veto, with this evidence. (Allowed by the owner on 2026-10-06, decision D21: it is row 0 of section 9.5.)
6. **Every labelled candidate tests the guard (in the harness).** The two runs of a pair, read the other way round (`swapped(measures)`), give the verdict the programme would get if that candidate were the reference, with no new measurement. The harness reports `swapped references <n> labelled candidates ...; the programme would get the commentary verdict against <k> of them`, and each such case is a `FAIL ... swapped reference` (exit 1), unless the owner labelled the reference itself as unwanted audio (that is its own FAIL). So every commentary, score, M&E track and audio description the owner labels on any disc is also a real test of a non-programme reference. In the designer's runs, no folder whose reference is the programme has a candidate with the swapped verdict `commentary`. The swap is an approximation in two ways, neither measured, both stated in `swapped()`: the forward run used the reference's centre channel as its dialogue curve, where a real swap would use its mixdown; and a 5.1 stem taken as the reference would use its own centre channel, which carries little or no dialogue, so its gaps would be longer than those of the reverse run.

**What data would set it.** The guard has no number of its own, so what has to be shown is how it behaves on real tracks:
- real titles where a commentary, an isolated score or an M&E track is the reference, or would be under 8.6's ranking. The harness's swapped references give these from every labelled non-programme candidate, and its first-of-language notes count the layouts;
- isolated scores with noise (not digital silence) between cues, beside a mono or 2.0 film;
- single-speaker feature commentaries with the film lowered under them (to confirm that the reverse run misses the pattern on them, as it does on Pan's);
- titles with two commentaries (the symmetric case).

### 9.4 Evidence

**A to D: the designer's runs** (89 tabled: the real pairs, references that are not the programme, dub stand-ins, pausing commentaries), each through the committed rule, the rule as first delivered, and the final rule.
- The real level folders were rebuilt with `proxy_levels.py` from the three proxies, for every choice of reference: main, core or commentary. The rebuilt values differ from the committed ones by 0.1 dB in 26 to 322 frames of 65,000 to 358,000 (at most 0.14%), and the committed rule gives on them the same measures and verdicts, at every printed figure, as on the folders measured on the mini PC.
- R1, R2 and R3 were rebuilt from the 16 kHz extracts with the same chain.
- The final rule computes the same measures as the delivered one on every run (every measure of the 89 tabled runs is identical to the delivered rule's, `results-new.jsonl`); only the verdicts of cross-language candidates differ.
- Column "Swapped" is the verdict the reference would get as a candidate if this track were the reference: the two runs read the other way round.
- "Reverse ... pattern" says whether the reverse run has the commentary pattern ("not judged": it has under 90 s of gaps).

Between the rule as first delivered and the final rule, 13 of these 89 verdicts change, all `commentary` -> `commentary-cross-language`: Pan's commentary against the main and against the core (P-1>3, P-2>3), and 11 constructed commentaries of table D (E-pause1-bed0, E-pause15-bed10, E-pause3of15-bed0, P-voice-bed6, -12 and -20, P-breath-bed6, -12, -20, -26 and -32). The designer's builder tagged every constructed reference of tables C and D as Spanish, the Office ones included, so every candidate there is a cross-language one; tagged English, as the Office really is, the three E-pause runs keep `commentary` (the verdict before row 4b, kept as `base_verdict` in `fix/out/now.jsonl`). Between the committed rule and the delivered one, the 9 changes of the first version stand: Pan's commentary twice and P-breath at -26 and -32 dB (reference-unclear -> commentary), and five programme runs against a commentary taken as the reference (keep or note -> reference-unclear). No run that should be kept has the commentary verdict under any of the three rules, and no run with the programme as reference has the swapped verdict `commentary`.

**A. Real pairs, the duplicates, the cores and the control.**

| Run | Wanted | Made from (stands in for) | DC | AG | Gaps (s) | Tenths | SMI | LS (dB) | Added (s) | Reverse gaps (s), AG | Reverse DC, tenths, SMI, LS: pattern | Committed rule | Delivered addendum | Final rule | Swapped |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P-1>3 | drop | Pan's Labyrinth: English commentary (packaging) against the Spanish DTS-HD MA 5.1 main | 0.08 | 0.54 | 929 | 9/10 | 0.51 | 10.8 | 2016 | 470, 0.31 | 0.06, 1/10, 0.39, 17.1: no | **reference-unclear** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-1>2 | keep | Pan's: derived DTS core against the main | 1.00 | 0.00 | 929 | 0/10 | n/a | 17.0 | 0 | 658, 0.00 | 1.00, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-2>3 | drop | Pan's: the commentary against the core as reference | 0.08 | 0.54 | 930 | 9/10 | 0.51 | 10.8 | 2021 | 470, 0.31 | 0.06, 1/10, 0.40, 17.0: no | **reference-unclear** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-2>1 | keep | Pan's: the main against the core as reference | 1.00 | 0.00 | 930 | 0/10 | n/a | 17.1 | 0 | 666, 0.00 | 1.00, 0/10, n/a, 17.0: no | keep | keep | keep | keep |
| E16-1>3 | drop | S03E16: commentary (listened; the fitting track) | 0.03 | 0.66 | 355 | 10/10 | 0.58 | 10.1 | 517 | 29, 0.10 | 0.02, 0/0, 0.36, 23.1: not judged | **commentary** | **commentary** | **commentary** | reference-unclear |
| E16-1>2 | keep | S03E16: derived core | 1.00 | 0.00 | 355 | 0/10 | n/a | 23.1 | 0 | 329, 0.00 | 1.00, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E16-2>3 | drop | S03E16: commentary against the core | 0.03 | 0.66 | 355 | 10/10 | 0.58 | 10.1 | 517 | 29, 0.10 | 0.02, 0/0, 0.37, 23.1: not judged | **commentary** | **commentary** | **commentary** | reference-unclear |
| E16-2>1 | keep | S03E16: main against the core | 1.00 | 0.00 | 355 | 0/10 | n/a | 23.1 | 0 | 328, 0.00 | 1.00, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| T3-1>3 | drop | S03E12-13: commentary (listened; the known miss) | 0.17 | 0.46 | 578 | 6/10 | 0.59 | 12.4 | 614 | 124, 0.20 | 0.14, 0/8, 0.45, 21.8: no | note-commentary | note-commentary | note-commentary | keep |
| T3-1>2 | keep | S03E12-13: derived core | 1.00 | 0.00 | 578 | 0/10 | n/a | 21.7 | 0 | 473, 0.00 | 1.00, 0/10, n/a, 21.8: no | keep | keep | keep | keep |
| T3-2>3 | drop | S03E12-13: commentary against the core | 0.17 | 0.46 | 578 | 6/10 | 0.59 | 12.4 | 614 | 124, 0.20 | 0.14, 0/8, 0.45, 21.7: no | note-commentary | note-commentary | note-commentary | keep |
| T3-2>1 | keep | S03E12-13: main against the core | 1.00 | 0.00 | 578 | 0/10 | n/a | 21.8 | 0 | 477, 0.00 | 1.00, 0/10, n/a, 21.7: no | keep | keep | keep | keep |
| R1 | drop | S03E16 commentary, 16 kHz extracts (design's R1) | 0.03 | 0.66 | 356 | 10/10 | 0.58 | 10.1 | 518 | 29, 0.10 | 0.02, 0/0, 0.36, 23.2: not judged | **commentary** | **commentary** | **commentary** | reference-unclear |
| R2 | keep | S03E15 exact duplicate, MD5 withheld so it is analysed (design's R2) | 1.00 | 0.00 | 208 | 0/6 | n/a | 15.8 | 0 | 154, 0.00 | 1.00, 0/6, n/a, 15.8: no | keep | keep | keep | keep |
| R2-swap | keep | R2 with the roles exchanged | 1.00 | 0.00 | 208 | 0/6 | n/a | 15.8 | 0 | 154, 0.00 | 1.00, 0/6, n/a, 15.8: no | keep | keep | keep | keep |
| R2-FL | keep | the duplicate's front-left channel as candidate | 0.12 | 0.00 | 208 | 0/6 | n/a | 24.2 | 0 | 479, 0.42 | 0.25, 6/9, 0.47, 15.8: no | keep | keep | keep | note-commentary |
| R3 | keep | control: E15 main against E16 main (unrelated) | 0.02 | 0.53 | 357 | 8/10 | 0.42 | 15.8 | 383 | 154, 0.37 | 0.02, 2/6, 0.46, 22.9: no | note-commentary | note-commentary | note-commentary | note-commentary |
| R3b | keep | control: E16 main against E15 main | 0.02 | 0.31 | 208 | 2/6 | 0.46 | 22.9 | 199 | 332, 0.52 | 0.02, 8/10, 0.42, 15.8: no | note-commentary | note-commentary | note-commentary | note-commentary |
| Reh-1>2 | keep | S03E15 rip (rehearsal): derived core; streams 3 and 4 are duplicates by MD5 (not analysed) | 1.00 | 0.00 | 207 | 0/6 | n/a | 15.8 | 0 | 154, 0.00 | 1.00, 0/6, n/a, 15.8: no | keep | keep | keep | keep |

**B. A reference that is not the programme** (a commentary, an M&E stand-in, a front-left channel), and the M&E stand-in as a candidate.

| Run | Wanted | Made from (stands in for) | DC | AG | Gaps (s) | Tenths | SMI | LS (dB) | Added (s) | Reverse gaps (s), AG | Reverse DC, tenths, SMI, LS: pattern | Committed rule | Delivered addendum | Final rule | Swapped |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P-3>1 | keep | Pan's: the Spanish main against the English commentary taken as reference | 0.06 | 0.31 | 470 | 1/10 | 0.39 | 17.1 | 796 | 666, 0.59 | 0.08, 9/10, 0.51, 10.8: yes | note-commentary | **reference-unclear** | **reference-unclear** | commentary |
| P-3>2 | keep | Pan's: the core against the commentary as reference | 0.06 | 0.31 | 470 | 1/10 | 0.40 | 17.0 | 794 | 658, 0.59 | 0.08, 9/10, 0.51, 10.8: yes | note-commentary | **reference-unclear** | **reference-unclear** | commentary |
| E16-3>1 | keep | S03E16: the main against its commentary as reference | 0.02 | 0.10 | 29 | 0/0 | 0.36 | 23.1 | 142 | 328, 0.66 | 0.03, 10/10, 0.58, 10.1: yes | keep | **reference-unclear** | **reference-unclear** (not judged: gaps 29 s) | commentary |
| E16-3>2 | keep | S03E16: the core against the commentary as reference | 0.02 | 0.10 | 29 | 0/0 | 0.37 | 23.1 | 142 | 329, 0.66 | 0.03, 10/10, 0.58, 10.1: yes | keep | **reference-unclear** | **reference-unclear** (not judged: gaps 29 s) | commentary |
| R1-swap | keep | R1 with the roles exchanged (16 kHz extracts) | 0.02 | 0.10 | 29 | 0/0 | 0.36 | 23.2 | 143 | 330, 0.66 | 0.03, 10/10, 0.58, 10.1: yes | keep | **reference-unclear** | **reference-unclear** (not judged: gaps 29 s) | commentary |
| T3-3>1 | keep | S03E12-13: the main against its commentary as reference | 0.14 | 0.20 | 124 | 0/8 | 0.45 | 21.8 | 248 | 477, 0.50 | 0.17, 8/10, 0.59, 12.4: no | keep | keep | keep | note-commentary |
| T3-3>2 | keep | S03E12-13: the core against the commentary as reference | 0.14 | 0.20 | 124 | 0/8 | 0.45 | 21.7 | 248 | 473, 0.50 | 0.17, 8/10, 0.59, 12.4: no | keep | keep | keep | note-commentary |
| R2-FL-swap | keep | S03E15: the main against the duplicate's front-left channel as reference (a reference without the dialogue) | 0.25 | 0.42 | 479 | 6/9 | 0.47 | 15.8 | 313 | 154, 0.00 | 0.17, 0/6, n/a, 24.2: no | note-commentary | note-commentary | note-commentary | keep |
| P-ME>1 | keep | Pan's: the main against an M&E stand-in (mixdown minus 0.2929 x centre) as reference | 0.54 | 0.06 | 1566 | 0/9 | 0.34 | 17.1 | 193 | 666, 0.00 | 0.51, 0/10, n/a, 19.3: no | keep | keep | keep | keep |
| P-ME>2 | keep | Pan's: the core against the M&E stand-in | 0.54 | 0.06 | 1566 | 0/9 | 0.34 | 17.0 | 191 | 658, 0.00 | 0.51, 0/10, n/a, 19.3: no | keep | keep | keep | keep |
| P-ME>3 | drop | Pan's: the commentary against the M&E stand-in | 0.08 | 0.61 | 1566 | 9/9 | 0.50 | 10.8 | 2696 | 470, 0.26 | 0.06, 1/10, 0.35, 19.3: no | note-commentary | note-commentary | note-commentary | keep |
| E16-ME>1 | keep | S03E16: the main against its M&E stand-in (mostly room tone) | 0.08 | 0.35 | 35 | 0/2 | 0.45 | 23.1 | 416 | 328, 0.00 | 0.06, 0/10, n/a, 20.1: no | note-commentary | note-commentary | note-commentary (not judged: gaps 35 s) | keep |
| E16-ME>2 | keep | S03E16: the core against the M&E stand-in | 0.08 | 0.35 | 35 | 0/2 | 0.45 | 23.1 | 416 | 329, 0.00 | 0.06, 0/10, n/a, 20.1: no | note-commentary | note-commentary | note-commentary (not judged: gaps 35 s) | keep |
| E16-ME>3 | drop | S03E16: the commentary against the M&E stand-in | 0.00 | 0.67 | 35 | 2/2 | 0.58 | 10.1 | 530 | 29, 0.60 | 0.00, 0/0, 0.34, 20.1: not judged | note-commentary | note-commentary | note-commentary (not judged: gaps 35 s) | note-commentary |
| T3-ME>1 | keep | S03E12-13: the main against its M&E stand-in | 0.08 | 0.40 | 52 | 1/1 | 0.50 | 21.8 | 831 | 477, 0.00 | 0.08, 0/10, n/a, 14.6: no | note-commentary | note-commentary | note-commentary (not judged: gaps 52 s) | keep |
| T3-ME>2 | keep | S03E12-13: the core against the M&E stand-in | 0.08 | 0.40 | 52 | 1/1 | 0.50 | 21.7 | 831 | 473, 0.00 | 0.08, 0/10, n/a, 14.6: no | note-commentary | note-commentary | note-commentary (not judged: gaps 52 s) | keep |
| T3-ME>3 | drop | S03E12-13: the commentary against the M&E stand-in | 0.01 | 0.64 | 52 | 1/1 | 0.58 | 12.4 | 615 | 124, 0.69 | 0.01, 7/8, 0.33, 14.6: no | note-commentary | note-commentary | note-commentary (not judged: gaps 52 s) | note-commentary |
| P-ME | ask (D14: drop when confirmed) | Pan's: the M&E stand-in as a candidate | 0.49 | 0.00 | 929 | 0/10 | n/a | 19.3 | 0 | 1566, 0.06 | 0.54, 0/9, 0.34, 17.1: no | keep | keep | keep | keep |
| E16-ME | ask (D14) | S03E16: the M&E stand-in as a candidate | 0.05 | 0.00 | 355 | 0/10 | n/a | 20.1 | 0 | 35, 0.35 | 0.08, 0/2, 0.45, 23.1: not judged | keep | keep | keep | note-commentary |
| T3-ME | ask (D14) | S03E12-13: the M&E stand-in as a candidate | 0.06 | 0.00 | 578 | 0/10 | n/a | 14.6 | 0 | 52, 0.40 | 0.08, 1/1, 0.50, 21.8: not judged | keep | keep | keep | note-commentary |

**C. The designer's dub stand-ins** (the reference is the real main track; the candidate is a 5.1 English-tagged track built from the reference's M&E and a new voice in the centre's place, measured as a lossy track). Prefixes: P- Pan's Labyrinth (other words: its own centre 600 s on, wrapped), E- S03E16 (other words: S03E12-13's centre from 300 s), T- S03E12-13 (its own centre 600 s on). "Voice" levels are relative to the original centre channel's weight in the mixdown. The gate is ffmpeg's `sidechaingate` keyed by the original centre channel (threshold near its P95 - 25 dB, range -60 dB).

| Run | Wanted | Made from (stands in for) | DC | AG | Gaps (s) | Tenths | SMI | LS (dB) | Added (s) | Reverse gaps (s), AG | Reverse DC, tenths, SMI, LS: pattern | Committed rule | Delivered addendum | Final rule | Swapped |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P-dubA-m6 | keep | same words: centre +3 semitones (asetrate+atempo), voice -6 dB | 0.93 | 0.00 | 929 | 0/10 | n/a | 16.9 | 0 | 750, 0.00 | 0.92, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-dubA-0 | keep | same words: centre +3 semitones, voice 0 dB | 0.96 | 0.00 | 929 | 0/10 | n/a | 17.0 | 0 | 690, 0.00 | 0.96, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-dubA-p6 | keep | same words: centre +3 semitones, voice +6 dB | 0.93 | 0.00 | 929 | 0/10 | n/a | 17.9 | 0 | 768, 0.00 | 0.92, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-dubB-0 | keep | same words: centre -3 semitones, 0 dB | 0.99 | 0.00 | 929 | 0/10 | n/a | 16.9 | 0 | 634, 0.00 | 0.99, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-dubC-0 | keep | same words: centre frequency-shifted +150 Hz, 0 dB | 0.97 | 0.01 | 929 | 0/10 | 0.12 | 16.7 | 29 | 618, 0.00 | 0.96, 0/10, n/a, 17.1: no | keep | keep | keep | keep |
| P-dubD-m6 | keep | other words gated to the original's windows (+150 ms, 300 ms release), +3 semitones, -6 dB | 0.27 | 0.10 | 929 | 0/10 | 0.42 | 15.9 | 414 | 569, 0.09 | 0.26, 0/9, 0.38, 17.1: no | keep | keep | keep | keep |
| P-dubD-0 | keep | other words gated, +3 semitones, 0 dB | 0.20 | 0.15 | 929 | 0/10 | 0.41 | 15.8 | 758 | 510, 0.07 | 0.19, 0/10, 0.38, 17.1: no | keep | keep | keep | keep |
| P-dubD-p6 | keep | other words gated, +3 semitones, +6 dB | 0.13 | 0.21 | 926 | 0/10 | 0.42 | 16.6 | 1296 | 614, 0.07 | 0.12, 0/10, 0.34, 17.1: no | keep | keep | keep | keep |
| P-dubE-0 | keep | other words, sloppy gate (+400 ms, 600 ms release), -3 semitones, 0 dB | 0.20 | 0.23 | 929 | 0/10 | 0.39 | 15.6 | 874 | 451, 0.09 | 0.19, 0/10, 0.38, 17.1: no | keep | keep | keep | keep |
| P-dubF-m6 | keep | other words at their own timing (no gate), +3 semitones, -6 dB | 0.26 | 0.22 | 929 | 0/10 | 0.42 | 15.5 | 586 | 429, 0.12 | 0.25, 0/9, 0.38, 17.1: no | keep | keep | keep | keep |
| P-dubF-0 | keep | other words at their own timing, +3 semitones, 0 dB | 0.20 | 0.33 | 929 | 3/10 | 0.41 | 15.3 | 1047 | 337, 0.12 | 0.17, 0/9, 0.38, 17.1: no | note-commentary | note-commentary | note-commentary | keep |
| E-dubA-m6 | keep | same words: centre +3 semitones (asetrate+atempo), voice -6 dB | 0.99 | 0.00 | 355 | 0/10 | n/a | 21.8 | 0 | 308, 0.00 | 0.98, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E-dubA-0 | keep | same words: centre +3 semitones, voice 0 dB | 0.99 | 0.00 | 355 | 0/10 | n/a | 23.0 | 0 | 332, 0.00 | 0.99, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E-dubA-p6 | keep | same words: centre +3 semitones, voice +6 dB | 0.99 | 0.00 | 355 | 0/10 | n/a | 23.7 | 0 | 348, 0.00 | 0.99, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E-dubB-0 | keep | same words: centre -3 semitones, 0 dB | 0.99 | 0.00 | 355 | 0/10 | n/a | 23.0 | 0 | 327, 0.00 | 0.98, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E-dubC-0 | keep | same words: centre frequency-shifted +150 Hz, 0 dB | 0.99 | 0.00 | 355 | 0/10 | n/a | 22.4 | 0 | 314, 0.00 | 0.99, 0/10, n/a, 23.1: no | keep | keep | keep | keep |
| E-dubD-m6 | keep | other words gated to the original's windows (+150 ms, 300 ms release), +3 semitones, -6 dB | 0.03 | 0.13 | 355 | 0/10 | 0.54 | 21.6 | 108 | 283, 0.20 | 0.05, 0/10, 0.47, 23.1: no | keep | keep | keep | keep |
| E-dubD-0 | keep | other words gated, +3 semitones, 0 dB | 0.04 | 0.13 | 355 | 0/10 | 0.54 | 22.3 | 126 | 313, 0.18 | 0.05, 0/10, 0.44, 23.1: no | keep | keep | keep | keep |
| E-dubD-p6 | keep | other words gated, +3 semitones, +6 dB | 0.05 | 0.13 | 356 | 0/10 | 0.55 | 23.5 | 124 | 338, 0.16 | 0.06, 0/10, 0.43, 23.1: no | keep | keep | keep | keep |
| E-dubE-0 | keep | other words, sloppy gate (+400 ms, 600 ms release), -3 semitones, 0 dB | 0.04 | 0.30 | 355 | 1/10 | 0.50 | 21.2 | 243 | 267, 0.26 | 0.03, 0/10, 0.44, 23.1: no | note-commentary | note-commentary | note-commentary | keep |
| E-dubF-m6 | keep | other words at their own timing (no gate), +3 semitones, -6 dB | 0.04 | 0.41 | 355 | 7/10 | 0.49 | 18.5 | 307 | 201, 0.32 | 0.04, 1/8, 0.44, 23.1: no | note-commentary | note-commentary | note-commentary | note-commentary |
| E-dubF-0 | keep | other words at their own timing, +3 semitones, 0 dB | 0.04 | 0.42 | 355 | 7/10 | 0.49 | 19.5 | 316 | 227, 0.33 | 0.03, 2/9, 0.44, 23.1: no | note-commentary | note-commentary | note-commentary | note-commentary |
| T-dubA-0 | keep | same words: centre +3 semitones, voice 0 dB | 1.00 | 0.00 | 578 | 0/10 | n/a | 21.8 | 0 | 488, 0.00 | 0.99, 0/10, n/a, 21.8: no | keep | keep | keep | keep |
| T-dubB-0 | keep | same words: centre -3 semitones, 0 dB | 0.99 | 0.00 | 578 | 0/10 | n/a | 21.9 | 0 | 476, 0.00 | 0.99, 0/10, n/a, 21.8: no | keep | keep | keep | keep |
| T-dubC-0 | keep | same words: centre frequency-shifted +150 Hz, 0 dB | 0.98 | 0.00 | 578 | 0/10 | n/a | 20.8 | 0 | 401, 0.00 | 0.98, 0/10, n/a, 21.8: no | keep | keep | keep | keep |
| T-dubD-0 | keep | other words gated, +3 semitones, 0 dB | 0.03 | 0.25 | 584 | 1/10 | 0.49 | 23.2 | 455 | 587, 0.20 | 0.02, 0/10, 0.53, 21.8: no | keep | keep | keep | keep |
| T-dubE-0 | keep | other words, sloppy gate (+400 ms, 600 ms release), -3 semitones, 0 dB | 0.02 | 0.33 | 578 | 1/10 | 0.49 | 22.5 | 494 | 545, 0.36 | 0.02, 3/10, 0.48, 21.8: no | note-commentary | note-commentary | note-commentary | note-commentary |
| T-dubF-m6 | keep | other words at their own timing (no gate), +3 semitones, -6 dB | 0.03 | 0.37 | 578 | 3/10 | 0.50 | 20.4 | 540 | 407, 0.34 | 0.03, 2/9, 0.49, 21.8: no | note-commentary | note-commentary | note-commentary | note-commentary |
| T-dubF-0 | keep | other words at their own timing, +3 semitones, 0 dB | 0.03 | 0.37 | 578 | 4/10 | 0.49 | 21.7 | 546 | 542, 0.36 | 0.02, 3/10, 0.49, 21.8: no | note-commentary | note-commentary | note-commentary | note-commentary |

**D. Commentaries that pause over an audible film** (built to see whether the old test's failure on Pan's is a property of the case or of the test).

| Run | Wanted | Made from (stands in for) | DC | AG | Gaps (s) | Tenths | SMI | LS (dB) | Added (s) | Reverse gaps (s), AG | Reverse DC, tenths, SMI, LS: pattern | Committed rule | Delivered addendum | Final rule | Swapped |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| E-pause1-bed0 | drop | S03E16 commentary, 1 s pause in every 6 s, episode at the commentary's own bed level in the pauses | 0.05 | 0.51 | 355 | 10/10 | 0.56 | 12.8 | 398 | 22, 0.07 | 0.02, 0/0, 0.44, 23.1: not judged | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| E-pause1-bed10 | drop | the same, episode 10 dB lower in the pauses | 0.04 | 0.54 | 355 | 10/10 | 0.56 | 13.1 | 425 | 23, 0.07 | 0.02, 0/0, 0.44, 23.1: not judged | note-commentary | note-commentary | note-commentary | keep |
| E-pause1-bed30 | drop | the same, episode 30 dB lower | 0.04 | 0.54 | 355 | 10/10 | 0.56 | 13.5 | 428 | 25, 0.11 | 0.02, 0/0, 0.43, 23.1: not judged | note-commentary | note-commentary | note-commentary | keep |
| E-pause15-bed0 | drop | 1.5 s pause in every 6 s, bed 0 dB | 0.14 | 0.46 | 355 | 8/10 | 0.54 | 11.4 | 311 | 64, 0.02 | 0.09, 0/0, 0.42, 23.1: not judged | note-commentary | note-commentary | note-commentary | keep |
| E-pause15-bed10 | drop | 1.5 s in 6 s, bed -10 dB | 0.12 | 0.51 | 355 | 10/10 | 0.56 | 12.4 | 350 | 88, 0.01 | 0.06, 0/5, n/a, 23.1: not judged | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| E-pause15-bed30 | drop | 1.5 s in 6 s, bed -30 dB | 0.05 | 0.54 | 355 | 10/10 | 0.56 | 13.5 | 426 | 146, 0.39 | 0.02, 5/10, 0.43, 23.1: no | note-commentary | note-commentary | note-commentary | note-commentary |
| E-pause3of15-bed0 | drop | 3 s pause in every 15 s, bed 0 dB | 0.14 | 0.51 | 355 | 9/10 | 0.57 | 12.6 | 395 | 68, 0.04 | 0.10, 0/1, 0.43, 23.1: not judged | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| E-pause3of15-bed10 | drop | 3 s in 15 s, bed -10 dB | 0.13 | 0.53 | 355 | 9/10 | 0.56 | 13.7 | 519 | 95, 0.01 | 0.07, 0/4, 0.48, 23.1: no | note-commentary | note-commentary | note-commentary | keep |
| E-pause3of15-bed20 | drop | 3 s in 15 s, bed -20 dB | 0.11 | 0.54 | 355 | 9/10 | 0.56 | 14.6 | 595 | 129, 0.00 | 0.03, 0/9, 0.49, 23.1: no | note-commentary | note-commentary | note-commentary | keep |
| E-pause5of20-bed0 | drop | 5 s pause in every 20 s, bed 0 dB | 0.25 | 0.50 | 355 | 10/10 | 0.57 | 13.2 | 386 | 82, 0.02 | 0.18, 0/2, 0.42, 23.1: not judged | note-commentary | note-commentary | note-commentary | keep |
| E-pause5of20-bed10 | drop | 5 s in 20 s, bed -10 dB | 0.25 | 0.51 | 355 | 10/10 | 0.57 | 14.5 | 506 | 116, 0.00 | 0.09, 0/8, 0.53, 23.1: no | note-commentary | note-commentary | note-commentary | keep |
| E-pause5of20-bed20 | drop | 5 s in 20 s, bed -20 dB | 0.24 | 0.51 | 355 | 10/10 | 0.56 | 16.1 | 582 | 183, 0.00 | 0.04, 0/9, 0.53, 23.1: no | note-commentary | note-commentary | note-commentary | keep |
| P-voice-bed0 | drop | the S03E16 commentary track looped over Pan's mixdown at del Toro's bed level (G +10.5 dB) | 0.30 | 0.46 | 929 | 7/10 | 0.58 | 9.0 | 1227 | 28, 0.00 | 0.22, 0/0, n/a, 17.1: not judged | note-commentary | note-commentary | note-commentary | keep |
| P-voice-bed6 | drop | the same, film 6 dB lower | 0.10 | 0.60 | 929 | 10/10 | 0.57 | 8.5 | 2152 | 36, 0.00 | 0.07, 0/0, n/a, 17.1: not judged | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-voice-bed12 | drop | film 12 dB lower | 0.03 | 0.61 | 929 | 10/10 | 0.57 | 9.2 | 2235 | 63, 0.02 | 0.02, 0/2, 0.13, 17.1: not judged | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-voice-bed20 | drop | film 20 dB lower | 0.01 | 0.62 | 929 | 10/10 | 0.57 | 9.9 | 2325 | 110, 0.24 | 0.00, 0/6, 0.36, 17.1: no | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-breath-bed6 | drop | as P-voice, the voice stopping 0.6 s in every 4 s (breath pauses), film -6 dB | 0.14 | 0.52 | 929 | 9/10 | 0.56 | 9.6 | 1788 | 170, 0.00 | 0.10, 0/6, n/a, 17.1: no | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-breath-bed12 | drop | breath pauses, film -12 dB | 0.06 | 0.53 | 929 | 9/10 | 0.56 | 10.9 | 1887 | 329, 0.00 | 0.04, 0/8, n/a, 17.1: no | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-breath-bed20 | drop | breath pauses, film -20 dB | 0.03 | 0.54 | 929 | 9/10 | 0.56 | 12.3 | 1974 | 577, 0.28 | 0.02, 0/8, 0.37, 17.1: no | **commentary** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-breath-bed26 | drop | breath pauses, film -26 dB | 0.02 | 0.54 | 929 | 9/10 | 0.56 | 12.7 | 1993 | 752, 0.40 | 0.01, 3/8, 0.38, 17.1: no | **reference-unclear** | **commentary** | **commentary-cross-language** | reference-unclear |
| P-breath-bed32 | drop | breath pauses, film -32 dB | 0.02 | 0.54 | 929 | 9/10 | 0.56 | 12.8 | 2001 | 863, 0.48 | 0.01, 9/9, 0.38, 17.1: no | **reference-unclear** | **commentary** | **commentary-cross-language** | reference-unclear |

Notes on the tables:
- The "Swapped" value `commentary` in the first five rows of table B is right: there the reference is the commentary, so the swap is the commentary judged against the programme. SMI shown as 0.50 in P-ME>3 and T3-ME>1 is 0.4965 and 0.4998: below 0.50.
- The E-pause runs cut the real commentary and paste the episode's own mix into the cuts. The bed in a pause therefore follows the programme exactly, the gain model explains it, and the reverse AG stays at 0.00-0.07. Only at -30 dB do the pauses count as gaps and the reverse AG reaches 0.39. They show that long, regular pauses with the programme itself underneath do not trip the old test. Several E-pause runs lose the pattern on LS (13.1-16.1 dB): cutting 17-25% of the voice widens the level spread. That is a property of the cut, not a finding about pausing commentaries.
- The P-voice and P-breath runs put a dense two-person commentary (with its own Office bed under it) over Pan's film. Their breath pauses keep a fixed 4 s rhythm. Real pauses vary, and a real commentary's bed is its own stereo mix of the film, not the 5.1 mixdown.

How far tables B to D stand in: the commentary-as-reference rows are real tracks in exchanged roles. The M&E stand-in is the mixdown with its centre removed, so centre-panned music and effects are gone too; it has no centre curve, so its mixdown is the dialogue curve; and for the Office it is mostly room tone. The front-left channel is a programme channel without most of the dialogue, not a real stem. No real isolated score or M&E track was available. The designer's builder tagged every constructed reference of tables C and D as Spanish, the Office ones included: every candidate there is a cross-language one, and every commentary verdict of table D is now `commentary-cross-language`. Tagged English, as the Office really is, the E-pause runs would keep `commentary`; the measures are the same.

**E. The review's constructions** (260 runs in 232 folders, `attack/levels/`), through the delivered rule and the final rule. The final rule was run twice on the dub stand-ins of S03E16 and S03E12-13: as the review tagged them (an English dub against the English reference: a same-language candidate) and retagged with a Japanese reference (`fix/levels-x/`: an English dub of a Japanese original, the anime case, whose 2.0 original is the `-nofc` variant).

| Group | Runs | Delivered rule | Final rule, as tagged | Final rule, E and T dubs of a Japanese original |
|---|---|---|---|---|
| Dub stand-ins, dry voice (another title's commentary track) gated to the original's windows: 150 ms late with a 300 ms release, or 250 ms late with 800 or 1200 ms; voice 0 or +6 dB; raw or compressed | 60 | commentary 28, note-commentary 22, keep 10 | commentary 24, commentary-cross-language 4, note-commentary 22, keep 10 | commentary-cross-language 28, note-commentary 22, keep 10 |
| Dub stand-ins, film-dialogue voice (the same title's centre further on), the same grid | 60 | note-commentary 37, keep 23 | note-commentary 37, keep 23 | note-commentary 37, keep 23 |
| Other dub stand-ins: loud at its own timing (L1, L2), lines running 1.2 s long (M1), added off-screen lines (N40, N70), same words at -7/+7 semitones (P7dn, P7up), jittered timing (Q1), own score mix (O6, O0c) | 52 | note-commentary 34, keep 18 | note-commentary 34, keep 18 | note-commentary 34, keep 18 |
| The programme in another slot: dynaudnorm, dialogue-boosted AC-3 192k 120 ms late, band-limited MP2 mono 300 ms late | 18 | keep 18 | keep 18 | keep 18 |
| The designer's 29 dub stand-ins again, without a centre channel | 29 | note-commentary 7, keep 22 | note-commentary 7, keep 22 | as tagged |
| A dub stand-in (11) and its compressed 2.0 companion, 40 ms late (12), against a Japanese-tagged reference (S-T-dub; S-T-dubcomp has no main-mix.txt and is refused) | 2 | commentary 2 | commentary-cross-language 1, note-commentary 1 | commentary-cross-language 1, note-commentary 1 |
| References that are not the programme: a commentary (W-*-commref), an M&E stand-in (W-P-meref), a score with noise between 40 s cues (W-P-scoreref), a front-left channel (W-e15-flref); the programme raw, compressed, dynaudnorm or hard-compressed as candidates | 30 | commentary 2, reference-unclear 5, note-commentary 16, keep 7 | commentary 2, reference-unclear 5, note-commentary 16, keep 7 | as tagged |
| The dry voice alone, no M&E, gated with 300, 800 and 1200 ms of release (V-*-voiceonly: three candidates of one folder and one voice, so the first is the peer of the other two) | 9 | commentary 4, note-commentary 4, keep 1 | note-commentary 8, keep 1 | as tagged |

So under the final rule no dub stand-in of a cross-language pair gets a drop verdict: the 28 dry-voice runs with the pattern get `commentary-cross-language` once the reference carries another language than the dub, and the companion in `S-T-dub` gets a note from its peer run. What still gets the plain `commentary` verdict: the two runs of the programme against a score with noise between cues (9.3, item 4), and the 24 dry-voice dubs of the Office titles as the review tagged them, English against the English reference (a dub under the reference's own tag, 9.2). The `V-*-voiceonly` runs lose their four commentary verdicts to the peer run only because the folder's three candidates are one voice.

**Cost.** Rebuilding the nine real-pair folders from the proxies took 46 s of wall time on this Mac, and analysing the first 64 folders with both runs about 80 s on 8 cores. Pan's Labyrinth takes about 16 s of CPU per candidate with the reverse run, which was already computed before; it is only kept whole now. The peer run adds one forward and one reverse analysis for a cross-language candidate with the pattern whose language has another stream: none on the committed corpus (Pan's English commentary is its only English stream).

**What the constructed cases cannot show.** Every speech sample is from one sitcom and one film. The stand-ins show mechanisms and the direction in which each measure moves, never margins on real tracks. No real dub, isolated score, M&E track, audio description, mono film or voice-over translation was measured. Pan's Labyrinth's commentary meets the forward pattern by small margins: AG 0.544 for 0.50, SMI 0.511 for 0.50, and 9 of 10 tenths, exactly 90%.

**What the S03E12-13 miss suggests (not acted on).** Its AG (0.46) and its spread (6/10 tenths) fail. Its gain against the core is +1.8 dB, against -6.3 dB for S03E16 and -0.2 dB for Pan's, which suggests that the episode is barely lowered under these voices. A louder bed leaves less sound 10 dB above the predicted level, so AG falls. The voices are also unevenly dense across the episode. Lowering AG_MIN or the tenths share to catch it would be a fit to one track, and it would bring the review's dub stand-ins further into the pattern. More commentaries over an unlowered bed, labelled before their verdict, are needed first.

### 9.5 Decision-rule rows that change (section 3)

Replacement for the **Scope** paragraph:

> **Scope.** The rule covers every audio stream in a wanted language that survives spec 8.6 step 1 (derived streams, flags 1, 2, 4 and 512, TheDiscDB descriptions, unwanted languages). The reference is the title's programme reference: the stream that 8.6 (steps 3 and 4) places first in the work's original language, or in English when the original language is English, unknown, none, or has no stream on the disc. Every other covered stream is a candidate, whatever its language: an English commentary on a Spanish film, and an English dub, are judged against the Spanish main track. A candidate in another language than the reference is never dropped by content (row 4b). A title with no stream in a wanted language has nothing to judge (`no_wanted_audio`). Streams with no language are not judged; they stay as 8.6 says (phase 0 measures them as candidates, for the dataset).

Replacement **rows 3, 4 and 5**, a new **row 4b**, and a new **row 0** (the position veto, owner decision D21, added 2026-10-06; it is a condition on the title, applied after rows 1 and 2 and before rows 3 to 9) (the other rows do not change). Rows are still tried in order; row 3's outcome now also carries row 5's review reason when row 5's condition holds, so a non-programme reference is flagged even when the forward run cannot be judged:

| # | Measured evidence | Outcome | Note or review reason |
|---|---|---|---|
| 0 | **Position veto** (owner decision D21, 2026-10-06), a condition on the title, not on a candidate: the programme reference is not the first stream of its language on the disc, and that first stream is lossless or has more than two channels (position is read only to keep, never to drop: D15) | every candidate that rows 1 and 2 leave (flags and exact duplicates still apply) is **kept**, whatever rows 3 to 9 say, before and after experiment 13's yes: no commentary is confirmed on the title | review reason `audio_reference_unclear` (phase 0: verdict `reference-unclear`; the report's `veto:` line names both streams, and the harness keeps what rows 3 to 9 said as `before_veto`) |
| 3 | Any of: (a) the analysis failed, or did not cover the whole title; (b) overlap < 0.90; (c) the reference is digitally silent (<= -90 dB) in more than 10% of frames; (d) gap time < 90 s; (e) no dialogue window; (f) withdrawn: the position veto is row 0 (D21); (g) TheDiscDB describes the reference as score, M&E, commentary or audio description | keep (programme audio) | note `audio_content_not_judged` with the reason; also review reason `audio_reference_unclear` when row 5's condition holds; also row 7's note when DC <= 0.50 and AG >= 0.30, or row 8's note when DC >= 0.80, AG >= 0.10 and AG_s >= 30 s |
| 4 | **Commentary pattern**, all of: DC <= 0.30; AG >= 0.50; at least 6 tenths with 10 s of gaps, AG >= 0.40 in at least 90% of them; SMI >= 0.50; LS <= 13 dB; added_s >= 60 s; row 5's condition does not hold; and the candidate is in the reference's language (or one of the two has no language code) | **Until experiment 13 answers yes: keep and flag for review.** After a yes, with the setting `drop_commentary_by_audio` on: **drop (confirmed commentary)** | before: review reason `commentary_detected`. After: note `commentary_dropped_by_audio`, named in the `filed` notification; the raw rip is kept for `retention_days` as the undo |
| 4b | Row 4's pattern on a candidate whose language tag names another language than the reference's (`commentary-cross-language`). Unless the **peer run** (the candidate against the stream 8.6 places first among the candidates of its own language, when that is another stream) has no commentary verdict: then row 7 | **keep and flag for review, before and after a yes**: content never drops a candidate in another language than the reference (`drop_commentary_by_audio` does not apply) until spec row 13 also holds for dubs and the owner lifts this row | review reason `commentary_detected` (the report says `cross-language`) |
| 5 | **Reverse commentary pattern**: the reverse run (the candidate as reference with its own mixdown as dialogue curve, the reference as candidate) passes checks (b) to (e) of row 3 on its own measures, and meets row 4's pattern with row 4's thresholds. The reference looks like a commentary over the candidate. Whatever rows 3 and 4 say of the forward run | keep and flag for review | review reason `audio_reference_unclear` (one of the two is not what its rank says: a commentary or a stem ranked first, or a stem among the candidates) |

Replacement for the **Thresholds** table's row "Reverse test":

| Threshold | Value | Measured on each side | Data needed to fix it |
|---|---|---|---|
| Reverse commentary pattern (row 5) | none of its own: row 4's thresholds and row 3's checks (b) to (e), applied to the reverse run | the three real commentaries' reverse runs: not judged (29 s of gaps), or no pattern (Pan's AG 0.31, 1/10, SMI 0.39, LS 17.1; S03E12-13 AG 0.20, 0/8); the programme against a real commentary as reference: the pattern on Pan's and S03E16 (row 5 fires), not on S03E12-13 (its forward run misses); M&E and front-left references: reverse AG 0.00, no pattern (row 5 cannot catch them; 9.3) | real tracks ranked first that are not the programme; the harness's swapped references from every labelled commentary, score, M&E and audio-description track; titles with two commentaries |
| Row 4b (cross-language) | none: a structural keep | the review's dub stand-ins with a dry voice and loose timing: the pattern on 28 of the 60 dry-voice runs (every one with lines 250 ms late and 800 or 1200 ms of release) (9.4 table E), further inside it than Pan's real commentary | real dubs in a wanted language, at least the floor of row 13's proposal (9.5), before the owner may lift the row |

Replacement for section 2, **item 13**:

> 13. **Reverse run (decides, keep-only).** The same analysis with the roles swapped: the candidate as reference, its own mixdown as dialogue curve, the reference as candidate. Every measure of it is kept (`rev_<name>`). If it passes the judgeability checks and meets the commentary pattern (row 5), the reference looks like a commentary over the candidate, and the pair is flagged `reference-unclear`, never confirmed. The earlier form (reverse AG >= 0.30 over at least 90 s of the candidate's gaps) is withdrawn. It fired on single-speaker commentaries whose film is lowered under the voice: Pan's Labyrinth's real commentary (470 s, 0.31), and a constructed one at -26 and -32 dB (0.40, 0.48). Its reverse AG was 0.00 against every M&E and front-left reference built (EXP-13 addendum, 9.3). Read the other way round, the same two runs also give the verdict the reference would get if the candidate were ranked first (the harness's swapped references).

New item 14 of section 2:

> 14. **Peer run (decides, keep-only).** For a candidate in another language than the reference that has the commentary pattern: the same analysis against the stream 8.6 places first among the candidates of the candidate's own language (when that is another stream), with that stream's mixdown as dialogue curve, from the same level folder. Without a commentary verdict there, the candidate follows a programme track of its language (the companion of a dub), and row 4b gives way to row 7's note.

Replacement bullet in **"How the rule treats particular tracks"**, "A reference that is not the programme":

> - **A reference that is not the programme.** The reference is the original-language track placed first by 8.6 with its guards G1 and G2, so a commentary or audio description in another language is never the reference, and a lossy stereo commentary does not outrank a lossless or first-on-disc main. A commentary that still becomes the reference is caught by row 5 when the programme is not compressed: the programme against Pan's Labyrinth's and S03E16's commentaries gets `reference-unclear`. A stem ranked first (an isolated score with more channels than a mono film, an M&E track) is not caught by row 5, because the programme adds the dialogue to the stem exactly as a commentary adds voices to the programme. Rows 3(c) and 3(d) catch a score with silence between cues; a score with noise between them is caught by nothing structural, and in constructions SMI alone kept a compressed programme from the commentary verdict and nothing kept a dry one (EXP-13 addendum 9.3). Question 10's position veto is the backstop, and only the owner can allow it. `args` and the harness note every reference that is not the first stream of its language, and the harness reports, for every labelled candidate, what the programme would get if that candidate were the reference.

New bullet in **"How the rule treats particular tracks"**:

> - **A dub in a wanted language** (an English dub of a foreign film, its 2.0 companion). A candidate against the original. Level curves do not hear language: a lip-synced dub with loose timing on a dialogue-dense title can have the commentary pattern (the review's stand-ins, EXP-13 addendum 9.2). It is never dropped by content (row 4b): at worst it is flagged for review, and its companion, which follows it, gets a note (the peer run). A dub tagged with the reference's own language is not protected by row 4b: it is the `same-language-dub` or `foreign-dub` kind, which the owner must bring to experiment 13.

New row for **section 1**'s table, after "Programme track of a mono or 2.0 film":

| Track kind | Wanted | Rule today (stand-in) |
|---|---|---|
| Dub in a wanted language, tagged with its own language (an English dub of a foreign film), and its companions | keep (R3, R4) | keep, a note, or `commentary-cross-language` (keep and review; never dropped by content: row 4b). Stand-ins: the designer's 29 never had the pattern; the review's with a dry voice and loose timing had it on 28 of the 60 dry-voice runs (every one with lines 250 ms late and 800 or 1200 ms of release); companions of a dub get a note from the peer run |

Consequences for the spec, for the controller (not made here):
- **D15**: "compared with the main track of its language" becomes "compared with the title's programme reference, the track 8.6 places first in the original language, else in English (EXP-13 addendum 9.1); a candidate in another language than that reference is never dropped by content until real dubs have been measured (addendum 9.2)".
- **Section 17 row 13**:
  - "for every extra same-language audio track" becomes "for every extra audio track in a wanted language";
  - "the reverse test of row 5 not met" becomes "row 5's reverse commentary pattern not met";
  - add, after the sentence on the fitting disc: "A candidate in another language than the reference gets at most the verdict `commentary-cross-language` (kept and flagged, never dropped): on a programme track it is a false verdict like the commentary verdict, but it does not count towards the 3 discs with a commentary verdict, and the answer covers only candidates in the reference's language. Content may drop a candidate in another language only after a further yes for which the labelled tracks include at least 3 dubs in a wanted language from at least 2 discs, at least 1 of them on a dialogue-heavy title, with no commentary pattern on any of them, and the owner's decision.";
  - in the list of kinds the owner brings, "every isolated score, M&E track, second same-language dub or narration" gains "and every dub in a wanted language".

  The phrases `audio_labels.exp13_requirements` reads are untouched by these edits, and the new dub phrase is the one it reads optionally (`at least {n} dubs in a wanted language from at least {n} discs, at least {n} of them on a dialogue-heavy title`).
- **8.8**: `audio_reference_unclear` now means "the reference looks like a commentary over a candidate"; `commentary_detected` is also raised by row 4b, and stays a review reason after experiment 13's yes for a candidate in another language.
- **8.6 step 5 (a defect to raise now)**: with `original_language_first` off, English goes first, and on Pan's Labyrinth the first and default track would be the English commentary, the only English stream. Until content may confirm a commentary, a lossy stereo English track that is the only English stream of a foreign title should not take the default flag over the original-language main (question 2 below).

### 9.6 What stays PROVISIONAL, and the data that settles it

- **Every threshold of section 3**, unchanged and still fitted on the S03E16 commentary. Nothing here was re-tuned: not AG_MIN or the tenths share for the S03E12-13 miss, not anything for Pan's Labyrinth, not anything for the review's dub stand-ins.
- **Row 5** has no number, but it inherits row 4's. It is settled by the swapped references the harness reports from every labelled non-programme candidate, and by any real title whose top-ranked track is not the programme.
- **Row 4b** has no number. It is lifted, if ever, by real dubs in a wanted language (row 13's proposed floor: 3 dubs, 2 discs, 1 on a dialogue-heavy title, none with the pattern) and the owner's decision. The review's stand-ins say what to look for: dubs whose lines run past the original's, dry booth voices, dialogue-dense titles (anime, sitcoms), a 2.0 original as the reference.
- **The reference rule's G1 and G2 approximation** (two channels or fewer instead of 256 kb/s; G2 without "no later criterion"). It is settled by mono and 2.0 films with a lossy stereo commentary, and by any disc where the lossless track is not the first of its language.
- **`ISO639`**: ffmpeg's ISO 639-1 to 639-2 pairs plus TMDb's own codes. Not checked against TMDb's language list (no network); a code missing there makes `args` say `original language unknown (TMDb '<xx>' ...)`, and Step 15 stops on it.
- **Two languages under one tag** (Cantonese and Mandarin as `chi`): not separable by tags; `args` notes the layout only when the first `chi` stream is lossless or has more than two channels, and the owner's label decides.
- **Pan's Labyrinth's commentary is recorded as fitted.** It meets the forward pattern by 0.044 (AG), 0.011 (SMI) and exactly 90% of tenths, and both the reference rule and the reverse rule were changed after seeing it. Spec row 13 says that a track used to fit a later threshold change is listed in `corpus/owner/audio-fitted.json` and counts no more, so the controller records it when applying `final/` (9.8, Step 13). It is not held-out evidence for the changed rule; feature-film commentaries from other discs are. Its disc still counts for its other tracks. (Under the final rule it gets `commentary-cross-language`, which does not count towards the 3 discs with a commentary verdict in any case.)

### 9.7 Changes to the scripts

Complete new contents are in `final/`:
- `audio_similarity.py` (the rule):
  - keeps the whole reverse run (`REVERSE_KEYS`, `rev_<name>`);
  - adds `reverse_measures()`, `reverse_pattern()` and `swapped()` (its docstring states both of its approximations);
  - `verdict()` applies rows 3 to 5 as above. Measures without the reverse block fall back to the earlier test (`EARLIER_REVERSE`, keep-only). `RULE` loses `reverse_gap_s_min` and `reverse_ag_max`, so `--set REVERSE_AG_MAX=...` is now refused;
  - `results()` marks each candidate `cross_language`, turns a cross-language `commentary` into `commentary-cross-language` (row 4b), and makes the peer run (`peer`: stream, measures, verdict), which can turn it into `note-commentary`; the report prints `(cross-language: eng against spa)`, the full `reverse:` line and a `peer:` line;
  - `is_lossless()`, `rank()`, `placed_first()` and `manifest_stream()` (spec 8.6 steps 3 and 4, also from a manifest line, whose profile reads `DTS-HD_MA`) move here from the harness;
  - `ISO639` is the full ISO 639-1 table with TMDb's own codes, and `TMDB_NO_LANGUAGE` (`xx`); `NO_LANGUAGE` moves here; `--titles` adds the `<- wanted x<n> (...)` mark.
- `audio_dataset.py` (the harness):
  - `original_language()` (with `xx`), `programme_reference()`, `wanted()`, `first_of_language()`, `peer_row()`;
  - `levels_args` prints the programme job, then the dataset-only jobs, and the three notes of 9.1; `reference_problem` checks against the programme reference;
  - each candidate carries `cross_language`, `swapped_verdict` and `peer` (verdicts hidden in the snapshot until the track is labelled, the peer run's included); each reference carries `ref_first`;
  - the verdict `commentary-cross-language` has its own column (`c-x`) in the kind-against-verdict table; on a programme kind it is a `FAIL` and a false verdict, but it is not a drop verdict and does not count towards the discs with a commentary verdict;
  - new `FAIL ... swapped reference` (exit 1); new lines in the experiment-13 section: `cross-language`, `dubs` (against row 13's numbers when it has the dub phrase), `swapped references`; a note for every folder whose reference is not the first stream of its language.
- `audio_labels.py`: the kind `dub` (programme, keep); `foreign-dub` reworded as a dub tagged with a wanted language it is not in; `exp13_requirements` also reads the optional dub phrase of row 13 (`dubs`, `dub_discs`, `dub_heavy`; None without it).
- `audio-levels.ps1`: comments only, lines 15-17 and 44-45 (the header no longer speaks of one job per language). ASCII, `measure_overall=RMS_level` once; no code, parameter or output changes. `proxy_levels.py` does not change.
- Tests: `test_audio_similarity.py` 18 -> 30 tests and `test_audio_dataset.py` 41 -> 54 (12 + 13 new). One existing assertion changed: `test_kind_against_verdict` expects one more zero, for the new `c-x` column. The fixture `folder()` gained an optional `langs` argument, and the earlier reverse test still passes through the fallback for measures without a reverse block. Whole suite: 154 tests, OK (`PYTHONDONTWRITEBYTECODE=1 python3 -W error -m unittest discover -s scripts/phase0 -p 'test_*.py'`, in `fix/repo/`: every `scripts/phase0/*.py` of the working copy at 495c3bd, which includes another session's uncommitted `subtitle_text.py` and its 26 tests, with `final/` laid over, `corpus` and `docs` linked; the same suite without `final/` runs 129 tests, so `final/` adds 25).
- Mutation test (`fix/tools/mutate.py`, the review's 24 mutations re-anchored plus 16 on the revision's own paths): 39 of 40 killed. The survivor removes the original language from `wanted()`; it is not equivalent when the original language has several codes (TMDb `cn` with a `chi` reference and a `yue` stream: the mutant leaves the `yue` stream out); a test now covers that layout.
- On a copy of the committed corpus the harness gives `RESULT ok`, with one change since the snapshot: `pans-labyrinth-2006-uhd/all-tracks stream 3 ... (commentary): reference-unclear -> commentary-cross-language`. With Pan's commentary recorded as fitted (9.8), its `not counted` line reads `pans-labyrinth-2006-uhd 1, the-office-2005-s3d3-bluray 7`.

### 9.8 Changes to Task 12 (plan)

**Header.** "Already applied by the controller" gains this addendum's `final/` (the five Python scripts and the comment-only `audio-levels.ps1`), applied in one commit that also records Pan's Labyrinth's commentary as fitted (Step 13).

**Placeholders** (before Step 13). Replace the LOG bullet with:

> - **LOG** is `audio-levels-SLUG-LABEL`, with `-M` appended when `audio_dataset.py args` prints more than one job for the label (it then does so itself: the programme reference's job and a dataset-only job per language that is not wanted), and `-2`, `-3` and so on appended on a repeat.

Replace, in the TITLES bullet, "A title with fewer than two audio streams in every language is measured with its reference alone (no `-Cand`), for the dataset only." with "A title with no other audio stream in a wanted language (English, the original language) or with no language code is measured with its reference alone (no `-Cand`), for the dataset only; `audio_dataset.py args` prints the job."

**Step 13.** The controller, when applying `final/`, in the same commit (before Task 12 resumes):

```bash
python3 scripts/phase0/audio_labels.py fitted corpus/owner pans-labyrinth-2006-uhd all-tracks 3 --rule "audio_similarity.py $(shasum -a 1 scripts/phase0/audio_similarity.py | cut -c1-40)" --note "reference and reverse rules changed after this track (EXP-13 addendum 9); forward margins AG 0.544, SMI 0.511, 9 of 10 tenths"
python3 scripts/phase0/audio_dataset.py; echo "exit=$?"
```

Expected: `fitted: pans-labyrinth-2006-uhd/all-tracks stream 3: no longer counts towards experiment 13`; then the harness ends `RESULT ok`, `exit=0`, with `not counted: pans-labyrinth-2006-uhd 1, the-office-2005-s3d3-bluray 7` and the change `pans-labyrinth-2006-uhd/all-tracks stream 3 in audio-levels-20ms-1-2-3 (commentary): reference-unclear -> commentary-cross-language`. It also covers any later rip of Disc A that carries the same packets (`eng-only` stream 1, today a reference with nothing to judge).

Then Step 13's own checks gain two lines:

```bash
grep -c 'def placed_first' scripts/phase0/audio_similarity.py
python3 scripts/phase0/audio_labels.py check corpus/owner
```

Expected: `1`; and `ok ...` for each labels file and `ok corpus/owner/audio-fitted.json (1 entries)`. The unit tests still end `OK`.

**Step 15**, full replacement:

- [ ] **Step 15: Choose the reference and the candidates of each title**

Which disc is in the drive: does not matter (this runs on the Mac).

For each (SLUG, LABEL) in TITLES (the Office pairs and SLUG_A first; the recordings must have been pulled with `ffprobe-streams.json` in them):

```bash
python3 scripts/phase0/audio_dataset.py args corpus/owner/SLUG LABEL
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
audio = [s for s in probe["streams"] if s.get("codec_type") == "audio"]
print(f"{slug}/{label}: title {rip['title']}, {len(audio)} audio streams in the file, {len(flags)} in the scan")
for k, s in enumerate(audio):
    tags = s.get("tags", {})
    f = flags[k] if k < len(flags) else None
    marks = [m for bit, m in ((1, "director-comments"), (2, "alt-director-comments"), (4, "visually-impaired"),
                              (512, "secondary"), (2048, "derived")) if f is not None and f & bit]
    print(f"  stream {s['index']}: {s.get('codec_name')} profile={s.get('profile')} {s.get('channels')} ch "
          f"{s.get('channel_layout')} lang={tags.get('language', 'und')} title={tags.get('title')!r} scan_flags={f} {' '.join(marks)}")
EOF
```

Expected:
- From `args`:
  - a first line `SLUG/LABEL: <n> audio streams (from <source>); original language <codes> (TMDb <xx>)`, or `original language English (TMDb en)`, or `original language none (TMDb xx: ...)`, or `original language unknown (<why>)`;
  - one `stream <n>: ... lang=<code>` line per audio stream;
  - the line `  programme reference: stream M (<codec ...>), placed first in <the original language | English | any language (no stream is in a wanted language)> (spec 8.6 steps 3 and 4); candidates: every other stream in <eng[, xxx]> or with no language, whatever its language`;
  - possibly one or more `  note: ...` lines: `the file has no stream in the original language` (an English-only rip of a foreign film; Disc A's `eng-only` holds only the commentary: measure it alone as printed), `the reference is not the first <lang> stream of the file` (a score, an M&E track or a commentary may be ranked above the programme; or two languages under one tag), or `the reference is a lossy stream of 2 channel(s), and stream <k> with no language code is lossless or has more than 2 channels`. Measure as printed, and name every note in the report with the title;
  - possibly `  dataset only: <lang> is not a wanted language; stream <k> against <...>` lines;
  - the job and `wait.sh` lines: first the programme reference's job (`-Main M -Cand C`, or `-Main M` alone when no other stream is wanted), then one dataset-only job per such line.
- M is the stream that spec 8.6 places first in the work's original language (most channels, then lossless, then the earlier stream; a lossy stream of two channels or fewer comes neither before a lossless one nor before the language's first stream when that has fewer channels), else in English. C is every other stream in English, in the original language or with no language. An English commentary or an English dub on a foreign film is a candidate against the original-language main track (EXP-13 addendum 9.1); content never drops it (row 4b). Keep derived cores (`derived`) and flagged streams in C: a core is a real lossy companion of the reference, and a flag is a free label.
- From the listing: one line per audio stream with ffprobe's stream number, codec, profile, channels, layout, language, track name, the scan's flags for that stream (k-th audio stream of the file against the k-th of the scan), and the names of the flags that are set.
- For the Office disc (original language English): `-Main 1 -Cand 2,3` (e16-stereo and t3-all-tracks) and `-Main 1 -Cand 2,3,4` (rehearsal), no note. For Disc A (Pan's Labyrinth, original language Spanish): `-Main 1 -Cand 2,3` for `all-tracks`, no note; `-Main 1` with the no-original-language note for `eng-only`. Their folders `audio-levels-20ms-1-2-3` and `audio-levels-20ms-1` already exist: Step 16 then says `already exists`, and Step 17 analyses them again.
- If `args` says `original language unknown (TMDb '<xx>' is not in audio_similarity.ISO639)`, stop and report: the table needs that code before the title is measured. If it says `no readable tmdb.json`, the title was not identified on TMDb; English is the reference language. Note the title for the report and go on.

If the two counts of audio streams differ, match the streams by codec, channels and language by hand, and say so in the report.

**Step 16**: replace "For each (SLUG, LABEL) and each language line of Step 15:" with "For each (SLUG, LABEL) and each job line that `args` printed in Step 15 (the programme reference's first):". The rest is unchanged. The `levels:` line's `main=` is the programme reference, which on a foreign film may be in another language than some candidates.

**Step 17**: in Expected, replace the candidate bullets with:
- for each candidate, either `verdict: duplicate of stream <n>` with its `row:` line, or:
  - its head line, ending `(cross-language: <candidate language> against <main language>)` when the two languages differ;
  - the lag and overlap line;
  - the `DC ... AG ... tenths ... SMI ... LS ... ADL ... added` line;
  - the `reverse:` line: `reverse: <s> s of gaps, DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x> dB, added <s> s: <no commentary pattern | commentary pattern (the main track looks like a commentary over this one) | not judged (<reason>)>`, or `reverse: not judged (too short)`;
  - for a cross-language candidate with the commentary pattern whose language has another stream, a `peer: against stream <k> (placed first in the candidate's language): DC <x>, AG <x>, tenths <a>/<b>, SMI <x>, LS <x> dB: <commentary pattern | <verdict>, so not confirmed>` line;
  - `verdict: <commentary | commentary-cross-language | reference-unclear | ad-pattern | note-commentary | note-ad | keep>` with its meaning, and `not judged: <reason>` when row 3 of the rule applies (`reference-unclear` may come with it: the reverse run has the commentary pattern although the forward run cannot be judged), or only `verdict: not-judged (too short)` for a track under 20 s;
  - a `row: | ... |` line for notes.md.

and add, after the Office checks:
- For Disc A, `all-tracks/audio-levels-20ms-1-2-3`, the second check of the reference and reverse rules (EXP-13 addendum 9.4, table A):
  - stream 3 (the English commentary) must come out `commentary-cross-language`, marked `(cross-language: eng against spa)`, with DC 0.08, AG 0.54, tenths 9/10, SMI 0.51, LS 10.8 dB, `reverse: 470 s of gaps, DC 0.06, AG 0.31, tenths 1/10, SMI 0.39, LS 17.1 dB, added 796 s: no commentary pattern`, and no `peer:` line (it is the only English stream);
  - stream 2 must come out `keep` with DC 1.00.

  Its earlier output said `reference-unclear`: replace `audio-content-20ms-1-2-3.txt` with the new output (`tee`), and say so in the report.
- For Disc A, `eng-only/audio-levels-20ms-1`: `no candidate stream: nothing to judge (measured for the tuning dataset only)`.

**Step 18**:
- In item 1's list of answers, add "dub (the programme in another language, tagged with that language)" after "isolated score, M&E, same-language dub".
- In the notes.md template, replace the first sentence with: "Extra audio tracks measured against the title's programme reference (the track spec 8.6 places first in the original language, else English; EXP-13 addendum 9.1) (`scripts/phase0/audio_similarity.py`; full output in `<label>/audio-content-20ms-<M>-<C>.txt`)."
- In item 5, add `dub` to the list of kinds.

**Step 19**:
- In point 1's list, add "dubs in a wanted language (row 13's proposed floor: 3 from 2 discs, 1 on a dialogue-heavy title; they gate only row 4b)".
- In point 2, add: "a foreign-language film with an English dub (and, if any, an English 2.0 companion of the dub), the one kind no stand-in can replace (EXP-13 addendum 9.2), and a mono or 2.0 film with an isolated score (9.3)".
- In point 3, the first condition becomes: "its `<-` mark names `eng` or the original language of the first line (TMDb gives two letters; the scan uses the three-letter code, for example `fr` is `fra` or `fre`), or is `<- wanted x<n> (...)` (one stream in each wanted language, such as an English commentary on a foreign film)".
- In point 3, the Expected description of `--titles` gains, after "`<- <language> x<n>` when one language has two or more streams,": "`<- wanted x<n> (<lang> <k>, ...)` when no wanted language has two streams but the wanted languages and no-language streams have two or more together,".

**Step 20**:
- In point 1, the false-verdict definition becomes: "a track with the verdict `commentary` or `commentary-cross-language` that the owner labelled as programme audio (`main`, `lossy-companion`, `alternate-mix`, `laugh-track-version`, `derived-core`, `dub`), or ...". The harness's `FAIL ... (programme audio), verdict commentary-cross-language` is such a false verdict.
- In point 1, "Answer yes" gains: "The yes covers candidates in the reference's language only: `commentary-cross-language` is never dropped, and it does not count towards the 3 discs with a commentary verdict (the harness counts `commentary` only)."
- Add a bullet: "**Swapped reference:** `exit=1` with `FAIL ... swapped reference` means that, were that labelled track ranked first, the programme would get the commentary verdict. Report it with both runs' measures. Until the owner decides question 4 below, treat it as blocking a yes."
- In point 4, the track table gains the columns "cross-language", "peer" and "swapped" (the harness snapshot's `cross_language`, `peer` and `swapped_verdict`), and the report states the harness's `cross-language`, `dubs` and `swapped references` lines, and every first-of-language note.
- In the consequences for the spec, add: D15's and row 13's wording (9.5), 8.8's meanings of `audio_reference_unclear` and `commentary_detected`, and the new rule row 4b.

**Global Constraints, the tuning-dataset paragraph, and Task 11 Step 11** (outside Task 12, but `args` changed under them):
- "prints one `scripts/phase0/job.sh audio-levels.ps1 ...` line and its `wait.sh` line per language with two or more audio streams, or one job for the reference alone" becomes "prints one job line and its `wait.sh` line for the title's programme reference against every other stream in a wanted language (or the reference alone), then one per language that is not wanted and has two or more streams (dataset only)".
- "`audio_dataset.py` exits 1 when ..." gains "or when a labelled programme track gets `commentary-cross-language`, or when a labelled candidate, taken as the reference, would give the programme the commentary verdict (`swapped reference`)".
- In Task 11 Step 11's Expected for `args`, the first line ends `; original language <...>`, and a `programme reference: ...` line (and possibly `note: ...` and `dataset only: ...` lines) come before the job lines. Under `exit=1`, the bullet `labelled <kind> (programme audio), verdict <commentary|duplicate>` becomes `verdict <commentary|commentary-cross-language|duplicate>`, and two bullets are added: "`swapped reference: labelled <kind> ...`: stop and report with the track's measures (EXP-13 addendum 9.3)" and "`note: ... is not the first <lang> stream of the file`: not a failure; name it in the report". The wrong-reference bullet reads "`wrong reference: measured against stream <M> ..., but the top-ranked <lang> stream is <X> (...), the title's programme reference (placed first in ...)`".

### 9.9 Questions for the owner

1. **Question 10 of section 1 (position veto, keep-only): answered yes by the owner, 2026-10-06 (D21).** It is row 0 of 9.5. The question was: in constructions, a score ranked above the programme (a lossless 2.0 score beside a mono film, with noise between its cues) gives a dry programme the commentary verdict, and only SMI keeps a compressed one from it (9.3). May the content rule refuse to confirm a commentary when the programme reference is not the first track of its language on the disc (when that first track is lossless or has more than two channels)? It reads position only to keep, never to drop. This revision recommended yes.
2. Spec 8.6 step 5: with `original_language_first` off, the English commentary of a foreign film whose only English stream it is (Pan's Labyrinth) becomes the first and default audio track. Disc A's `eng-only` rip shows the same from the selection side: an English-only selection of this film keeps only the commentary. Should a lossy stereo English stream that is the only English stream of a foreign title never take the default flag from the original-language main, until content may confirm it?
3. Do you have a foreign-language film with an English dub, and if possible an anime or TV series with one? They are the one kind no stand-in replaces, and row 4b (a candidate in another language is never dropped by content) stays until at least 3 such dubs from 2 discs, one of them dialogue-dense, have been measured with no commentary pattern. A film with dubs in other languages would also help: their dubs can be measured against the programme from the dataset's proxy (`proxy_levels.py`, after `tuning-fetch.ps1`) without the disc. When those dubs exist: should content then be allowed to drop a commentary in another language than the film (an English commentary on a foreign film), or should row 4b stay for good?
4. When a labelled track, if ranked first, would give the programme the commentary verdict (`swapped reference`), should that block experiment 13's yes, like a false verdict? This addendum assumes yes.
5. The review offered a format veto as an alternative or an addition to row 4b: never confirm by content a candidate that is lossless or has more than two channels. It would also protect a 5.1 dub tagged with the original's language (row 4b cannot: it sees only tags), but not a Dolby Digital 2.0 dub, and it reads the format (codec and channel count), which this design does not use as evidence (section 2; D15 sets aside stereo width and position). Do you want it?
6. A voice-over translation (a translator talking over the audible original) in a wanted language: keep or drop? To this rule it looks like a commentary (open question 3); in another language than the reference it gets at most `commentary-cross-language`.
7. D15 says a candidate is compared with the main track of its language. This addendum compares every wanted-language candidate with the title's programme reference and never drops one in another language by content (9.5). May D15 be reworded so? The proposed wording (9.5) replaces "compared with the main track of its language" with "compared with the title's programme reference, the track 8.6 places first in the original language, else in English (EXP-13 addendum 9.1); a candidate in another language than that reference is never dropped by content until real dubs have been measured (addendum 9.2)". The spec's section 17 row 13 already has 9.5's wording; D15 is unchanged until you answer.
8. Row 4b keeps `commentary_detected` as a review reason before and after experiment 13's yes, for a candidate in another language than the reference. D15 leaves to you whether a track with the commentary pattern also sends the job to review (against D12's "no review"), and question 8 of section 1 asks it only for the time before a yes. Should row 4b's flag hold the job for review after a yes, or only add a note? Until you answer question 8 of section 1, row 4b's "flag for review" means whatever that answer decides (review, or only a note).

Recorded without a question (it was question 4 of the first version): Pan's Labyrinth's commentary is listed in `audio-fitted.json` (9.6, 9.8), as spec row 13 requires for a track a later rule change was made after.

### 9.10 The adversarial review: what was done with each finding

Each finding was reproduced with the delivered scripts before anything was changed (`fix/out/confirm1.jsonl`, `confirm2.jsonl`, `harness-delivered.txt`, `args-probes.txt`).

| # | Finding (severity) | Reproduced | What was done |
|---|---|---|---|
| 1 | An English dub of a foreign film is now judged against the original, and loosely timed dub stand-ins with a dry voice get `commentary`, further inside the pattern than Pan's real commentary (blocker) | yes: T 250/800 raw DC 0.01, AG 0.60, 10/10, SMI 0.57, LS 10.6; E 250/800 compressed AG 0.54; P 250/1200 +6 dB compressed AG 0.60, LS 6.1; all `commentary` | **Applied, structurally.** Row 4b: a cross-language candidate gets at most `commentary-cross-language` (keep, review; never dropped by content, `drop_commentary_by_audio` does not apply), in `results()`. In the harness it is a FAIL and a false verdict on a programme kind, has its own column, and does not count towards the discs with a commentary verdict. 9.2 rewritten with the numbers; "dry booth voices with timing slop on dialogue-dense titles" are named as what to look for. No threshold changed. The format veto is offered to the owner (question 5). Not covered: a dub tagged with the reference's own language (9.2) |
| 2 | A stem ranked above the programme still gives the programme a false commentary verdict; the addendum's guards 1-4 do not cover it (important) | yes: `W-P-scoreref-dryprog` 21>1 and 21>2 `commentary`; `W-P-scoreref` 21>4, `W-P-commref` 3>4, `W-e15-flref` 31>4 kept by SMI alone; `args` makes the score the reference | **Partly applied.** 9.3 now says that SMI alone is the guard there and that nothing guards a dry programme; the held-out data list gains scores with noise between cues; `swapped()` states the centre-channel approximation for a 5.1 stem. `args` and the harness note every reference that is not the first stream of its language (when that stream is lossless or has more than two channels). **Declined:** making question 10's veto part of the rule without the owner: the design reserves position to a keep-only veto only the owner may allow, and D15 calls positional rules brittle. Question 1 now recommends it, with this evidence |
| 3 | The per-language second opinion was rejected for a decode and a cross-folder verdict it does not need (important) | yes: `S-T-dub` cand 12 `commentary` (DC 0.01, AG 0.64); its own-language run DC 1.00, AG 0.00 | **Applied** as the peer run (9.1, section 2 item 14); 9.1's claim corrected: no decode, one folder; it protects companions, not the lone dub |
| 4 | Pan's commentary counts as experiment-13 evidence although the rules were changed after it (important) | yes: `commentaries 1 of 8, from 1 of 4 discs`, `commentary verdicts on 1 of 3 discs` | **Applied.** Recording it in `audio-fitted.json` is a step for the controller in the commit that applies `final/` (9.8, Step 13); question 4 of the first version became a statement. Checked on a copy of the corpus: `not counted: pans-labyrinth-2006-uhd 1, ...` |
| 5 | Experiment 13 can say yes with no dub measured; the harness does not count `dub` (important) | yes | **Partly applied.** The harness prints a `dubs` line (count, discs, dialogue-heavy titles, how many with the pattern), against row 13's numbers when the row has the proposed phrase, which `exp13_requirements` reads optionally; 9.5 proposes the phrase (3 dubs, 2 discs, 1 dialogue-heavy). **Declined:** adding dubs to the main "not answered" reasons. Under row 4b the yes never covers a candidate in another language, so missing dubs do not bear on it; they gate lifting row 4b instead. A dub with the pattern still makes the answer no (false verdict) |
| 6 | 6 of 24 mutations survive; two matter (minor) | yes | **Applied.** New tests: G1 alone, a dataset-only folder, every verdict of an unlabelled track hidden in the snapshot (swapped and peer included), a reference labelled unwanted is no swapped-reference FAIL, `en` named English, plus tests for row 4b, the peer run, ISO639 and the notes. Of 40 mutations (the review's 24 re-anchored, 16 of the revision's own) 39 are killed; the survivor drops the original language from `wanted()`, which survives only because no test mixed two codes of the original language; a test now does |
| 7 | Edge cases: an `und` lossless main loses the reference to a lossy stereo English stream; Cantonese and Mandarin under one tag; 46-code ISO639 (minor) | yes, all three with `args` | **ISO639 applied** (ffmpeg's ISO 639-1 to 639-2 pairs for 190 codes, `sh` given its codes by hand, `mo` given `mol`, plus TMDb's `cn`; `xx` means no language). **Cantonese/Mandarin**: noted in 9.1 and 9.6, and `args` notes the layout when the first `chi` stream is lossless or has more than two channels. **`und` main: the broad fix declined** (ranking English and no-language streams together would let a `zxx`-tagged score become the reference); `args` notes the layout instead, the app never judges no-language streams, and the harness already fails on a reference labelled commentary |
| 8 | `audio-levels.ps1`'s header still speaks of one job per language (minor) | yes | **Applied**, comments only, in `final/audio-levels.ps1` (ASCII; no code changed) |

What the review found to hold was not re-argued here; its "holds up" list was rechecked by the runs of 9.4 (E): programme copies in another slot are kept, row 5 fires only with a commentary as the reference, and removing the old reverse test lost nothing measurable.
