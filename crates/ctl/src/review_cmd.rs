//! `lazymkvctl review` (phase 3 design, parts 6.3 and 11; rulings T32-R1, T32-R3, T32-R6, T32-E15): the owner's command
//! for settling a held job. It opens one connection, asks the engine for the job's review document, prints the sheet
//! (`review_doc::render`), asks one short question per open item (one letter, or Enter), and sends one `review_action`
//! with the answers. Standard error never holds a subtitle line: the owner's dialogue text goes to standard output
//! only, as part of the sheet.

use std::io::{BufRead, BufReader, Read, Write};

use lazymkv_core::ipc::{read_reply, Envelope, Reply, PROTOCOL_VERSION};
use lazymkv_core::review_doc::{self, Answers, HeldAs, Item, ItemKind, ReviewDoc};
use lazymkv_core::settings::SettingsProblem;
use lazymkv_dev::Args;
use lazymkv_platform::IpcStream;
use serde_json::{json, Value};

use crate::engine_cmds::{open, print_json, print_refusal, CLOSED, ENGINE_COMMANDS};
use crate::{argument_error, Env};

/// The line that ends a review nothing came of: the owner said "not yet", or the input ended.
const NOTHING_CHANGED: &str = "Nothing was changed.";
/// The line after the engine's `ok` to the answers (ruling T32-R6): it says how to watch the job, and does not wait for
/// the remux.
const ACCEPTED: &str = "Accepted. The job is filing; watch it with: lazymkvctl watch";
/// The line after the engine's `ok` to a discard.
const DISCARDED: &str = "Discarded. The job's rip is being deleted.";
/// What guided mode says after the sheet of a job that is not held (it is in the library), where nothing can be answered.
const NOT_HELD: &str = "This job is not held, so there is nothing to answer.";
/// The question after the last item's; Enter is "not yet" (ruling T32-E29).
const FILE_IT_NOW: &str = "File it now?  y yes   n not yet   x discard the job     [Enter = n]";
/// What is said before `FILE_IT_NOW` when no answer changes an item (ruling T32-E27).
const NO_CHANGE: &str = "Your answers change nothing: the job would file as it stands.";
/// What is said when a typed line is none of the choices, before the question is asked again.
const NOT_A_CHOICE: &str = "That is not one of the choices.";
/// The message of exit 4 when the engine's review document is not one this lazymkvctl reads.
const UNREADABLE_DOCUMENT: &str = "lazymkvctl: the engine's review document is not one this version of lazymkvctl reads";
/// The message of exit 4 for a line of the engine that is not a message; serde's text is left out, since it can quote
/// the line, which can hold the owner's subtitle lines.
const UNREADABLE_REPLY: &str = "the engine's reply could not be read";

/// What the words after `review` ask for.
struct Options {
    job: String,
    /// `--all`: the document lists every item, not only those awaiting an answer.
    all: bool,
    /// `--json`: print the document itself, and nothing more.
    json: bool,
    /// Each `--answer ID=WORD`, as given: the title key may be left out of the id.
    answers: Vec<(String, String)>,
    /// `--yes`: send the answers without asking.
    yes: bool,
}

/// The words after `review` read as `Options`; Err is the argument error's text.
fn options(words: &[String]) -> Result<Options, String> {
    let mut args = Args::new(words, &["--all", "--json", "--answer", "--yes"]);
    let (all, json, yes) = (args.switch("--all"), args.switch("--json"), args.switch("--yes"));
    let given = args.flags("--answer")?;
    let mut left = args.positionals()?;
    if left.is_empty() {
        return Err("the following arguments are required: JOB".to_string());
    }
    let job = left.remove(0);
    if !left.is_empty() {
        return Err(format!("unrecognized arguments: {}", left.join(" ")));
    }
    let mut answers = Vec::new();
    for answer in given {
        match answer.rsplit_once('=') {
            Some((id, word)) if !id.trim().is_empty() && !word.trim().is_empty() => {
                answers.push((id.trim().to_string(), word.trim().to_string()));
            }
            _ => return Err(format!("--answer takes ID=WORD, as in \"t00-00801 sub 9=sdh\", not \"{answer}\"")),
        }
    }
    if json && (yes || !answers.is_empty()) {
        return Err("--json prints the document and sends nothing; it does not go with --answer or --yes".to_string());
    }
    Ok(Options { job, all, json, answers, yes })
}

/// `lazymkvctl review JOB [--all] [--json] [--answer ID=WORD ...] [--yes]` (`argv[0]` is `review`): one connection,
/// where request 1 is `review`, then `review_action` (or `discard_preview` and `discard`). Exits 0 when the engine
/// accepted, when the owner said "not yet" or the input ended, and when only the sheet was printed; 1 when the engine
/// refused; 2 for an argument error; 4 when no engine answered or it closed the connection.
pub(crate) fn run(argv: &[String], env: Env) -> i32 {
    let Env { connector, stdin, out, err, interactive, .. } = env;
    let options = match options(&argv[1..]) {
        Ok(options) => options,
        Err(message) => {
            let synopsis = ENGINE_COMMANDS.iter().find(|(command, _)| *command == "review").map_or("review", |(_, text)| text);
            let _ = writeln!(err, "usage: lazymkvctl {synopsis}");
            return argument_error(err, "review", &message);
        }
    };
    let mut peer = match Peer::connect(connector) {
        Ok(peer) => peer,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 4;
        }
    };
    let document = match peer.ask("review", json!({"job": options.job, "all": options.all})) {
        Ok(document) => document,
        Err(failure) => return failure.report(err),
    };
    if options.json {
        print_json(out, &document);
        return 0;
    }
    let Ok(doc) = serde_json::from_value::<ReviewDoc>(document) else {
        let _ = writeln!(err, "{UNREADABLE_DOCUMENT}");
        return 4;
    };
    let session = Session { peer, doc, job: options.job.clone(), input: BufReader::new(stdin), out, err };
    if options.yes || !options.answers.is_empty() {
        session.on_the_command_line(&options)
    } else if interactive {
        session.guided()
    } else {
        session.sheet_only()
    }
}

/// The connection to the engine, and the id of the next request.
struct Peer {
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    next_id: u64,
}

/// Why a request got no `ok`.
enum Failure {
    /// The connection closed, or a line could not be read: exit 4, with this text on standard error.
    Gone(String),
    /// The engine answered `error`: exit 1.
    Refused { message: String, problems: Vec<SettingsProblem> },
}

impl Failure {
    /// Prints the failure on `err` as `engine_cmds` does and returns its exit code.
    fn report(self, err: &mut dyn Write) -> i32 {
        match self {
            Failure::Gone(message) => {
                let _ = writeln!(err, "{message}");
                4
            }
            Failure::Refused { message, problems } => {
                print_refusal(err, &message, &problems);
                1
            }
        }
    }
}

impl Peer {
    /// Connects; Err is the message of exit 4 when there is no engine to reach.
    fn connect(connector: Option<&dyn lazymkv_platform::IpcConnector>) -> Result<Peer, &'static str> {
        let IpcStream { reader, writer } = open(connector)?;
        Ok(Peer { reader, writer, next_id: 1 })
    }

    /// Sends one request and returns the `result` of the engine's `ok` to it. Every other line is skipped: a push, and
    /// the reply to any other id.
    fn ask(&mut self, kind: &str, payload: Value) -> Result<Value, Failure> {
        let id = self.next_id;
        self.next_id += 1;
        let envelope = Envelope { v: PROTOCOL_VERSION, kind: kind.to_string(), id: Some(id), payload };
        let line = serde_json::to_string(&envelope).expect("an envelope of JSON values serialises");
        let sent = self
            .writer
            .write_all(line.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush());
        if sent.is_err() {
            return Err(Failure::Gone(CLOSED.to_string()));
        }
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Err(Failure::Gone(CLOSED.to_string())),
                Ok(_) => {}
                Err(error) => return Err(Failure::Gone(format!("lazymkvctl: {error}"))),
            }
            match read_reply(line.trim_end()) {
                Err(message) => return Err(Failure::Gone(format!("lazymkvctl: {}", unreadable(&message)))),
                Ok(Reply::Ok { id: Some(answered), result }) if answered == id => return Ok(result),
                Ok(Reply::Error { id: Some(answered), message, problems }) if answered == id => {
                    return Err(Failure::Refused { message, problems });
                }
                Ok(_) => {}
            }
        }
    }
}

