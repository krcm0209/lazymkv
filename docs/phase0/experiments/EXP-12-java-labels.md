# Experiment 12: Java runtime and main-feature labels

- Date: 2026-10-06 (mini PC clock)
- Disc(s): pans-labyrinth-2006-uhd, UHD, drive 1 (D:)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 12

## Question

On a UHD with Java menus, does the scan report a Java runtime in use (`3344`) and main-feature labels in
attribute 49?

## What was run

`summarize_scan.py corpus/owner/pans-labyrinth-2006-uhd` on the online scan (`scan.txt`); the offline scan
(`scan-offline.txt`) checked for `3344` as well. `docs/phase0/environment.md` notes that Java 8 (1.8.0_471) is
installed and MakeMKV's `app_Java` setting is unset.

## What was observed

- `Java in use: no (MSG:3344 absent)` in the online scan; the offline scan has no `3344` either.
- No title has a comment (attribute 49): the summary's comment column is empty for both titles (`00800.mpls`,
  `00055.mpls`). No `FPL_MainFeature` label anywhere.
- The disc lists only two titles over 120 s and 36 short clips; its menus did not run under Java here.

## Answer

**Not answered by this disc.** Pan's Labyrinth (2019 4K) showed no Java menus to MakeMKV: no `3344` and no labels.
Whether that is because the disc has no BD-J menus or because MakeMKV did not find a Java runtime (`app_Java`
unset) cannot be told from this scan; `3344` is printed only when MakeMKV runs BD-J. The check is repeated on each
later UHD until one answers it (Task 15 Step 6).

## Consequence for the spec

None yet. If no later UHD answers yes, label rule 8.3.2 is unavailable and titles are chosen by database and
structure only (section 17 row 12). On this disc, structure alone identifies the main feature: one title of
1:59:10 against one of 5:40.
