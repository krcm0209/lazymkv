# lazymkv design

Date: 2026-10-04; updated with the phase 0 results, 2026-10-12 (`docs/phase0/REPORT.md`).
Status: design approved section by section by the owner; revised after an independent review; awaiting the owner's review of this document.

## 1. Purpose

lazymkv replaces the MakeMKV GUI. The owner inserts a disc, walks away, and comes back to an ejected disc (an open tray, or a slot-loading drive's disc pushed out) and a correctly named file in a Jellyfin/Plex library. It drives `makemkvcon` (the owner holds a MakeMKV licence) and decides titles and tracks from the owner's rules and public databases.

Non-goals: transcoding, ripping extras, managing the media servers, bundling or modifying MakeMKV, driving MakeMKV's undocumented GUI protocol.

## 2. Owner requirements and decisions

Requirements (R):

- R1. Rip only the highest-quality video: the main feature for movies, the episodes for TV. No extras.
- R2. If two titles are the same except one has chapter markers, take the one with chapters.
- R3. Keep every audio track in the wanted languages. The best track comes first: generally the one with the most channels; ties go to the track closest to master quality (Atmos/TrueHD, DTS:X/DTS-HD MA, and so on). (Changed by the owner on 2026-10-04 from "one track per wanted language".) Exact duplicates are dropped (D13).
- R4. The wanted languages are English, always, and the work's original language when it is not English.
- R5. Keep English subtitles that transcribe the dialogue (regular and SDH when both exist) and forced English subtitles when available.
- R6. Never keep commentary subtitles. Drop commentary and audio-description audio when a flag, a picture-in-picture marking or a database entry confirms it; an unconfirmed track is kept (D12). The same holds for isolated-score and music-and-effects tracks (D14). What counts as confirmation: D15.
- R7. When a disc obscures which title or track is which, use the most accurate public data available.
- R8. File the result automatically, with Jellyfin/Plex cross-compatible naming.

Decisions (D) made during design:

- D1. When unsure, rip now into staging and queue the disc for review. Never block the walk-away. Review of a held job does not need the disc again.
- D2. Multiple cuts of a film on one disc are all ripped and filed as editions.
- D3. Signals: eject the tray, a Home Assistant notification, and a generic webhook. No Windows toast.
- D4. No mandatory local staging. A staging path and library paths are configurable and may be any local or network location. In the owner's setup they are on a NAS.
- D5. Stack: Rust with GPUI Kit for the UI, exact-pinned versions.
- D6. Track selection approach "A": rip every track of each chosen title, decide from the ripped file, write the final file with one remux. Selecting tracks at rip time is a later fast path (section 16).
- D7. The owner views the machine only on its own monitor. Remote Desktop behaviour is not a requirement.
- D8. A later Linux or macOS port is possible, so OS-specific behaviour sits behind interfaces.
- D9. Defaults (the first three are settings in section 14):
  - when ordering audio, a lossy track with more channels comes before a lossless track with fewer (R3 read literally), recorded as a note on the job. Exception: the guards in 8.6 always apply;
  - for non-English works the original language is the first and default audio track;
  - a TMDb-only identification may file automatically;
  - dvdcompare.net is not used in the first version.
- D10. Both of the owner's drives read UHD, and most discs will be UHD. UHD is the primary case for design, testing and acceptance.
- D11. A copy of a film at a different resolution (for example the Blu-ray in a UHD combo pack) is filed beside the existing copy as a version, using the servers' multi-version naming (8.9). It is not sent to review.
- D12. All audio tracks in the wanted languages are kept. Only tracks confirmed as commentary or audio description are dropped; a track that merely looks like a commentary is kept, with no review. The owner accepts that an unflagged commentary will sometimes stay in the file.
- D13. (Owner, 2026-10-05.) An audio track that is an exact duplicate of another track of the same title (identical stream packets) is dropped; one copy is kept. Found on the first disc of phase 0: four episodes carry their lossless 5.1 track twice.
- D14. (Owner, 2026-10-05.) Isolated-score and music-and-effects tracks are not wanted. They are dropped when confirmed, on the same footing as commentary and audio description (R6); an unconfirmed track is kept.
- D15. (Owner, 2026-10-05.) Stereo width is not a requirement for calling a track unwanted, and a rule based on a track's position on the disc is too brittle to decide it. Whether a track is a commentary, an audio description or a score is judged from flags, database entries and, once experiment 13 has answered yes, the track's content compared with the main track of its language (docs/phase0/experiments/EXP-13-design.md). Until then a track with the commentary pattern is kept; whether it also sends the job to review, against D12's "no review", is still to be decided by the owner.
- D16. (Owner, 2026-10-05.) Until the content rules are trusted, every rip is run with as much debug logging as the tools offer, and keeps the information needed to tune the rules as more discs go through: the tools' full output, the measures and evidence behind every decision, and enough of each audio track to compute new measures later without the disc. The owner's label for a track, once given, is kept with it.
- D17. (Owner, 2026-10-06.) Installing lazymkv must be dead simple for other users: they are not expected to install any sidecar program by hand. Every redistributable tool the app needs (mkvmerge, ffprobe/ffmpeg, an OCR engine and its models) ships inside the app's installer, with the tools' licences and version pins; the app never asks the user to find and install them, and it never downloads a tool, a model or any other executable content at run time (the owner, same day: fetching is a security risk; bundle everything). MakeMKV cannot be redistributed, so it is the one program the app may find missing: the app detects it, says exactly what to install and checks the version (4.4); whether its official installer can be run unattended from the app is a later question. This reverses the "never bundled" rule that 4.4 and 19 carried before this date; those sections are to be rewritten in the phase 2 design (the engine design), and phase 0 choices of OCR engine are made with D17 in view (prefer an engine that can ship with a Rust program on Windows and later Linux/macOS).
- D18. (Owner, 2026-10-06.) Section 6.3 (whole-disc copy, then scan and extract from the copy) is kept, with a crash fallback: when MakeMKV's scan of the copy ends without MSG:5011 and a TCOUNT line (experiment 3: makemkvcon 2.0.0 died with an access violation on Disc A's copy because of one playlist), the copy's scan counts as failed and the titles are ripped directly from the disc; the copy stays on staging as the backup. Verification treats any makemkvcon exit without its completion message as a failure, whatever the exit code (9.2, 9.4).
- D19. (Owner, 2026-10-06.) A subtitle or audio stream's language is taken from the disc's tag; OCR of subtitle text is not used to verify it for now (experiment 14 may still report a mismatch as an observation). This keeps the ASCII-only OCR engine (ocrs) sufficient; the owner allowed a trial of ocrs on the mini PC the same day.
- D20. (Owner, 2026-10-06.) Subtitle track policy, refining R5 and R6: two English dialogue tracks with the same dialogue and neither SDH: keep one; a regular track and an SDH track: keep both (R5); on a foreign-language film, an English dub transcript ("dubtitles") is kept beside the English translation; lyrics or sing-along tracks and trivia or pop-up text tracks are not wanted and are dropped when confirmed (an unconfirmed one is kept and reviewed, as D12's principle requires). Until experiment 14 answers yes, the text rule only adds reviews to 8.7's decisions and never changes a label or drops a track on its own.
- D21. (Owner, 2026-10-06.) The audio content rule carries a keep-only veto: when the title's programme reference (the original-language track that 8.6 ranks first) is not the first track of its language on the disc, and that first track is lossless or has more than two channels, the rule confirms no commentary on the title; every candidate is kept and the job carries the review reason audio_reference_unclear. Position never decides what a track is (D15); here it only withholds a drop when the reference choice looks inverted (EXP-13-design.md section 9, question 1). Cost accepted by the owner: a review on discs that place an original mono or stereo mix before a 5.1 remix.
- D22. (Owner, 2026-10-06.) The OCR engine is ocrs (pure Rust, MIT/Apache code), with its two models embedded in the executable as plain bytes (CC-BY-SA-4.0: the licence text and attribution ship with the app, `lazymkv licenses` prints them and `lazymkv export-models <dir>` writes the unmodified model files out, so no technological measure restricts them) and nothing downloaded at run time (D17). Embedding, not separate files, keeps `cargo install` complete for the binary (the models come as small crates under crates.io's 10 MB limit); the installers (.exe, .dmg, .deb) carry ffmpeg and mkvmerge as files in a per-platform resources directory, which `cargo install` users supply on PATH instead. Chosen after the trial in docs/phase0/ocr-engine-research.md: on the same 600 frames it gave the experiment 14 rule the same measures as Tesseract; it is about ten times slower on a full 1080p frame, so the app OCRs only the subtitle's glyph box, which it has anyway because it decodes the PGS/VobSub bitmaps itself; Tesseract stays the fallback if that does not recover the speed. A pure-Rust engine also keeps `cargo install` seamless for the binary (the end-user path remains the installer that carries ffmpeg and mkvmerge). Tesseract on the mini PC remains phase 0's measuring tool.
- D23. (Owner, 2026-10-06.) What a track is follows from its content, never from the disc's packaging or from labels a user types in: the app asks the user for no track information up front, and phase 0 does not use packaging as ground truth. For subtitles the read text is the truth (an SDH track has sound cues and speaker labels; a plain track has none); for audio, a human listens only where the measures leave a track ambiguous. Experiment tallies count a track whose content a reader has checked; "the packaging says" is an observation at most.


## 3. Target environment

Measured on the target machine on 2026-10-04:

- Windows 11 Pro (build 26200), Intel Celeron J4125 (4 cores), 7.8 GB RAM, 119 GB SSD with about 70 GB free, Intel UHD Graphics 600.
- MakeMKV 2.0.0 at `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`, upgraded from 1.18.3 on 2026-10-04 (section 19). Every phase 0 recording was made with it (`MSG:1005`: `MakeMKV v2.0.0 win(x64-release)`).
- One USB slim optical drive attached at a time; the owner swaps between two when a disc fails to read. Both, as MakeMKV lists them up to the firmware revision (`docs/phase0/environment.md`; the serial numbers are left out): drive 1 `BD-RE HL-DT-ST BD-RE BU40N 1.03`, tray-loading; drive 2 `BD-RE PIONEER BD-RW  BDR-XS07U 1.01` (two spaces after `BD-RW`, as MakeMKV prints it), slot-loading, the drive attached since phase 0's Task 14. Both read the same UHD in phase 0 (EXP-05, EXP-07).
- Rust 1.99.0 (MSVC), CMake 4.4.3 and Visual Studio 2022 Build Tools with the Windows SDK are installed. Builds run on this machine over SSH; GPUI release builds must be built on Windows. A GUI cannot be launched onto the desktop from an SSH session.
- Smart App Control was enforcing on 2026-10-06 and blocked `cargo` (the Rust toolchain is not Authenticode-signed); the owner turned it off the same day (`docs/phase0/environment.md`). An unsigned lazymkv would be blocked the same way on a machine where it is on (sections 18 and 19).
- The owner's account is an administrator. `makemkvcon` sees the drive from an elevated session.

GPUI Kit spike on this machine (throwaway code, since deleted): clean debug build 13 min 07 s, clean release build 18 min 36 s, one-line rebuild 13 s debug and 22 s release, lowest free RAM during a build 0.31 GB. Rendering correct on the UHD 600 through Direct3D 11.1. Release executable 23 MB, about 63 MB RAM. CPU 0.1% with the window closed, 2.4% with the window open updating once a second, 8.9% at four updates a second.

## 4. Architecture

### 4.1 Processes

Three executables:

- `lazymkv-engine.exe`: headless. Owns jobs, runs the external tools, persists state, sends notifications.
- `lazymkv.exe`: the GPUI Kit tray application: status, review queue, history, settings, setup self-test.
- `lazymkvctl.exe`: a command-line client for the same IPC protocol. It is how the engine is driven and tested before the UI exists, and over SSH afterwards.

A Task Scheduler task with a logon trigger, "run with highest privileges" and "run only when user is logged on" starts the engine as the owner's own account. MakeMKV needs administrator rights to reach optical drives on Windows, and its licence key and preferences live in that user's registry hive, so the engine runs elevated as that same user. The engine starts the UI and restarts it if it exits unexpectedly. A UI crash never interrupts a job. A named mutex enforces one engine instance.

When the owner chooses Quit in the tray menu, the UI tells the engine and exits; the engine keeps running and does not restart the UI until the next logon or until `lazymkv.exe` is started again.

### 4.2 Workspace

| Crate | Purpose | Depends on |
|---|---|---|
| `core` | Pure logic: robot-output parser, disc model, fingerprint and disc-ID functions (over a file listing and file bytes handed in), reconciler, title planner, track planner, subtitle classifier, namer, confidence and disposition, job state reducer, language and codec tables | nothing OS-specific |
| `platform` | Traits and the Windows implementation (4.3) | `core` types |
| `tools` | Drivers for `makemkvcon`, `mkvmerge`, `ffprobe`: build command lines, run under a watchdog, stream parsed output | `core`, `platform` |
| `clients` | TheDiscDB, TMDb, Home Assistant webhook, generic webhook | `core` |
| `engine` | Job orchestration, persistence, IPC server; the engine executable | all of the above |
| `ctl` | `lazymkvctl` | `core` types, IPC client |
| `dev` | `lazymkv-dev`: the developer and tuning commands over recordings, decision D16's first form (`retune`, `decide`, `labels`, the readers ported from phase 0's Python, `golden`). It owns the I/O `core` leaves out: reading recordings and the evidence clone, running `ffprobe` and `ffmpeg`, writing files. Phase 2 folds `retune` and `decide` into `lazymkvctl` | `core` |
| `ui` | GPUI Kit application | `core` types, IPC client |

`core` has no I/O. Everything in it is tested against recorded inputs and runs on any OS.

### 4.3 Platform interfaces

Each has a Windows implementation in the first version:

- `DriveWatcher`: media and drive arrival and removal. Windows: a hidden top-level window receiving `WM_DEVICECHANGE`, CD-ROM device-interface notifications for drive hot-swap, and a polling fallback.
- `Ejector`: lock, dismount, allow removal, eject. Best effort; a failure produces an `eject_failed` notification. Verified in phase 0 (EXP-07-eject.md): all four steps succeeded and the disc came out on both drives, the tray-loading one and the slot-loading one; no step failed while the eject still worked.
- `ProcessGuard`: run a child in a Job Object with kill-on-close; kill the whole tree; run at below-normal priority when asked.
- `KeepAwake`: prevent system sleep while any job is active.
- `ShareConnector`: connect to a network path with stored credentials (deviceless connection, never mapped drive letters); report reachability and free space.
- `SecretStore`: Windows Credential Manager.
- `Autostart`: create and remove the Task Scheduler task.
- `IpcTransport`: named pipe, with access limited to the owner's account.

### 4.4 External programs

Lookup order: the path in settings, then the folder the app ships them in (D17), then the default install folder or registry entry, then `PATH`. Before D17 (2026-10-06) this section read "located at runtime, never bundled"; under D17 only MakeMKV is found rather than shipped, and the table below is to be revised in the phase 2 design (the engine design: which tools ship, their licences, and how the OCR engine of experiment 14 is delivered). The last column is what phase 0 installed on the target machine (section 19). MediaInfo 26.05 and Tesseract OCR 5.4.0 were also installed there, for phase 0's measurements only; they are not programs of the app.

| Program | Use | Minimum version | Installed (phase 0) |
|---|---|---|---|
| MakeMKV (`makemkvcon64.exe`) | scan, rip, whole-disc copy | 2.0.0 (section 9.1) | 2.0.0, upgraded from 1.18.3 on 2026-10-04 |
| MKVToolNix (`mkvmerge.exe`) | header checks, subtitle extraction, remux | 54.0 (track flags used in 8.10) | mkvmerge v100.0 (MKVToolNix 100.0.0) |
| FFmpeg (`ffprobe.exe`) | audio profile detection, subtitle packet timings | 6.1 (Atmos and DTS:X profile names) | ffprobe 9.0.2 (FFmpeg 9.0.2); the Atmos and DTS:X profile names are not yet verified (EXP-06: not run) |
| Java runtime | used by MakeMKV to analyse Blu-ray menus | any that MakeMKV accepts | Java 8 (1.8.0_471), with MakeMKV's `app_Java` unset; MakeMKV did not report using it on a UHD with Java menus (no `3344`; EXP-12-java-labels.md: not answered) |
| ocrs (Rust crate, compiled in; models embedded) | reading bitmap subtitles for experiment 14's rule (D22) | 0.13.1 pinned | nothing to install: compiled into lazymkv (D22); 0.13.1 was built and trialled on the target machine (`docs/phase0/ocr-engine-research.md`). Measured in phase 0 (docs/phase0/ocr-engine-research.md, crop trial): the glyph-box crop does not cut the detector's cost, because ocrs 0.13.1's detection model takes a fixed 800 x 600 input (854 ms per read frame cropped, against about 930 ms on the full frame, on the J4125); the remedy to try first in phase 2 (the engine design) is to skip ocrs's detector and run its recognition on line boxes found from the bitmap's rows (recognition is about 240 ms of the 854); D22's Tesseract fallback applies only if that remedy also fails |

### 4.5 IPC protocol

Newline-delimited JSON over the pipe. Every message is `{ "v": 1, "type": ..., "id": ..., "payload": ... }`.

- On connect the engine sends `snapshot`: job summaries, settings (without secrets), self-test results.
- Engine to client: `job_update` (state changes, with the job's plan and review reasons), `progress` (at most once a second), `selftest_result`.
- Client to engine: `review_action` (the actions in section 11), `settings_set`, `retry`, `cancel`, `discard`, `rip_again`, `run_selftest`, `ui_quit`. Each gets an `ok` or `error` reply with the same `id`.
- The engine owns `%ProgramData%\lazymkv\settings.json`. Secrets are stored through `SecretStore` and never sent over the pipe after being set.
- A client that reconnects gets a fresh `snapshot`.

### 4.6 Version pins

`gpui-kit =0.7.0`, `gpui-pre =0.3.7`, `gpui-tray =0.1.4`, with `Cargo.lock` committed. GPUI Kit pins weekly snapshots of Zed's GPUI and ships breaking releases often, so upgrades are deliberate and never automatic.

## 5. Identifiers and paths

- **Fingerprint**: TheDiscDB's content hash of the disc: the MD5 of the file sizes, each as a little-endian 64-bit integer, of the `BDMV/STREAM/*.m2ts` files sorted by name (Blu-ray and UHD) or of every file in `VIDEO_TS` (DVD). It needs only a directory listing. Two independent open-source implementations agree on this. Outcome of phase 0's check (`docs/phase0/environment.md`, Fingerprint check (Task 8)): the fingerprint is proven: TheDiscDB knows the-office-2005-s3d3-bluray (bluray) by it. TheDiscDB later knew `pans-labyrinth-2006-uhd` (UHD) by it too, and neither of the two DVDs recorded (8.1). It is the canonical key for a physical disc pressing.
- **Disc ID**: for Blu-ray and UHD, the SHA-1 of the bytes of `AACS/Unit_Key_RO.inf` (falling back to the copy under `AACS/DUPLICATE`); for DVD, libdvdread's disc ID over the IFO files. Used for TheDiscDB lookups alongside the fingerprint. Verified in phase 0 (EXP-05-offline-and-unit-key.md): the Blu-ray and UHD disc ID equals MakeMKV's own `DISCID` (its debug log), and `Unit_Key_RO.inf` was readable in both drives. TheDiscDB's `/api/discid/` knew neither of the two discs it knows by fingerprint, and its `global_disc_id` differs from this disc ID on both (8.1). The DVD disc ID is not yet implemented: phase 0's capture tool computes only the AACS disc ID, so a DVD is looked up by its fingerprint alone until it is.
- **Job ID**: `<fingerprint>-<n>`, where `n` is one more than the highest `n` of any earlier job for that fingerprint (1 for the first).
- **Title key**: `t<NN>-<source>`, where `NN` is the title's order in the plan and `<source>` is the source file stem (for example `t00-00800`), or `title<original title number>` on DVD.

Paths:

| What | Where |
|---|---|
| Job metadata | `%ProgramData%\lazymkv\jobs\<job id>\` (state, logs, database answers, navigation-file copies, probe output) |
| Direct rip | `<staging>\<job id>\rip\<title key>\` |
| Whole-disc copy | `<staging>\<job id>\backup\` |
| Titles extracted from a copy | `<staging>\<job id>\extract\<title key>\` |
| Subtitle evidence | `<staging>\<job id>\evidence\<title key>.mks` |
| Partial final file | `<library root for the media type>\.lazymkv-partial\<job id>-<title key>.mkv.partial` |
| Season sessions | `%ProgramData%\lazymkv\sessions\<tmdb show id>-s<NN>.json` |
| Remembered decisions | `%ProgramData%\lazymkv\remembered\<fingerprint>.json` |

Job state is written atomically (temporary file, then rename).

A remembered-decisions file holds the TMDb ID and kind; for each title its source file, segment map, duration, role (feature, cut with edition name, episode numbers, or ignored); and audio and subtitle overrides keyed by the stream's signature (type, language, codec, channels, position among identical streams). Titles are matched by source file, segment map and duration within 2 s, never by title key. A remembered decision sets the confidence it covers to 1.00.

## 6. Pipeline

One disc is in the disc phase at a time. One job is in the file phase at a time (`file_phase_concurrency`, default 1), in first-in first-out order, with `mkvmerge` and `ffprobe` at below-normal priority. The file phase of earlier discs runs while the next disc rips. Jobs waiting for review occupy neither the drive nor the file-phase slot.

### 6.1 Disc phase, common steps

1. **Detect.** Media-arrival events are debounced per drive. After an arrival the engine waits `drive_settle_s`, then waits up to `drive_ready_timeout_s` for Windows to mount the volume; a drive that is still not ready is retried, and only then reported. Nothing is ejected or failed on a first probe. Then run `makemkvcon64 -r --cache=1 info disc:9999` once and read the `DRV` lines. The drive index is matched by the drive letter in the seventh `DRV` field and is resolved again before every `makemkvcon` call; it is never cached.
2. **Fingerprint and job lookup.** Compute the fingerprint and disc ID, read the volume label and the disc name from `BDMV/META/DL/bdmt_*.xml`, and copy the small navigation files (`BDMV/PLAYLIST/*.mpls`, `BDMV/index.bdmv`, `BDMV/MovieObject.bdmv`, DVD `*.IFO`) into the job folder. Then:
   - a job for this fingerprint that has not finished its disc phase, or failed with `MediaReadError`, resumes (only titles not yet verified are ripped);
   - a job that is past the disc phase (file phase, held for review, or filed) sends `already_ripped` and ejects, unless the owner armed "rip again" for it, in which case a new job starts;
   - otherwise (including when the earlier job was discarded) a new job starts. A new job applies any remembered decisions for the fingerprint.
3. **Database lookup.** TheDiscDB by disc ID and by fingerprint (8.1), while the scan runs.
4. **Scan.** `makemkvcon64 -r --messages=-stdout --progress=-same --minlength=<minlength_s> --cache=<cache_mb> --profile=<job profile> info disc:N`. The same flags are used for every later call on this disc, because MakeMKV's title numbers depend on them. The scan records whether MakeMKV reported using a Java runtime. Not yet verified (EXP-12-java-labels.md: not answered): on a UHD with Java menus, with Java 8 installed and MakeMKV's `app_Java` unset, the scan reported none (no `3344`).
5. **Identify and plan.** Decide the disc kind (8.2), pick the structural candidates (8.3), run the TMDb match (8.1), then fix the plan: the titles to rip, the mode (6.2 or 6.3), and the identity and title confidences. Check staging free space (section 7). Persist the plan with its evidence.

### 6.2 Direct mode

Used when title confidence is at or above `auto_threshold`.

6. **Rip.** One `mkv disc:N <index> <staging>\<job id>\rip\<title key>\` call per planned title, with the job profile. The directory is emptied first. While the call performs its own disc scan, build its title list and confirm that the planned title still has the planned index before the saving message (`MSG:5015` with the job profile, else `MSG:5014`; saving begins). On a Blu-ray or UHD the list comes from the call's `MSG:3307` and `MSG:3308` lines, and the planned source file (attribute 16) must still have the planned index. A DVD prints no `MSG:3307` or `MSG:3308` line and its scan has no attribute 16 (none in the four DVD rips and the two DVD scans recorded; 9.4), so on a DVD the list comes from the call's `MSG:3028` lines ("Title #N was added (C cell(s), H:MM:SS)"), one per title in index order, and the planned title is known by its DVD title number, scan attribute 24: the (index + 1)-th `MSG:3028` line must carry an N equal to the planned title's attribute 24, compared as numbers (the attribute is written `01` where N is `1`). On a mismatch, or when the planned index has no such line before the saving message: kill the process, re-run the scan to refresh the mapping, and retry, at most twice. Verified in phase 0 (EXP-02-index-guard.md) on a UHD (`pans-labyrinth-2006-uhd`, both of its 2 titles): each call's `3307` lines numbered the titles as the scan did and came before `MSG:5015`, and stopping at `MSG:5015` left nothing written. Not yet verified on a disc with angles or duplicate titles (EXP-02-index-guard.md): that UHD has neither, and no `3308` line has been seen. The DVD guard is read from the recordings, not tested by a stopped rip: in all four DVD rips every `MSG:3028` line came before `MSG:5015`, and their N, in order, equalled the scan's attribute 24 of titles 0, 1, 2 and so on, although N is not the 0-based index (short-circuit-2-1988-dvd: `#1` and `#3` to `#8` are titles 0 to 6, with attribute 24 `01` and `03` to `08`, `#2` having been skipped as too short, `MSG:3025`; `corpus/owner/short-circuit-2-1988-dvd/all-tracks-2/rip.txt` lines 126 to 147 against the attribute 24 lines of its `scan.txt`, 175 for title 0 and 399 for title 1; short-circuit-1986-dvd: `#1`, its one title). Not yet verified on a DVD: that stopping at `MSG:5015` leaves nothing written (no DVD rip was stopped), that `MSG:3028` lines come in index order on other discs (two DVDs seen), and a rip call that skips other titles than its scan did.
7. **Verify.** `mkvmerge -J` on the output (a header read): duration within 2 s of the scan (on a DVD, from 2 s shorter to 1% longer: see below), chapter count as the scan reported, and the track list aligned to the scan's stream rows as an ordered match on type, output codec (attributes 41 and 42), language and channels. The alignment carries MakeMKV-only flags (9.5) onto output tracks. A title that fails verification is re-ripped once while the disc is still in; a second failure adds the review reason `verify_failed`. A title that saved but logged read errors (`MSG:2003`) is kept and adds the review reason `read_errors`, with the error count and offsets.

   Verified in phase 0 (EXP-01-profile-and-order.md, EXP-11-language-selection.md), on a UHD, a Blu-ray and two DVDs:
   - Duration: within 2 s on the UHD (0.1 s) and the Blu-ray episodes (at most 0.9 s). Every DVD file ran longer than the scan's attribute 9: 6623.6 s against 6606 s (short-circuit-2-1988-dvd, in both of its rips; EXP-01) and 5919.9 s against 5904 s (short-circuit-1986-dvd's rip), 0.27% each. The scan's duration of a DVD title is short, so on a DVD the check accepts a file from 2 s shorter to 1% longer than the scan (provisional, set in Task 17 from these two DVDs; short-circuit-1986-dvd's figure is from its recording, `all-tracks/mkvmerge.json`). Not yet verified beyond these two DVDs.
   - Chapters: the file's chapter count equalled attribute 8 on every rip (9.3).
   - Output codec: attribute 41 appears only on a converted stream (a DVD's closed captions: 41 `Text`, 42 `( Lossy conversion )`). Every other stream has no attribute 41, has 42 `( Lossless conversion )` and arrives in its input codec (attributes 5 to 7): `MpegH` HEVC as `HEVC/H.265/MPEG-H`, `DTS-HD MA` as `DTS-HD Master Audio`, `DTS` as `DTS`, `DD` as `AC-3`, `PGS` as `HDMV PGS`, `Mpeg2` as `MPEG-1/2`, DVD subtitles as `VobSub`. The alignment therefore compares attribute 41 where present (`Text` arrives as `SubRip/SRT`) and the input codec otherwise.
   - Order: the file's tracks follow the scan's order, with one exception: a text-converted subtitle track (a DVD's closed captions) comes right after the audio tracks, before every bitmap subtitle track, although the scan lists it after them. The alignment expects it there. Pairing by type and language alone hides the swap when the two subtitle tracks share a language (EXP-11).
   - Removed streams: a derived forced-only subtitle stream that turned out empty is not in the file, and `MSG:4047` says so ("Forced subtitles track #N turned out to be empty and was removed from output file"). N counts the title's selected streams in scan order from 0, not the scan's streams; with the job profile (9.3) the two coincide unless a stream is left out (`-sel:mvcvideo` on a 3D title, not tested). The verification maps N through the profile's selection and does not count that stream as missing.
   - Read errors: short-circuit-2-1988-dvd's scan and its successful rip logged `MSG:2003` "READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION" at offset 1,048,576 (3 in the rip), and MakeMKV still reported the title saved (`MSG:5036`); under this step such a rip carries `read_errors`.
8. **Eject and notify.** After `makemkvcon` has exited: eject, and send `disc_done`.

### 6.3 Whole-disc copy mode

Used when title confidence is below `auto_threshold` (an unknown disc with rival or decoy titles, or a TV disc no database maps). Such a job always goes to review, so titles are extracted only after the owner decides. Experiment 3 answered no (EXP-03-whole-disc-copy.md): the copy works, but MakeMKV 2.0.0 crashed scanning it. The mode is kept, with a crash fallback (D18, step 6a).

6. **Copy.** `backup --decrypt disc:N <staging>\<job id>\backup\`. Success is `MSG:5070` with no `MSG:5069`. `MSG:5079` (completed with hash failures) keeps the copy and adds the review reason `copy_hash_errors`. Verified in phase 0 (EXP-03-whole-disc-copy.md) on a 56.9 GB UHD with Java menus: the copy to the share completed (`MSG:5070`, no read or write error, every stream file at the disc's size) in 39 min at 23.0 MB/s, the rate of the direct rip of its feature. `backup` writes `BDMV`, `CERTIFICATE` and `MAKEMKV` straight into the target folder, with no folder of its own, and accepts an existing empty folder; the AACS files are kept under `MAKEMKV\AACS`, beside a 16-byte `MAKEMKV\CMAP\<clip>.cmap` per stream clip and `MAKEMKV\discattd.dat`. So `info file:` and `mkv file:` are pointed at the target folder itself (pointing `info file:` at its `BDMV` changes nothing, EXP-03: MakeMKV opens the copy root either way; `mkv file:` was not run).
6a. **Scan the copy**, while the disc is still in the drive: `info file:<staging>\<job id>\backup\` with the flags of the disc scan (6.1 step 4). A scan that ends without `MSG:5011` followed by a `TCOUNT` line has failed, whatever the exit code (D18, 9.2). When that scan has failed, the job switches at once to Direct mode for every title that passes the candidate filter (8.2), most likely first, waiting in `AwaitingSpace` if they do not fit (section 17, row 3's fallback); the copy stays on staging as the backup, and a note records the switch. Seen in phase 0 (EXP-03-whole-disc-copy.md): on the UHD above this scan crashed within 12 s (exit code 0xC0000005, no message, empty standard error) because of one playlist, `00055.mpls`; a partial copy without it scanned to the end. Not yet verified: whether the scan of a copy succeeds on another disc.
7. **Eject and notify** as in 6.2 step 8. The job enters `NeedsReview`.
8. **After approval** (section 11): each title the owner assigned is matched by source file, segment map and duration between the disc-time scan and the copy's scan (step 6a), then extracted with `mkv file:<backup> <index> <staging>\<job id>\extract\<title key>\` and verified as in 6.2 step 7. The disc-time scan, with its labels, stays the reference for planning and review; the copy is used only for extraction. Not yet verified (EXP-03-whole-disc-copy.md): no extraction from a copy was run, since the copy's scan crashed.

If the copy fails on a read error, the job fails with `MediaReadError`. When the disc is inserted again (in either drive) the copy restarts from scratch. If it fails a second time, the job switches at once, while the disc is still in the drive, to Direct mode for every title that passes the candidate filter (8.2), most likely first; a note records the switch.

### 6.4 File phase

9. **Inspect.** `ffprobe -show_streams` on the staged file by its path: exact audio profile (Atmos, DTS:X, DTS-HD MA versus High Resolution) and channel layout. This reads the header and a few frames. Not yet verified (EXP-06: not run): which `ffprobe` profile strings mark Atmos and DTS:X, and whether `ffprobe` or MediaInfo tells them apart; neither format has been tested. The one UHD recorded has neither (its owner names none; EXP-01 found its lossless track, DTS-HD MA 5.1, as `DTS-HD MA` in `ffprobe` and `DTS-HD Master Audio` / `XLL` in MediaInfo).
10. **Subtitle evidence.** Skipped when the title has exactly one English subtitle stream. Otherwise extract every subtitle stream to a small file (`mkvmerge -o <evidence file> -D -A -s <ids> <raw>`; one full read of the staged file) and read each stream's packets from that small file (`ffprobe -show_entries packet=pts_time,duration_time,size -of json`, one call per stream). This yields the event counts and start times for 8.7. MakeMKV writes statistics tags on every track (EXP-01-profile-and-order.md: `BPS`, `DURATION`, `NUMBER_OF_FRAMES`, `NUMBER_OF_BYTES`, `SOURCE_ID`), but `NUMBER_OF_FRAMES` counts packets, not events: on the PGS tracks recorded it is twice the event count (a 30-byte clear packet follows every event; 8.7), and on the DVD's VobSub tracks it equals it. So the event counts and start times still come from this extraction. **Text (8.7, D22).** Where this step runs, the English and no-language bitmap streams (PGS, VobSub) of the small file are also read by OCR: each event's bitmap, decoded by the app, is read with ocrs (4.4), and 8.7's **Text** judges the result. Section 17 row 14 (b) answered no, so the text is read only where this step runs (two or more English subtitle streams), and a title with one English track keeps it as "English" without the flag (row 14's (b) fallback). How the app decodes the bitmaps and finds the text lines is to be designed in phase 2 (the engine design, 4.4). Phase 0 read the same streams from the same small file with `scripts/phase0/ocr-subs.ps1` (ffmpeg's render of the bitmaps, then Tesseract; EXP-14-design.md, section 3, "Where it runs and what it costs").
10a. **Packet MD5 and tuning evidence (D16).** The MD5 of every audio stream's packets, which 8.6 needs to drop exact duplicates (D13), is computed in every case: by the decode below while `keep_tuning_evidence` is on, otherwise by one pass that reads the audio packets without decoding them. While `keep_tuning_evidence` is on, one full decode of the staged file's audio streams, whether or not step 10 ran: per audio stream, the measures of the content rule (section 17 row 13, EXP-13-design.md; 8.6 once experiment 13 has answered) where it is a reference or a candidate, the MD5 of its packets, and its compact copy for section 7. It reads the whole file once more than step 10 (measured on the target machine in phase 0: `seconds=` of the audio-levels jobs).
11. **Decide** tracks (8.6, 8.7) and disposition (8.8). A pure function of the scan, the probes, the database answers and settings.
12. **Disposition gate.** A job with any review reason, or any confidence below `auto_threshold`, enters `NeedsReview` and stops here until approved.
13. **Remux.** `mkvmerge` writes the partial final file with the kept tracks, their order, languages, flags and names (8.10).
14. **Verify and file.** `mkvmerge -J` on the partial file must match the decision exactly and the source duration within 2 s. Then rename it into its final path (8.9); the rename is always within one library root. The job records, for each file it filed, the exact path, size and modification time. A title that the fast path (section 16, item 1) finishes without a remux is verified against the scan as in 6.2 step 7, with its `MSG:4047` and closed-caption rules (EXP-11-language-selection.md).
15. **Notify** `filed`. The raw rip or disc copy is kept for `retention_days` as an undo.

## 7. Storage

- `staging_root`, `movies_root` and `tv_root` are settings and may be local or UNC paths. Network paths are connected through `ShareConnector`.
- The staging root may not be inside a library root; settings refuse it. Partial files live only in `.lazymkv-partial` and under staging, so the media servers never index them.
- Setup tests each root with a write, rename and delete inside it.
- **Space checks.** Before a Direct rip: the sum of the planned titles' sizes plus 5%. Before a whole-disc copy: the disc size plus 2%. Before extraction and before a remux: the size of the files to be written plus 5%. When staging and a library share a volume, the needs are added.
- **Short of space, or share unavailable:** the job waits (`AwaitingSpace` or `AwaitingShare`) and sends `waiting`. During the disc phase the disc stays in the tray for as long as the wait lasts. A share that drops mid-rip fails that title; lazymkv waits for the share and re-rips the title.
- **Overwriting.** lazymkv never overwrites a library file it did not create in the same job. A path collision with any other file adds the review reason `path_collision`. Re-finalising a job (section 11) may replace only the file recorded for that job, and only if its size and modification time still match the record: the new file is written and verified as a partial file, then replaces the old one. If the new decision gives a different path, the new file is filed first and the job's old file is then deleted. A recorded file that no longer matches its record is left alone: a same-path decision then raises `path_collision`, and a different-path decision files the new file and adds the note `old_file_kept`.
- **Retention.** A cleanup task in the engine deletes the raw data of `Filed` jobs older than `retention_days`, counted from when they were filed. When space is short it deletes oldest-first among `Filed` jobs only. Data of held, failed and in-progress jobs is never deleted automatically; if only such data remains, new discs wait in `AwaitingSpace`.
- **Tuning evidence (D16).** While `keep_tuning_evidence` is on, every ripped title also leaves under `tuning_root` what is needed to re-run and re-tune the decision rules later, without the disc or the raw rip:
  - a compact copy of every audio stream, made in step 10a's decode: one Matroska file per title holding each stream's mono mixdown, that stream's centre channel when its layout has one besides others, and the side signal (L-R)/2 of a two-channel stream, as FLAC at 8 kHz, 24 bits, 18 dB below the measured signal, with the peak of each track (a track at full scale fails the evidence). That is about 50 MB per hour per track, or about 1.5 GB for a 2.5-hour film with six audio streams;
  - the packet MD5 of every audio stream, the level curves and measures behind every audio verdict, the stream descriptions (`ffprobe`, `mkvmerge -J`), the scan, and the decision record with the rule version and the thresholds in force;
  - the text OCR read from every subtitle stream that 6.4 step 10 reads, the frames it was read from, and the subtitle evidence file (`evidence.mks`, the bitmaps with their palettes), so that the text rule can be re-run, and another OCR engine tried, without the disc. Phase 0 keeps them under `$TuningData\<slug>\<label>\subs-ocr\` (`scripts/phase0/ocr-subs.ps1`: the text, the rendered frames and a copy of `evidence.mks`; D16), and its recordings get only counts from them, never text (section 15). Their form in the app is to be designed in phase 1;
  - the owner's label for a track, when one is given in review, with how the owner knows, whether the owner is sure, and the labels it replaced.

  The retention cleanup never touches `tuning_root`; only the owner deletes from it. When `tuning_root` is unreachable or short of space, the job goes on and gets the note `tuning_evidence_pending`; the retention cleanup does not delete that job's raw data until its tuning evidence has been written, which is retried whenever `tuning_root` is reachable. `tuning_root` may not be inside `staging_root` or a library root; settings refuse it, and setup tests it with a write, rename and delete like the other roots.

## 8. Decision rules

### 8.1 Identification

| Step | Evidence | Identity confidence | Review reason |
|---|---|---|---|
| 1 | A remembered decision for this fingerprint | 1.00 | |
| 2 | TheDiscDB match on both the disc ID and the fingerprint | 0.99 | |
| 3 | TheDiscDB match on one of them | 0.95 | |
| 4 | TMDb search on the disc's own name: exactly one plausible result, and the runtime check passes | 0.90 | |
| 5 | TMDb search with several plausible results, or a failed runtime check | 0.50 | `identity_ambiguous` |
| 6 | Nothing | 0 | `identity_unknown` |

- TheDiscDB is queried at `https://thediscdb.com/api/discid/{id}` (returns an array) and `https://thediscdb.com/api/dischash/{hash}` (returns one object); both are anonymous. A match supplies a TMDb ID. If the entry has no TMDb ID, TMDb is searched with the entry's title and year.
- The TMDb record is fetched with its external IDs and supplies title, year, original language, runtime, the IMDb and TVDB IDs and, for TV, seasons and episodes with runtimes.
- The disc's own name is the `bdmt` disc name, else the volume label. Before cleaning, a season number and a disc number are parsed from it and kept as evidence. Cleaning replaces underscores with spaces and removes disc numbers (`DISC 1`, `D1`), season markers, region and format tags, and trailing years.
- A TMDb search result is plausible when its title or original title equals the cleaned name after case-folding and removing punctuation. Only the first page of results is considered. Movies and TV are searched together, and a TMDb identity is always the pair of kind and ID, because the two kinds number their records separately.
- A generic or placeholder name (for example `DVD_VIDEO`, `DISC_1`, `LOGICAL_VOLUME_ID`, `VOLUME_SET_ID`, or a name with no letters) is never searched; identity is then step 6.
- Runtime check for movies: at least one planned feature or cut (8.3, 8.4), or the longest rival when none is planned, is within 3% of the TMDb runtime. For TV: the median episode runtime of the parsed season (of the whole series when no season was parsed) is within 15% of the median duration of the disc's episode-length groups (8.2).
- When `tmdb_only_autofile` is off, step 4 is capped at 0.84 and adds `identity_ambiguous`.
- Verified in phase 0 (EXP-05-offline-and-unit-key.md): `AACS/Unit_Key_RO.inf` was readable in both drives, so step 2 is open to a Blu-ray or UHD in either. Step 2 has not been seen, though: for both discs TheDiscDB knows by fingerprint (a Blu-ray and a UHD) its `/api/discid/` answered 404, and its `global_disc_id` differs from the disc ID (5). The fingerprint is the primary key.
- TMDb needs the owner's API key, shows TMDb attribution in the UI, and caches responses for at most six months.
- If TheDiscDB or TMDb cannot be reached, the disc phase never waits: planning proceeds on structure alone and the lookup is recorded as pending. After eject the job waits in `AwaitingLookup`, retrying hourly, before the file phase decides anything.
- TheDiscDB's response fields, as the 18 answers saved in `corpus/thediscdb-api/` (live answers fetched on 2026-10-04, not pinned to a data commit; its `README.md`) and the owner's 2 recorded answers give them: a `dischash` answer is one object and a `discid` answer an array of such objects; a miss is HTTP 404 with an empty body. Each object carries `content_hash`, `global_disc_id`, `format` (`UHD`, `Blu-ray` or `Blu-Ray`, `DVD`), `media` (`type` is `Movie` or `Series`; `title`, `full_title`, `year`, `external_ids.tmdb`, `external_ids.imdb`), `release` (`title`, `year`, `slug`, `locale` (only `en-us` seen), `region_code`, `upc`), `disc` (`index`, `name`, `slug`) and `titles[]`. Each title has `index`, `source_file`, `segment_map`, `duration` (`H:MM:SS`), `size` (bytes), `display_size`, `resolution`, `chapters[]` (`index`, `title`), `tracks[]` and, only when TheDiscDB has described the title, `file_name` and `item` (`title`, `type`, and `season` and `episode` where the item belongs to an episode; `type` is `MainMovie`, `Episode`, `Extra`, `Featurette`, `DeletedScene`, `Trailer` or `Other`). Each track has `index`, `type` (`Video`, `Audio` or `Subtitles`) and `name` (the codec name, for example `Dolby Digital`). A video track adds `resolution` and `aspect_ratio`; an audio track adds `audio_type` (the channel layout: `Stereo`, `Surround 5.1`, `Surround 7.1`; there is no channel count), `language_code` (three letters) and `language`; a subtitle track adds `language_code` and `language`. An audio or subtitle track sometimes has `description` (58 of 877 audio and 30 of 2018 subtitle tracks), often only a language name such as `English`, otherwise a text such as `Director Commentary`. The main movie is the title whose `item.type` is `MainMovie`. A track description means commentary when it contains "comment"; audio description when it contains "audio desc", "descriptive", "DVS" or "visually impaired"; SDH when it contains "SDH" or "hearing" (all case-insensitive).

### 8.2 Disc kind

The **candidate filter** discards menu loops (titles with no audio stream, with more than one chapter every two minutes, or whose segment map repeats one clip) and titles shorter than `minlength_s`. The remaining titles are **grouped** by identical segment map; a group's duration is that of its titles.

TheDiscDB states the kind when it knows the disc. Otherwise the disc is TV when three or more groups each last between 15 and 120 minutes and the longest of them is at most 15% longer than the shortest (the upper bound covers miniseries with feature-length episodes); or when the disc's name, before cleaning, contains a season marker (`season`, `series` or `s` followed by a number). A disc number alone never makes a disc TV. Otherwise it is a movie.

### 8.3 Movie titles

Database titles are matched to scan titles on source file name and segment map, then exact size and duration within 2 s. MakeMKV title numbers are never used for matching. MakeMKV truncates long segment maps with `...`; when either side is truncated, only the untruncated prefix is compared, together with source file, size and duration.

At planning time nothing has been ripped, so any use of the audio ranking in 8.6 skips its `ffprobe` criterion.

First applicable rule:

1. **Database.** TheDiscDB marks the main movie and it reconciles with the scan: title confidence 0.98.
2. **Labels.** Used only when the scan reported a Java runtime in use. Not yet verified (EXP-12-java-labels.md: not answered, leaning no): on a UHD with Java menus, with Java 8 installed and MakeMKV's `app_Java` unset, the scan printed no `3344` and no label; setting `app_Java` to the installed Java is the one change left to try. If no later UHD shows labels, this rule is unavailable and titles are chosen by database and structure only (section 17, row 12). A label is a title comment that starts `FPL_MainFeature` (optionally preceded by `SEG_`). Its suffix is read from this table:

   | Suffix | Meaning | Edition name |
   |---|---|---|
   | none | the feature | Theatrical Cut |
   | `_R` | theatrical cut (observed on one disc) | Theatrical Cut |
   | `_UR` | unrated cut | Unrated Cut |
   | `_EXT` | extended cut | Extended Cut |
   | `_Narrative` | audio-description variant: ignored | |
   | a trailing language code such as `_JPN` | dub variant of the same cut | |
   | anything else | an unnamed cut | |

   Labelled titles are grouped by suffix with any trailing language code removed; one group is one cut. Within a cut, the variant whose audio covers the most wanted languages is used, then the one without a language suffix, then the lower title number.

   - Exactly one cut: 0.90.
   - Two or more cuts with different recognised suffixes: each is planned as an edition, 0.90.
   - An unnamed cut: 0.60 with the review reason `unnamed_cut`.

   Other labelled playlists (for example a copy-disclaimer playlist) are not the feature.
3. **Structure**, on the groups from 8.2:
   1. If a group's segment map is the concatenation of two or more other groups' maps, it is a "play all" title: treat the disc as TV (8.5).
   2. Keep groups at least `feature_min_minutes` long (default 45). If none, keep the longest group.
   3. Call a kept group a rival if its duration is within 10% of the longest kept group's. Shorter groups are extras and are ignored.
   4. Within each rival group, choose the title to rip: the one with chapters (R2); then the one whose audio and subtitle streams are a superset of every other title's in the group. If no title is such a superset (the titles carry different audio), the group is ambiguous: 0.60, review reason `title_ambiguous`.
   5. One rival: it is the feature, 0.85.
   6. Several rivals. The segment difference of two maps is the number of segments in exactly one of them divided by the length of the longer. If every pair of rivals differs by at most 0.25, they are cuts or decoys that cannot be told apart without rule 1 or 2: 0.60, review reason `title_ambiguous`.
   7. Otherwise the rivals are largely different content: 0.60, review reason `multiple_features`.
4. **Runtime sanity.** Once TMDb is known, a feature chosen by rule 3 whose duration is more than 10% from the TMDb runtime drops to 0.60 with `title_ambiguous`.

Duration alone never decides between rivals: decoy playlists can match the real one in duration and size.

### 8.4 Cuts

Each genuine cut is planned and filed as an edition (D2). Edition names come from TheDiscDB's title for the item, else from the label table in 8.3, else from the owner in review. They are sanitised like titles and capped at 32 characters.

Edition naming is decided per film, not per job, so that copies filed from different discs group correctly:

- a cut the evidence names as anything other than theatrical always carries its edition name, even when it is the only cut on the disc;
- the theatrical or main cut carries "Theatrical Cut" only when this job files two or more cuts, or when the film's library folder already holds an edition-named file;
- a later job names a cut the same way an earlier filed copy of that cut was named.

### 8.5 TV

- **TheDiscDB maps the disc:** each episode title is ripped in Direct mode; "play all" titles are dropped. Title confidence 0.98. Multi-episode titles are filed as multi-episode files as TheDiscDB describes them.
- **Otherwise:** title confidence 0.60 with the review reason `tv_unmapped`, so whole-disc copy mode is used. Episode-length titles are pre-assigned to episodes by runtime as a suggestion.
- **Season session:** a record keyed by TMDb show ID and season number, holding the discs seen, the episodes assigned, and the next unassigned episode. A disc joins a session when its identity is confirmed (by TheDiscDB or in review). A disc finds its session by show ID and the season number parsed from its name (8.1); without a parsed season it uses the show's single open session; failing that, suggestions start at the first episode and review asks for the season. Suggestions start at the session's next unassigned episode. A session closes when every episode is assigned or when the owner closes it.
- Episode numbers follow TMDb's aired order.

### 8.6 Audio

**Languages.** Wanted languages are English, plus the original language when TMDb gives one that is not English (R4). TMDb's two-letter code is mapped through a table in `core` to every equivalent three-letter code (for example `fr` to `fre` and `fra`; `cn` to `chi`, `zho` and `yue`), and a stream matches if its code is in the set. A stream's language is its own language code (attribute 3). A stream with no code or `und` is kept and treated as belonging to no language.

**Codecs.** A table in `core` maps MakeMKV's codec names (attributes 5 to 7) to a codec family, a fidelity tier and a display name. Phase 0 recorded the codec names in 6.2 step 7; phase 1 built the table from the recordings (`core::codec`). Channel layout names come from the channel count (attribute 14): 1 is 1.0, 2 is 2.0, 6 is 5.1, 7 is 6.1, 8 is 7.1.

**Position.** "First track of its language" means the lowest disc position among that language's streams that survive step 1.

1. **Drop** (R6, D12):
   - streams MakeMKV derived from another stream (flag 2048), such as the AC-3 core of a TrueHD track. These are duplicates MakeMKV synthesises, not separate tracks on the disc;
   - streams flagged director's comments, alternate director's comments or visually impaired (flags 1, 2, 4; set on some DVDs);
   - secondary (picture-in-picture) audio (flag 512);
   - an exact duplicate of another audio stream of the same title (identical stream packets: 6.4 step 10a computes an MD5 of every audio stream's packets in every case, one ffmpeg pass per file, as `scripts/phase0/audio-levels.ps1` does; equal MD5 = duplicate): one copy is kept (D13);
   - streams a TheDiscDB track description identifies as commentary or audio description (8.1), or as an isolated score or a music-and-effects track (D14; no such description was seen in phase 0, so how TheDiscDB words them is not yet known). Other described tracks are kept. TheDiscDB's track list carries no flags, so descriptions are used only when it aligns with the scan's streams as a whole, in order, on type, language, codec and channels: the alignment is tried against the scan's full stream list and against the list without derived streams, and exactly one of the two must align completely;
   - streams in a language that is not wanted.
2. **Keep** every remaining stream: all tracks in the wanted languages, and any stream with no language.
3. **Rank** the kept tracks within each language, to decide their order. Compare in order:
   1. channel count;
   2. fidelity tier: (a) lossless: TrueHD, DTS-HD MA, LPCM, FLAC; (b) high-rate lossy: DTS-HD High Resolution, E-AC-3 at 768 kb/s or more, DTS at 1.5 Mb/s or more; (c) other lossy: AC-3, lower-rate E-AC-3 and DTS; (d) MPEG audio and AAC;
   3. object-based extension (Atmos, DTS:X) as reported by `ffprobe` (the profile strings are not yet verified: EXP-06: not run, 6.4 step 9);
   4. bit depth, then sample rate;
   5. bitrate from attribute 13, for lossy tracks only (the field is meaningless for lossless tracks);
   6. earlier disc position.

   With `prefer_most_channels` off, criteria 1 and 2 swap.
4. **Guards**, so that an unconfirmed commentary does not become the first track of its language. These apply whatever the D9 setting:
   - G1: a lossy track at 256 kb/s or less is never placed before a lossless track of the same language;
   - G2: if the top-ranked track is lossy, has two channels or fewer, is not the first track of its language, and beats the first track on channel count but on no later criterion, the first track of the language is placed first instead.
5. **Order and flags.** Languages in this order: with `original_language_first` on, the original language (when it is not English), then English; with it off, English, then the original language. Streams with no language come last. Within a language, ranked order. The very first audio track gets the default flag. Tracks in the original language get the original flag; for English works those are the English tracks.
6. **Notes and gaps:**
   - a kept track that is lossy, has two channels or fewer and is not the first track of its language: note `possible_commentary`, naming the track;
   - the first track is lossy while a lossless track of the same language with fewer channels exists: note `lossless_original_available`;
   - no English stream but the original language is present: note `no_english_audio`;
   - the original language is wanted but no stream matches: review reason `original_language_missing`;
   - no stream in any wanted language and none without a language: review reason `no_wanted_audio`.

**Content (D15, D21; section 17 row 13).** Each other audio stream in a wanted language is measured against the title's programme reference (6.4 step 10a): the track that steps 3 and 4 place first among the original-language streams, or among the English streams when the original language is English, unknown, none or absent from the disc (EXP-13-design.md, sections 9.1 and 9.5). This is the reference phase 0 measured against (`audio_similarity.placed_first`, `audio_dataset.programme_reference`; the harness applies G1 by channel count, since `streams.json` carries no bit rate, and G2 without its "no later criterion" clause; both can only make the first-on-disc or lossless track the reference). D21's "the original-language track that 8.6 ranks first" is this track: step 4's guards apply, and for an English work, or one whose original language is unknown, none or not on the disc, it is the English track so placed. A title with no stream in a wanted language has no reference and nothing to judge (`no_wanted_audio`). Experiment 13 has not answered (EXP-13-design.md: needs more discs), so content drops nothing: a track with the commentary pattern stays in the file (D15; row 13's fallback). Whether it also sends the job to review (`commentary_detected`, row 13's fallback) is the owner's decision, still open under D15. Veto (D21): when the programme reference is not the first track of its language on the disc, and that first track is lossless or has more than two channels, the rule confirms no commentary on the title; every candidate is kept and the job carries the review reason `audio_reference_unclear` (8.8).

Track confidence for audio is 0.95, because nothing is dropped on a guess. The review reasons above hold the job regardless.

### 8.7 English subtitles

Every subtitle stream that no rule below keeps is dropped, including all streams in other languages.

**Text (D20, D22, D23).** What a subtitle track is follows from its text (D23): an SDH track has sound cues and speaker labels, a plain track has none. The app reads the text of the English bitmap tracks with ocrs (D22) and judges it with experiment 14's rule (EXP-14-design.md, section 3). Section 17 row 14 (b) answered no, so the text is read only where 6.4 step 10 runs (two or more English subtitle streams); a title with one English track keeps it as "English" without the flag, as before. Measured in phase 0 (docs/phase0/ocr-engine-research.md, crop trial): the glyph-box crop does not cut the detector's cost, because ocrs 0.13.1's detection model takes a fixed 800 x 600 input (854 ms per read frame cropped, against about 930 ms on the full frame, on the J4125); the remedy to try first in phase 1 is to skip ocrs's detector and run its recognition on line boxes found from the bitmap's rows (recognition is about 240 ms of the 854); D22's Tesseract fallback applies only if that remedy also fails. The policy that rule serves (D20): two English dialogue tracks with the same dialogue and neither SDH, keep one; a regular track and an SDH track, keep both (R5); on a foreign-language film, an English dub transcript is kept beside the English translation; lyrics or sing-along tracks and trivia or pop-up text tracks are dropped when confirmed, and an unconfirmed one is kept and reviewed. Experiment 14 has not answered (EXP-14-design.md: needs more discs). Until it answers yes, the rules below decide by counts and timing, and the text only adds reviews (D20; section 17 row 14's fallback): where the text's verdict differs from theirs, the review reason `subtitle_ambiguous` with the note `subtitle_text_disagrees`. The text never drops a track and never sets or removes a flag.

**Set aside first.** These take no part in counting, alignment or grouping:

- derived forced-only streams (flags 4096 and 2048 together). The parent of such a stream is the nearest preceding non-derived subtitle stream of the same language. Verified in phase 0 (EXP-01-profile-and-order.md; `corpus/owner/the-office-2005-s3d3-bluray/scan.txt`): each forced-only stream directly follows its parent in the scan (all 8: the UHD's 2 in EXP-01, the Blu-ray's 6 in its scan; the DVDs have none). A forced-only stream that turns out empty is not in the file: MakeMKV removes it and says so with `MSG:4047` (6.2 step 7). Every one recorded so far was empty (the UHD's two and those of the Blu-ray episodes);
- DVD streams flagged director's comments (flags 1 or 2): dropped. Not yet verified: neither DVD of phase 0 flags any stream;
- the DVD closed-caption stream. Verified in phase 0 (`corpus/owner/short-circuit-2-1988-dvd/notes.md`, EXP-01, EXP-11): MakeMKV 2.0.0 lists it as a subtitle stream with codec `CC` (attribute 5 `S_CC608/DVD`), attribute 41 `Text`, attribute 42 `( Lossy conversion )`, tree info (attribute 30) "CC" U+2192 "Text English ( Lossy conversion )" (written here as `CC->Text English ( Lossy conversion )`) and language `eng`, after the bitmap subtitle streams. In the file it is a SubRip/SRT track (`S_TEXT/UTF8`) placed before the bitmap subtitle tracks (6.2 step 7), and MakeMKV makes it the default subtitle track (8.10). Whether it aligns with the dialogue tracks is not yet verified (EXP-06: not run; experiment 14 sets it aside unread).

**Events.** Counts and start times come from step 10 of the pipeline. For text streams each packet is one event. For image streams (PGS, VobSub) a packet larger than `sub_clear_max_bytes` (default 64) is one event and smaller packets, which clear the screen, are ignored. Verified in phase 0 (EXP-14-design.md, section 2): on the PGS tracks of a UHD and a Blu-ray the clear packets are 30 bytes and the smallest event 2,029 bytes. The DVD's VobSub tracks have no clear packets (their smallest packet is 148 bytes), so each of their packets is an event. A track is **full** when it has at least 10% as many events as the largest English track.

- **Alignment.** `aligned(A, B)` is the share of A's events that start within `subs_tolerance_ms` (default 300) of some event in B. Two full tracks are in the same group when the larger of `aligned(A, B)` and `aligned(B, A)` is at least `subs_same_min` (provisional default 0.70), and in different groups when it is at most `subs_diff_max` (provisional default 0.30). In between: review reason `subtitle_ambiguous`. Not yet verified (EXP-06: not run): no title recorded in phase 0 has two full English tracks, so the defaults stay provisional. Experiment 14 keeps these timing bands as its guard and fallback (EXP-14-design.md, section 1).
- **Dialogue group.** With one full track, it is the dialogue track. With several, the dialogue group is the one containing the parent of a forced-only stream that has events, else the one TheDiscDB describes as subtitles or SDH, else the largest group if it has two or more tracks. If none of these picks a group, review reason `subtitle_ambiguous`.
- **Regular and SDH.** Keep up to two tracks of the dialogue group. If two, the one with more events is SDH: hearing-impaired flag, named "English (SDH)"; the other is "English". A TheDiscDB description saying SDH overrides the count. The closed-caption stream is SDH by type and is kept when the dialogue group has no SDH track. More than two tracks in the dialogue group: review reason `subtitle_ambiguous`. The text adds reviews to these labels as **Text** above says.
- **Commentary or trivia.** A full track outside the dialogue group is dropped (R6).
- **Forced.** Keep at most one, in this order: a forced-only stream with events whose parent is the regular dialogue track, then one whose parent is the SDH track; a DVD stream flagged forced (flag 4096 without 2048; not yet verified whether MakeMKV sets it: neither DVD of phase 0 has a forced track, and short-circuit-2-1988-dvd's seven VobSub tracks are unflagged full tracks); a separately authored track that is not full and has fewer than 150 events. Flag it forced, name it "English (Forced)".
- **Other tracks** that are not full and have 150 events or more are dropped, with the note `partial_subtitle_dropped`.

Track confidence for subtitles:

| Case | Confidence |
|---|---|
| Flags or database descriptions decide every track, or the title has no English subtitle stream | 0.95 |
| Exactly one full English track | 0.90 |
| Alignment decides between two or more full tracks, with every pair outside the in-between band | 0.85 |
| `subtitle_ambiguous` was raised | 0.60 |

The job's track confidence is the lower of the audio and subtitle values.

### 8.8 Confidence, review reasons and disposition

Identity and title confidence are fixed at planning (6.1 step 5). Verification and the runtime sanity check can lower them. Track confidence does not exist until the Decide step (6.4 step 11). A review decision sets the confidences it covers to 1.00; nothing else raises one.

A job files automatically only if it has no review reason and all three confidences are at or above `auto_threshold` (0.85). Otherwise it is held for review with its rip or disc copy kept (D1).

| Review reason | Raised by |
|---|---|
| `identity_unknown`, `identity_ambiguous` | 8.1 |
| `title_ambiguous`, `multiple_features`, `unnamed_cut` | 8.3 |
| `tv_unmapped` | 8.5 |
| `original_language_missing`, `no_wanted_audio` | 8.6 |
| `audio_reference_unclear` | 8.6 (the content rule's veto, D21) |
| `subtitle_ambiguous` | 8.7 |
| `path_collision` | 7, 8.9 |
| `copy_hash_errors` | 6.3 |
| `verify_failed`, `read_errors` | 6.2 |

Title confidence below `auto_threshold` at planning time selects whole-disc copy mode.

Limits of D1: a Direct-mode job contains only the titles that were planned. If the owner later wants a different title from that disc, the disc has to be inserted again ("rip again"). Whole-disc copy mode exists so that this never happens for jobs the planner itself was unsure about.

### 8.9 Naming

In the patterns below, `<Res>` is the resolution label and `<Ed>` an edition name.

Movies: `<movies_root>\Title (Year) {tmdb-ID}\Title (Year) {tmdb-ID} - <Res>.mkv`. `<Res>` comes from the height of the first video track: 2000 or more is `2160p`, 1000 or more `1080p`, 700 or more `720p`, 576 `576p`, anything else `480p`.

Versions (D11): both servers treat files in one movie folder named `<folder name> - <label>` as versions of the same film. Every movie file carries its resolution label from the start, so the 1080p disc of a UHD combo pack files beside the 2160p copy without renaming anything. A second copy at the same resolution and edition is a `path_collision`.

Editions, by the `edition_style` setting:

| Style | Layout | Notes |
|---|---|---|
| `combined` (default) | same folder: `Title (Year) {tmdb-ID} - <Res> <Ed> {edition-<Ed>}.mkv` | intended to give Plex an edition and Jellyfin a version label; not yet verified (EXP-09: not run, it needs the owner's test library) |
| `plex` | one folder per edition: `Title (Year) {tmdb-ID} {edition-<Ed>}\Title (Year) {tmdb-ID} {edition-<Ed>} - <Res>.mkv` | the layout Plex documents; Jellyfin lists each as a separate item |
| `jellyfin` | same folder: `Title (Year) {tmdb-ID} - <Res> <Ed>.mkv` | Jellyfin's version picker; Plex treats them as plain versions |

Plex editions need Plex Pass. If experiment 9 shows `combined` misbehaving on either of the owner's servers, the default becomes `plex` when the owner has Plex Pass, else `jellyfin`. Experiment 9 has not been run, so `combined` stays the default.

TV: `<tv_root>\Show (Year) {tmdb-ID}\Season 01\Show (Year) - S01E01 - Episode Title.mkv`; multi-episode files use `S01E01-E02`; specials go in `Season 00`. Episode titles are cut to 90 characters.

Title and year always come from the same TMDb record as the ID.

Jellyfin 10.x does not read the curly-brace tag (Jellyfin 12 does). With `jellyfin10_compat` on, a second tag is inserted immediately after `{tmdb-ID}`:

- movies: ` [tmdbid-ID]`, in the folder name and the file name alike, so the file name still begins with the folder name. Example, `combined` style: `Film (2010) {tmdb-12345} [tmdbid-12345]\Film (2010) {tmdb-12345} [tmdbid-12345] - 2160p Extended Cut {edition-Extended Cut}.mkv`;
- TV: on the show folder only, never on season folders or episode files;
- Jellyfin 10.x reads a number between 1900 and 2099 as a year. For such a TMDb ID the tag is ` [imdbid-ttID]` for movies and ` [tvdbid-ID]` for shows; when TMDb gives no such ID the tag is left out and the note `compat_tag_omitted` is recorded.

How Jellyfin 10.x shows these `jellyfin10_compat` names is not yet verified (EXP-09: not run).

Every path component is sanitised: Unicode-normalised (NFC); `:` replaced by ` -`; `/ \ | ? * " < >` and control characters removed; `[ ] { }` removed from titles and edition names; repeated spaces collapsed; trailing spaces and periods trimmed; a reserved device name gets `_` appended; each component capped at 255 bytes. A filed file is never renamed; a changed decision files a new file and removes the job's old one (section 7).

### 8.10 Track metadata in the final file

Order: video; audio as in 8.6; subtitles forced, regular, SDH. A Dolby Vision enhancement layer is kept as MakeMKV outputs it; whether that is one video track or two is not yet verified (EXP-01-profile-and-order.md: the one UHD recorded is HDR10 without Dolby Vision, and its file has one video track). Every kept track has a language tag, except streams with no language. Flags set through `mkvmerge`: default, forced, hearing-impaired, original. Every kept track's default flag is set explicitly, because MakeMKV sets its own: it made a DVD's converted closed-caption track the default subtitle track although the scan marks the English VobSub default (EXP-01), and on an English-only rip of a Spanish film it made the one English audio track, a commentary, the default (EXP-11). The default subtitle is the forced track when the default audio is English; when the default audio is not English, the regular English track is default and the forced track is not. Audio names are the codec display name and the channel layout, for example "TrueHD Atmos 7.1". Chapters are kept.

## 9. MakeMKV integration

### 9.1 Versions

Supported: MakeMKV 2.0.0 and later 2.x. The facts in this section come from 1.18.x sources and logs; phase 0 re-verified them against MakeMKV 2.0.0 recordings of four discs (a UHD, a Blu-ray and two DVDs): the line layouts and value formats hold (9.4), and 9.2 to 9.5 mark what the recordings corrected or added. The MakeMKV version (`MSG:1005`) is recorded with every job and recording. A version below `makemkv_min_version`, or an unknown major version, is reported by the self-test.

### 9.2 Commands

Only robot mode (`-r`) is used. The process exit code is recorded with the log but decides nothing on its own: MakeMKV exits 0 on most failures. A run that exits without its completion message is a failure, whatever the exit code (D18): a scan without `MSG:5011` followed by a `TCOUNT` line, a rip without its success summary (9.4), a whole-disc copy without `MSG:5070`. Verified in phase 0 (EXP-03-whole-disc-copy.md): MakeMKV 2.0.0 crashed (exit code 0xC0000005) while scanning a folder copy, within 12 s, with no message and empty standard error; every scan that listed titles (6 scans of 4 discs) printed `MSG:5011` just before `TCOUNT`. Such a crash is `Unknown` (10.2), with the full log and the exit code, except in 6.3 step 6a.

- Drive list: `info disc:9999`.
- Scan: `info disc:N`.
- Rip: `mkv disc:N <index> <dir>`.
- Whole-disc copy: `backup --decrypt disc:N <dir>`; later `info file:<dir>` and `mkv file:<dir> <index> <out>`.

Scan and rip use identical `--minlength`, `--cache` and `--profile` values.

### 9.3 Job profile

Each job gets a generated conversion profile: a copy of MakeMKV's shipped default profile with a unique name and every `defaultSelection` value replaced by the literal string `+sel:all,-sel:mvcvideo,=100:all` (every stream except 3D video, with equal weights so the output order follows the scan order). Nothing else is changed, and the profile never refers to MakeMKV's own default-selection preference. If MakeMKV rejects the profile (`MSG:1009`), the call runs with MakeMKV's default profile and verification decides whether the result is usable.

Verified in phase 0 (EXP-01-profile-and-order.md): the profile is honoured in robot mode. No rip printed `MSG:1009`, and `MSG:5015` names the profile; a profile with a language selection was honoured too (EXP-11). The output follows the scan order, except that a text-converted subtitle track comes before the bitmap ones (6.2 step 7). MakeMKV's default profile has `insertFirstChapter00IfMissing="true"`, but the file's chapter count equalled attribute 8 on every rip (22 on the UHD and 28 on a DVD, EXP-01 and EXP-11; also on the Blu-ray episodes and the other DVD), so 6.2 step 7 requires that count exactly. Not yet verified: a title whose scan reports no chapters, where that setting could add one.

### 9.4 Output parsing

Line layouts, with title and stream numbers 0-based:

- `MSG:code,flags,count,"text","format","param"...`
- `PRGC:code,id,"name"` and `PRGT:code,id,"name"`; `PRGV:current,total,max`
- `TCOUNT:n`
- `CINFO:attribute,code,"value"`
- `TINFO:title,attribute,code,"value"`
- `SINFO:title,stream,attribute,code,"value"`

Value formats, verified in phase 0 on MakeMKV 2.0.0: every value in the owner's recordings matches (34 titles' durations, sizes and segment maps, 34 video sizes and 74 bitrates, in 6 scans of 4 discs; all 31 MakeMKV outputs recorded print `MakeMKV v2.0.0 win(x64-release)`): duration (9) `H:MM:SS`; size (11) decimal bytes; video size (19) `WxH`; bitrate (13) a number with a unit; segment map (26) a comma-separated list of clip numbers. Robot output is UTF-8 as far as phase 0 saw: its one non-ASCII character, the arrow (U+2192) in the closed-caption stream's tree info (attribute 30), `CC->Text English ( Lossy conversion )` (short-circuit-2-1988-dvd), is valid UTF-8 and comes out garbled when read as cp1252. Not yet verified on a disc title with non-ASCII characters (EXP-08: not run). A quote inside a field is written `\"` and a backslash `\\`: the quotes inside fields that the recordings hold (`MSG:5072` of EXP-03's copy, `MSG:1003` debug lines of its partial-copy scans) are backslash-escaped, never doubled. Quoted fields are parsed tolerating both backslash-escaped and doubled quotes. Text from MakeMKV is display-only: it is never used to build paths, and output files are found by listing the directory.

`DRV:index,state,999,fsflags,"drive name","disc label","device"`: state 0 empty, 1 tray open, 2 disc inserted, 3 loading, 256 no drive, 257 unmounting. On Windows the device field is the drive letter.

Attribute IDs used (from MakeMKV's public-domain `apdefs.h`): 1 type, 2 name, 3 language code, 5 codec ID, 6 codec short, 7 codec long, 8 chapter count, 9 duration, 11 size in bytes, 13 bitrate, 14 audio channels, 16 source file name, 17 sample rate, 18 sample size, 19 video size, 21 frame rate, 22 stream flags, 24 original title ID, 25 segment count, 26 segment map, 28 metadata language, 30 tree info, 40 channel layout, 41 output codec, 42 output conversion type, 49 comment. Video streams carry no attribute 3. Attribute 28 is the metadata language, not a stream's own: it is `eng` on every stream of the two titles EXP-11 lists, the Spanish, French and Thai ones included. A DVD scan has no attribute 16 (EXP-11). Attribute 13 is text with units ("1.5 Mb/s") and is parsed as such.

Messages acted on:

| Code | Meaning | Use |
|---|---|---|
| 1005 | version banner | record the version |
| 1009 | profile parsing error | fall back as in 9.3 |
| 2003 | read error | read-error watchdog; `MediaReadError` |
| 2019 | write error | `Output` |
| 3338 | downloading the latest keys (`HK`) | logged; a normal online message, not a failure, and its absence is no trigger (EXP-05) |
| 3028 | a DVD title added ("Title #N was added (C cell(s), H:MM:SS)"), one line per title in index order; N is the DVD title number, attribute 24, not the index (6.2 step 6) | index guard on a DVD |
| 3307, 3308 | file added as title #N (0-based); 3308 is the angle form; a DVD prints neither | index guard on a Blu-ray or UHD |
| 3309 | a title was skipped as equal to another (meaning not confirmed) | logged; the guard compares whole title lists |
| 3344 | Java runtime in use | labels in 8.3 are trusted only when seen |
| 4047 | a forced subtitles track turned out empty and was removed from the output file; its "#N" counts the title's selected streams in scan order from 0, not the scan's streams (EXP-01, EXP-11) | verification (6.2 step 7) does not count that stream as missing |
| 5003 | failed to save title | that title failed |
| 5004, 5037 | N saved, M failed | partial failure |
| 5005, 5036 | N titles saved; copy complete | rip success summary |
| 5010 | failed to open disc; also ends a scan that found no title of `--minlength` or more (EXP-03) | open failure |
| 5011 | operation successfully completed; printed just before `TCOUNT` by every scan that listed titles | scan complete (9.2, D18) |
| 5014, 5015 | saving N titles into directory; 5015 also names the profile, and every rip with the job profile prints 5015, not 5014 (EXP-02) | data reading begins |
| 5021, 5050, 5052, 5055, 5095 | key or evaluation problems | `Licence` |
| 5069, 5070, 5079 | backup failed; completed; completed with hash failures | whole-disc copy result (6.3) |

Other projects' logs also show `3032` (region mismatch) and `3303` with `5010` and no titles (the disc's key is unknown to MakeMKV). Not yet verified: no disc of phase 0 produced them, so they stay `Unknown`.

Seen in phase 0 but not acted on: `1002` (a `LIBMKV_TRACE` line), `1003` (debug lines), `1004` (debug log enabled), `1011` (LibreDrive mode), `2023` (the count of read errors of one type), `3006` (opening files on the hard drive), `3007` (direct disc access mode), `3025` (a title shorter than `--minlength`, skipped), `4007` (AV synchronization issues found in a file, printed with `4047`), `5072` (backing up the disc into a folder), `5081` (backup done) and `5085` (content hash table loaded).

A title rip succeeded only if a rip success summary reporting one title was seen, none of 5003, 5004 or 5037 was seen, and exactly one `.mkv` exists in the title's directory. Seen in phase 0: every successful rip printed `5005` and `5036`; the one failed rip (short-circuit-2-1988-dvd, an uncorrectable sector) printed `5003`, `5004` and `5037`, and MakeMKV had removed its partial file itself.

### 9.5 Stream flags

Attribute 22 is a decimal bitmask: 1 director's comments, 2 alternate director's comments, 4 visually impaired, 256 core audio, 512 secondary audio, 1024 has core audio, 2048 derived stream, 4096 forced subtitles. 2304 marks a derived core; 6144 marks a derived forced-only subtitle track. Unknown bits (16384, 32768 and 65536 are seen in real output) are ignored. Blu-ray discs carry no commentary flag; DVDs can. Not yet verified on a DVD: neither DVD of phase 0 flags any stream (their notes in `corpus/owner/`).

## 10. State, failures and watchdogs

### 10.1 States

Main line: `Detected → Fingerprinted → Scanned → Planned → Ripping` (or `Copying`) `→ RipVerified → Ejected → Inspecting → Decided → Remuxing → Verified → Filed`.

Other states: `NeedsReview`, `Extracting` (titles being extracted from a disc copy after approval), `AwaitingShare`, `AwaitingSpace`, `AwaitingMakeMkv`, `AwaitingNetwork`, `AwaitingLookup`, `AwaitingTool`, `Failed(class)`, `Discarded`.

The state machine is a pure reducer in `core`: (state, event) gives (new state, actions). The engine performs the actions and feeds results back as events. Every transition is persisted before its actions run. A job in a waiting state stores the state it will return to.

Events: media arrived or removed; fingerprint, scan, plan, title rip, verification, copy, extraction, probe, decision, remux and file verification each finishing or failing (with a failure class); ejected; share lost or back; space short or freed; licence or version message; MakeMKV version changed; network needed; lookup pending or answered; tool missing or found; review approved (with overrides), discarded, retry, rip again, re-finalise; engine restart.

Actions: run scan, rip a title, copy the disc, extract a title, verify a file, eject, probe, extract subtitle evidence, decide, remux, file, notify, start a timer, delete job data.

Transitions beyond the main line:

| From | Event | To |
|---|---|---|
| `Planned` | title confidence below threshold | `Copying` |
| `Copying` | copy complete and its scan lists titles (6.3 step 6a) | `Ejected`, then `NeedsReview` |
| `Copying` | copy complete but its scan failed: no `MSG:5011` and `TCOUNT` (D18) | `Ripping` (Direct mode, 6.3 step 6a); the copy is kept |
| `Decided` | review reason or low confidence | `NeedsReview` |
| `NeedsReview`, held after a disc copy with titles not yet extracted | Approve | `Extracting`, then `Inspecting` |
| `NeedsReview`, held at the disposition gate | Approve | `Decided` (with the owner's overrides), then `Remuxing` |
| `NeedsReview` | Discard | `Discarded` |
| any disc-phase state | share unreachable / out of space | `AwaitingShare` / `AwaitingSpace`; back to the interrupted state when resolved |
| any file-phase state | the same | the same |
| `Scanned`, `Ripping`, `Copying` | licence or version message | `AwaitingMakeMkv`; back when the installed version or key changes (checked at engine start and at each disc insertion) |
| `Scanned`, `Ripping`, `Copying` | disc needs a key from the internet | `AwaitingNetwork`; retried every 10 minutes |
| `Ejected`, `Inspecting` | a database answer the decision needs is still pending | `AwaitingLookup`; retried hourly. The disc phase never waits for a lookup |
| `Copying` | second read failure for this job | `Ripping` (Direct mode, 6.3) |
| any | required tool missing | `AwaitingTool`; back when the self-test finds it |
| `Ripping`, `Copying` | read failure | `Failed(MediaReadError)`; re-inserting the disc resumes |
| `Failed(*)` | Retry | the state that failed |
| `Filed` | Re-finalise from history | `Decided` |

Resuming after a restart: `Ripping` and `Extracting` empty the title's directory and run again; `Copying` empties the copy directory and runs again; `Remuxing` deletes the partial file and runs again; header-only steps simply run again. A disc-phase state whose disc is no longer in the drive waits for the disc.

### 10.2 Failure classes

| Class | Trigger | Result |
|---|---|---|
| `DriveIncapable` | MakeMKV reports the drive cannot read this disc type | `Failed`; no retry. Recorded in `%ProgramData%\lazymkv\drives.json`, and the UI warns when that drive next gets that disc type |
| `MediaReadError` | read errors with the staging share healthy; a stalled read; a failed title | `Failed`, resumable. Verified titles are kept. Notification says "try the other drive" |
| `Licence` | key or evaluation messages | `AwaitingMakeMkv`; the notification carries the exact message |
| `NeedsMakeMkvUpdate` | MakeMKV reports the disc needs a newer version | `AwaitingMakeMkv` |
| `NeedsNetwork` | MakeMKV cannot fetch a key it needs for this disc | `AwaitingNetwork` |
| `Output` | share unreachable, out of space, or a write error (the messages of a share drop are not yet verified: EXP-04: not run) | `AwaitingShare` or `AwaitingSpace` |
| `LookupUnavailable` | TheDiscDB or TMDb unreachable | `AwaitingLookup` |
| `ToolMissing` | a required program is not found or too old | `AwaitingTool`. Without `mkvmerge` no disc is started. Without `ffprobe` the disc phase runs and the file phase waits. Without Java, or when MakeMKV did not report using it, label rule 8.3.2 does not apply and a note records why (on the one UHD with Java menus recorded, MakeMKV did not report it with Java 8 installed: EXP-12, not answered) |
| `Unknown` | anything else, including a `makemkvcon` run that exits without its completion message (9.2, D18), except the copy's scan of 6.3 step 6a | `Failed`, with the full log and the exit code attached |

When a title fails, the staging share's reachability and any write error are checked first: those make it `Output`. Only a failure with the share healthy and read errors seen is `MediaReadError`.

The message texts for `DriveIncapable`, `NeedsNetwork` and `NeedsMakeMkvUpdate` are not yet captured. Experiment 5 did not capture `NeedsNetwork` (EXP-05-offline-and-unit-key.md): with MakeMKV's network access blocked, a UHD never scanned on the machine opened and listed the same titles as online, with no message of its own (MakeMKV's key database, cached data and LibreDrive were present; which of them let it open is not isolated). The only difference online was `MSG:3338` ("Downloading latest HK ..."), a normal message whose absence is no trigger. So `NeedsNetwork` stays `Unknown` until seen (section 17 row 5's fallback). The other two cannot be reproduced on the owner's drives and licence, and stay `Unknown` until one is seen in the field. No licence or key message (`5021`, `5050`, `5052`, `5055`, `5095`) was seen in phase 0.

Read errors seen in phase 0 (`corpus/owner/short-circuit-2-1988-dvd/notes.md`): a DVD rip in the Pioneer drive met one damaged sector of `VTS_01_1.VOB` and logged `MSG:2003` 24 times at that one offset (`MEDIUM ERROR:L-EC UNCORRECTABLE ERROR` and `STATUS_DEVICE_DATA_ERROR`; each message names the file or device and the offset), then `MSG:2023` ("Encountered 17 errors of type 'Read Error'", fewer than the 24 `MSG:2003` lines), `5003`, `5004` and `5037` after 712 s, and MakeMKV removed its partial file: a `MediaReadError`. After the owner cleaned the disc the rip succeeded. The same disc's scan and successful rip also logged `MSG:2003` "READ OF SCRAMBLED SECTOR WITHOUT AUTHENTICATION" at offset 1,048,576, which did not stop either (6.2 step 7).

### 10.3 The tray

The disc is ejected when lazymkv no longer needs it: after a successful disc phase, and when a disc is recognised as already ripped. It is not ejected on a failure or while waiting. An ejected disc (an open tray, or a disc pushed out of a slot-loading drive) therefore always means "insert the next disc". Verified in phase 0 (EXP-07-eject.md): eject works on both drives. The drive kept attached, the Pioneer BDR-XS07U, is slot-loading, so on it D3's "eject the tray" and `disc_done`'s "the tray opened" mean that the disc was pushed out; owner-facing texts say that the disc was ejected, which is true for both kinds of drive. Not yet verified: what the slot-loading drive does when an ejected disc is left in its slot, and the open-tray signal on it.

### 10.4 Watchdogs

Every external process runs under `ProcessGuard`. Only one `makemkvcon` runs at a time, and the engine checks that no earlier one is still alive before starting another. The self-test and each disc phase also check that the MakeMKV GUI is not running, since it holds the drive and the settings.

- `makemkvcon`: no progress (`PRGV`) for `stall_timeout_s` (default 300), or more than `read_error_rate` (default 20) `MSG:2003` lines in a minute, marks the drive as struggling in the UI. MakeMKV's own retries decide when a title fails.
- A scan that prints nothing for `stall_timeout_s`, or runs longer than `scan_timeout_min` (default 30; UHD scans with Java analysis can take many minutes), is ended and retried once. Seen in phase 0: the UHD scans took 16 to 19 s, offline as online, with no Java analysis (EXP-05, EXP-12), so how long a scan with Java analysis takes is not yet verified; the watchdog remains the only guard for an offline disc that would hang (EXP-05). A scan of a folder copy crashed within 12 s, before any watchdog would act (EXP-03; 9.2).
- `mkvmerge` remux and extraction: no progress output for `stall_timeout_s`.
- Header-only calls (`mkvmerge -J`, `ffprobe -show_streams`): a 120-second limit.
- If the staging share becomes unreachable during a rip or copy, the engine ends the attempt after `stall_timeout_s` without progress and classifies it as `Output`, not `MediaReadError`.
- A blocked read can keep `makemkvcon` from exiting. The engine waits up to `exit_wait_min` (default 30) for it to exit, then kills the process tree, does not eject, and, after the share check in 10.2, fails the job as `MediaReadError` with the note "process did not exit". Not yet verified (EXP-04: not run): whether `makemkvcon` exits after a share drop. On an uncorrectable sector MakeMKV's own retries ended the title (24 at one offset over 712 s) and the process exited (10.2).

## 11. Review queue

A held job keeps, in its job folder and staging: the rip or disc copy, the scan output, navigation-file copies, database answers, the plan with reasons and confidences, probe output and subtitle evidence.

Actions:

- **Set identity**: search TMDb and pick the film or show. For TV this also attaches the disc to a season session.
- **Assign titles**: map titles to the feature, to named cuts (with an editable edition name), or to episodes. Disc-copy jobs offer every title that passes the candidate filter (8.2); Direct jobs offer the titles that were ripped, and so does a disc-copy job whose copy could not be scanned (6.3 step 6a, D18).
- **Override tracks**: for audio, choose which tracks to keep, their order and the default track, and mark a kept track as commentary (it is then dropped); for subtitles, choose which tracks to keep and each one's role (forced, regular, SDH).
- **Resolve collision**: for `path_collision`, keep both (the new file gets a distinguishing label such as `- 2160p 2`), replace the existing file after an explicit confirmation (the old file is moved into the job's staging folder and kept for `retention_days`), or discard.
- **Preview**: open a ripped title in the default player; for a disc-copy job, open the title's stream file from the copy. This is how the owner checks which title is which episode.
- **Approve**: continue with the reviewed decisions (extraction first for disc-copy jobs).
- **Discard**: delete the job's staging data.
- **Retry**: for failed jobs.
- **Remember**: store the decisions against the fingerprint; a later job for the same fingerprint applies them.
- **Rip again**: arm a new job for a disc that was already ripped.
- **Re-finalise** (from history, while the raw data is retained): change decisions for a filed job and file again under the rules in section 7.

## 12. Notifications

Home Assistant (webhook) and the generic webhook receive the same JSON. Fields: `event`, `job_id`, `fingerprint`, `title` (when known), `drive`, `reason` or `class` (when relevant), `message`, `time`.

| Event | When |
|---|---|
| `disc_done` | disc phase succeeded and the disc was ejected (the tray opened, or a slot-loading drive pushed it out: 10.3); processing continues |
| `already_ripped` | a known disc was inserted and ejected |
| `filed` | a file was placed in the library |
| `needs_review` | a job was held, with its review reasons |
| `waiting` | a job entered an `Awaiting*` state, with the reason (`share`, `space`, `makemkv`, `network`, `lookup`, `tool`) |
| `resumed` | a waiting job continued |
| `failed` | a job failed, with its class; `MediaReadError` includes "try the other drive" |
| `eject_failed` | the disc should be removed by hand |

Delivery: a 10-second timeout, three attempts with backoff. A notification that cannot be delivered is logged and never blocks a job.

## 13. User interface

- **Tray icon** with a menu (Open, Quit). Closing the window keeps the app in the tray.
- **Status**: the current disc, stage and progress; the file-phase job and queue; the "drive struggling" state; waiting jobs with their reasons.
- **Review**: held jobs with their evidence and the actions in section 11.
- **History**: filed, failed and discarded jobs with their decisions and notes.
- **Settings**: the registry in section 14, plus share credentials, the TMDb key and webhook URLs.
- **Setup self-test**: drive visible to MakeMKV; MakeMKV licensed and at a supported version; Java found and reported in use by a scan; MKVToolNix and FFmpeg found at their minimum versions; each root reachable and passing the write-rename-delete test; TMDb key valid; webhooks reachable.

Progress is updated at most once a second.

## 14. Settings

| Key | Default | Meaning |
|---|---|---|
| `staging_root`, `movies_root`, `tv_root` | none | paths (section 7) |
| `minlength_s` | 120 | MakeMKV minimum title length, seconds |
| `cache_mb` | 1024 | MakeMKV read cache, MB. Not yet verified (EXP-04: not run); every phase 0 rip ran at 1024 straight to the share, with free RAM at least 2.97 GB during the UHD rips (EXP-01, EXP-11); the lowest of any rip was 1.79 GB (short-circuit-1986-dvd, DVD) |
| `auto_threshold` | 0.85 | confidence needed to file automatically |
| `feature_min_minutes` | 45 | shortest title considered a feature |
| `tmdb_only_autofile` | on | D9: a TMDb-only identification may file automatically |
| `prefer_most_channels` | on | D9: when ordering audio, channel count is compared before fidelity tier |
| `original_language_first` | on | D9: original-language audio first for non-English works |
| `subs_tolerance_ms` | 300 | start-time tolerance for subtitle alignment |
| `subs_same_min`, `subs_diff_max` | 0.70, 0.30 | provisional alignment thresholds; not yet verified (EXP-06: not run) |
| `sub_clear_max_bytes` | 64 | image-subtitle packets this small clear the screen (8.7; EXP-14-design.md: clear packets 30 bytes, smallest event 2,029 bytes) |
| `edition_style` | `combined` | 8.9; not yet verified (EXP-09: not run) |
| `jellyfin10_compat` | off | add the square-bracket ID tag |
| `retention_days` | 7 | how long raw data of filed jobs is kept |
| `keep_tuning_evidence` | on | D16: keep the tuning evidence of every ripped title (6.4 step 10a, section 7) |
| `tuning_root` | none (required while `keep_tuning_evidence` is on) | where the tuning evidence is kept; not inside `staging_root` or a library root |
| `file_phase_concurrency` | 1 | simultaneous file-phase jobs |
| `drive_settle_s` | 10 | wait after a disc arrives before touching the drive |
| `drive_ready_timeout_s` | 120 | how long to wait for the volume to mount |
| `scan_timeout_min` | 30 | longest allowed scan |
| `stall_timeout_s` | 300 | watchdog stall time |
| `read_error_rate` | 20 | read errors per minute that mark a drive as struggling |
| `exit_wait_min` | 30 | wait for a stuck `makemkvcon` to exit |
| `makemkv_min_version` | 2.0.0 | lowest supported MakeMKV |
| tool paths | auto | overrides for the programs in 4.4 |

Fixed values, not settings: the 3% and 15% runtime checks, the 2-second duration match, the 10% and 150-event subtitle limits, the 256 kb/s guard, the space margins.

## 15. Testing

- **`core`**: unit tests over recorded inputs: robot output from the owner's discs and from public logs. The main public source is TheDiscDB's data repository (MIT-licensed): about 11,000 real `makemkvcon` scan logs across UHD, Blu-ray and DVD, each with a parsed form and human-labelled title types. A pinned subset serves as parser fixtures, and the labelled titles as ground truth for the title rules in 8.3 and 8.5. Small MIT-licensed scan logs from the riplex and mkvbot projects add Windows-recorded cases. Further inputs: `mkvmerge -J` and `ffprobe` output; TheDiscDB and TMDb responses. Every rule in section 8 has a test for its normal case and for each guard, review reason and confidence row. The state reducer has a test per transition and per resume rule.
- **`tools` and `engine`**: stand-in executables for `makemkvcon`, `mkvmerge` and `ffprobe` that replay recordings with timing and injected failures (read errors, stalls, index drift, share loss, hash failures). Whole jobs run without a disc.
- **`clients`**: recorded HTTP responses.
- **`ui`**: GPUI's windowless test harness for the review and settings screens.
- **Acceptance** on real discs on the target machine, UHD first: a UHD film with Atmos or DTS:X; a UHD or Blu-ray with a commentary track and commentary subtitles; a two-cut disc; a non-English film with an English dub; a disc with decoy playlists; a TV disc; a DVD with a flagged commentary, closed captions and forced subtitles.
- **Rules against the stored evidence (D16).** `lazymkvctl retune [--set <threshold>=<value>]` re-runs the current decision rules over the tuning evidence of every job and over the phase 0 recordings, and joins the result with the owner's labels. Until `lazymkvctl` exists, `retune` is `lazymkv-dev retune [--set <name>=<value> ...]` (the `dev` crate, 4.2), the Rust port of phase 0's harness (pull request krcm0209/lazymkv#2); it covers the audio content rule of 8.6 and, with `--subtitles`, the subtitle rules of 8.7, which phase 1 brought under it after that port, as EXP-14-design.md expected (the end of this bullet). It prints, for each kind of track, the verdicts it gets, then every disagreement, and the verdicts that a changed threshold would alter. It exits non-zero when a track the owner labelled as programme audio would be dropped, when evidence was measured against another reference than the rule's own choice, and, separately, when some evidence could not be read. Every change to a measure or threshold of the content rule is run through it before release. On the phase 0 evidence it must reproduce the verdicts in `corpus/owner/audio-dataset.json`. `corpus/owner/` is the private repository `lazymkv-evidence`, cloned into the checkout and ignored by this repository (`corpus/README.md`), so `retune` and the recording reader (below) use it when it is present and report its absence otherwise, as `lazymkv-dev retune` does (exit 3, saying the evidence repository is not cloned there). Reproducing the verdicts also needs the audio level series, which neither repository holds (the evidence repository's `.gitignore`: `*/*/audio-levels-*/*.txt`; it keeps each level folder's `streams.json` and `audio-proxy.json`, and its `proxy-check.json` where the run wrote one, but no level series): they are kept in the tuning dataset (`$TuningData`, section 7) and in the working copy that pulled the recordings. On a clean clone of the evidence repository the harness therefore ends with exit 3 ("its level files are not here") until the level folders are copied back from the tuning dataset, and `retune` depends on the tuning evidence in the same way. `lazymkv-dev retune --subtitles [--text DIR]` judges every English or no-language subtitle track the owner labelled in a recording's `subtitle-labels.json` (below; edited with `lazymkv-dev labels --subtitles set`, `unset` and `check`) with the subtitle text rule of 8.7 over the OCR text given with `--text` (8.7's classifier for a stream the text rule sets aside), with section 14's default subtitle thresholds (`--subtitles` takes no `--set`). It counts the labels the owner made by reading or watching the track (D23), is sure of and gave before seeing the verdict (`saw_verdict` false), against the kinds section 17 row 14 requires (a recording counts as a feature film or a TV episode by its `tmdb.json`, else by its recorded TheDiscDB answer, else by 8.2's structural rule; the UK-style SDH track from a counted `sdh` label whose `style` is `uk`), never a label on one of the two discs row 14 names as fitting discs; prints the costly verdicts row 14 defines (a dialogue track dropped; SDH placed as regular, or regular as SDH; a track forced wrongly, or a forced track placed as dialogue; a commentary, trivia or lyrics track kept; each without a review); and exits 1 on any of them, 2 when a subtitle labels file does not validate, and 3 when there is no label or a labelled track could not be judged.

**Recordings.** A recording is one directory per disc, as phase 0 wrote them (`corpus/owner/<slug>/`: written on the mini PC by `scripts/phase0/record-disc.ps1` and the experiment scripts, with `lazymkv-capture`, and copied to the Mac by `scripts/phase0/pull-recording.sh`; `corpus/owner/README.md`, in the evidence repository). `corpus/owner/` is the private repository `lazymkv-evidence`, cloned into the checkout and ignored by this repository: the recordings are committed there, never here, so `retune` and the recording reader use them when the clone is present and report its absence otherwise (`corpus/README.md`). Each recording holds:

- `meta.json`, with exactly the keys `volume_label`, `format` (`bluray` or `dvd`), `fingerprint`, `disc_id` (the SHA-1 of `AACS/Unit_Key_RO.inf`, never the file; null on a DVD, whose disc ID is not yet computed (section 5), or when the file could not be read), `file_count`, `total_bytes` and `captured_unix`. It holds neither the MakeMKV version nor whether Java was in use: the version is the `MSG:1005` line of each recorded MakeMKV output, and Java use is whether a scan printed `3344` (6.1 step 4);
- `files.json` (the path and size of every file on the disc), `nav/` (the `bdmt` file and the navigation files of 6.1 step 2), `thediscdb-hash.json`, `thediscdb-discid.json` and `tmdb.json` (the lookups' answers) and `lookup-status.txt` (each lookup's HTTP status), and `scan.txt` (raw robot output);
- beside every recorded run `<name>` (a scan, a rip, a probe, an extraction): `<name>.stderr`, `<name>.timing.jsonl` (when each output line arrived) and `<name>.exit.json` (the exit code), written by `lazymkv-capture record`, and, for a MakeMKV run since D16, `<name>.makemkv-debug.txt` (MakeMKV's debug log);
- one folder per rip or other run, named by its label (for example `all-tracks`, `eng-only`, `rehearsal`, `backup`): the run's output and its summary (`rip.txt` and `rip-summary.json` for a rip; `backup-summary.json` for a disc copy); for a rip, `free-ram.csv` and, since D16, `system.csv` and `system-events.json`; `mkvmerge.json`, `ffprobe-streams.json`, `mediainfo.json` and `probe-summary.json`; the evidence extraction's and the remux's `mkvmerge` output; the subtitle packet lists (`subs/`: `index.json` and `sub-<n>.packets.json`); the OCR's counts and timings (`subs-ocr/`: `summary.json` and `sub-<n>.frames.json`, never text; a second reading in its own folder, such as `subs-ocr-changes`); and the audio level folders (`audio-levels-20ms-<streams>/`, of which the evidence repository keeps `streams.json`, `audio-proxy.json` and, where the run wrote one, `proxy-check.json`: see `retune` above);
- the owner's `notes.md`, audio labels (`audio-labels.json`, D16) and subtitle labels (`subtitle-labels.json`, format `lazymkv subtitle labels v1`, phase 1: per track the rip label, the stream, the kind, how the owner knows it, whether the owner is sure, the date, whether the owner saw the rule's verdict first (`saw_verdict`; such a label never counts), a note, and the style (`uk` for a UK-style SDH track, else null)), and, for the whole corpus, `audio-fitted.json` and the harness's baseline `audio-dataset.json`.

The stand-in executables replay `scan.txt` and rip logs with their original timing. The recording reader of phase 1 (`lazymkv-dev plan` and `decide`, `docs/phase1/decision-core.md`) is built from this layout; what of it the app's own recordings keep is designed in phase 2, with the engine that writes them.

## 16. Build phases

Each phase gets its own implementation plan and ends with something that runs and is tested on its own. Phases 2 to 5 end with acceptance on real discs.

| Phase | Delivers |
|---|---|
| 0. Evidence | Installs (section 19); the capture script and recording format (section 15); experiments 1 to 14; a corpus of recordings, with at least five saved TheDiscDB responses per endpoint and three known fingerprint test vectors; a pinned subset of TheDiscDB's public scan logs; the stand-in executables; this document updated with the results |
| 1. Decision core | `core`, with a command that reads a recording and prints the plan, decisions, confidences and review reasons as JSON. Tested on any OS. Its first step is the Rust port of phase 0's analysers (pull request krcm0209/lazymkv#2, `docs/superpowers/specs/2026-10-13-rust-port-design.md`): the robot-output parser, the reconciler, the audio content rule and the subtitle text rule in `core`, and `lazymkv-dev`, whose `decide` is that command's first form. The rest of `core` followed (`docs/phase1/decision-core.md`): the disc model, identification (8.1), the title planner (8.2 to 8.5), the track planner (8.6, 8.7, 8.10), the namer (8.9), the verification of 6.2 step 7, confidence and disposition (8.8), remembered decisions, season sessions and the job state reducer (10.1). The command is `lazymkv-dev plan` (the disc phase) with `lazymkv-dev decide`'s `plan` and `decision` sections (the file phase); `lazymkv-dev retune --subtitles` brings the subtitle rules under the harness (section 15). Phase 2 folds `plan` and `decide` into `lazymkvctl` |
| 2. Direct-mode engine | `platform` (Windows), `tools`, `clients`, the engine with persistence and notifications, and `lazymkvctl`. A known movie disc goes from insertion to a filed file with no UI |
| 3. Unsure discs and failures | Whole-disc copy mode, the review data model and its commands through `lazymkvctl`, failure classes, waiting states, watchdogs, resume |
| 4. Editions and TV | Cuts as editions; TheDiscDB-mapped TV; season sessions |
| 5. UI | The GPUI Kit tray app (section 13) and the Task Scheduler autostart |

Later features, in rough order of value:

1. Rip-time selection by language. When identity confidence is 0.99 or more, MakeMKV rips only the wanted-language and no-language audio and the English subtitles, writing straight into the library root's partial folder. A title whose ripped tracks need no drop and are already in the order 8.6 and 8.10 require is finished with in-place flag and name edits and a rename, with no remux. Any other title is remuxed as now. Not yet verified (EXP-11-language-selection.md: not answered): on a UHD title and a DVD title, a selection by language kept exactly the predicted streams under the audio language filter and `subtitle*eng`, but `nolang`, `special` and `core` on its own were not exercised (they wait for a disc with a language-less audio stream, a DVD with a flagged commentary and an English-only rip of a title whose derived cores are English). The wanted languages are English and the original language (R4), never English alone: on a Spanish film, English alone keeps only the commentary, which MakeMKV then makes the default audio track. The in-place edits set every default flag (8.10), and a title finished without a remux is verified as in 6.2 step 7, mapping `MSG:4047` through the selection and expecting a closed-caption track before the bitmap ones. Writing less did not shorten the UHD rip (2245 s with and without the selection).
2. Automatic TV episode numbering without a database, including splitting one title into episodes at chapters.
3. dvdcompare.net as a cross-check on commentary counts and cut runtimes, after asking its maintainers. Its robots.txt blocks AI agents, so sample pages for parser development would be saved by the owner.
4. Reading `bluray_project.bin` for playlist names, and running disc menus headlessly through libbluray with MakeMKV's libmmbd to observe which playlist a menu entry starts.
5. Audio analysis to detect commentaries the rules miss.
6. A local mirror of TheDiscDB, and contributing identifications back.
7. Linux and macOS implementations of `platform`.

## 17. Experiments (phase 0)

| # | Question | If the answer is no | Result (phase 0) |
|---|---|---|---|
| 1 | Is the job profile honoured in robot mode? Does output track order equal scan order, do output codecs match attributes 41 and 42, and does the output chapter count equal attribute 8? Does a derived forced-only stream directly follow its parent? On a Dolby Vision UHD, is the output one video track or two? Does MakeMKV write statistics tags (subtitle counts) into its output? | Use MakeMKV's default profile and align tracks on their full attribute signature; an alignment that is not unique becomes `verify_failed`. Without statistics tags, counts come from the evidence extraction (already the design) | yes (EXP-01-profile-and-order.md): the profile is honoured, output codecs are the unconverted input codecs (attribute 41 only on converted streams), the chapter count equals attribute 8, each forced-only twin follows its parent (an empty one is removed, `MSG:4047`) and statistics tags are written; the order follows the scan except that a DVD's text-converted closed captions come before the bitmap subtitles; Dolby Vision not answered (no Dolby Vision disc) |
| 2 | On a disc with angles or duplicate titles: start a rip for every title index, stop each after `5014`, and compare. Do the `3307`/`3308` title numbers equal the index ripped in every run, and do they precede `5014`? | Drop the pre-read guard; drift is caught by verification after the read | yes, but on a disc without angles or duplicate titles, so the condition is not yet met (EXP-02-index-guard.md): both titles of a UHD were numbered as in the scan, before `MSG:5015` |
| 3 | Does `backup --decrypt` to a network path work on a UHD with Java menus, and does `info file:` on the copy list the same titles (source file, segment map, duration) as the disc scan? | Title-unsure discs rip every title that passes the candidate filter (8.2) in Direct mode, most likely first, waiting in `AwaitingSpace` if they do not fit | no (EXP-03-whole-disc-copy.md): the copy to the share worked on a UHD with Java menus, but MakeMKV 2.0.0 crashed scanning it, so `info file:` listed no title; D18 keeps 6.3 and applies this fallback when the copy's scan fails |
| 4 | Ripping a UHD title straight to the owner's share at `cache_mb` 1024: does free RAM stay above 1 GB, with no slow-write warning from MakeMKV? Also record the throughput, the messages a share drop produces and the time of a NAS-to-NAS remux of a UHD title | Lower `cache_mb` to 512, then 256. A slow remux changes no design, only expectations | not run: the owner has not done the NAS step (`EXP-04-rip-to-share.md` not written); the rips of experiments 1 and 3 give the throughput and RAM figures (sections 14 and 18) |
| 5 | With MakeMKV's network access blocked (a temporary firewall rule added by the owner, so SSH stays up), what does it report for a UHD never scanned on this machine? Run before any other scan of that disc. Can `AACS/Unit_Key_RO.inf` be read from a mounted UHD in both drives? | `NeedsNetwork` stays `Unknown` until seen. Without the file, lookups use the fingerprint only (identity 0.95 at best from TheDiscDB) | no message, and yes (EXP-05-offline-and-unit-key.md): a never-seen UHD opened offline with the same titles and no message of its own, so `NeedsNetwork` stays `Unknown`; `Unit_Key_RO.inf` was readable in both drives |
| 6 | Does `ffprobe` tell Atmos and DTS:X apart on real rips? On at least three discs with two or more English subtitle tracks (including known commentary subtitles, SDH and DVD closed captions): does every same-content pair align at 0.70 or more and every different-content pair at 0.30 or less, and what packet size separates "clear" packets? | Without profile detection from `ffprobe`, MediaInfo is tried as the detector; failing that, the object-audio criterion in 8.6 is skipped and ties go on to its later criteria. Without a clean separation, any title with two or more full English tracks raises `subtitle_ambiguous` | not run (`EXP-06-audio-profiles-and-subtitles.md` not written): no disc with Atmos or DTS:X and no title with two or more full English subtitle tracks was recorded; the one UHD recorded has neither format (EXP-01's `ffprobe` profile `DTS-HD MA`; its `notes.md`); experiment 14 keeps 8.7's timing bands, so the alignment half stays open |
| 7 | Does programmatic eject work on both drives? | The `eject_failed` notification asks the owner to remove the disc | yes on both drives (EXP-07-eject.md): all four steps succeeded; drive 2 is slot-loading and pushes the disc out |
| 8 | What encoding and quote escaping does robot output use for a title with non-ASCII characters and quotes? | Decode as UTF-8, replacing invalid bytes; MakeMKV text is display-only already | not run (`EXP-08-encoding.md` not written): the recordings' one non-ASCII character is valid UTF-8, and the quotes inside fields that the recordings hold (`MSG:5072`, `MSG:1003` debug lines) are backslash-escaped, never doubled (9.4); a non-ASCII disc title is not yet verified |
| 9 | On the owner's Plex and Jellyfin, using short generated test files: does a single file named with a resolution label show normally, do a 2160p and a 1080p file in one folder show as versions, and do `combined` edition names show correctly? (No disc needed.) | The edition default becomes `plex` or `jellyfin` (8.9). If a lone labelled file misbehaves on either server, stop and return to the owner: D11 and 8.9 are revisited before phase 2 | not run: it needs the owner's test library (`EXP-09-naming.md` not written) |
| 10 | What share of at least 20 of the owner's discs does TheDiscDB know (fingerprint step only)? | No design change; a low share makes the TMDb-plus-structure route the main one and sets test priorities | not run as an experiment (`EXP-10-thediscdb-hit-rate.md` not written): of the 4 discs recorded, TheDiscDB knows 2 by fingerprint (the-office-2005-s3d3-bluray, pans-labyrinth-2006-uhd) and not the 2 DVDs; not yet verified (EXP-10: 4 of about 20 discs) |
| 11 | With a profile that selects by language (wanted-language and no-language audio, English subtitles, no derived cores, no flagged commentary), does MakeMKV rip exactly the streams predicted from the scan, and in what order? | The rip-time fast path (section 16) is not built; every title is remuxed | not answered (EXP-11-language-selection.md): on a UHD title and a DVD title the audio language filter and `subtitle*eng` kept exactly the predicted streams; `nolang`, `special` and `core` on its own were not exercised |
| 12 | On a UHD with Java menus, does the scan report a Java runtime in use (`3344`) and main-feature labels in attribute 49? | Label rule 8.3.2 is unavailable; titles are chosen by database and structure only | not answered, leaning no (EXP-12-java-labels.md): on a UHD with Java menus, with Java 8 installed and `app_Java` unset, the scan printed no `3344` and no labels |
| 13 | Measured over whole titles on the owner's own discs, and compared with the owner's label for every extra audio track in a wanted language, does the content rule of 8.6 give its commentary verdict (row 4: DC <= 0.30; AG >= 0.50 over all the dialogue gaps; at least 6 of the ten equal tenths of the overlap span each holding at least 10 s of dialogue gaps, with AG >= 0.40 in at least 90% of those tenths; SMI >= 0.50; LS <= 13 dB; added sound >= 60 s; the judgeability checks of row 3 passed and row 5's reverse commentary pattern not met) to **no** programme track, and to at least one commentary on each of at least 3 different discs? Before the answer may be yes, the labelled tracks must include: at least 8 commentaries from at least 4 discs, at least 2 of them on feature films and at least 1 with the film clearly audible under the voices; at least 1 audio-description track if the owner has a disc with one (all that the owner has, up to 3); at least 20 programme tracks other than the reference, at least 6 of them separately encoded lossy companions or alternate mixes (not MakeMKV-derived cores, not exact duplicates); at least 1 title whose reference has no centre channel (mono or 2.0) and at least 1 dialogue-heavy or speech-only title, each with an extra track; and every isolated score, M&E track, second same-language dub or narration, and every dub in a wanted language, the owner has. A track counts only when its content was checked before the verdict (a person listened to it; D23); the packaging, the menu, a flag and TheDiscDB are recorded as observations and do not count on their own. Every threshold was fitted on two tracks of the Office disc (`the-office-2005-s3d3-bluray`: the S03E16 commentary, title 0, and the S03E15 duplicate, title 1), so no track of that disc counts towards any of the totals above, and that disc is not one of the 4 discs or of the 3 discs with a commentary verdict; its tracks are still measured and reported, and a programme track of that disc with the commentary verdict still makes the answer no. A candidate in another language than the reference gets at most the verdict `commentary-cross-language` (kept and flagged, never dropped): on a programme track it is a false verdict like the commentary verdict, but it does not count towards the 3 discs with a commentary verdict, and the answer covers only candidates in the reference's language. Content may drop a candidate in another language only after a further yes for which the labelled tracks include at least 3 dubs in a wanted language from at least 2 discs, at least 1 of them on a dialogue-heavy title, with no commentary pattern on any of them, and the owner's decision. A disc or track used to fit a later threshold change is listed in `corpus/owner/audio-fitted.json` and counts no more either, nor does any track with its packets. | Content never drops a track: a track with the commentary pattern stays in the file and is flagged for review (`commentary_detected`); every other look-alike gets a note (D12). If the owner's discs cannot supply the tracks listed, the answer is "not answered": the report gives the count per kind against the list and names the missing kinds, and the same fallback applies. A single programme track with the commentary verdict makes the answer no, whatever else was measured. | not answered: needs more discs (EXP-13-design.md; its harness `audio_dataset.py`): 0 of the 8 commentaries, 0 of the 20 programme tracks and 0 of the 6 separately encoded ones count so far (the one derived core counted before 2026-10-12 is labelled from its scan flag, an observation under D23), and there is no false verdict |
| 14 | (a) Read by OCR (`ocr-subs.ps1`, Tesseract, English model) and judged by the text rule of 8.7 (EXP-14-design.md section 3, every threshold as set there), and compared with the owner's label for every English and no-language subtitle track of the titles read, does the rule give **no costly verdict**, that is: no track the owner names as dialogue (regular, SDH, translation, dub transcript, closed captions) dropped, with or without a review; no track the owner names SDH labelled regular without review, and none the owner names regular labelled SDH without review; no track flagged forced without review that the owner does not name forced, and no forced track labelled regular or SDH without review; no commentary, trivia or lyrics track kept as regular, SDH or forced, each without review; and does it place at least half of the labelled tracks without review? Before (a) may be yes, the labelled tracks must include, from discs other than the two fitting discs: at least 3 titles from at least 2 discs (at least 1 feature film and 1 TV episode) that each have both a regular and an SDH English track; at least 6 SDH and 6 regular English tracks in all; at least 1 real forced English track with events (separately authored, or a MakeMKV forced-only stream that was not empty); at least 1 commentary subtitle track; at least 1 DVD with English VobSub subtitles (and its closed-caption stream if it has one); at least 1 UK-style SDH track (speakers by colour, sounds without brackets) if the owner has a UK disc; and every trivia, lyrics, dub-transcript or second-translation track the owner has. A track counts only when its content was checked before the verdict (its text was read, by OCR text or by watching it; D23); the packaging, the menu, a flag, the player and TheDiscDB are recorded as observations and do not count on their own. Every threshold was fitted on `the-office-2005-s3d3-bluray` (S03E16) and `pans-labyrinth-2006-uhd`, so none of their tracks counts towards any total above; they are still read and reported, and a costly verdict on them still makes the answer no. (b) On the J4125, does reading one English track take at most 150 s per hour of running time (ffmpeg plus tesseract, as `summary.json` gives them) for every track read with the chosen render mode? | (a) no, or not answered: the text only adds reviews. 8.7 decides as today; where the text verdict differs, `subtitle_ambiguous` with the note `subtitle_text_disagrees`; the text never drops a track and never sets or removes a flag. If the owner's discs cannot supply the tracks listed, the answer is "not answered": the report gives the count per kind against the list and names the missing kinds, and the same fallback applies. A single costly verdict makes the answer no, whatever else was measured. (b) no: the text runs only where 6.4 step 10 runs today (two or more English subtitle streams), and a title with one English track keeps it as "English" without the flag, as today. | (a) not answered: needs more discs (EXP-14-design.md): no costly verdict on the tracks read (three SDH tracks of the-office-2005-s3d3-bluray, the English and Spanish tracks of pans-labyrinth-2006-uhd, the English VobSub of short-circuit-2-1988-dvd), but apart from the two fitting discs only that one regular track is labelled. (b) no (Tesseract, canvas and changes renders: 361-487 s per hour of running time on the UHD and the Blu-ray episodes, 117 s/h on the DVD; summary.json): the text is read only where 6.4 step 10 runs (6.4, 8.7) |

## 18. Risks

- **GPUI Kit churn.** Exact pins, a thin UI, and an engine that does not depend on the UI limit the damage. Free RAM fell to 0.31 GB during the spike build; the real dependency tree is larger.
- **Commentary on Blu-ray and UHD** has no flag. An unflagged commentary that no database describes stays in the file as an extra audio track (D12); the job carries a `possible_commentary` note when a track looks like one. Commentary subtitles are separated by timing, which can be wrong or send a disc to review.
- **TheDiscDB coverage** is small and mostly US releases. Of the 4 discs recorded in phase 0 it knows 2 by fingerprint (a Blu-ray and a UHD) and neither DVD, and its `/api/discid/` knew neither of the two discs it was asked about (the DVDs have no disc ID yet; 8.1). Not yet verified (EXP-10: 4 of about 20 discs).
- **Ripping straight to a network share** (D4) is what other auto-rippers advise against: a share that stalls or drops mid-rip costs a full re-read of that title. The share checks, the stall watchdog and the re-rip while the disc is still in the tray are the mitigations; a local staging path remains a setting.
- **Network cost.** Each UHD title crosses the network two to three times after eject. Measured in phase 0 on a 54 GB UHD feature ripped to the NAS staging share (EXP-01-profile-and-order.md): the rip took 37 min (23.1 MB/s), the subtitle evidence extraction 13 min and a full remux from the share back to the share 27 min, so those two file-phase reads take about 40 minutes per such title; a whole-disc copy of the 56.9 GB disc took 39 min at 23.0 MB/s, the rip's rate (EXP-03). A share drop, and `cache_mb` against free RAM, are not yet verified (EXP-04: not run).
- **MakeMKV behaviour** is partly undocumented. Recordings carry version numbers, and the stand-in tools make regressions visible. MakeMKV 2.0.0 can also crash without any message (EXP-03); 9.2 treats a run without its completion message as a failure.
- **Whole-disc copy.** Verified in phase 0 (EXP-03-whole-disc-copy.md): the copy to the share works (56.9 GB in 39 min, 23.0 MB/s), but MakeMKV 2.0.0 crashed without a message scanning the copy of that UHD (one playlist, `00055.mpls`), so nothing could be extracted from it. D18 keeps 6.3 and falls back to direct rips when the copy's scan fails (6.3 step 6a). Extraction from a copy is not yet verified.
- **Code signing.** Smart App Control, enforcing on the target machine until the owner turned it off on 2026-10-06, blocked the unsigned Rust toolchain (`docs/phase0/environment.md`). An unsigned lazymkv installer or executable would be blocked the same way wherever it is on, so they need code signing (phase 1 design; D17).

## 19. Setup prerequisites

- On the target machine: done in phase 0 (`docs/phase0/environment.md`; winget, approved by the owner on 2026-10-04), with the versions of 4.4: MakeMKV upgraded from 1.18.3 to 2.0.0, MKVToolNix 100.0.0 (mkvmerge v100.0) and FFmpeg 9.0.2 installed; Java 8 (1.8.0_471) is installed. MediaInfo 26.05 and, on 2026-10-06, Tesseract OCR 5.4.0 were installed as well, for phase 0's measurements only.
  (These are the phase 0 prerequisites on the owner's machine. For other users D17 applies: the released app ships everything except MakeMKV (nothing is downloaded at run time), and the setup steps a user performs by hand are installing MakeMKV and running lazymkv's own installer.)
- A TMDb API key created by the owner.
- Staging and library paths, with share credentials entered on the target machine.
- A Home Assistant webhook, and optionally a generic webhook URL.
- Which media servers the owner runs and their versions, and whether Plex Pass is available (for 8.9): not yet known (EXP-09: not run; it needs the owner's test library).
- A code-signing certificate for lazymkv's installer and executables (section 18, Code signing).
- Optional: Windows auto-logon, so the logon task starts after a power cut.
- Discs for phase 0, UHD where possible: a film with Atmos or DTS:X; one with a commentary track and commentary subtitles; a two-cut disc; a disc with decoy playlists; one with Java menus; one with angles or duplicate titles; one never scanned on this machine; a combo pack with its Blu-ray; a non-English film; a TV disc; a disc whose title has non-ASCII characters; a DVD with closed captions; about 20 discs in all for experiment 10; and both drives.
- Access to both media servers for experiment 9.

## 20. Sources

- MakeMKV command line: https://makemkv.com/developers/usage.txt
- MakeMKV attribute and flag definitions (`apdefs.h`, public domain), as mirrored in makemkv-oss source trees.
- TheDiscDB: https://thediscdb.com and its open data and tooling.
- TMDb API: https://developer.themoviedb.org
- GPUI Kit: https://gpui-kit.com ; tray: https://github.com/domenkozar/gpui-tray
- MKVToolNix: https://mkvtoolnix.download ; FFmpeg: https://ffmpeg.org
- Jellyfin and Plex naming documentation.
- Automatic Ripping Machine (https://github.com/automatic-ripping-machine/automatic-ripping-machine) and sibling projects (Engram, riplex, BluForge, mkvbot, auto-dvd-rippr, medialibrarian), studied for failure modes and operating lessons. No code is taken from Engram (AGPL) or BluForge (non-commercial licence).
- TheDiscDB data repository (MIT), for scan-log fixtures.

Research notes with per-claim sources and verification verdicts were produced during design on 2026-10-04 and inform sections 8 to 10.
