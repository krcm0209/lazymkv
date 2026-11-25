# The menu crawler: lazymkv reads a disc's menu itself: design

Date: 2026-11-24.
Status: the owner corrected the direction of the menu work on 2026-11-24 and answered five questions in the design
conversation that followed (CO-1 to CO-5, part 3). The rest is the designer's, listed in part 15. Nothing is built.
Only step 1, a trial on the Mac, is to be planned from this document; every later step has its own plan and waits for
the owner at the stop before it.

"The spec" is `docs/superpowers/specs/2026-10-04-lazymkv-design.md`; "the menu maps design" is
`docs/superpowers/specs/2026-11-17-menu-maps-design.md`; "the engine design" is
`docs/superpowers/specs/2026-11-24-engine-menu-maps-design.md`; "the format" is `docs/menu-maps/format.md`; "the
phase 3 design" is `docs/superpowers/specs/2026-11-24-phase-3-design.md`. "The spike" is the menu work's own tooling
and ledgers, which are not on `main`: the tools are under `experiments/menu-spike/menu/` on the local branch
`claude/lazymkv-disc-menu-cv-1228ba` (their `README.md` describes them as built), and the captures, maps and session
folders are private data outside the repository. CO-1 to CO-5 are the owner's answers and CM-1 to CM-21 the
designer's calls, both recorded here.

## 1. Intent

The menu work exists so that a person who puts any disc into lazymkv gets what the disc's own menu says about its
tracks, as a label or as evidence beside the content analysis. The owner said so again on 2026-11-24:

> "The idea with a menu interpreter was so that any arbitrary user of lazymkv could pop in their disc and lazymkv could
> attempt to use the disc's menu to label tracks and/or have extra evidence to support track labeling. It's not
> feasible to expect every possible disc to have a pregenerated map. In fact, it's more than likely a user will have no
> pregenerated maps."

Today a map is made ahead of time, by AI agents on a Mac, for one disc at a time, and 13 discs have one. The engine
design answered how a job uses a map that exists, and would have had the engine look one up in a folder. That serves
only discs somebody mapped before. This design is the missing half: lazymkv makes the map itself, during the job, with
no agent, no person and no network. It calls the part that does so the crawler.

Whether plain code can explore a menu nobody has seen is not known. So the design is a road of four steps with a stop
after each, and its first step is a trial that answers that question before anything is built into the engine.

## 2. What exists today

The mechanical half of map-making works, on the Mac only (the spike):

- **A driver** runs the disc's Java menu on libbluray 1.5.0 and Java 17, from the disc's small files alone. It serves
  every video clip as blank packets, so no video is read or decrypted. It runs a script of key presses and logs each
  write of the audio and subtitle registers with its writer, each playlist start, and after every key a hash of the
  picture and the rectangle that changed.
- **The effect of a choice is read from that log, never from a picture:** the playlist, the audio stream and the
  subtitle stream in the last state the driver logs while the film's clock runs. The scripts let the film run three
  seconds first; that is a habit of the scripts and not a check of the log reader, so the crawler must keep it
  itself.
- **A replayer** runs a finished map's scripts three times with no agent and compares the effects.
- **A sandbox** (macOS `sandbox-exec`) holds the session: no network, writes to one folder, reads of the disc copy and
  the runtime only, no child process.

The exploring half is done by agents, and no code does it:

- An agent writes every key script, gets past warnings and language choosers, finds the sections, tells which item is
  highlighted, reads each label by sight, and decides when it is done. A second agent re-reads every label, and a
  third settles disputes. This took about 0.55 million tokens for each disc.
- The spike's ledger records two findings a crawler must meet: no rule picks the right frame from a script
  mechanically, and the rectangle that changed is not always where the label is.
- lazymkv's own text recognition (the `lazymkv-ocr` crate) has never been tried on a menu label.

What the measurements say:

- A session takes 30 to 45 seconds, and the menu settles 4 to 19 seconds after start. Agents needed 18 to 31
  sessions for each disc. Replays were repeatable to the pixel on the one Mac.
