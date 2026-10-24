# Experiment 9: do the names of spec 8.9 show correctly on the owner's media server?

- Date: 2026-10-21 (mini PC clock)
- Disc(s): none: generated files
- MakeMKV version: not used
- Spec reference: section 17, row 9

## Question

On the owner's media server, using short generated test files named as spec 8.9 prescribes: does a single file
named with a resolution label show normally (Alien), do a 2160p and a 1080p file in one folder show as versions of
one film (Inception), and do the `combined` edition names show correctly (Blade Runner)? The owner runs Jellyfin
12.1 and no Plex server (P36), so the experiment runs on Jellyfin alone and the Plex layouts of spec 8.9 stay
untested.

## What was run

- The setup. The owner runs Jellyfin 12.1 on an M4 Mac mini, which reads the NAS over NFS. The mini PC writes to
  the NAS over SMB. The test library is `\\unas-pro\media\lazymkv-test-library\Movies`, an empty folder the owner
  made and added to Jellyfin as a Movies library. PLEX_VERSION: no Plex server. PLEX_PASS: no Plex server.
  JELLYFIN_VERSION: 12.1. Task 16's Plex questions (Steps 1, 5 and 6) were therefore answered "no Plex server" and
  asked of Jellyfin only.
- `scripts/phase0/make-test-library.ps1`, written for this experiment (P35). It makes two clips with ffmpeg on C:
  before the first copy, then copies five files into the library. Both clips are 8 seconds long, one 3840x2160 and
  one 1920x1080, H.264 and AAC in Matroska. The script refuses a library folder that is not empty, never overwrites
  a file, and deletes nothing in the library.
- The combined run (Step 4): `scripts/phase0/job.sh make-test-library.ps1 exp09-combined` (the default
  `-Style combined`), then `scripts/phase0/wait.sh exp09-combined`. Log `exp09-combined.log`; the summary says
  `"ok": true`, `"run": "combined"`, started `2026-10-21 19:57:55`; the log ends `RESULT ok`. The library line read
  `library: \\unas-pro\media\lazymkv-test-library\Movies (0 entries, hidden ones included): none`. The five files:

  ```
  \\unas-pro\media\lazymkv-test-library\Movies\Alien (1979) {tmdb-348}\Alien (1979) {tmdb-348} - 2160p.mkv  34,252,261 bytes
  \\unas-pro\media\lazymkv-test-library\Movies\Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 2160p.mkv  34,252,261 bytes
  \\unas-pro\media\lazymkv-test-library\Movies\Inception (2010) {tmdb-27205}\Inception (2010) {tmdb-27205} - 1080p.mkv  8,160,017 bytes
  \\unas-pro\media\lazymkv-test-library\Movies\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Theatrical Cut {edition-Theatrical Cut}.mkv  34,252,261 bytes
  \\unas-pro\media\lazymkv-test-library\Movies\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Final Cut {edition-Final Cut}.mkv  34,252,261 bytes
  ```

- The `jellyfin` run (Step 6), because question 3 was no on the combined layout and spec 8.9 names `jellyfin` for an
  owner without Plex Pass. The owner emptied the test folder and scanned Jellyfin again, then the job was started as
  `scripts/phase0/job.sh make-test-library.ps1 exp09-jellyfin -Style jellyfin`. It took three attempts. The first
  two failed at the first copy (see "The deleted folders" below), each leaving a summary with `"ok": false`. The
  third added `-Force`, which Task 16 allows only because the summary on disc said `"ok": false`. The log kept,
  `exp09-jellyfin.log`, is the third attempt's: summary `"ok": true`, `"run": "jellyfin"`, `"style": "jellyfin"`,
  started `2026-10-21 20:15:47`, ending `RESULT ok`, library line again `(0 entries, hidden ones included): none`.
  Alien and Inception are named as in the combined run, with the same sizes; the two Blade Runner files, in one
  folder, are:

  ```
  \\unas-pro\media\lazymkv-test-library\Movies\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Theatrical Cut.mkv  34,252,261 bytes
  \\unas-pro\media\lazymkv-test-library\Movies\Blade Runner (1982) {tmdb-78}\Blade Runner (1982) {tmdb-78} - 2160p Final Cut.mkv  34,252,261 bytes
  ```

- Step 7 (the `[tmdbid-ID]` names that Jellyfin 10.x needs) was not run: Jellyfin is not 10.x.

