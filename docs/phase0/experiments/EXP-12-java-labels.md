# Experiment 12: Java runtime and main-feature labels

- Date: 2026-10-06 (mini PC clock)
- Disc(s): pans-labyrinth-2006-uhd, UHD, drive 1 (D:)
- MakeMKV version: MakeMKV v2.0.0 win(x64-release)
- Spec reference: section 17, row 12
- Tools: the Python this report names was deleted by the Rust port (pull request krcm0209/lazymkv#2); `summarize_scan.py` is now `lazymkv-dev scan`, with the same arguments and lines. The commands below are as they were run; the scripts as they last were are `git show d3a754d:scripts/phase0/<file>`.

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
- The disc does carry BD-J content: `files.json` lists `BDMV/BDJO/00000.bdjo`, `BDMV/BDJO/00001.bdjo` and
  `BDMV/JAR/00001.jar`. Yet the scan took 19 s (the plan expected ten minutes or more when Java analysis runs),
  printed no `3344` and gave no labels, with Java 8 installed and MakeMKV's `app_Java` unset.
- The disc lists two titles over 120 s and 31 short clips.

## Answer

**Not answered by this disc, and leaning no.** Pan's Labyrinth (2019 4K) has BD-J files, but MakeMKV printed no
`3344` and no labels. Whether MakeMKV did not run the disc's Java (it did not find a runtime with `app_Java`
unset, or chose not to) or ran it and found nothing to label cannot be told from this scan. The check is repeated
on each later UHD until one answers it (Task 15 Step 6); if none does, the answer is no in this configuration,
and setting `app_Java` to the installed Java 8 is the one change worth trying before that.

## Consequence for the spec

None yet. If no later UHD answers yes, label rule 8.3.2 is unavailable and titles are chosen by database and
structure only (section 17 row 12). On this disc, structure alone identifies the main feature: one title of
1:59:10 against one of 5:40.
