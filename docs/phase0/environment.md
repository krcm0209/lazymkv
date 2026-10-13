# Mini PC environment (2026-10-04)

Output of `scripts/phase0/setup-check.ps1`, run in the owner's desktop session
with `scripts/phase0/job.sh setup-check.ps1 setup-check`:

```text
makemkvcon : MSG:1005,0,1,"MakeMKV v2.0.0 win(x64-release) started","%1 started","MakeMKV v2.0.0 win(x64-release)"
mkvmerge   : C:\Program Files\MKVToolNix\mkvmerge.exe : mkvmerge v100.0 ('Do Hot Girls Like Chords') 64-bit
ffprobe    : C:\Users\USER\AppData\Local\Microsoft\WinGet\Links\ffprobe.exe : ffprobe version 9.0.2-full_build-www.gyan.dev Copyright (c) 2007-2026 the FFmpeg developers
ffmpeg     : C:\Users\USER\AppData\Local\Microsoft\WinGet\Links\ffmpeg.exe
mediainfo  : C:\Users\USER\AppData\Local\Microsoft\WinGet\Links\MediaInfo.exe : MediaInfoLib - v26.05
java       : 
drive      : D: is MakeMKV disc index 0
free RAM   : 4.02 GB
TMDB_TOKEN : NOT SET
profiles   : all-tracks=True eng-only=True
staging    : \\unas-pro\media\lazymkv-staging : write, rename and delete OK
staging free: 38124.7 GB
PHASE0-DONE status=ok exit=-1
```

Notes on that output:

- `java` is blank because MakeMKV's `app_Java` setting is unset; MakeMKV then looks for
  Java itself (Java 8, 1.8.0_471, is installed). Experiment 12 checks whether it is used.
- `TMDB_TOKEN` had not been set by the owner yet when this was recorded. The owner set it on
  2026-10-05; TMDb accepts it (HTTP 200 from `/3/authentication`) both from an ssh session and
  from a desktop-session job. The token itself is stored nowhere in this repository.
- `status=ok` means the script did not throw. `exit=-1` is not a failure: the value is only
  the exit code of the last native command, here `makemkvcon64 info disc:9999`, which PowerShell stops early once
  `Get-DiscIndex` has the line it needs.
- The staging share is reachable only from the owner's desktop session (saved
  credentials). From an SSH session `\\unas-pro\media` gives "Access is denied".

## System