/// `read_reply`'s refusal, as it may be printed: its protocol-version sentence stands, and any other is replaced, since
/// serde's text can quote the line.
fn unreadable(message: &str) -> &str {
    if message.starts_with("the engine speaks protocol version") {
        message
    } else {
        UNREADABLE_REPLY
    }
}

/// What the owner typed at an item's question.
enum Asked {
    /// A letter of the item's: the answer word it stands for.
    Word(&'static str),
    /// Enter: the item keeps what it stands as.
    Enter,
    /// The input ended.
    Ended,
}

/// What the owner typed at the last question.
enum Verdict {
    File,
    NotYet,
    Discard,
    Ended,
}

/// One review: the connection, the document, and the owner's streams.
struct Session<'a> {
    peer: Peer,
    doc: ReviewDoc,
    job: String,
    input: BufReader<&'a mut dyn Read>,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
}

impl Session<'_> {
    /// The sheet, then a blank line. Standard output only: the sheet holds the owner's subtitle lines.
    fn print_sheet(&mut self) {
        let _ = writeln!(self.out, "{}", review_doc::render(&self.doc));
    }

    /// No terminal and no answers: the sheet, and how to answer it.
    fn sheet_only(mut self) -> i32 {
        self.print_sheet();
        if self.doc.held == HeldAs::Filed {
            let _ = writeln!(self.out, "{NOT_HELD}");
            return 0;
        }
        let _ = writeln!(self.out, "To answer it, run lazymkvctl review {} at a terminal, or give --answer ID=WORD and --yes.", self.job);
        0
    }

    /// `--answer` words, or `--yes`: no questions. Without `--yes` the answers are shown, with the one line that sends
    /// them, and nothing is sent.
    fn on_the_command_line(mut self, options: &Options) -> i32 {
        let words: Vec<(String, String)> =
            options.answers.iter().map(|(id, word)| (self.full_id(id), word.clone())).collect();
        if let Err(message) = self.refuse_given(&words) {
            return argument_error(self.err, "review", &message);
        }
        let answers = match review_doc::answers_of(&self.doc, &words) {
            Ok(answers) => answers,
            Err(message) => return argument_error(self.err, "review", &message),
        };
        if !options.yes {
            let _ = writeln!(self.out, "Nothing was sent. To send these answers, add --yes:");
            let _ = writeln!(self.out, "{}", one_line(&self.job, &words));
            return 0;
        }
        let unanswered = self.unanswered_collisions(&words);
        self.send(&answers, &unanswered)
    }

    /// The `--answer` words refused before anything is sent: a second answer for one item (its full and its short id
    /// count as one), in core's words for an item named twice; and, on a job with several titles, an id without its
    /// title key, with core's refusal and how to give the key.
    fn refuse_given(&self, words: &[(String, String)]) -> Result<(), String> {
        let ids: Vec<&String> = self.doc.titles.iter().flat_map(|title| &title.items).map(|item| &item.id).collect();
        let mut seen: Vec<&str> = Vec::new();
        for (id, word) in words {
            if seen.contains(&id.as_str()) {
                return Err(review_doc::named_twice(id));
            }
            seen.push(id);
            let keyed = self.doc.titles.iter().any(|title| id.starts_with(&format!("{} ", title.key)));
            if self.doc.titles.len() > 1 && !keyed && !ids.contains(&id) {
                let refusal = match review_doc::answers_of(&self.doc, &[(id.clone(), word.clone())]) {
                    Err(refusal) => refusal,
                    Ok(_) => continue,
                };
                let example = ids.iter().find(|each| each.ends_with(&format!(" {id}"))).or(ids.first());
                if let Some(example) = example {
                    return Err(format!("{refusal} (this job has several titles: give the title key, as in {example})"));
                }
                return Err(refusal);
            }
        }
        Ok(())
    }

    /// The path of each collision item the sheet asks about that `words` leave unanswered: approving without an
    /// answer for it holds the job again (ruling T32-E26).
    fn unanswered_collisions(&self, words: &[(String, String)]) -> Vec<String> {
        let answered = |item: &Item| words.iter().any(|(id, _)| *id == item.id);
        self.questions()
            .into_iter()
            .filter(|(_, item)| item.kind == ItemKind::Collision && !answered(item))
            .map(|(_, item)| item.path.clone().unwrap_or(item.describe))
            .collect()
    }

    /// The answers that change an item, before `FILE_IT_NOW` (ruling T32-E27; design part 6.3): `Your answers:` and
    /// one line per answer (`review_doc::answer_line`), under each title's line when the job has several titles; or
    /// `NO_CHANGE` when none changes an item.
    fn print_answers(&mut self, words: &[(String, String)]) {
        let questions = self.questions();
        let changed: Vec<(&String, &Item, &str)> = questions
            .iter()
            .filter_map(|(key, item)| {
                words.iter().find(|(id, _)| *id == item.id).map(|(_, word)| (key, item, word.as_str()))
            })
            .collect();
        if changed.is_empty() {
            let _ = writeln!(self.out, "{NO_CHANGE}");
            return;
        }
        let _ = writeln!(self.out, "Your answers:");
        let mut named: Option<&str> = None;
        for (key, item, word) in changed {
            if self.doc.titles.len() > 1 && named != Some(key.as_str()) {
                let _ = writeln!(self.out, "{}", self.title_line(key));
                named = Some(key);
            }
            let _ = writeln!(self.out, "  {}", review_doc::answer_line(item, word));
        }
    }

    /// `id` as the document names an item: whole, or without its title key when the job has one title.
    fn full_id(&self, id: &str) -> String {
        let named = |each: &str| self.doc.titles.iter().flat_map(|title| &title.items).any(|item| item.id == each);
        match self.doc.titles.as_slice() {
            [only] if !named(id) && named(&format!("{} {id}", only.key)) => format!("{} {id}", only.key),
            _ => id.to_string(),
        }
    }

    /// The sheet, a question per open item in the sheet's order, then `FILE_IT_NOW`.
    fn guided(mut self) -> i32 {
        self.print_sheet();
        if self.doc.held == HeldAs::Filed {
            let _ = writeln!(self.out, "{NOT_HELD}");
            return 0;
        }
        let mut words: Vec<(String, String)> = Vec::new();
        let mut named: Option<String> = None;
        for (key, item) in self.questions() {
            // The sheet names no title, so a job with several says which one the questions that follow are about.
            if self.doc.titles.len() > 1 && named.as_deref() != Some(key.as_str()) {
                let _ = writeln!(self.out, "{}", self.title_line(&key));
                named = Some(key);
            }
            match self.ask_item(&item) {
                Asked::Word(word) if word != item.stands => words.push((item.id.clone(), word.to_string())),
                Asked::Word(_) | Asked::Enter => {}
                Asked::Ended => return self.nothing_changed(),
            }
        }
        self.print_answers(&words);
        let unanswered = self.unanswered_collisions(&words);
        for path in &unanswered {
            let _ = writeln!(self.out, "Not answered: the file at {path}. Filing now would hold the job again.");
        }
        match self.ask_file_it() {
            Verdict::File => {}
            Verdict::NotYet | Verdict::Ended => return self.nothing_changed(),
            Verdict::Discard => return self.discard(),
        }
        let answers = match review_doc::answers_of(&self.doc, &words) {
            Ok(answers) => answers,
            Err(message) => return argument_error(self.err, "review", &message),
        };
        let _ = writeln!(self.out, "The same answers in one line, for next time:");
        let _ = writeln!(self.out, "{}", one_line(&self.job, &words));
        self.send(&answers, &unanswered)
    }

    /// The items to ask about, each with its title's key, in the sheet's order: those of the points this sheet
    /// answers. A point it does not answer, or that has no item (the rip point; a collision point whose path holds
    /// nothing now), is shown and not asked.
    fn questions(&self) -> Vec<(String, Item)> {
        let items = self.doc.titles.iter().flat_map(|title| title.items.iter().map(move |item| (&title.key, item)));
        let asked = self.doc.points.iter().filter(|point| point.answerable).flat_map(|point| &point.items);
        asked
            .filter_map(|id| items.clone().find(|(_, item)| &item.id == id))
            .map(|(key, item)| (key.clone(), item.clone()))
            .collect()
    }

    /// The line that names title `key` before its questions: the key the sheet's points are made from, and the file
    /// name of the `Files as:` line the sheet shows for it.
    fn title_line(&self, key: &str) -> String {
        let file = self
            .doc
            .titles
            .iter()
            .find(|title| title.key == key)
            .and_then(|title| title.final_path.as_deref())
            .and_then(|path| path.rsplit(['/', '\\']).next());
        match file {
            Some(file) => format!("Title {key} (files as {file}):"),
            None => format!("Title {key}:"),
        }
    }

    /// One line of the owner's input, without its line end; None when the input ended or cannot be read.
    fn read_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_string()),
        }
    }

    /// Asks `item`'s question until the owner types a letter it offers, or Enter.
    fn ask_item(&mut self, item: &Item) -> Asked {
        loop {
            let _ = writeln!(self.out, "{}", review_doc::question(item));
            let _ = self.out.flush();
            let Some(typed) = self.read_line() else { return Asked::Ended };
            if typed.is_empty() {
                return Asked::Enter;
            }
            match review_doc::letter_word(item, &typed.to_lowercase()) {
                Some("replace") => match self.confirm_replace(item) {
                    Some(true) => return Asked::Word("replace"),
                    Some(false) => {}
                    None => return Asked::Ended,
                },
                Some(word) => return Asked::Word(word),
                None => {
                    let _ = writeln!(self.out, "{NOT_A_CHOICE}");
                }
            }
        }
    }

    /// Whether the owner typed the word `replace` for `item`; None when the input ended.
    fn confirm_replace(&mut self, item: &Item) -> Option<bool> {
        let path = item.path.as_deref().unwrap_or(&item.describe);
        let _ = writeln!(self.out, "Replace files this job's film over the file at {path}. Type replace to go on:");
        let _ = self.out.flush();
        let typed = self.read_line()?;
        if typed != "replace" {
            let _ = writeln!(self.out, "Not replaced.");
        }
        Some(typed == "replace")
    }

    /// Asks `FILE_IT_NOW` until the owner types one of its letters, or Enter, which is "not yet" (ruling T32-E29).
    fn ask_file_it(&mut self) -> Verdict {
        loop {
            let _ = writeln!(self.out, "{FILE_IT_NOW}");
            let _ = self.out.flush();
            let Some(typed) = self.read_line() else { return Verdict::Ended };
            match typed.to_lowercase().as_str() {
                "y" | "yes" => return Verdict::File,
                "n" | "no" | "" => return Verdict::NotYet,
                "x" => return Verdict::Discard,
                _ => {
                    let _ = writeln!(self.out, "{NOT_A_CHOICE}");
                }
            }
        }
    }

    fn nothing_changed(&mut self) -> i32 {
        let _ = writeln!(self.out, "{NOTHING_CHANGED}");
        0
    }

    /// Sends the owner's answers as `review_action`'s `approve`, and says how to watch the job when the engine accepts
    /// (ruling T32-R6); it does not wait for the remux. With a collision left `unanswered` the engine holds the job
    /// again, so each of its paths gets `Accepted. The job will be held again until the file at <path> is answered.`
    /// in place of `ACCEPTED` (ruling T32-E26).
    fn send(&mut self, answers: &Answers, unanswered: &[String]) -> i32 {
        match self.peer.ask("review_action", approval(&self.job, answers)) {
            Ok(_) if unanswered.is_empty() => {
                let _ = writeln!(self.out, "{ACCEPTED}");
                0
            }
            Ok(_) => {
                for path in unanswered {
                    let said = format!("Accepted. The job will be held again until the file at {path} is answered.");
                    let _ = writeln!(self.out, "{said}");
                }
                0
            }
            Err(failure) => failure.report(self.err),
        }
    }

    /// `x`: asks the engine what a discard deletes, asks the owner to type `discard`, and only then sends it.
    fn discard(&mut self) -> i32 {
        discard_job(&mut self.peer, &self.job, Some(&mut self.input), &mut *self.out, &mut *self.err)
    }
}