- Of 327 choices in the 13 maps, 304 were confirmed by three replays. Of the 23 that were not, 11 are buttons whose
  labels are drawn in the video, which is blank in a session.
- One disc loses key presses when sessions run side by side, and none when they run one at a time.
- Of 40 captured discs 31 have a Java menu, 7 are Blu-ray without Java and 2 are DVDs. The 13 mapped discs are all
  Java, from 7 of the 10 Java file families the captures hold.
- The files a Java menu needs are 0.5 to 130 MB for each disc, about 15 MB at the median. One disc holds 2.1 GB of
  padding in its Java folder.

What the engine has and lacks:

- A job copies the disc's index, movie object, playlists and metadata into `nav\` while the disc is in the drive, in
  the state `Detected`, before MakeMKV opens the disc (`core::snapshot::is_nav_file`, `tools::snapshot`). It does not
  copy the Java programs, the clip information or the certificates.
- The Decide step runs after the eject. The disc is not in the drive then.
- Every helper process runs in its own Windows Job Object with a time cap and, for the OCR worker, a memory cap. No
  helper has a restricted token or a file-system limit (`platform::Spawn`).
- lazymkv ships no Java runtime (ARCH-7). MakeMKV uses the one the user installed.
- Nothing on Windows runs a disc's menu. The driver, the session runner and the sandbox are for macOS. The spike's
  feasibility report proposes an AppContainer, a network block and a Job Object, and says a Java runtime inside an
  AppContainer "rests on one anecdote".

## 3. The owner's answers of 2026-11-24 (CO-1 to CO-5)

Asked "Design the rip-time map-maker next and park the share lookup?", the owner answered "Yes, design that". Then:

- **CO-1.** Asked "When lazymkv reads a disc's menu during a job, who does the exploring (finding the audio section,
  seeing which item is highlighted, reading the labels)? Today an AI agent does it, at about 0.55 million tokens per
  disc; no plain code does it yet.", the owner chose "Plain code only (Recommended)" over "Code first, AI as fallback"
  and "AI model explores".
- **CO-2.** Asked "Which way should the plain-code menu explorer be built?", the owner chose "A: effect-first crawler
  (Recommended)" over "B: style profiles" and "A first, B where A fails".
- **CO-3.** Asked "The first step is a trial of the crawler on your 13 mapped discs, scored against the agent-made
  maps. What result is good enough to go on to the Windows step? (You get the full numbers either way.)", the owner
  chose "Never wrong, right on 7 of 13 (Recommended)" over "Never wrong, right on 10 of 13" and "Never wrong, any
  coverage".
- **CO-4.** Shown the summary this document is written from and asked "Does that summary of the rip-time menu reader
  match what you want, so I can write it up as the design document?", the owner chose "Yes, write it up
  (Recommended)".
- **CO-5.** After a reviewer read this document as a builder would, asked "You chose "never wrong, right on 7 of 13"
  for the trial. Two of the 13 mapped discs have no commentary or audio description at all, so the crawler cannot be
  "right" on them in any real sense; 11 discs have at least one. How should the seven be counted?", the owner chose
  "7 of the 11 (Recommended)" over "6 of the 11" and "7 of 13, empty discs count".

## 4. Goals, non-goals and anti-goals

Goals
1. For a disc nobody has mapped, lazymkv makes a menu map during the job, with no person, no agent and no network.
2. The crawler writes the map format as it is (`lazymkv menu map v1`), so the reader, the rule and `decide --menu`
   work on its maps unchanged.
3. Every claim in a crawler's map was seen: the stream from a register, the label from a picture, both again in
   replays. What was not confirmed is recorded as unconfirmed and gives no role.
4. A disc whose menu the crawler cannot read decides exactly as it does today, and the job says why in one line.
5. Each step ends with numbers and a stop at which the owner decides.

