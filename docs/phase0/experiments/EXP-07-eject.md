# Experiment 7: does programmatic eject work on both drives?

- Date: 2026-10-06
- Disc(s): pans-labyrinth-2006-uhd, UHD, in drive 1 (D:) and drive 2 (D:)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release) (the `MSG:1005` line of `drive2/drives.txt` and of
  `backup/backup.txt`; the eject itself does not use MakeMKV)
- Spec reference: section 17, row 7

## Question

Does programmatic eject work on both drives?

## What was run

- `lazymkv-capture eject --drive <letter>` over ssh (`C:\lazymkv\target\release\lazymkv-capture.exe`, in an
  elevated session, as opening `\\.\D:` for writing and locking the volume require), once per drive. It runs the
  four steps of spec 4.3's `Ejector` (lock, dismount, allow removal, eject), reports each as `ok` or `error N`, and
  exits 0 when the eject step succeeded.
- Before each eject, `tasklist | findstr /i makemkv` checked that no MakeMKV program was running.
- Drive 1 (Task 14 Step 8): `eject --drive D`, with Disc A in it.
- Drive 2 (Task 14 Step 9): the owner moved Disc A from drive 1 to drive 2, unplugging drive 1 first, so Windows
  gave drive 2 the letter `D:` too. `second-drive.ps1 -Slug pans-labyrinth-2006-uhd -Letter D` confirmed the disc by
  its fingerprint (`RESULT ok`; EXP-05), then `eject --drive D`.
- The owner watched each drive and was asked whether its tray opened.

## What was observed

Drive 1:
- Name as MakeMKV lists it, up to the firmware revision: `BD-RE HL-DT-ST BD-RE BU40N 1.03` (from
  `docs/phase0/environment.md`), letter `D:`, a tray-loading drive.
- `tasklist | findstr /i makemkv` printed nothing.
- EJECT1: `lock=ok, dismount=ok, allow-removal=ok, eject=ok`, `exit=0`.
- TRAY1: the owner: "The tray opened."

Drive 2:
- Name as MakeMKV lists it, up to the firmware revision: `BD-RE PIONEER BD-RW  BDR-XS07U 1.01` (two spaces after
  `BD-RW`, as MakeMKV prints it), letter `D:`, MakeMKV index 0. Its recorded `DRV` line, in
  `corpus/owner/pans-labyrinth-2006-uhd/drive2/drives.txt`, with the serial number replaced by `SERIAL`:
  `DRV:0,2,999,12,"BD-RE PIONEER BD-RW  BDR-XS07U 1.01 SERIAL","PANS_LABYRINTH","D:"`.
- It is a slot-loading drive: it has no tray.
- `tasklist | findstr /i makemkv` printed nothing.
- EJECT2: `lock=ok, dismount=ok, allow-removal=ok, eject=ok`, `exit=0`.
- TRAY2: the owner: "Drive 2 doesn't use a tray, it's slot-fed, but the disc was pushed out, yes."

No step reported `error N` on either drive. Both drives were run; none was left out.

## Answer

**Yes:** on both drives the eject exited 0 with all four steps `ok`, and the disc came out: drive 1's tray
opened, and drive 2, a slot-loading drive with no tray, pushed the disc out of its slot. For drive 2, "the tray
opened" is read as "the disc was pushed out", the slot-loader's form of the same eject.

## Consequence for the spec

- Spec 4.3 (`Ejector`): none. Lock, dismount, allow removal and eject all reported `ok` on both drives; no step
  failed while the eject still worked. The `eject_failed` notification (row 7's fallback) stays the answer to a
  failed eject only.
- Wording: the drive kept attached after Task 14 (drive 2) is slot-loading. Section 10.3 ("An open tray therefore
  always means 'insert the next disc'"), D3 ("eject the tray") and `disc_done` ("the tray opened") hold for it with
  "tray opened" read as "disc pushed out"; owner-facing texts should say the disc was ejected, which is true for
  both kinds of drive. Not observed: what drive 2 does when an ejected disc is left in the slot; 10.3's open-tray
  signal is untested on a slot-loading drive.