/// `lazymkvctl discard JOB [--yes]` (`argv[0]` is `discard`): frees a job's data. At a terminal it asks the engine
/// what a discard deletes, asks the owner to type `discard`, and only then sends it; with `--yes` it sends only
/// `discard`. Without a terminal and without `--yes` nothing is sent. Exits 0 when the engine accepted or the owner
/// did not type the word; 1 when the engine refused; 2 for an argument error; 4 when no engine answered.
pub(crate) fn discard(argv: &[String], env: Env) -> i32 {
    let Env { connector, stdin, out, err, interactive, .. } = env;
    let mut args = Args::new(&argv[1..], &["--yes"]);
    let yes = args.switch("--yes");
    let parsed = args.positionals().and_then(|mut left| match left.len() {
        0 => Err("the following arguments are required: JOB".to_string()),
        1 => Ok(left.remove(0)),
        _ => Err(format!("unrecognized arguments: {}", left[1..].join(" "))),
    });
    let job = match parsed {
        Ok(job) if yes || interactive => job,
        Ok(_) => {
            let message = "discard asks you to type the word discard, which needs a terminal; to discard without \
                           asking, add --yes";
            return argument_error(err, "discard", message);
        }
        Err(message) => {
            let synopsis = ENGINE_COMMANDS.iter().find(|(command, _)| *command == "discard").map_or("discard", |(_, text)| text);
            let _ = writeln!(err, "usage: lazymkvctl {synopsis}");
            return argument_error(err, "discard", &message);
        }
    };
    let mut peer = match Peer::connect(connector) {
        Ok(peer) => peer,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 4;
        }
    };
    let mut input = BufReader::new(stdin);
    discard_job(&mut peer, &job, if yes { None } else { Some(&mut input) }, out, err)
}

/// The discard of `job` on `peer`: with `input`, the preview and the question first, and nothing is sent unless the
/// owner types the word; without it (`--yes`), `discard` alone.
fn discard_job(peer: &mut Peer, job: &str, input: Option<&mut dyn BufRead>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if let Some(input) = input {
        let preview = match peer.ask("discard_preview", json!({"job": job})) {
            Ok(preview) => preview,
            Err(failure) => return failure.report(err),
        };
        if !confirm_discard(job, &preview, input, &mut *out) {
            let _ = writeln!(out, "{NOTHING_CHANGED}");
            return 0;
        }
    }
    match peer.ask("discard", json!({"job": job})) {
        Ok(_) => {
            let _ = writeln!(out, "{DISCARDED}");
            0
        }
        Err(failure) => failure.report(err),
    }
}

/// The command line that gives `words` in one go: `lazymkvctl review JOB --answer "ID=WORD" ... --yes`.
fn one_line(job: &str, words: &[(String, String)]) -> String {
    let answers: String = words.iter().map(|(id, word)| format!("--answer \"{id}={word}\" ")).collect();
    format!("lazymkvctl review {job} {answers}--yes")
}

/// The payload of the `review_action` that approves with `answers`: with no answers and no labels it is the plain
/// approval, as `approve` sends it.
fn approval(job: &str, answers: &Answers) -> Value {
    let mut payload = json!({"job": job, "action": "approve"});
    if !answers.overrides.is_empty() {
        payload["overrides"] = serde_json::to_value(&answers.overrides).expect("overrides serialise");
    }
    if !answers.labels.is_empty() {
        payload["labels"] = serde_json::to_value(&answers.labels).expect("labels serialise");
    }
    payload
}

/// A size in decimal gigabytes with one decimal, as `41.3 GB`.
fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// The question before a discard, asked on `out` and answered on `input` (ruling T32-E15; shared with `lazymkvctl
/// discard`). `preview` is the engine's reply to `discard_preview`: `{"bytes", "files"}` for the job's staging folder,
/// or `{"unknown": <reason>}` when its size is not known. The question names `job` (ruling T32-E28). True only when
/// the next line read is the word `discard`, white space around it ignored; any other line, or the end of the input,
/// is false.
pub(crate) fn confirm_discard(job: &str, preview: &Value, input: &mut dyn BufRead, out: &mut dyn Write) -> bool {
    let counted = (preview.get("bytes").and_then(Value::as_u64), preview.get("files").and_then(Value::as_u64));
    let size = match counted {
        (Some(bytes), Some(1)) => format!("1 file, {}", gigabytes(bytes)),
        (Some(bytes), Some(files)) => format!("{files} files, {}", gigabytes(bytes)),
        _ => {
            let reason = match preview.get("unknown") {
                Some(Value::String(reason)) => reason.clone(),
                Some(other) => other.to_string(),
                None => "the engine gave no size".to_string(),
            };
            format!("its size is unknown ({reason})")
        }
    };
    let _ = writeln!(
        out,
        "Discard deletes job {job}'s rip: {size}. The disc would have to be ripped again. Type discard to go on:"
    );
    let _ = out.flush();
    let mut typed = String::new();
    matches!(input.read_line(&mut typed), Ok(read) if read > 0) && typed.trim() == "discard"
}