Non-goals (each has a later home in part 16)
- A language model at rip time, in any role (CO-1).
- Blu-ray discs without a Java menu, and DVDs.
- A navigator for one menu style (the menu maps design, 5.5). The crawler assumes no style.
- Any change to the rule, and any new role. The crawler only supplies maps.
- Sharing maps between users, and a map made ahead of time as a routine step.

Anti-goals (these would make it a failure even if it works)
- A wrong role in a map. A crawler that reads fewer menus and is never wrong is the aim; one that reads more and is
  sometimes wrong is a failure (CO-3).
- A disc's Java running outside a sandbox, anywhere, ever (the menu maps design's anti-goal stands).
- A job that fails, is held, or runs past its time cap because of the crawler.
- A crawler tuned to the 13 mapped discs and passed off as general.
- The owner typing, photographing or checking anything by hand as a routine step.
- A disc title, a person's name or a map's text in the public repository. Every example in this document is made up.

## 5. Constraints

- The engine runs on Windows. The trial runs on the Mac, where the driver and the sandbox are.
- Nothing is built on, written to or changed on the mini PC without the owner's explicit yes.
- No new crate without the owner's yes. lazymkv ships no Java runtime (ARCH-7).
- One renderer: the engine and `lazymkv-dev decide` print a decision with the same function (R-B1), so a job must be
  replayable from its own folder. The map a job decided with is kept in that folder.
- The owner's captures, maps and recordings are private data, read only, and never copied into the repository.

## 6. Approaches considered

Who explores (CO-1):

| Approach | What it is | Trade-off |
|---|---|---|
| **Plain code (chosen)** | A program in lazymkv explores and reads | Works offline for every user at no cost for each disc. Unproven. |
| Code first, a model as fallback | A model explores where code cannot | Each user needs a key and a network; pictures of the menu leave the machine. |
| A model explores | Today's agent method inside the job | Proven on 13 discs. No menu is read without a key, and every disc costs tokens. |

How plain code explores (CO-2):

| Approach | What it is | Trade-off |
|---|---|---|
| **A. Effect-first crawler (chosen)** | Walk the menu; an item that changes the audio or subtitle register is a choice; read only those labels | Assumes no style, so it can meet an unseen disc. Unproven; some menus will give a partial map or none. |
| B. Style profiles | One navigator for each menu family, picked by file fingerprint | Reliable where a profile exists. 40 captures already fall into 12 file families with different layouts inside a family; a stranger's disc often has none. |
| C. Read the menu's files without running them | Parse the authoring files; no Java, no sandbox | Measured in the spike's Stage A: a likely role on roughly a sixth to a third of the weighted discs, and one family keeps its labels in compiled code. Dropped. |

B stays possible later for a family the crawler cannot handle; the boundary in part 7.1 is the one the menu maps
design drew for it.

## 7. The crawler

### 7.1 The boundary

One contract, the menu maps design's: given a disc's menu files, return a map or say plainly that it could not. The
rest of lazymkv sees only maps. Behind the contract the crawler is one more way of steering the sessions the driver
already runs; the agents were another.

### 7.2 What it does

1. **Start.** Run a session from the disc's first-play title and wait until the picture settles. A warning, a logo
   and a language chooser are screens like any other; nothing is special-cased by name.
2. **Walk.** From a settled screen press each navigation key and ENTER, one at a time. After each key the driver's log
   says which of four things happened: nothing; a small area changed (the highlight moved); most of the picture
   changed (another screen); a playlist started and its clock runs (a film or a clip started). Screens are told apart
   by their settled picture. A menu keeps state, so a screen is reached again by replaying its key path from the
   start.
3. **Find the choices by effect.** An item is an audio or subtitle choice when choosing it changes the audio or
   subtitle stream. Two cases cover the menus seen so far. A choice that applies at once writes the register when
   ENTER is pressed; the log names the writer as the disc's Java. A choice that applies at Play shows only when the
   film is started afterwards, so the crawler chooses the item, starts the film by the path it found for plain Play,
   and reads the effect as the spike's rule does. The stream always comes from the log.