## What was observed

The combined run, the owner's answers on Jellyfin 12.1 (there is no Plex server to ask):
1. Alien appears once, with the right poster and title. Yes.
2. Inception appears once, with two versions to choose from, 2160p and 1080p. Yes.
3. Blade Runner appears once, as one film with two versions, but the Version list shows the raw file suffix instead
   of edition names: `2160p Final Cut {edition-Final Cut}` and `2160p Theatrical Cut {edition-Theatrical Cut}`. The
   owner showed this in a screenshot. No.
4. Nothing else odd. None of the three films was missing and the server gave no reason to skip one.

Step 6, STYLE `jellyfin`, question 3 on Jellyfin 12.1: Blade Runner appears once, with versions `2160p Theatrical Cut`
and `2160p Final Cut`, and nothing else is odd. Yes.

Step 7: not run, Jellyfin is not 10.x.

PLEX_VERSION: no Plex server. PLEX_PASS: no Plex server. JELLYFIN_VERSION: 12.1.

No names were added to the script's ignore list: the library line listed `none` in both runs that completed.

The 8-second length: no film was missing in either run, so it was not a factor and there was nothing to rule out.

### The deleted folders

After the owner deleted the combined run's three film folders from the Mac, the first two `jellyfin`-layout runs
failed at the first copy with `Could not find a part of the path '...\Alien (1979) {tmdb-348}\...'`, and their
summaries said `"ok": false`. The script's emptiness check had passed each time, because the share listed the
library folder as empty.

A probe job from the mini PC (a temporary script that created and removed only its own probe entries) found that a
new folder name could be created and written into, while each of the three deleted names was reported by the share
as already existing, yet was invisible in a listing and could not be written into. The NAS's own file manager showed
the folders gone, and Jellyfin still listed the films until its next scan. Some minutes later a second probe created
all three names normally, and the third run, with `-Force`, succeeded.

The cause was not established. The likeliest reading is that the SMB server held the deleted folders in a
delete-pending state while another client kept them open. The incident does not bear on the answer below, but it
bears on phase 2 (see the consequences).

## Answer

**No:** on Jellyfin 12.1 the `combined` layout fails question 3 (Blade Runner is one film with two versions, but
the Version list shows the raw `{edition-...}` suffix), although questions 1 and 2 hold (Alien once, Inception once
with its 2160p and 1080p versions), and the `jellyfin` layout shows the two editions cleanly; Plex was not tested,
as the owner runs no Plex server.

## Consequence for the spec

- Spec 8.9 and section 14 (`edition_style`): the default changes from `combined` to `jellyfin`, as 8.9 says for an
  owner without Plex Pass, and Step 6's run confirms that layout shows correctly on Jellyfin 12.1: Blade Runner
  once, with versions `2160p Theatrical Cut` and `2160p Final Cut`. The sentence "Experiment 9 has not been run,
  so `combined` stays the default" and the notes "not yet verified (EXP-09: not run)" take this result. `combined`
  is recorded as showing the raw `{edition-...}` suffix in Jellyfin's Version list. The `plex` layout and the
  `combined` layout's intended effect on Plex stay untested. The change bears on phase 4, since phase 2 files
  single-cut movies.
- D11 and the versions rule of 8.9: none. Questions 1 and 2 are yes, so a lone file with a resolution label shows
  normally and a 2160p and a 1080p file in one folder show as versions of one film. They are not revisited, and
  spec 17 row 9's stop-and-return condition is not met.
- Spec 17, row 9: the result is this report; the answer is no for the `combined` layout and the default becomes
  `jellyfin`.
- Spec 19, the open item "Which media servers the owner runs and their versions, and whether Plex Pass is
  available": the value is PLEX_VERSION no Plex server, PLEX_PASS no Plex server, JELLYFIN_VERSION 12.1, written
  into spec 19 with P36 on the same branch as this report.
- `jellyfin10_compat` (8.9, section 14): Step 7 was not run, because the owner's Jellyfin is 12.1. How Jellyfin
  10.x shows the `[tmdbid-ID]` names remains unverified, and no name is known to misbehave.
- Phase 2: the engine's rename into a library may meet the same state after an owner deletes a folder, so its
  `path_collision` handling should treat an "already exists" answer for a folder that a listing does not show as a
  collision to report, not as success.