#[cfg(test)]
mod tests {
    use super::confirm_discard;
    use crate::{run, Env};
    use lazymkv_core::ipc::{reply_error, reply_ok};
    use lazymkv_core::remembered::{
        overrides_from_value, AudioOverride, AudioTrackOverride, Collision, CollisionAnswer, Overrides, SubtitleOverride,
        SubtitleTrackOverride,
    };
    use lazymkv_core::review_doc::{
        self, DocIdentity, DocTitle, Evidence, HeldAs, Item, ItemKind, LabelKind, Point, PointKind, ReviewDoc,
        ReviewLabel, DOC_FORMAT,
    };
    use lazymkv_core::review::ReviewReason;
    use lazymkv_core::testsupport::Scratch;
    use lazymkv_core::tracks::SubtitleRole;
    use lazymkv_dev::evidence::Ctx;
    use lazymkv_platform::fakes::{MemoryConnector, MemoryIpc};
    use lazymkv_platform::{ConnectError, IpcStream, IpcTransport};
    use serde_json::{json, Value};
    use std::io::{BufRead, Write};
    use std::thread::{self, JoinHandle};

    const JOB: &str = "made-up-1";
    const KEY: &str = "t00-00801";
    const FINAL: &str = "M:/Movies/Made Up (2000)/Made Up (2000) - 2160p.mkv";
    const LINE_ALPHA: &str = "Made-up line alpha";
    const LINE_BETA: &str = "Made-up line beta";
    const SNAPSHOT: &str = r#"{"v":1,"type":"snapshot","id":null,"payload":{"jobs":[]}}"#;
    const PUSH: &str = r#"{"v":1,"type":"job_update","id":null,"payload":{"id":"made-up-1"}}"#;
    const ACCEPTED: &str = "Accepted. The job is filing; watch it with: lazymkvctl watch";
    const FILE_IT_NOW_LINE: &str = "File it now?  y yes   n not yet   x discard the job     [Enter = n]";

    fn item(kind: ItemKind, id: String, stream: Option<u32>, stands: &str, order: Option<u32>, answers: &[&str]) -> Item {
        Item {
            id,
            kind,
            title: 2,
            stream,
            path: (kind == ItemKind::Collision).then(|| FINAL.to_string()),
            describe: if kind == ItemKind::Collision { FINAL.to_string() } else { "English  made-up track".to_string() },
            stands: stands.to_string(),
            order_if_kept: order,
            answers: answers.iter().map(|word| word.to_string()).collect(),
            evidence: Vec::new(),
        }
    }

    fn lines(mut item: Item, texts: &[&str]) -> Item {
        item.evidence.extend(texts.iter().map(|text| Evidence { kind: "lines".to_string(), text: text.to_string() }));
        item
    }

    fn point(kind: PointKind, reason: ReviewReason, sentence: &str, answerable: bool, items: &[&Item]) -> Point {
        Point {
            kind,
            reasons: vec![reason],
            title_key: Some(KEY.to_string()),
            sentence: sentence.to_string(),
            answerable,
            items: items.iter().map(|each| each.id.clone()).collect(),
        }
    }

    fn doc_of(points: Vec<Point>, items: Vec<Item>) -> ReviewDoc {
        ReviewDoc {
            format: DOC_FORMAT.to_string(),
            job: JOB.to_string(),
            held: HeldAs::Gate,
            since_unix: 1_700_000_000,
            identity: DocIdentity {
                step: "tmdb_one".to_string(),
                confidence: 0.9,
                title: Some("Made Up".to_string()),
                year: Some(2000),
            },
            reasons: Vec::new(),
            notes: Vec::new(),
            points,
            titles: vec![DocTitle {
                key: KEY.to_string(),
                scan_title: 2,
                seconds: Some(6000),
                chapters: Some(12),
                bytes: Some(1_000),
                staged_path: Some("S:/made-up/title.mkv".to_string()),
                final_path: Some(FINAL.to_string()),
                items,
            }],
        }
    }

    const SUBTITLE_WORDS: [&str; 5] = ["regular", "sdh", "forced", "commentary", "drop"];
    const AUDIO_WORDS: [&str; 4] = ["keep", "default", "commentary", "drop"];
    const COLLISION_WORDS: [&str; 2] = ["keep_both", "replace"];

    /// Two English subtitle tracks the decision keeps as regular, which it cannot tell apart.
    fn subtitles_doc() -> ReviewDoc {
        let first = lines(item(ItemKind::Subtitle, format!("{KEY} sub 9"), Some(9), "regular", None, &SUBTITLE_WORDS),
                           &[LINE_ALPHA]);
        let second = lines(item(ItemKind::Subtitle, format!("{KEY} sub 10"), Some(10), "regular", None, &SUBTITLE_WORDS),
                            &[LINE_BETA]);
        let sentence = "Subtitles: two full English tracks that do not match each other. lazymkv cannot tell which one \
                        carries the film's dialogue.";
        let point = point(PointKind::Subtitles, ReviewReason::SubtitleAmbiguous, sentence, true, &[&first, &second]);
        doc_of(vec![point], vec![first, second])
    }

    /// Two kept audio tracks, neither played by default, and one that sounds like a commentary.
    fn audio_doc() -> ReviewDoc {
        let first = item(ItemKind::Audio, format!("{KEY} audio 1"), Some(1), "keep", Some(0), &AUDIO_WORDS);
        let second = item(ItemKind::Audio, format!("{KEY} audio 8"), Some(8), "keep", Some(1), &AUDIO_WORDS);
        let sentence = "Audio: one track sounds like people talking over the film, as a commentary does. lazymkv does \
                        not drop a track on that alone.";
        let point = point(PointKind::Audio, ReviewReason::CommentaryDetected, sentence, true, &[&first, &second]);
        doc_of(vec![point], vec![first, second])
    }

    fn collision_doc() -> ReviewDoc {
        let colliding = item(ItemKind::Collision, format!("{KEY} collision"), None, "", None, &COLLISION_WORDS);
        let sentence = "Already in the library: a file this job did not make is already where this title would be filed. \
                        lazymkv never replaces a file it did not make without asking.";
        let point = point(PointKind::Collision, ReviewReason::PathCollision, sentence, true, &[&colliding]);
        doc_of(vec![point], vec![colliding])
    }

    /// Only a rip point: it has no item, and discarding is its other way.
    fn rip_doc() -> ReviewDoc {
        let sentence = "The rip: the drive could not read parts of the disc, so the ripped file may skip or stutter in \
                        places. Filing it accepts the rip as it is; the other way is to discard the job and rip the disc \
                        again.";
        doc_of(vec![point(PointKind::Rip, ReviewReason::ReadErrors, sentence, false, &[])], Vec::new())
    }

    fn value_of(doc: &ReviewDoc) -> Value {
        serde_json::to_value(doc).unwrap()
    }

    /// What the scripted engine does with the next request it reads.
    enum Say {
        /// An `ok` reply carrying this result.
        Ok(Value),
        /// An `error` reply with this message.
        Error(&'static str),
        /// Reads the request, then closes the connection without a reply.
        Hangup,
    }

    /// Accepts one connection, writes a snapshot, then answers each request it reads with the next `Say` (a push of
    /// another job first, which the client must skip), and a request past the script with an error, so a client that
    /// sends too much fails instead of waiting. Returns every request it read; None when nothing connected.
    fn peer(ipc: MemoryIpc, says: Vec<Say>) -> JoinHandle<Option<Vec<Value>>> {
        thread::spawn(move || {
            let Ok(mut stream) = ipc.accept() else {
                return None;
            };
            let send = |lines: &[String], stream: &mut IpcStream| {
                for line in lines {
                    writeln!(stream.writer, "{line}").unwrap();
                }
                stream.writer.flush().unwrap();
            };
            send(&[SNAPSHOT.to_string()], &mut stream);
            let mut says = says.into_iter();
            let mut requests = Vec::new();
            loop {
                let mut line = String::new();
                if stream.reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
                let request: Value = serde_json::from_str(line.trim_end()).unwrap();
                let id = request["id"].as_u64();
                requests.push(request);
                let reply = match says.next() {
                    Some(Say::Ok(result)) => reply_ok(id, result),
                    Some(Say::Error(message)) => reply_error(id, message, &[]),
                    Some(Say::Hangup) => break,
                    None => reply_error(id, "unexpected request", &[]),
                };
                send(&[PUSH.to_string(), reply], &mut stream);
            }
            Some(requests)
        })
    }