4. **Read the labels of those items only.** The label area is found from pictures of the same screen with the
   highlight on this item and on another item. lazymkv's own text recognition reads it. A role (commentary, audio
   description) comes from the label's words by the rules `core::menu::role` already has.
5. **Confirm on the format's ladder.** A choice enters the map at level `read`. It becomes `checked` when a second
   session, run from the start, reaches the same item and gives the same effect, and a second reading of the label
   from another picture agrees with the first (7.3). It becomes `replayed` when three more replays agree. The format
   gives no role to a choice below `replayed`, so an unconfirmed reading can drop nothing.
6. **Stop.** When no reached screen has an untried key, or when the time cap is reached. What is confirmed by then is
   the map. A map that covers part of a menu says so.

### 7.3 Rules that keep it from being wrong

- **A claim needs agreement, not a threshold.** A label counts when two readings of it are the same after
  normalising case, spacing and accents (the menu maps design, 5.5). The two readings must be of different pictures
  of the label: the item highlighted, and the same item with the highlight elsewhere. A replay repeats to the pixel,
  so reading the same frame twice proves nothing. A label with one readable picture, or two readings that differ,
  stays at `read`.
- **One highlight.** When two areas of a screen both look highlighted, or the changed area holds no text, the item's
  label is not claimed. It is recorded as unconfirmed with the reason.
- **No text, no role.** An icon, a tick box or a label drawn in the video (blank in a session) gives a choice without
  a label, which has no role.
- **An "off" wording needs a person's decision** before it gives a role (the format, "The off-decisions file"). A
  crawler's map has none, so such a choice gives no role. This rule exists already and is not changed.
- **A stalled start is not an effect.** The spike's guards stay: no effect is read when the film's clock does not
  run, or when a wait for a playlist timed out.
- **The rule's own guards are untouched:** the programme reference is never dropped, the stream plain Play selects is
  never confirmed, and a Keep override wins.

### 7.4 Time and timing

- Waits are for the picture to settle or for an event, with a longest wait, and not fixed gaps between keys: a
  slower machine must not turn a script that was right into one that is wrong.
- Sessions run one at a time unless a measurement on the machine shows that side by side is safe there. The key
  presses the spike lost were lost when sessions ran side by side, and never when they ran one at a time.
- A crawl has a time cap. The trial measures how long discs take; the cap's value is set from those numbers in the
  step that builds the engine's part.

### 7.5 The player the crawler pretends to be

A menu can behave differently for another region or language. The spike only ever ran region A, English. The crawler
takes the menu language from the first of the user's wanted languages and records the profile in the map, as maps do
now. The trial runs the profile the agent-made maps were made with.

## 8. Step 1: the trial

A trial of the crawler on the Mac, on the spike's driver, session runner, log reader, replayer and sandbox as they
are. It touches neither the engine nor the mini PC, and it is the only step planned from this document.

- **No model while a crawl runs.** Agents may write the crawler; a crawl makes no model call.
- **lazymkv's own text recognition.** Labels are read with the engine and models lazymkv ships, not with another
  reader, because that is what a user will have.
- **A disc's Java runs only in the existing sandbox.**
- **Where the code lives.** The spike's branch has the driver and the session tools but none of `main`'s text
  recognition, role rules or `decide --menu`; `main` has those and none of the spike's tools. The trial works on a
  local branch that holds both: the spike's branch with `main` merged in. The crawler is written beside the spike's
  tools, in their language. It is a trial: what carries over is the method and the numbers. The engine's crawler is
  written for the engine in step 4.
- **Reading a picture.** Nothing shipped reads text from an arbitrary picture: `lazymkv-ocr` takes a subtitle stream
  and its reader takes a grey image. The trial adds one small developer command that reads the text in a PNG with
  the `lazymkv_ocr` library. Cutting the label out and turning colour into grey are written for the trial and are
  part of what it measures. No dependency is added and no engine file is touched (CM-17).

