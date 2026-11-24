# Phase 2 pull request 7: acceptance, design notes

Written 2026-11-23 from the brainstorm with the owner, on `main` at 324ae50 (pull requests 28 and 29 merged). The
binding design is `docs/superpowers/specs/2026-10-21-phase-2-engine-design.md` (parts 6.3, 9.5, 11.3, 12.6 and 12.7;
the owner's decisions P8, P10, P11, P21, P22, P24, P25, P27, P28 and P36; ruling R-A2) as the notes of pull requests 5
and 6 amend it (`docs/superpowers/specs/2026-11-06-phase-2-pr5-design-notes.md`: B1 (scope), G1;
`docs/superpowers/specs/2026-11-10-phase-2-pr6-design-notes.md`: C1), and, above it,
`docs/superpowers/specs/2026-10-04-lazymkv-design.md` (section 16 row 2, section 17 row 6). These notes record what
the owner decided for this pull request, the amendments that follow, the calls the planner settles, and what is left
to the builder. The plan that follows is written with superpowers:writing-plans and executed subagent-driven, as pull
request 6 was. Its workspace rules: Rust is built and tested on the Mac (`--locked --offline`) and by CI on the three
systems, and nothing is built on the mini PC; signed commits as
`krcm0209 <17315774+krcm0209@users.noreply.github.com>` with no attribution line; no secret, title of the owner's disc,
job ID, host, user name or drive serial in the repository, the ledger or the notes; RP21; no subtitle text in any file.

## 1. Intent

Phase 2 ends when real discs prove it (spec section 16 row 2): a disc goes in, the owner gives no command, and a
correctly named file with the right tracks appears in the library. Pull requests 2 to 6 built that on stand-in tools,
on recordings and in single steps on the mini PC. No whole job has yet run on a real disc with the programs a user
would install, no real job has read several English subtitle streams (T5-R47), and nothing has compared what the
engine decided inside a job with what `lazymkvctl plan` and `decide` say about that job's recording afterwards (P11).

Why now: it is the last pull request of phase 2, and phase 3 (unsure discs, the review commands) builds on a phase 2
that is known to work.

## 2. Scope

In:

- `lazymkv-dev compare-job` (design part 12.7; G1);
- a way to put the release zip's tree behind the desktop task (decision A1, below);
- a fault in the relayed `decide` that reading the code for these notes turned up (call 9);
- the identification rule of ruling A2 (section 3);
- a rehearsal of the comparison without a disc, on the jobs the mini PC already holds (call 16);
- choosing the second disc (P10, P27);
- the sitting on the mini PC: design part 12.7's checklist on both discs;
- the phase 2 notes (`docs/phase2/engine.md`): results, findings, the spec facts the run verifies, the test count,
  the deferral of the DVD disc ID [LIB-6], and the stale lines that still place `compare-job` in pull request 6;
- any fix the run shows to be needed, each as its own commit.

Non-goals:

- a public release or a tag (P28 stands);
- any phase 3 behaviour: a plan in copy mode still fails with the disc left in, and a held job is approved unchanged;
- the points of pull request 21's review that are marked "not yet ruled on", except X1's question in call 22;
- menu maps: the engine passes no map, so they play no part in the run;
- a DVD or a TV disc;
- the frozen OCR paths (T5-R54). A fault the run finds there is recorded as a finding; reopening the trial is the
  owner's decision.

Anti-goals (these would make it a failure even if the checklist is ticked):

- a pass reached by loosening `compare-job` after it printed a difference. A difference is ruled on by the owner;
  the command's rules do not change during the sitting;
- a disc title, job ID or path of the owner's entering the repository;
- a job recording committed anywhere (P22);
- the owner typing long commands or copying values by hand. The owner swaps discs, says yes or no, and looks at
  Jellyfin.

## 3. The owner's decision (2026-11-23; the git-ignored `.superpowers/pr7/decisions.txt` holds the dated line)

- **A1. The programs under test.** Acceptance runs the release zip that the release workflow builds for the pull
  request's head, not a build made on the mini PC. It tests what a user would install, the bundled `ffmpeg.exe`,
  `ffprobe.exe` and `mkvmerge.exe` included, and needs no build on the mini PC, whose system drive had no room for
  one on 2026-11-21.

A1 amends, for the acceptance run only:

- P24 and R-A2 ("the phase 2 engine on the mini PC uses the tools already installed there") and pull request 6's C1
  ("the desktop task keeps the installed tools"): during acceptance the desktop task runs the zip's tree with the
  zip's tools;
- design part 12.7, "Before the first disc": "The built binaries are in the run folder" becomes "The release zip's
  tree is in the run folder, and its SHA-256 (Secure Hash Algorithm 256) is the workflow's".

- **A2 (2026-11-23, during the sitting). A film whose name a TV entry shares.** Both shortlisted second discs were
  identified at step 5 because TMDb lists a TV entry with the film's exact name. The owner ruled: when preferring the
  disc's structural kind leaves exactly one result, that result's record is fetched, and if the runtime check passes
  it is step 4. Built in this pull request before the second disc runs (spec 8.1 rows 4 and 5; `core::lookup`). The
  fetch is the one film among TV entries: the record is chosen while the scan runs, before the plan gives the
  structural kind, and the choice takes no kind, so later rounds fetch the same record; a TV disc of that shape keeps
  step 5 until TV is built (phase 4). (Wording corrected after the final review recorded in `docs/phase2/engine.md`,
  finding A2-2; it said the lookups run before the scan.)

## 4. What the code does today

Read on 2026-11-23; each line is a fact the calls below rest on.

- `state.json` is `core::record::JobRecord`; its `plan` is a `core::plan::Plan`. `lazymkvctl plan` prints a
  `PlanDoc` whose `plan` key is the same type (`crates/dev/src/cmd/plan.rs`). The two can be compared key for key.
- `<title key>/decide.json` is `EngineDecideDoc`: decide's `DecideDoc` followed by two keys, `audio_rule_version`
  and `lazymkv_version` (`crates/dev/src/cmd/decide.rs`). On stand-ins, `decide` over the job folder already prints
  that file's bytes without the two keys (`crates/fake-tools/tests/whole_job.rs`), on all three systems.
- Design part 12.7 says `compare-job` "ignores exactly the job ID and slug, times, staging and partial paths, and
  `fetched_at`". The two plans hold none of these. In the two decide documents of one job folder the slug is the job
  ID on both sides, the paths name the same files, and no time or `fetched_at` key exists; `fetched_at` lives only in
  the saved TMDb copies.
- A job that was planned with a lookup pending has its plan made again at Decide and written back into `state.json`;
  `refresh_identity` adds the sentence "the identity was made again with the answered lookups (8.1): ..." to the
  plan's evidence (`crates/core/src/plan/mod.rs`). `lookup_pending` is false afterwards, so that sentence is the
  mark.
- `decide --job` is given `--text` only once the job's tuning copy is done (`crates/engine/src/relay.rs`). Before
  that, its subtitle section is skipped while the engine's is present.
- Once the copy is done, `decide --job` is given `--text` for every title, also for one whose step 10 never ran
  (one English subtitle stream, as on Pan's Labyrinth). The engine wrote that title's subtitle section as "no OCR
  text" (`staged_text` in `crates/engine/src/file_phase.rs`, `no_text_entry`); `decide` with `--text` looks for
  the text, finds none and skips the section with another sentence (`reading_folder` and `subtitle_section` in
  `crates/dev/src/cmd/decide.rs`). Read from the code, not yet run: the two documents then differ. The stand-in
  comparison of `whole_job.rs` passes no `--text`, so it does not meet this.
- A filed disc that is inserted again is answered with `already_ripped` and ejected (design part 6.3). The mini PC's
  data folder holds two filed jobs: pull request 4's disc and pull request 5's replay of Pan's Labyrinth. Remembered
  decisions are read but never written in phase 2 (design part 7.4), so those jobs change no later plan.
- The release workflow runs for a pull request only when the crates, the packaging or the build files change, or
  when it is started by hand; its artifact is kept 14 days (`.github/workflows/release.yml`). The zip does not
  hold `scripts/engine-task.ps1`.
- The engine renders the decision with what the library already holds (`library_facts`); `decide` on a folder uses
  empty facts. For a film the library does not hold, the two are the same.
- `lazymkvctl` runs only `plan`, `decide` and `retune` of the developer commands (`LOCAL_COMMANDS`,
  `crates/ctl/src/lib.rs`). `compare-job` is reachable as `lazymkv-dev compare-job` only, and `lazymkv-dev` is not
  in the zip.
- The tool lookup takes a settings path first, then the `resources` folder beside the engine's `bin`, then the
  install folder, then `PATH` (`crates/tools/src/lookup.rs`). `resources` counts only when neither the folder nor
  the file can be written by the user's filtered token. The mini PC's settings name the installed `ffprobe` and
  `ffmpeg` by path.
- `scripts/engine-task.ps1` installs only from `C:\lazymkv\target\release` into `C:\lazymkv\run`. Pull request 6 ran
  the zip's tree on the mini PC as a console engine over SSH, never through the desktop task; only the desktop
  session reaches the share (P5).
- Since review round 1 of pull request 21 the programs link the C runtime statically (X1). No program built that
  way has run on the mini PC.

## 5. Approaches

**The programs under test.** The release zip (chosen, A1), or a build on the mini PC as pull requests 3 to 5 ran.
The build would need disk space freed first and would use the FFmpeg installed on the mini PC, so the bundled build
would end phase 2 without having run a job.

**The comparison.**

| Approach | What it is | Trade-off |
|---|---|---|
| **A. A comparer of saved files (chosen)** | `compare-job` reads the job folder and the saved output of `lazymkvctl plan --job` and `decide --job`, and prints the differences | What the design describes. The outputs come from the zip's own `lazymkvctl` through the engine, so the shipped programs are what is checked; the comparer itself runs anywhere |
| B. A comparer that runs `plan` and `decide` itself | `compare-job` calls dev's library on the job folder | One step fewer, but it would compare the engine with the comparer's own build, not with the zip's `lazymkvctl`, and it could not run where the job folder's share paths do not resolve |
| C. No command | Compare the files by hand with a diff tool | Nothing to build. Not repeatable, and the lookup-pending rule would be applied by eye |

## 6. The planner's calls (settled here; the owner may veto any)

**`compare-job`**

1. **It reads files only.** It never contacts the engine, runs a tool or reads the share. It runs on the Mac, on a
   copy of the job folder pulled from the mini PC and on the saved outputs.
2. **Its form.** `lazymkv-dev compare-job <job folder> --plan FILE [--decide KEY=FILE ...] [--plan-and-audio]`.
   `--plan` is the saved output of `lazymkvctl plan`; each `--decide` pairs one of the job's title keys with the
   saved output of `lazymkvctl decide` for it. It gets a row in dev's `COMMANDS`; `lazymkvctl` does not gain it.
3. **The whole comparison (the default; the job's own recording).** `state.json`'s `plan` must equal the saved
   plan document's `plan`. Each title's `decide.json`, without its last two keys, must equal the saved decide
   document. Values are compared as parsed JSON, so line endings and key spacing do not count; everything else
   does. Nothing is ignored. Every title of the job that has a `decide.json` needs its `--decide`, and a missing or
   unknown key is an argument error. This amends design part 12.7's sentence "It ignores exactly the job ID and
   slug, times, staging and partial paths, and `fetched_at`": on one job folder those fields are absent or equal
   (section 4), the stand-in job already compares equal byte for byte, and a difference on the real machine is a
   finding the owner should see.
4. **A job planned with a lookup pending.** Recognised by `refresh_identity`'s sentence in `state.json`'s plan,
   through a constant that `core` exports so the two cannot drift. The plan comparison then covers the plan's
   `identity` only, and each decide comparison covers everything except its plan section, of which only the
   `identity` is compared. The output says so in one line (design part 12.7; RP-4).
5. **The plan-and-audio comparison (`--plan-and-audio`; another recording of the same disc).** For design part
   12.7's second Pan's Labyrinth run, on phase 0's recording with the job's `settings.json` and without `--text`.
   It compares the whole plan, and for each `--decide` pair the audio section and the audio tracks of the decision
   section. It compares nothing else, and its output lists what it left out (the slug, the label and the title key,
   the input paths and the lookup status, the reconcile section, the subtitles; since the final review's finding
   CR-3 (`docs/phase2/engine.md`), the printed line names every part of a decide document that this mode does not
   compare, the reviews, the plan section's `settings_problems` and `plan`, and `menu` among them). The phase 0
   side's outputs come from the zip's `lazymkvctl` on the mini PC, in its local form, over the copy of the
   evidence clone that the mini PC holds, or over a copy of that one recording when the clone there is missing or
   behind. Phase 0's recording holds lookup answers saved in October, so a plan difference that comes from
   changed TheDiscDB or TMDb data is possible; it is a finding, and the rehearsal of call 16 shows it before the
   sitting.
6. **When the outputs are saved.** After the job's tuning copy is done, so that `decide --job` is given `--text`.
   They are saved on the mini PC as bytes (through `cmd.exe`'s redirection, as `assemble.ps1` does), never through
   Windows PowerShell's `>`.
7. **Output and exit codes.** One line per difference: which document, the JSON pointer and both values. Exit 0
   with no difference, 1 with any, 2 for an argument error or an unreadable file. The printed values can hold a
   film's title, so the notes record pointers and counts, never values.
8. **Tests.** Unit tests on made-up documents for each rule (equal, one difference, the two dropped keys, a
   lookup-pending plan, plan-and-audio, each argument error). Tests on stand-in whole jobs with the tuning copy
   done, whose outputs are made with the words the relay builds for `plan --job` and `decide --job`, `--text`
   included: one job whose title has a single English subtitle stream, so that step 10 does not run, and one with
   two, so that it does. The command exits 0 on each, and exits 1 after one value in a saved output is changed.
9. **The fault of section 4's sixth line is settled before the zip is built.** Its test comes first: the
   single-stream job of call 8, which is expected to fail today. The rule the fix must meet: for a title whose
   step 10 did not run, `decide --job --title` prints the subtitle section the engine wrote. The fix belongs in the
   relay, which gives `--text` only for a title whose OCR text is in the job's dataset copy; `decide`'s own reading
   of a `--text` folder is phase 0's and stays as the goldens pin it. If the test passes today, the reading was
   wrong: the call lapses and the notes say so.

**The zip behind the desktop task**

10. **`scripts/engine-task.ps1` gains `-Zip FILE -Sha256 HEX`.** With them it installs a release tree in place of
   the build folder's three programs:
   - it refuses unless FILE's SHA-256 is HEX, before anything is extracted. HEX is read on the Mac from the
     workflow artifact's `SHA256SUMS`;
   - it extracts the zip into a fresh folder below the run folder, under the run folder's existing protection, so
     that the user's filtered token can write neither `bin` nor `resources`. An earlier tree is removed first;
   - it refuses a tree that lacks any of the three programs in `bin` or any of the three tools in `resources`;
   - it registers `lazymkv-engine` at the tree's `bin\lazymkv-engine.exe` with the settings the task has today,
     starts it and waits for `lazymkvctl status`, as now;
   - every refusal that does not concern the build folder stays (elevation, a running task, an engine already
     answering, `-Data`'s rules, the link and planted-file checks, the run folder's parent with
     `-AcceptWritableBuild`). Without `-Zip` the script behaves exactly as today, and the tests pin both forms
     (`crates/fake-tools/tests/scripts.rs`).
11. **Settings for the run.** The settings in force with `tool_paths.ffprobe` and `tool_paths.ffmpeg` removed, and
    design part 12.7's roots. `makemkvcon` is found in its install folder, as now. The self-test must report
    `mkvmerge`, `ffprobe` and `ffmpeg` as found in `resources`, at the versions of `packaging/tools.json`; this
    line joins "Before the first disc". The earlier settings are saved in the private ledger first.
12. **Which zip.** The `lazymkv-release-dry-run` artifact of the release workflow's run on the branch head that
    holds calls 1 to 10. The notes record the run's number, the commit and the zip's SHA-256, and the zip is kept
    with the evidence of call 20, since the artifact lasts 14 days. A later commit that changes only documents or
    scripts needs no new zip. A fix in the crates or the packaging does: the workflow's run on that head, started
    by hand when the changed paths do not start it.
13. **The data folder stays.** The run uses the mini PC's existing data folder, with its settings changed as call
    11 says. Before Pan's Labyrinth goes in, `lazymkvctl rip-again` is armed on the earlier filed job of that disc,
    so that the insertion starts a new job (design part 6.3).

**The second disc**

14. **What it must be.** Design part 12.7's list (a film TheDiscDB does not know and TMDb identifies, two or more
    English subtitle streams, ideally Atmos or DTS:X), and what the engine needs for the job to end as the
    checklist allows:
    - TMDb gives exactly one plausible result and the runtime check passes (spec 8.1 step 4); amended by A2
      (section 3): or several plausible results, of which exactly one is of the disc's structural kind and every
      other is of the other kind, and the runtime check passes;
    - the plan is in Direct mode with one main title: no second playlist of the same length that the planner
      cannot tell apart (T4-R20);
    - no hold other than `subtitle_ambiguous` is expected, so a disc without a commentary or audio description
      track is preferred.
15. **How it is chosen.** From evidence on hand first: the MakeMKV scans of the owner's discs that the menu spike
    captured (40 discs, outside the repository), each run through `lazymkv-dev plan`; the spike's decide documents
    for the discs it ripped; and the lookups the engine itself would make for the disc (TheDiscDB by disc ID and
    fingerprint, TMDb by name). If no captured disc qualifies, further discs from the owner's collection list are
    scanned in a sitting. The owner is shown up to three candidates by title in the session and picks; the
    repository names the disc by a slug only. If the chosen disc is held for another reason, it is replaced by the
    next candidate and the hold is recorded as a finding (design part 12.7).

**The rehearsal and the sitting**

16. **A rehearsal without a disc.** Once the zip is installed and before any disc goes in, `compare-job` runs on
    the two filed jobs the mini PC already holds: the whole comparison on each, and the plan-and-audio comparison
    of the Pan's Labyrinth one against phase 0's recording. Older builds made those jobs, so what it prints is read
    as findings, not as acceptance results. A difference that the real run would also meet comes to the owner
    before the sitting. It needs the owner's yes of call 18 and nothing else from the owner.

17. **Who does what.** The owner: empties the test library's `Movies` folder, inserts each disc, says yes to each
    approval, and checks each film on Jellyfin. The controller, over SSH: installs the zip and the settings, runs
    the self-test, watches the jobs, runs `lazymkvctl approve` on the owner's yes, saves the outputs, pulls the job
    folders, runs `compare-job`, and reads the notifications through the Home Assistant connector. For the Jellyfin
    check the controller states what the decision names (the title, the default audio track, the default subtitle
    track) and the owner answers yes or no for each film.
18. **Before it starts.** The owner's yes to stopping the engine and registering the task again (a standing
    change); a window agreed with any other session that uses the mini PC; the first step of the plan lists every
    standing change the sitting makes.
19. **Order.** The checks before the first disc; Pan's Labyrinth (held on `commentary_detected`, approved
    unchanged, filed), its two comparisons; the second disc, its comparison and its OCR and audio-profile checks;
    Jellyfin for both; the paths check. A UHD takes about one and a half to two and a half hours to rip before its
    analysis, so the sitting is a day with two disc swaps.
20. **Where the evidence lives.** The pulled job folders and saved outputs stay in the git-ignored
    `.superpowers/pr7/` and are never committed (P22). The notes get the results by slug.
21. **When something fails.** A fault in lazymkv is fixed in this pull request, the new head's zip is installed,
    and the disc runs again with `rip-again` and a fresh `movies_root` (design part 6.3). A fault in a tool or in
    the disc is recorded as a finding and brought to the owner.

**After the run**

22. **X1's open question** (review round 1 of pull request 21: whether the zip's smoke run and the worker check
    are repeated after the static C runtime). The acceptance run counts as the repeat of the smoke run: the three
    statically linked programs run two whole jobs from the zip's tree, and the OCR worker reads the second disc.
    The worker check's memory measurement is not repeated.
23. **What the mini PC keeps.** The zip's tree and the run's tool settings stay installed after the sitting, since
    nothing newer exists; the earlier settings stay in the ledger. The test library's files, the job folders and
    the staging and tuning data are listed for the owner, who removes them or says yes to their removal.
24. **The test count.** Design part 12.6 asks for the mini PC's `cargo test --workspace -- --list` count. With
    nothing built there, the notes record the counts of CI's three jobs on the final head.
25. **The notes.** `docs/phase2/engine.md` gains "Pull request 7": the checklist with each item's result, the
    findings, the profile string `ffprobe` printed for an Atmos or DTS:X track against OR7's spelling (with the
    correction to spec section 17 row 6 and `core` if it differs), the counts, the LIB-6 deferral, and what phase 2
    leaves to phase 3. RP-4 and T16-r1 are corrected to name pull request 7. Spec section 16 row 2 gets its result.