    /// Drops the test's only connector, then returns what the peer read; fails here when lazymkvctl never connected.
    fn finish(server: JoinHandle<Option<Vec<Value>>>, connector: MemoryConnector) -> Vec<Value> {
        drop(connector);
        let requests = server.join().expect("the scripted peer's thread panicked; its message is printed above");
        requests.expect("lazymkvctl never connected to the scripted peer")
    }

    /// `argv` run with `stdin` at a terminal or not: (exit code, standard output, standard error).
    fn call(argv: &[&str], connector: &MemoryConnector, stdin: &str, interactive: bool) -> (i32, String, String) {
        let scratch = Scratch::new("ctl-review");
        let ctx = Ctx::for_test(&scratch.0);
        let mut input = stdin.as_bytes();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let argv: Vec<String> = argv.iter().map(|word| word.to_string()).collect();
        let env = Env { ctx: &ctx, connector: Some(connector), stdin: &mut input, out: &mut out, err: &mut err, interactive };
        let code = run(&argv, env);
        (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
    }

    /// A whole review of `doc`: the engine answers the document, then each of `then`.
    fn review(argv: &[&str], doc: &ReviewDoc, then: Vec<Say>, stdin: &str, interactive: bool)
        -> (i32, String, String, Vec<Value>)
    {
        let (ipc, connector) = MemoryIpc::new();
        let mut says = vec![Say::Ok(value_of(doc))];
        says.extend(then);
        let server = peer(ipc, says);
        let (code, out, err) = call(argv, &connector, stdin, interactive);
        (code, out, err, finish(server, connector))
    }

    fn review_request() -> Value {
        json!({"v": 1, "type": "review", "id": 1, "payload": {"job": JOB, "all": false}})
    }

    fn kinds(requests: &[Value]) -> Vec<&str> {
        requests.iter().map(|request| request["type"].as_str().unwrap()).collect()
    }

    /// The answers a `review_action` request carries, read as the engine reads them.
    fn answers_in(request: &Value) -> (Overrides, Vec<ReviewLabel>) {
        let payload = &request["payload"];
        assert_eq!((request["type"].as_str(), payload["job"].as_str(), payload["action"].as_str()),
                   (Some("review_action"), Some(JOB), Some("approve")));
        let overrides = overrides_from_value(&payload["overrides"]).unwrap();
        // A key the request leaves out is the empty list.
        let labels = payload.get("labels").cloned().unwrap_or_else(|| json!([]));
        (overrides, serde_json::from_value(labels).unwrap())
    }

    fn sdh_and_commentary() -> (Overrides, Vec<ReviewLabel>) {
        let overrides = Overrides {
            subtitles: vec![
                SubtitleTrackOverride { title: 2, stream: 9, action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } },
                SubtitleTrackOverride { title: 2, stream: 10, action: SubtitleOverride::Drop },
            ],
            ..Overrides::default()
        };
        let label = |stream, word: &str| ReviewLabel { title: 2, stream, kind: LabelKind::Subtitle, label: word.to_string() };
        (overrides, vec![label(9, "sdh"), label(10, "commentary")])
    }