- Windows: Microsoft Windows [Version 10.0.26200.9457]
- Intel Celeron J4125, 7.8 GB RAM, 119 GB SSD
- PowerShell 5.1.26100.9444
- Rust 1.99.0 (MSVC), sources in `C:\lazymkv\src`, build output in `C:\lazymkv\target`
- Smart App Control is ENFORCING on the mini PC (registry `HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy`,
  `VerifiedAndReputablePolicyState` = 1) as of 2026-10-06 14:14: `cargo` is blocked ("An Application Control policy has
  blocked this file", os error 4551; Code Integrity events 3077/3118 name `rustup.exe` loading the toolchain's cargo).
  `rustc --version` still runs, and the already-built `lazymkv-capture.exe` runs in jobs and over ssh. Builds on the
  mini PC are impossible until the owner decides what to do about the policy (nothing here changes a security setting).
  Microsoft's documentation (read 2026-10-06): Smart App Control has no per-app exception; the Rust toolchain is not
  Authenticode-signed, so cargo runs only on a per-file cloud reputation verdict that Windows re-queries and that can
  change (the same symptom is reported in rust-lang/rust#160163: cargo blocked, rustc not); turning SAC off is in
  Windows Security > App & browser control > Smart App Control settings, and on recent builds (Insider 26220.7070;
  reported for 26200.8246) it can be turned on again without a clean install, which older documentation still denies.
  The owner turned Smart App Control OFF on 2026-10-06 (about 14:50; the machine is build 26200.9457, 25H2):
  `VerifiedAndReputablePolicyState` = 0, cargo runs, and the stand-in built in 8.7 s.
  Consequence for the product (D17): an unsigned lazymkv.exe would be blocked on such machines; the installer and the
  executables need code signing (phase 1 design, section 18 risks).

## Installed for phase 0 (winget, approved by the owner on 2026-10-04)

| Package | winget ID | Version |
|---|---|---|
| MakeMKV | `GuinpinSoft.MakeMKV` | 2.0.0 (upgraded from 1.18.3) |
| MKVToolNix | `MoritzBunkus.MKVToolNix` | 100.0.0 |
| FFmpeg | `Gyan.FFmpeg` | 9.0.2 |
| MediaInfo (command line) | `MediaArea.MediaInfo` | 26.05 |
| Tesseract OCR (approved 2026-10-06, for experiment 14: reading bitmap subtitles) | `UB-Mannheim.TesseractOCR` | 5.4.0.20240606 (leptonica 1.84.1; languages `eng`, `osd`; `C:\Program Files\Tesseract-OCR\tesseract.exe`) |

## Optical drives

As MakeMKV lists them in its `DRV` line, up to the firmware revision (the serial number that follows it is left out):

- Drive 1 (`D:` in Tasks 7 to 14): `BD-RE HL-DT-ST BD-RE BU40N 1.03`
- Drive 2 (`D:` when attached in Task 14): `BD-RE PIONEER BD-RW  BDR-XS07U 1.01`
- Attached after Task 14: drive 2, at `D:` (`$Drive` in `C:\lazymkv\phase0\config.ps1`,
  confirmed by the setup check `setup-check-after-drives`).

Per-unit serial numbers are not kept in public documents; the full `DRV` lines stay in the recordings, in the
private evidence repository (`corpus/owner/`).

## Paths

- Staging for phase 0: `\\unas-pro\media\lazymkv-staging`
- The owner's libraries (never written to in phase 0): `\\unas-pro\media\Movies`, `\\unas-pro\media\Shows`
- Job profiles: `C:\lazymkv\phase0\profiles\all-tracks.mmcp.xml`, `eng-only.mmcp.xml`
- Owner settings (not in the repository): `C:\lazymkv\phase0\config.ps1`

## Fingerprint check (Task 8)

The fingerprint and AACS disc ID computed on this machine by `scripts/phase0/record-disc.ps1`
(drive D:), compared with TheDiscDB (`scripts/phase0/hit_rate.py` and the check in Task 8 Step 7).
A dischash hit proves the fingerprint; the content_hash of a discid record is an independent check
of it, and the global_disc_id of a dischash record checks the disc ID.

| Slug | Format | Disc ID | dischash | discid | discid record's content_hash | dischash record's global_disc_id |
|---|---|---|---|---|---|---|
| the-office-2005-s3d3-bluray | bluray | yes | hit | miss | none | DIFFERENT |

Outcome: the fingerprint is proven: TheDiscDB knows the-office-2005-s3d3-bluray (bluray) by it.
The disc ID of the-office-2005-s3d3-bluray differs from TheDiscDB's global_disc_id; reported.

About that difference: our disc ID is the SHA-1 of `AACS/Unit_Key_RO.inf`, and `certutil -hashfile`
on the mini PC gives `meta.json`'s `disc_id` for that file and for its `AACS/DUPLICATE` copy, so the
SHA-1 step has no bug. That does not show that TheDiscDB derives its ID from this file. TheDiscDB's `global_disc_id` for this disc (`CC1C9581...D34F`) is not the SHA-1
of any file under `AACS` or `CERTIFICATE` on this copy. Either TheDiscDB's entry was made from
another pressing with the same file sizes, or it derives the ID differently. One disc cannot tell
which. The Blu-ray and UHD discs of Tasks 9 to 15 may, if the same comparison (Task 8 Step 7's check)
is run on each; until then the "disc ID and fingerprint both match" step of spec 8.1 is unproven.

First disc (the-office-2005-s3d3-bluray): volume label in `meta.json` against `vol D:`: same. Navigation-file
copies in the recording: read-only. Scan job rehearsal (`record-rehearsal`): RESULT ok, 11 titles,
scan took 0 minutes (24 seconds); the second run was refused.

The owner inserted this disc without naming it. It was identified before MakeMKV ran: the fingerprint
lookup at TheDiscDB returned a record for The Office (2005), disc "Season Three Disc Three" of the
release "2020 Complete Series Blu-ray", and the 8-byte header of `BDMV/index.bdmv` (`INDX0200`) shows a Blu-ray, not a UHD, so it
could not be Disc A.