26. **How the branch reaches GitHub.** Branch `phase-2-acceptance`; a draft pull request from the first task, one
    signed commit per task, CI green on the three systems and CodeQL at 0 before the next task; marked ready for
    review only on the owner's word. The plan is written inline with one Opus reviewer.

## 7. For the owner to confirm with these notes

The calls that change an earlier answer or settle a point marked as the owner's:

- call 3: `compare-job` ignores nothing on the job's own recording (design part 12.7 listed fields to ignore);
- call 9: the relayed `decide` is changed in this pull request, if its test fails as expected;
- call 22: the acceptance run stands in for the repeated smoke run, and the worker check is not repeated;
- call 23: the mini PC keeps the zip's tree and its bundled tools after the sitting;
- call 24: the test count comes from CI.

Still to come from the owner during the work: the pick among the candidate discs (call 15) and the yes before the
task is registered again (call 18).

## 8. Risks

- **No captured disc qualifies.** Most of the 40 scans show several English audio tracks or a second playlist of
  the main title's length. The fallback is a scanning sitting on more of the collection.
- **The second disc is held for a commentary.** Each attempt costs a rip of two hours or more. Call 15's use of
  the spike's decide documents lowers the odds; it cannot remove them.
- **First use.** The bundled FFmpeg has measured one rip and never run inside a job; the statically linked
  programs have run no job; `engine-task.ps1`'s new form has run only in tests before the sitting.
- **A strict comparison.** Call 3 may print a difference that the stand-in jobs never met, as section 4's sixth
  line would have. The rehearsal of call 16 is there to meet such a difference before the owner's day; one that
  appears only in the sitting is a finding for the owner, not a reason to change the rule mid-sitting.
- **MakeMKV's licence.** A beta key that has run out stops the run; the self-test reports it before the first disc.

## 9. Left to the builder

File and function names; the JSON pointers that make up "the audio tracks of the decision section" in call 5, shown
in the plan for review; the wording of the output lines; how the script lays the tree out below the run folder and
which of `smoke.ps1`'s checks it shares; the form of the saved-output commands on the mini PC; the ledger's layout
under `.superpowers/pr7/`.