    #[test]
    fn the_sheet_is_printed_and_nothing_is_sent_without_a_terminal_or_answers() {
        let doc = subtitles_doc();
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], "s\nc\ny\n", false);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.starts_with(&review_doc::render(&doc)), "{out}");
        assert!(out.contains(LINE_ALPHA) && out.contains(LINE_BETA), "{out}");
        assert_eq!(requests, vec![review_request()]);
    }

    #[test]
    fn guided_answers_send_one_review_action() {
        let doc = subtitles_doc();
        let (code, out, err, requests) =
            review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "s\nc\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["review", "review_action"]);
        assert_eq!(requests[1]["id"], 2);
        assert_eq!(answers_in(&requests[1]), sdh_and_commentary());
        for item in &doc.titles[0].items {
            assert_eq!(out.matches(&review_doc::question(item)).count(), 1, "{out}");
        }
        assert!(out.contains(&format!("{FILE_IT_NOW_LINE}\n")), "{out}");
        assert!(!out.contains("(files as"), "a job with one title names no title: {out}");
        let one_line = format!("lazymkvctl review {JOB} --answer \"{KEY} sub 9=sdh\" --answer \"{KEY} sub 10=commentary\" --yes");
        assert!(out.ends_with(&format!("{one_line}\n{ACCEPTED}\n")), "{out}");
    }

    #[test]
    fn an_audio_point_takes_keep_default_commentary_and_drop() {
        let doc = audio_doc();
        let (code, _, err, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "p\nc\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        let overrides = Overrides {
            audio: vec![
                AudioTrackOverride { title: 2, stream: 1, action: AudioOverride::Keep { order: 0, default: true } },
                AudioTrackOverride { title: 2, stream: 8, action: AudioOverride::Commentary },
            ],
            ..Overrides::default()
        };
        let label = ReviewLabel { title: 2, stream: 8, kind: LabelKind::Audio, label: "commentary".to_string() };
        assert_eq!(answers_in(&requests[1]), (overrides, vec![label]));
        // `d` drops and `k` keeps, in the same way.
        let (code, _, _, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "d\nk\ny\n", true);
        assert_eq!(code, 0);
        let (overrides, labels) = answers_in(&requests[1]);
        assert_eq!(overrides.audio, vec![AudioTrackOverride { title: 2, stream: 1, action: AudioOverride::Drop }]);
        assert!(labels.is_empty());
    }

    #[test]
    fn enter_keeps_every_item_and_yes_approves_unchanged() {
        let doc = subtitles_doc();
        let (code, out, err, requests) =
            review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "\n\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(requests[1], json!({"v": 1, "type": "review_action", "id": 2,
                                        "payload": {"job": JOB, "action": "approve"}}));
        assert!(out.ends_with(&format!("lazymkvctl review {JOB} --yes\n{ACCEPTED}\n")), "{out}");
    }

    #[test]
    fn not_yet_sends_nothing() {
        let doc = subtitles_doc();
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], "s\nc\nn\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(requests, vec![review_request()]);
        assert!(out.ends_with("Nothing was changed.\n"), "{out}");
    }

    #[test]
    fn a_letter_the_item_does_not_offer_is_asked_again() {
        let doc = subtitles_doc();
        let (code, out, _, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "q\ns\nc\ny\n", true);
        assert_eq!(code, 0);
        let first = review_doc::question(&doc.titles[0].items[0]);
        assert_eq!(out.matches(&first).count(), 2, "{out}");
        assert_eq!(answers_in(&requests[1]), sdh_and_commentary());
        // A letter of another kind of item is not offered either: `k` is not a subtitle answer.
        let (_, out, _, _) = review(&["review", JOB], &doc, vec![], "k\n", true);
        assert_eq!(out.matches(&first).count(), 2, "{out}");
    }

    #[test]
    fn input_that_ends_in_the_middle_sends_nothing() {
        let doc = subtitles_doc();
        for stdin in ["s\n", "", "s\nc\n"] {
            let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], stdin, true);
            assert_eq!((code, err.as_str()), (0, ""), "{stdin:?}");
            assert_eq!(requests, vec![review_request()], "{stdin:?}");
            assert!(out.ends_with("Nothing was changed.\n"), "{stdin:?}: {out}");
        }
    }

    #[test]
    fn answers_on_the_command_line_need_yes() {
        let doc = subtitles_doc();
        let (first, second) = (format!("{KEY} sub 9=sdh"), "sub 10=commentary");
        let (code, out, err, requests) = review(&["review", JOB, "--answer", &first, "--answer", second], &doc, vec![], "", false);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(requests, vec![review_request()], "nothing is sent without --yes");
        let one_line = format!("lazymkvctl review {JOB} --answer \"{KEY} sub 9=sdh\" --answer \"{KEY} sub 10=commentary\" --yes");
        assert!(out.contains("Nothing was sent."), "{out}");
        assert!(out.ends_with(&format!("{one_line}\n")), "{out}");
        // With --yes the same request goes as the guided answers (the title key may be left out of an id).
        let argv = ["review", JOB, "--answer", &first, "--answer", second, "--yes"];
        let (code, out, err, requests) = review(&argv, &doc, vec![Say::Ok(Value::Null)], "", false);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["review", "review_action"]);
        assert_eq!(answers_in(&requests[1]), sdh_and_commentary());
        assert_eq!(out, format!("{ACCEPTED}\n"));
        // --yes alone approves as it stands.
        let (code, _, _, requests) = review(&["review", JOB, "--yes"], &doc, vec![Say::Ok(Value::Null)], "", false);
        assert_eq!(code, 0);
        assert_eq!(requests[1]["payload"], json!({"job": JOB, "action": "approve"}));
    }

    #[test]
    fn an_answer_word_the_item_lacks_exits_2_before_anything_is_sent() {
        let doc = subtitles_doc();
        let cases = [
            (format!("{KEY} sub 9=replace"), "\"replace\" is not an answer for t00-00801 sub 9"),
            (format!("{KEY} sub 99=drop"), "no item \"t00-00801 sub 99\" on this sheet"),
            ("sub 99=drop".to_string(), "no item \"sub 99\" on this sheet"),
        ];
        for (answer, said) in cases {
            let (code, out, err, requests) = review(&["review", JOB, "--answer", &answer, "--yes"], &doc, vec![], "", false);
            assert_eq!((code, out.as_str()), (2, ""), "{answer}");
            assert!(err.contains(&format!("lazymkvctl review: error: {said}")), "{answer}: {err}");
            assert_eq!(requests, vec![review_request()], "{answer}: only the document was asked for");
        }
        // A word with no `=`, an unknown flag and a missing job are refused before any connection.
        let refusing = MemoryConnector::failing(ConnectError::NotRunning);
        for argv in [&["review", JOB, "--answer", "sub 9"][..], &["review", JOB, "--now"], &["review"], &["review", "a", "b"]] {
            let (code, out, err) = call(argv, &refusing, "", false);
            assert_eq!((code, out.as_str()), (2, ""), "{argv:?}");
            assert!(err.starts_with("usage: lazymkvctl review JOB "), "{argv:?}: {err}");
            assert!(err.contains("lazymkvctl review: error: "), "{argv:?}: {err}");
            assert!(!err.contains("no engine"), "{argv:?} connected: {err}");
        }
        let (code, _, err) = call(&["review", JOB, "--json", "--yes"], &refusing, "", false);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn x_asks_for_the_word_discard() {
        let doc = rip_doc();
        let preview = json!({"bytes": 41_300_000_000u64, "files": 3});
        let asked = "Discard deletes job made-up-1's rip: 3 files, 41.3 GB. The disc would have to be ripped again. Type discard to go on:";
        let (code, out, err, requests) =
            review(&["review", JOB], &doc, vec![Say::Ok(preview.clone()), Say::Ok(Value::Null)], "x\ndiscard\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["review", "discard_preview", "discard"]);
        assert_eq!(requests[1]["payload"], json!({"job": JOB}));
        assert_eq!(requests[2]["payload"], json!({"job": JOB}));
        assert!(out.contains(&format!("{asked}\n")), "{out}");
        assert!(out.ends_with("Discarded. The job's rip is being deleted.\n"), "{out}");
        // Any other word asks the engine for nothing more.
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![Say::Ok(preview)], "x\nno\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["review", "discard_preview"], "the preview is read-only; discard is not sent");
        assert!(out.ends_with("Nothing was changed.\n"), "{out}");
    }

    #[test]
    fn replace_asks_for_the_word_replace() {
        let doc = collision_doc();
        let asked = format!("Replace files this job's film over the file at {FINAL}. Type replace to go on:");
        let (code, out, err, requests) =
            review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "r\nreplace\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        let replaced = Overrides {
            collision: vec![CollisionAnswer { path: FINAL.to_string(), action: Collision::Replace }],
            ..Overrides::default()
        };
        assert_eq!(answers_in(&requests[1]), (replaced, vec![]));
        assert!(out.contains(&format!("{asked}\n")), "{out}");
        // Any other word takes the answer back and asks the question again; `k` then keeps both.
        let (code, out, _, requests) =
            review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "r\nno\nk\ny\n", true);
        assert_eq!(code, 0);
        let question = review_doc::question(&doc.titles[0].items[0]);
        assert_eq!(out.matches(&question).count(), 2, "{out}");
        let kept = Overrides {
            collision: vec![CollisionAnswer { path: FINAL.to_string(), action: Collision::KeepBoth }],
            ..Overrides::default()
        };
        assert_eq!(answers_in(&requests[1]), (kept, vec![]));
        // Input that ends at the question sends nothing.
        let (code, out, _, requests) = review(&["review", JOB], &doc, vec![], "r\n", true);
        assert_eq!((code, requests), (0, vec![review_request()]));
        assert!(out.ends_with("Nothing was changed.\n"), "{out}");
    }

    /// Two titles of one job, each holding a subtitle track 9 (the same stream number) kept as regular.
    fn two_titles_doc() -> ReviewDoc {
        let mut doc = subtitles_doc();
        doc.titles[0].items.truncate(1);
        doc.points[0].items.truncate(1);
        let mut second = doc.titles[0].clone();
        second.key = "t01-00802".to_string();
        second.scan_title = 3;
        second.final_path = Some("M:/Movies/Made Up (2000)/Made Up (2000) - Extra.mkv".to_string());
        second.items[0].id = "t01-00802 sub 9".to_string();
        second.items[0].title = 3;
        let mut second_point = doc.points[0].clone();
        second_point.title_key = Some(second.key.clone());
        second_point.items = vec![second.items[0].id.clone()];
        doc.points.push(second_point);
        doc.titles.push(second);
        doc
    }

    #[test]
    fn a_job_with_two_titles_names_the_title_before_its_questions() {
        let doc = two_titles_doc();
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "s\nd\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        let first = "Title t00-00801 (files as Made Up (2000) - 2160p.mkv):";
        let second = "Title t01-00802 (files as Made Up (2000) - Extra.mkv):";
        let question = review_doc::question(&doc.titles[0].items[0]);
        // Each title line heads its questions, and again its answers before `File it now?` (ruling T32-E27).
        assert_eq!(out.matches(first).count() + out.matches(second).count(), 4, "{out}");
        let listed = format!("Your answers:\n{first}\n  track 9: SDH\n{second}\n  track 9: drop\n{FILE_IT_NOW_LINE}\n");
        assert!(out.contains(&listed), "{out}");
        assert_eq!(out.matches(&question).count(), 2, "the same question is asked for both titles: {out}");
        let at = |text: &str, from: usize| from + out[from..].find(text).unwrap_or_else(|| panic!("{text}: {out}"));
        let named_first = at(&format!("{first}\n"), 0);
        let asked_first = at(&question, named_first);
        let named_second = at(&format!("{second}\n"), asked_first);
        let asked_second = at(&question, named_second);
        assert!(named_first < asked_first && asked_first < named_second && named_second < asked_second);
        let overrides = Overrides {
            subtitles: vec![
                SubtitleTrackOverride { title: 2, stream: 9, action: SubtitleOverride::Keep { role: SubtitleRole::Sdh } },
                SubtitleTrackOverride { title: 3, stream: 9, action: SubtitleOverride::Drop },
            ],
            ..Overrides::default()
        };
        let label = |title, word: &str| ReviewLabel { title, stream: 9, kind: LabelKind::Subtitle, label: word.to_string() };
        assert_eq!(answers_in(&requests[1]), (overrides, vec![label(2, "sdh")]));
        assert!(out.contains("--answer \"t00-00801 sub 9=sdh\" --answer \"t01-00802 sub 9=drop\" --yes"), "{out}");
    }

    #[test]
    fn a_job_that_is_not_held_is_shown_and_not_asked() {
        let mut doc = subtitles_doc();
        doc.held = HeldAs::Filed;
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], "s\nc\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(requests, vec![review_request()]);
        assert!(out.starts_with(&review_doc::render(&doc)), "{out}");
        assert!(out.ends_with("This job is not held, so there is nothing to answer.\n"), "{out}");
        assert!(!out.contains("File it now?") && !out.contains(&review_doc::question(&doc.titles[0].items[0])), "{out}");
        // The one-line form is still sent: the engine's reply is its answer.
        let refused = "job made-up-1 is filed, not held at the disposition gate";
        let argv = ["review", JOB, "--answer", "sub 9=sdh", "--yes"];
        let (code, _, err, requests) = review(&argv, &doc, vec![Say::Error(refused)], "", true);
        assert_eq!((code, err), (1, format!("lazymkvctl: {refused}\n")));
        assert_eq!(kinds(&requests), ["review", "review_action"]);
    }

    #[test]
    fn json_prints_the_document_itself() {
        let doc = subtitles_doc();
        let (code, out, err, requests) = review(&["review", JOB, "--json", "--all"], &doc, vec![], "s\nc\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap(), value_of(&doc));
        assert_eq!(requests, vec![json!({"v": 1, "type": "review", "id": 1, "payload": {"job": JOB, "all": true}})]);
    }

    #[test]
    fn subtitle_lines_go_to_standard_output_only() {
        let doc = subtitles_doc();
        let refused = "job made-up-1 is filed, not held at the disposition gate";
        let (code, out, err, _) = review(&["review", JOB], &doc, vec![Say::Error(refused)], "s\nc\ny\n", true);
        assert_eq!(code, 1);
        assert_eq!(err, format!("lazymkvctl: {refused}\n"));
        assert!(out.contains(LINE_ALPHA) && out.contains(LINE_BETA), "{out}");
        assert!(!err.contains(LINE_ALPHA) && !err.contains(LINE_BETA), "{err}");
        // Not when the engine's document cannot be read either.
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![Say::Ok(json!({"format": LINE_ALPHA}))]);
        let (code, out, err) = call(&["review", JOB], &connector, "", false);
        finish(server, connector);
        assert_eq!((code, out.as_str()), (4, ""));
        assert!(!err.contains(LINE_ALPHA), "{err}");
    }

    #[test]
    fn a_refused_answer_exits_1_with_the_engines_words() {
        let doc = subtitles_doc();
        let refused = "the answers name t00-00801 sub 9 twice";
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![Say::Error(refused)], "s\nc\ny\n", true);
        assert_eq!((code, err), (1, format!("lazymkvctl: {refused}\n")));
        assert_eq!(kinds(&requests), ["review", "review_action"]);
        assert!(!out.contains(ACCEPTED), "{out}");
        // A refusal of the document itself, too.
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![Say::Error("no job made-up-1")]);
        let (code, out, err) = call(&["review", JOB], &connector, "", false);
        finish(server, connector);
        assert_eq!((code, out.as_str(), err.as_str()), (1, "", "lazymkvctl: no job made-up-1\n"));
    }

    #[test]
    fn a_connection_closed_after_the_sheet_exits_4() {
        let doc = subtitles_doc();
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![Say::Hangup], "s\nc\ny\n", true);
        assert_eq!(code, 4);
        assert_eq!(err, "lazymkvctl: the engine closed the connection before replying\n");
        assert!(out.contains(LINE_ALPHA), "the sheet was printed first: {out}");
        assert_eq!(kinds(&requests), ["review", "review_action"]);
        // No engine at all.
        let refusing = MemoryConnector::failing(ConnectError::NotRunning);
        let (code, out, err) = call(&["review", JOB], &refusing, "", false);
        assert_eq!((code, out.as_str(), err.as_str()), (4, "", "lazymkvctl: no engine is running\n"));
    }

    fn confirmed(preview: Value, typed: &str) -> (bool, String) {
        let mut out = Vec::new();
        let mut input = typed.as_bytes();
        (confirm_discard(JOB, &preview, &mut input, &mut out), String::from_utf8(out).unwrap())
    }

    #[test]
    fn confirm_discard_asks_with_the_size_and_takes_only_the_word() {
        let known = json!({"bytes": 41_300_000_000u64, "files": 12});
        let asked = "Discard deletes job made-up-1's rip: 12 files, 41.3 GB. The disc would have to be ripped again. Type discard to go on:\n";
        assert_eq!(confirmed(known.clone(), "discard\n"), (true, asked.to_string()));
        assert_eq!(confirmed(known.clone(), "  discard \r\n").0, true);
        for typed in ["", "no\n", "Discard now\n", "\n", "y\n"] {
            assert_eq!(confirmed(known.clone(), typed), (false, asked.to_string()), "{typed:?}");
        }
        let zero = json!({"bytes": 0, "files": 0});
        assert!(confirmed(zero, "").1.starts_with("Discard deletes job made-up-1's rip: 0 files, 0.0 GB. "));
        let one = json!({"bytes": 2_000_000_000u64, "files": 1});
        assert!(confirmed(one, "").1.starts_with("Discard deletes job made-up-1's rip: 1 file, 2.0 GB. "));
    }

    #[test]
    fn confirm_discard_says_when_the_size_is_unknown() {
        let unknown = json!({"unknown": "the staging share did not answer: cable out"});
        let asked = "Discard deletes job made-up-1's rip: its size is unknown (the staging share did not answer: cable out). \
                     The disc would have to be ripped again. Type discard to go on:\n";
        assert_eq!(confirmed(unknown.clone(), "discard\n"), (true, asked.to_string()));
        assert_eq!(confirmed(unknown, "stop\n"), (false, asked.to_string()));
    }

    /// `argv` run against a scripted engine that answers each of `then` (no review document first).
    fn discarding(argv: &[&str], then: Vec<Say>, stdin: &str, interactive: bool) -> (i32, String, String, Vec<Value>) {
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, then);
        let (code, out, err) = call(argv, &connector, stdin, interactive);
        (code, out, err, finish(server, connector))
    }

    fn discard_requests(requests: &[Value]) -> Vec<Value> {
        requests.iter().map(|request| json!({"type": request["type"], "payload": request["payload"]})).collect()
    }

    const ASKED_KNOWN: &str =
        "Discard deletes job made-up-1's rip: 3 files, 41.3 GB. The disc would have to be ripped again. Type discard to go on:\n";

    #[test]
    fn discard_asks_with_the_size_and_sends_on_the_word() {
        let preview = json!({"bytes": 41_300_000_000u64, "files": 3});
        let (code, out, err, requests) =
            discarding(&["discard", JOB], vec![Say::Ok(preview), Say::Ok(Value::Null)], "discard\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["discard_preview", "discard"]);
        assert_eq!(requests[0]["id"], 1);
        assert_eq!(requests[1]["id"], 2);
        assert_eq!(requests[1]["payload"], json!({"job": JOB}));
        assert_eq!(out, format!("{ASKED_KNOWN}Discarded. The job's rip is being deleted.\n"));
    }

    #[test]
    fn any_other_word_sends_nothing_and_exits_0() {
        let preview = json!({"bytes": 41_300_000_000u64, "files": 3});
        for typed in ["no\n", "\n", "", "Discard\n", "y\n"] {
            let (code, out, err, requests) = discarding(&["discard", JOB], vec![Say::Ok(preview.clone())], typed, true);
            assert_eq!((code, err.as_str()), (0, ""), "{typed:?}");
            assert_eq!(discard_requests(&requests), vec![json!({"type": "discard_preview", "payload": {"job": JOB}})],
                       "{typed:?}: only the read-only preview goes out");
            assert_eq!(out, format!("{ASKED_KNOWN}Nothing was changed.\n"), "{typed:?}");
        }
    }

    #[test]
    fn yes_skips_the_question() {
        let (code, out, err, requests) =
            discarding(&["discard", JOB, "--yes"], vec![Say::Ok(Value::Null)], "", false);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(discard_requests(&requests), vec![json!({"type": "discard", "payload": {"job": JOB}})]);
        assert_eq!(out, "Discarded. The job's rip is being deleted.\n");
        // The same at a terminal: no question either.
        let (code, out, _, requests) = discarding(&["discard", "--yes", JOB], vec![Say::Ok(Value::Null)], "", true);
        assert_eq!((code, kinds(&requests)), (0, vec!["discard"]));
        assert!(!out.contains("Type discard"), "{out}");
    }

    #[test]
    fn an_unknown_size_says_so() {
        let preview = json!({"unknown": "the staging share did not answer: cable out"});
        let (code, out, err, requests) =
            discarding(&["discard", JOB], vec![Say::Ok(preview), Say::Ok(Value::Null)], "discard\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["discard_preview", "discard"]);
        assert!(out.starts_with("Discard deletes job made-up-1's rip: its size is unknown (the staging share did not answer: cable out). \
                                 The disc would have to be ripped again. Type discard to go on:\n"), "{out}");
    }

    #[test]
    fn without_a_terminal_and_without_yes_it_exits_2_saying_so() {
        let refusing = MemoryConnector::failing(ConnectError::NotRunning);
        let (code, out, err) = call(&["discard", JOB], &refusing, "discard\n", false);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("lazymkvctl discard: error: ") && err.contains("--yes"), "{err}");
        assert!(!err.contains("no engine"), "nothing was connected to: {err}");
        // Through a scripted engine: not even the preview is asked for.
        let (ipc, connector) = MemoryIpc::new();
        let server = peer(ipc, vec![]);
        let (code, out, _) = call(&["discard", JOB], &connector, "discard\n", false);
        drop(connector);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(server.join().unwrap().is_none_or(|requests| requests.is_empty()));
    }

    #[test]
    fn discard_exit_codes_follow_review() {
        // A refused preview (an unknown job) exits 1 with the engine's words and asks nothing.
        let (code, out, err, requests) = discarding(&["discard", JOB], vec![Say::Error("no job made-up-1")], "discard\n", true);
        assert_eq!((code, out.as_str(), err.as_str()), (1, "", "lazymkvctl: no job made-up-1\n"));
        assert_eq!(kinds(&requests), ["discard_preview"]);
        // A refused discard exits 1 and does not say it discarded.
        let (code, out, err, _) = discarding(&["discard", JOB, "--yes"], vec![Say::Error("job made-up-1 is running")], "", false);
        assert_eq!((code, err.as_str()), (1, "lazymkvctl: job made-up-1 is running\n"));
        assert!(!out.contains("Discarded"), "{out}");
        // A closed connection exits 4.
        let (code, _, err, _) = discarding(&["discard", JOB, "--yes"], vec![Say::Hangup], "", false);
        assert_eq!((code, err.as_str()), (4, "lazymkvctl: the engine closed the connection before replying\n"));
        // Argument errors exit 2 before any connection.
        let refusing = MemoryConnector::failing(ConnectError::NotRunning);
        for argv in [&["discard"][..], &["discard", "a", "b"], &["discard", JOB, "--now"]] {
            let (code, out, err) = call(argv, &refusing, "", true);
            assert_eq!((code, out.as_str()), (2, ""), "{argv:?}");
            assert!(err.starts_with("usage: lazymkvctl discard JOB [--yes]\n"), "{argv:?}: {err}");
            assert!(!err.contains("no engine"), "{argv:?}: {err}");
        }
        let (code, _, err) = call(&["discard", JOB], &refusing, "discard\n", true);
        assert_eq!((code, err.as_str()), (4, "lazymkvctl: no engine is running\n"));
    }

    /// Ruling T32-E29: Enter at `File it now?` is "not yet": nothing is sent, `Nothing was changed.`, exit 0. Any
    /// other text that is not a choice is said so and the question is asked again.
    #[test]
    fn an_empty_line_at_file_it_now_does_not_approve() {
        let doc = subtitles_doc();
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], "\n\n\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out.matches(FILE_IT_NOW_LINE).count(), 1, "{out}");
        assert_eq!(requests, vec![review_request()]);
        assert!(out.ends_with(&format!("{FILE_IT_NOW_LINE}\nNothing was changed.\n")), "{out}");
        let (code, out, _, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "\n\nq\ny\n", true);
        assert_eq!(code, 0);
        assert_eq!(out.matches(FILE_IT_NOW_LINE).count(), 2, "{out}");
        assert_eq!(out.matches("That is not one of the choices.").count(), 1, "{out}");
        assert_eq!(kinds(&requests), ["review", "review_action"]);
    }

    /// Ruling T32-E27 (design 6.3): before `File it now?` the answers that change an item are listed, each named as
    /// its question names it, with the choice in the question's own words.
    #[test]
    fn the_answers_are_listed_before_the_question() {
        let doc = subtitles_doc();
        let (code, out, err, _) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "s\nc\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        let listed = format!("Your answers:\n  track 9: SDH\n  track 10: commentary (dropped)\n{FILE_IT_NOW_LINE}\n");
        assert!(out.contains(&listed), "{out}");
        assert!(!out.contains("Your answers change nothing"), "{out}");
        // An answer equal to how the item stands is not listed.
        let (_, out, _, _) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "r\nd\ny\n", true);
        assert!(out.contains(&format!("Your answers:\n  track 10: drop\n{FILE_IT_NOW_LINE}\n")), "{out}");
    }

    /// Ruling T32-E27: when no answer changes an item, the one sentence says so before the question.
    #[test]
    fn answers_that_change_nothing_say_so() {
        let doc = subtitles_doc();
        let (code, out, err, _) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "\nr\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        let said = format!("Your answers change nothing: the job would file as it stands.\n{FILE_IT_NOW_LINE}\n");
        assert!(out.contains(&said), "{out}");
        assert!(!out.contains("Your answers:"), "{out}");
    }

    /// Ruling T32-E26: a collision left unanswered is said before `File it now?`, and after the engine accepts the
    /// approval the line says the job will be held again, in place of the accepted sentence; the one-line form says
    /// the same. An answered collision changes nothing of this.
    #[test]
    fn an_unanswered_collision_is_said_before_and_after_filing() {
        let doc = collision_doc();
        let before = format!("Not answered: the file at {FINAL}. Filing now would hold the job again.\n{FILE_IT_NOW_LINE}\n");
        let after = format!("Accepted. The job will be held again until the file at {FINAL} is answered.\n");
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "\ny\n", true);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(kinds(&requests), ["review", "review_action"]);
        assert!(out.contains(&before), "{out}");
        assert!(out.ends_with(&after), "{out}");
        assert!(!out.contains(ACCEPTED), "{out}");
        // The one-line form with --yes.
        let (code, out, _, _) = review(&["review", JOB, "--yes"], &doc, vec![Say::Ok(Value::Null)], "", false);
        assert_eq!((code, out), (0, after.clone()));
        // Answered, the ordinary sentence.
        let (code, out, _, _) = review(&["review", JOB], &doc, vec![Say::Ok(Value::Null)], "k\ny\n", true);
        assert_eq!(code, 0);
        assert!(!out.contains("Not answered") && out.ends_with(&format!("{ACCEPTED}\n")), "{out}");
    }

    /// A second `--answer` for one item (its full id and its short id count as one) is refused before anything is
    /// sent, in core's words for an item named twice.
    #[test]
    fn the_same_item_answered_twice_is_refused() {
        let doc = subtitles_doc();
        let (first, second) = (format!("{KEY} sub 9=sdh"), "sub 9=drop");
        let argv = ["review", JOB, "--answer", &first, "--answer", second, "--yes"];
        let (code, out, err, requests) = review(&argv, &doc, vec![], "", false);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.contains("lazymkvctl review: error: the answers name t00-00801 sub 9 twice"), "{err}");
        assert_eq!(requests, vec![review_request()], "only the document was asked for");
    }

    /// On a job with several titles a short id is refused with a hint to give the title key.
    #[test]
    fn a_short_id_on_a_job_with_several_titles_asks_for_the_title_key() {
        let doc = two_titles_doc();
        let (code, out, err, requests) = review(&["review", JOB, "--answer", "sub 9=sdh", "--yes"], &doc, vec![], "", false);
        assert_eq!((code, out.as_str()), (2, ""));
        let said = "lazymkvctl review: error: no item \"sub 9\" on this sheet (this job has several titles: give the title \
                    key, as in t00-00801 sub 9)";
        assert!(err.contains(said), "{err}");
        assert_eq!(requests, vec![review_request()]);
    }

    #[test]
    fn a_job_that_is_not_held_gets_no_hint_to_answer_it() {
        let mut doc = subtitles_doc();
        doc.held = HeldAs::Filed;
        let (code, out, err, requests) = review(&["review", JOB], &doc, vec![], "", false);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(requests, vec![review_request()]);
        assert!(out.ends_with("This job is not held, so there is nothing to answer.\n"), "{out}");
        assert!(!out.contains("To answer it"), "{out}");
        // A held job still gets the hint.
        let (_, out, _, _) = review(&["review", JOB], &subtitles_doc(), vec![], "", false);
        assert!(out.contains("To answer it, run lazymkvctl review"), "{out}");
    }

    #[test]
    fn help_and_the_command_table_name_review() {
        let (_, connector) = MemoryIpc::new();
        let (code, out, _) = call(&["--help"], &connector, "", false);
        assert_eq!(code, 0);
        assert!(out.contains("\n  review JOB [--all] [--json] [--answer ID=WORD ...] [--yes]\n"), "{out}");
    }
}
