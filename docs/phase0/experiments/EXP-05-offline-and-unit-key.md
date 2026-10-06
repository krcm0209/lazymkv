# Experiment 5: MakeMKV offline on a never-seen UHD, and the AACS unit key file

- Date: 2026-10-06 (mini PC clock; 2026-10-05 evening on the Mac)
- Disc(s): pans-labyrinth-2006-uhd, UHD (meta.json format bluray; `BDMV/index.bdmv` header `INDX0300`), drive 1 (D:, HL-DT-ST BD-RE BU40N 1.03) and drive 2 (D:, BD-RE PIONEER BD-RW  BDR-XS07U 1.01 SERIAL)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 5

## Question

With MakeMKV's network access blocked (a temporary firewall rule added by the owner, so SSH stays up), what does it
report for a UHD never scanned on this machine? Run before any other scan of that disc. Can `AACS/Unit_Key_RO.inf`
be read from a mounted UHD in both drives?

## What was run

- The rule, as verified read-only before Disc A went in: one rule `lazymkv-exp05`, Enabled Yes, Direction Out,
  Action Block, Program `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`, Profiles Domain,Private,Public; the
  firewall's Private and Public profiles both `State ON`. After the offline runs the owner removed it; `netsh` then
  answered `No rules match the specified criteria.`
- GUI_ANSWER: no MakeMKV process before or after insertion; on insertion nothing opened (the owner's word, and
  `tasklist` printed no MakeMKV row at any point).
- Disc A was identified before MakeMKV saw it: `Get-InsertedDisc` (lazymkv-capture only), the `INDX0300` header,
  and TheDiscDB by fingerprint ("Pan's Labyrinth (2006), UHD, release 2019 4K"). Its name is in none of MakeMKV's
  cached `MKB20_*.tgz` files. That is consistent with MakeMKV never having opened it here, but it is not proof: the
  two scans of Disc A created no `MKB20_*` file either (see the listings below), so the cache does not record
  every disc. The owner's word that the disc was new to this machine is the real basis.
- Order of the MakeMKV runs and data-folder listings:
  1. listing 0 (before any contact);
  2. Disc A inserted; snapshot and lookups with `record-disc.ps1 -NoScan` (lazymkv-capture and curl, no MakeMKV);
     fingerprint `6F39192C77BB1773D14982AD87B27DF6` found in no other recording;
  3. the drive listing offline (`drives-offline.txt`), then listing 1;
  4. the offline scan job `record-offline` with `-ScanMayFail` (`scan-offline.txt`), then listing 2;
  5. the rule removed by the owner;
  6. the drive listing online (`drives-online.txt`), then listing 3;
  7. the online scan job `record-online` (`scan.txt`), then listing 4.
- `Assert-DiscIs pans-labyrinth-2006-uhd` checked the disc's fingerprint before each of the four MakeMKV runs.
- Every MakeMKV call ran with `--debug` (decision D16); each run's debug log is beside its output.
- Rehearsal on the-office-2005-s3d3-bluray's disc (Task 9 Step 2). PROBE_ANSWER: on a disc MakeMKV had already
  scanned on this machine, the drive listing printed the banner, the DRV lines and `MSG:5010` "Failed to open disc"
  after the DRV lines (the normal answer to `disc:9999`), and changed nothing in MakeMKV's data folders.

## What was observed

Runs (from `.exit.json`): none was ended early.

| Run | exit code | elapsed | lines |
|---|---|---|---|
| drives-offline.txt | 0 | 6.1 s | 21 |
| scan-offline.txt | 0 | 16.3 s | 278 |
| drives-online.txt | 0 | 7.0 s | 21 |
| scan.txt (online) | 0 | 18.9 s | 275 |

The blocked network made MakeMKV wait for nothing: the offline scan was 2.6 s faster than the online one.

Titles: 2 offline, 2 online. `compare_scans.py scan-offline.txt scan.txt`: `titles: A=2 B=2 common=2`,
`only in A: 0`, `only in B: 0`, `comment differs: 0`, exit 0. Title 0 is `00800.mpls` (1:59:10, 22 chapters,
51.7 GB, HEVC Main10 2160p; Spanish DTS-HD MA 5.1 with its DTS core, English DD 2.0, English and Spanish PGS each
with a forced-only twin); title 1 is `00055.mpls` (0:05:40, 34 segments, no audio). 31 titles under 120 s were
skipped (`MSG:3025`), the same ones in both scans.

`ONLY HERE` messages (1003 DEBUG lines excluded):
- `drives-offline.txt`: none. Its only disc-position message, `5010 Failed to open disc`, is printed by the
  online drive listing too.
- `scan-offline.txt`: none.
- Online only: `scan.txt` has `3338 "Downloading latest HK to C:\Users\USER/.MakeMKV ..."` (disc position), with
  no failure message after it. Neither offline run printed a `3338` line.