### 8.1 The discs

- **The 13 mapped discs** are the answer key. Eleven of them have a commentary or an audio description to find and
  two have none (8.2). Five of the eleven are held back: until the final score the crawler is not run on
  them, and nothing of theirs is read for it by a person or an agent. That covers their maps, label pictures, key
  scripts, session folders, replay records, fixtures and the ledger's notes about them; every brief for an agent that
  writes the crawler names the five and forbids them. Which five is fixed before any crawler code is written, by a
  rule and not by choice: of the eleven, the five whose slug has the lowest SHA-256. The other six and the two with
  nothing to find are the discs the crawler is developed on.
- **The 18 Java captures without a map** are already on the Mac. They are run once at the end, for counts only: did
  the crawl finish, how many choices it found and confirmed, how long it took. Nothing there can be scored. The one
  padded with gigabytes may be left out.

### 8.2 The score (CO-3, CO-5)

The measure is what the rule would do with a map. For each disc `lazymkv-dev decide --menu` is run on the disc's
recording twice, once with the agent-made map and once with the crawler's, both without an off-decisions file, since a
crawler's map never has one. The key is the agent-made map's confirmations: each audio stream the menu confirms as a
commentary or an audio description, with its role. Without off-decisions the 13 maps give 27 confirmations on 11
discs; the other two discs have none.

- **Wrong:** a confirmation from the crawler's map that the key does not hold: a role on a stream the key gives no
  role, or another role than the key's. A confirmation from a choice the agent-made map does not hold at all counts
  as wrong unless it is shown to be real: a blind agent reads the label picture, as the spike's second reader did,
  and the playlist's stream table has the stream. Each such case is listed in the report with its picture.
- **Missed:** a confirmation of the key that the crawler's map does not give. A label misread so that its role word
  is lost is a miss. It is not wrong: in a job it would mean no drop.
- **Right on a disc:** the crawler's map gives every confirmation of the key, with the same stream and the same role,
  and nothing on the disc is wrong. Only the 11 discs with a confirmation can be right.

The trial passes when all three hold:

1. No disc of the 13 has a wrong confirmation.
2. At least 7 of the 11 discs are right (CO-5).
3. At least 2 of the 5 held-back discs are right (CM-19).

The report gives the held-back five apart from the others. It also gives, for every disc: sessions, wall time, audio
and subtitle choices found and confirmed against the agent-made map's, labels read and how many equal the agent-read
label after normalising, and each failure with its reason.

### 8.3 The stop

The owner gets the report whatever it says. When the trial does not pass, nothing further is built: the report says
which menus failed and why, and the owner chooses between a profile for the failing families, another approach, and
stopping.

## 9. Step 2: the runtime and the sandbox on Windows

Its own plan, written after step 1 passes, and built on the mini PC only with the owner's yes. In outline:

- libbluray with its Java part, the driver and the session runner, built for Windows.
- A containment equal to the Mac's: no network, writes to one folder, reads of the disc copy and the runtime only, no
  child process, a time cap, a memory cap. `platform::Spawn` has no restricted token or file-system limit today, so
  this is new platform code.
- The 13 maps replayed on the mini PC: the effects must equal the Mac's. Timing is measured on its processor, which is
  much slower than the Mac's.
- Which Java: the one the user installed for MakeMKV (CM-6). The lowest version that is safe is decided here.

The stop: when no containment on Windows meets that list, nothing ships on Windows, and the owner is told what failed.

## 10. Step 3: the job keeps the menu's files

