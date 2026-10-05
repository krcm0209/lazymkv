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

## Installed for phase 0 (winget, approved by the owner on 2026-10-04)

| Package | winget ID | Version |
|---|---|---|
| MakeMKV | `GuinpinSoft.MakeMKV` | 2.0.0 (upgraded from 1.18.3) |
| MKVToolNix | `MoritzBunkus.MKVToolNix` | 100.0.0 |
| FFmpeg | `Gyan.FFmpeg` | 9.0.2 |
| MediaInfo (command line) | `MediaArea.MediaInfo` | 26.05 |

## Optical drives

As MakeMKV lists them in its `DRV` line:

- Drive 1 (attached, `D:`): `BD-RE HL-DT-ST BD-RE BU40N 1.03 SERIAL`
- Drive 2: not attached when this was recorded. Its `DRV` line is added in Task 14, when the
  owner swaps drives.

## Paths

- Staging for phase 0: `\\unas-pro\media\lazymkv-staging`
- The owner's libraries (never written to in phase 0): `\\unas-pro\media\Movies`, `\\unas-pro\media\Shows`
- Job profiles: `C:\lazymkv\phase0\profiles\all-tracks.mmcp.xml`, `eng-only.mmcp.xml`
- Owner settings (not in the repository): `C:\lazymkv\phase0\config.ps1`