The drive listing did not open Disc A: after its `DRV` lines it printed only `5010 Failed to open disc` (the
answer to `disc:9999`), and listing 0 to listing 1 shows no difference. This matches PROBE_ANSWER.

Labels: `meta.json` `volume_label` `PANS_LABYRINTH` equals the label in the `D:` DRV line of `drives-offline.txt`.

MakeMKV's data folders: DATA_FOLDERS is `C:\Users\USER\.MakeMKV` (10 files); `app_DataDir` is not set; the three
other candidate folders are absent. Before MakeMKV first saw Disc A the folder held `keydb.cfg` (63.8 MB),
`_private_data.tar`, a firmware dump and seven `MKB20_*.tgz` files from UHDs opened earlier. The four diffs:
listing 0 to 1 none; 1 to 2 none (the offline scan wrote and changed nothing); 2 to 3 none; 3 to 4 one change,
`_private_data.tar` grew from 15055872 to 15074816 bytes at the online scan's time, matching the `3338` download.
No file written offline was read online, so the online scan is a clean baseline. `keydb.cfg`, the cached
`_private_data.tar` (dated 2026-09-10, the file the online HK download then rewrote) and LibreDrive were all
present for the offline runs; which of them let the disc open offline is not isolated by this experiment.

Neither scan printed `MSG:1009`.

The debug logs add: both scans say `Network access is ENABLED` (printed in the blocked run too, so it is not the
firewall's state), both show `Using LibreDrive mode` and `DISCID=C337194670AF8C954C320875D671653F7EF2C256`, and
apart from timestamps the two logs differ only in the online `3338` HK download line, which the robot output
also carries. MakeMKV's `DISCID` equals our `disc_id`.

Disc ID: `disc_id` is 40 hex characters (`C337194670AF8C954C320875D671653F7EF2C256`): `AACS/Unit_Key_RO.inf`
(65536 bytes, and its `AACS/DUPLICATE` copy) was readable in drive 1. TheDiscDB answered 404 for this ID while
knowing the disc by fingerprint, and its record's `global_disc_id` (`B5F0E23E...`) differs from ours: the same
DIFFERENT outcome as the first disc (Task 8's check), now on a UHD.

Second drive (Task 14): Disc A in drive 2 (`D:`, `BD-RE PIONEER BD-RW  BDR-XS07U 1.01 SERIAL`), snapshot in
`corpus/owner/pans-labyrinth-2006-uhd/drive2/snapshot/meta.json`, made by `scripts/phase0/second-drive.ps1`.
Fingerprint equal to drive 1's: yes. Disc ID in drive 2: read (40 hex characters). Equal to drive 1's: yes.

## Answer

Rule 3 applies: **no distinct message was seen; the disc opened offline.** There was no `ONLY HERE` message in
either offline run, no offline `3338`, no `start`-position difference either, and both scans listed the same two
titles. The only difference is on the online side: `3338 "Downloading latest HK ..."` and a larger
`_private_data.tar`. On this machine a never-seen UHD opened offline and listed the same titles as online; the
only difference was the online download.

`AACS/Unit_Key_RO.inf` was readable in both drives: in drive 1 (disc ID computed, and equal to MakeMKV's own
`DISCID`) and in drive 2 (Task 14: the file and its `AACS/DUPLICATE` copy listed, 65536 bytes each, and the same disc
ID as drive 1 computed from it).

Not answered by this disc: what MakeMKV prints when a disc needs a key it does not have and cannot download. That
needs a UHD whose keys are not in `keydb.cfg`; none of the owner's discs so far has shown it.

## Consequence for the spec

- Section 10.2: `NeedsNetwork` stays `Unknown`, as the fallback says; no message marks the offline case on this
  machine. The absence of `3338` is not usable as a trigger (an online scan of a known disc also lacks it).
- Section 9.4: `3338` ("Downloading latest %1 to %2") is a normal online message, not a failure.
- Section 10.4: the scan watchdog (`scan_timeout_min`) remains the only guard for an offline disc that would hang.
- Section 8.1: the disc ID is MakeMKV's `DISCID` (confirmed by the debug log); TheDiscDB's `/api/discid/` missed it
  for the second disc in a row while `/api/dischash/` hit. The fingerprint is the primary key.
- Row 5, second question: `AACS/Unit_Key_RO.inf` was readable in both drives, so row 5's fallback (lookups on a
  drive without the file use the fingerprint only, identity 0.95 at best from TheDiscDB) is not needed on either.
- The debug logs (decision D16) confirm the robot output rather than add to it here: their one difference is the
  same `3338` line. They do add `DISCID`, which settled the disc-ID question of Task 8.