Its own plan. The job copies what a Java menu needs while the disc is in the drive, in the state `Detected`, where it
copies `nav\` today and before MakeMKV opens the disc: `BDMV\JAR`, `BDMV\BDJO`, `BDMV\CLIPINF`, `BDMV\AUXDATA` and
`CERTIFICATE`. Nothing from `AACS` and nothing from `BDMV\STREAM` is copied; the clips' sizes are already in
`files.json`.

- A size cap, so that a disc padded with gigabytes costs nothing: over the cap no menu file is kept and a note says
  so.
- The files go beside `nav\`, in a folder of their own, so that a recording and the evidence corpus stay as they are
  (CM-8).
- A disc without a Java menu keeps nothing more than today.

## 11. Step 4: the engine makes the map and Decide reads it

Its own plan. In outline:

- A new step in the file phase, before Decide. The disc is gone by then; the crawler works from the files step 3
  kept.
- The crawler runs as a helper process inside step 2's containment, below normal priority, under a time cap.
- Its map is written into the job folder. Decide reads it with the shared reader (`read_menu_pair`, `menu_input`)
  that pull request #42 adds, and `decide.json` gains the `menu` block.
- No map, a refused map and a crawler that failed or ran out of time all decide as today, with one line in `job.log`
  and one note on the job.
- A setting switches menu reading off. It stays off by default until the owner accepts it on real discs.
- A replay of the job reads the job's own map and does not crawl again.

From the engine design, parts 10.3 to 15 carry over where they do not speak of a share (the code path, what
`decide.json` gains, what a job shows, the rule changes, undo, trust). Its parts 7, 8, 10.1 and 10.2 (the setting for
a maps folder, publishing, the lookup on a share) are parked: they serve a map made ahead of time, which is no longer
the routine way.

## 12. How far a crawler's map is trusted

The owner ruled that the menu's word alone drops a track it confirms. That ruling was made for maps whose labels
agents read and re-read. Whether the same holds for a map a program read is not decided here. It is put to the owner
after step 1, with the trial's numbers: the same trust; evidence only, where the menu and the content analysis must
agree for a drop and a disagreement holds the job; or notes only. Until then a crawler's map is used only in the
trial's measurement. A crawler's map names its maker, so the rule can tell the two kinds apart.

What a wrong claim could cost, under the guards that stay: one track that is neither the programme reference nor the
stream plain Play selects is dropped from one job, and the job's note names it and the menu's words.

A disc's own code produces the log a map is made from. A disc that escaped the Java runtime inside the sandbox could
write a false log, so a map is evidence about a disc only as far as that disc is trusted. The worst a false map does
is the paragraph above; the worst an escape from the sandbox does is step 2's subject.

## 13. What stays and what is parked

- **Pull request #42 stays.** The off-decisions' binding, the rule's notes and reference rules and `menu off` act on
  a map however it was made. `menu publish` stays as a developer command.
- **The engine design's lookup is parked,** as part 11 says. The engine design's status line says so from this
  document's commit on.
- **The 13 agent-made maps** are the trial's answer key and are not published anywhere.

## 14. Risks

- **Finding the highlighted item** is the part code has never done. Icon menus light a third of a percent of the
  frame; two things can look highlighted; the changed area is not always the label.
- **Reading stylised labels** with lazymkv's text recognition has never been measured.
- **A choice equal to plain Play shows no effect,** so the crawler cannot prove the default audio entry is a choice.
  The rule needs no role on that stream, so this costs coverage of the map, not safety.
- **Combinations and pop-up menus** are not explored by the agents either; the crawler's map is no more complete than
  theirs.
- **Thirteen discs are one shelf, not a sample.** A pass says the crawler works on these menus. The 18 other captures
  give only counts.
- **Timing on a slow processor** is unmeasured, and one disc already loses keys under load.
- **The sandbox on Windows** is unproven, and a safe Java version is not chosen.
- **A map is made on one machine.** Pictures differ between systems even where effects agree, so a map is replayed
  where it was made.

## 15. The designer's calls, for the owner to check

Each with what it costs if wrong.

- **CM-1.** The first version covers Java menus only. Cost: 9 of the 40 captured discs (7 Blu-ray, 2 DVD) get no map
  until their own design.
- **CM-2.** The crawler writes format v1 and climbs its ladder: a second session is the second reader, three replays
  make `replayed`. Cost: none known; a format change would be its own step.
- **CM-3.** A label counts only when two different pictures of it, highlighted and not, read alike. Cost: a label
  with one readable picture, or one the reader gets right once and wrong once, is lost, and its track is not
  dropped.
- **CM-4.** Five of the 11 discs with something to find are held back, picked by the lowest SHA-256 of the slug.
  Cost: the crawler is developed on eight discs, two of them with nothing to find, and the five may all be of one
  family by chance; the report says which families each group holds.
- **CM-5.** A confirmation from a choice the agent-made map lacks counts as wrong unless a blind agent reading of
  its picture and the stream table show it is real. Cost: the pass can rest on an agent's reading of a case the
  owner did not see; each is listed with its picture.
- **CM-6.** The engine uses the Java the user installed for MakeMKV, and without one reads no menu. Cost: a user
  without Java gets no menu reading; shipping a runtime would be the owner's decision against ARCH-7.
- **CM-7.** The trial is written in the spike's language beside its tools and is not the engine's crawler. Cost: the
  method is written twice.
- **CM-8.** The menu's files go in a folder of their own beside `nav\`. Cost: the crawler's helper has to put two
  folders together into one disc tree.
- **CM-9.** The crawl runs after the eject, in the file phase, from kept files. Cost: the job is longer by the crawl,
  up to the cap; running it during the rip would save that time and compete with the rip for a slow processor.
- **CM-10.** Menu reading is a setting that is off until the owner accepts it on real discs. Cost: nobody gets it
  before then.
- **CM-11.** How far a crawler's map is trusted waits for the trial's numbers (part 12). Cost: step 4 cannot be
  planned to its end before that answer.
- **CM-12.** The crawler takes the menu language from the user's first wanted language. Cost: a menu that differs by
  language may be read in a language the role words do not cover, and then gives no role.
- **CM-13.** The engine design's lookup is parked, not withdrawn, and `menu publish` stays. Cost: a developer command
  and four parts of a merged design that nothing uses for now.
- **CM-14.** Only step 1 is planned now. Cost: the later steps' outlines here may change when their plans are
  written.
- **CM-15.** The 18 unmapped captures are run for counts and are not scored. Cost: the trial cannot say whether the
  crawler is right on them; scoring them would need agent-made maps of them first.
- **CM-16.** A failed trial stops the work and goes to the owner; no profile and no model fallback is built without a
  new answer. Cost: none; it is the stop the owner chose.
- **CM-17.** The trial works on a local branch that is the spike's with `main` merged in, and adds one developer
  command that reads the text in a picture with the `lazymkv_ocr` library. Cost: a branch that is never merged as it
  is, and a command the engine's crawler will replace.
- **CM-18.** The score is taken without off-decisions on both maps. Cost: the three confirmations that rest on a
  person's decision are not in the key, so the trial says nothing about them; a user's job would not have them
  either.
- **CM-19.** At least 2 of the 5 held-back discs must be right. Cost: a crawler that generalises poorly can pass
  with 2 of 5, and a good one can fail on an unlucky five; the report shows both groups.
- **CM-20.** A missed role is a miss and not a wrong claim. Cost: none for safety; a crawler whose text recognition
  often loses the role word fails on coverage instead.
- **CM-21.** The comparison is of what `decide --menu` confirms, on the recordings the earlier acceptance used.
  Cost: a difference between the two maps that changes no confirmation shows only in the report's counts.

## 16. Left to the builder, and left for later

Left to the builder of step 1: how screens are told apart when a menu is animated; how the label area is cut from
two pictures; how the text recognition is called on a picture; the order in which keys are tried; how many sessions a
crawl may spend; the report's layout.

Left for later, each its own design:

- Blu-ray menus without Java, which need their menu clips decrypted, and DVDs, which need another driver.
- A profile for a menu family the crawler cannot handle.
- Roles other than commentary and audio description, and subtitles, when the owner rules on them.
- Pop-up menus, combinations of choices, and a second player profile for one disc.
- A crawl during the rip.
- Sharing a map between users.
