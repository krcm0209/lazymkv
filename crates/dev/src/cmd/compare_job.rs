//! `lazymkv-dev compare-job`, the comparison rules (pull request 7's notes, calls 3 to 5). The engine's job folder
//! (state.json and one decide.json per title) is compared with the saved output of `lazymkvctl plan` and `lazymkvctl
//! decide` for the same job. This file holds the pure function `compare` and the command's `run`: arguments, file
//! reading, output and exit codes (calls 2 and 7).

use crate::evidence::Ctx;
use crate::pyjson::{structural_diff, Tolerance};
use crate::{usage_error, Args};
use lazymkv_core::plan::REFRESHED_IDENTITY;
use serde_json::{json, Map, Value};
use std::io::Write;

/// What is compared (pull request 7's notes, calls 3 to 5): `Whole` the plan and every decide document; `PlanAndAudio`
/// the plan and the two audio parts of each decide document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The plan and every decide document.
    Whole,
    /// The plan and the two audio parts of each decide document.
    PlanAndAudio,
}

/// The output lines in order, and how many of them are differences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The output lines, the last being the verdict.
    pub lines: Vec<String>,
    /// How many of the lines are differences (the prefixed `plan: ` and `decide <key>: ` lines).
    pub differences: usize,
}

/// The first line of a report on a job planned with a lookup pending (design part 12.7).
const PENDING_LINE: &str =
    "the job was planned with a lookup pending: only the plan's identity is compared (design part 12.7)";
/// The line that says what `PlanAndAudio` leaves out (pull request 7's notes, call 5).
const AUDIO_ONLY_LINE: &str = "not compared: slug, label, reconcile, subtitles, reviews, the plan section's slug, inputs and settings, and the decision section apart from decision.audio (design notes, call 5)";
/// The two keys the engine's decide.json has and `lazymkvctl decide`'s output has not.
const EXTRA_KEYS: [&str; 2] = ["audio_rule_version", "lazymkv_version"];

/// Exact comparison: the engine's side is `expected` (pull request 7's notes, call 3).
fn exact() -> Tolerance {
    Tolerance { abs: 0.0, loose: 0.0, loose_keys: &[], ignore: Vec::new() }
}

/// Compares the engine's files with the saved CLI output (pull request 7's notes, calls 3 to 5). `state`: state.json.
/// `own`: (title key, decide.json) for every title that has one, in key order. `saved_plan`: lazymkvctl plan's output.
/// `saved`: (title key, lazymkvctl decide's output). Err is an argument-level problem's text (exit 2); Ok is the
/// report (exit 0 or 1). The caller passes each title key at most once in `own` and in `saved`.
pub fn compare(
    state: &Value,
    own: &[(String, Value)],
    saved_plan: &Value,
    saved: &[(String, Value)],
    mode: Mode,
) -> Result<Report, String> {
    let plan = match state.get("plan") {
        Some(plan) if !plan.is_null() => plan,
        _ => return Err("the job has no plan".to_string()),
    };
    let saved_plan = saved_plan.get("plan").ok_or_else(|| "the saved plan output has no plan key".to_string())?;
    for (key, document) in own {
        for name in EXTRA_KEYS {
            if document.get(name).is_none() {
                return Err(format!("{}/decide.json lacks {}", key, name));
            }
        }
    }
    if mode == Mode::Whole {
        if let Some((key, _)) = own.iter().find(|(key, _)| find(saved, key).is_none()) {
            return Err(format!("no --decide for {}", key));
        }
    }
    if let Some((key, _)) = saved.iter().find(|(key, _)| find(own, key).is_none()) {
        return Err(format!("the job has no decide.json for {}", key));
    }

    let pending = plan["evidence"]["sentences"]
        .as_array()
        .map_or(false, |list| list.iter().any(|s| s.as_str().map_or(false, |s| s.starts_with(REFRESHED_IDENTITY))));
    let tolerance = exact();
    let mut lines = Vec::new();
    if pending {
        lines.push(PENDING_LINE.to_string());
    }
    let mut differences: Vec<String> = Vec::new();

    let (plan_left, plan_right) =
        if pending { (only(plan, &["identity"]), only(saved_plan, &["identity"])) } else { (plan.clone(), saved_plan.clone()) };
    differences.extend(structural_diff(&plan_left, &plan_right, &tolerance).into_iter().map(|line| format!("plan: {}", line)));

    for (key, document) in own {
        let Some(saved_document) = find(saved, key) else { continue };
        let (left, right) = match mode {
            Mode::Whole => {
                let mut left = document.clone();
                if let Some(map) = left.as_object_mut() {
                    for name in EXTRA_KEYS {
                        map.remove(name);
                    }
                }
                let (mut left, mut right) = (left, saved_document.clone());
                if pending {
                    left = identity_only(&left);
                    right = identity_only(&right);
                }
                (left, right)
            }
            Mode::PlanAndAudio => (audio_parts(document), audio_parts(saved_document)),
        };
        differences
            .extend(structural_diff(&left, &right, &tolerance).into_iter().map(|line| format!("decide {}: {}", key, line)));
    }
    let count = differences.len();
    lines.extend(differences);

    if mode == Mode::PlanAndAudio {
        lines.push(AUDIO_ONLY_LINE.to_string());
        for (key, _) in own {
            if find(saved, key).is_none() {
                lines.push(format!("not compared: title {}", key));
            }
        }
    }
    if let Some(titles) = state.get("titles").and_then(Value::as_array) {
        for key in titles.iter().filter_map(|title| title.get("key").and_then(Value::as_str)) {
            if find(own, key).is_none() {
                lines.push(format!("not compared: title {} has no decide.json", key));
            }
        }
    }
    lines.push(if count == 0 { "compare-job: no difference".to_string() } else { format!("compare-job: {} difference(s)", count) });
    Ok(Report { lines, differences: count })
}

/// The document of the title `key`, if the list has one.
fn find<'a>(list: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    list.iter().find(|(name, _)| name == key).map(|(_, value)| value)
}

/// `{k0: {k1: ... value[k0][k1]...}}`: the value's one nested part under its own path, nothing else; a missing
/// part is left out, so the comparison names it absent.
fn only(value: &Value, path: &[&str]) -> Value {
    let mut inner = value;
    for name in path {
        match inner.get(*name) {
            Some(next) => inner = next,
            None => return Value::Object(Map::new()),
        }
    }
    let mut out = inner.clone();
    for name in path.iter().rev() {
        out = json!({ *name: out });
    }
    out
}

/// A decide document with its `/plan` section replaced by `{"plan": {"identity": ...}}` (call 4).
fn identity_only(document: &Value) -> Value {
    let mut out = document.clone();
    if let Some(map) = out.as_object_mut() {
        if map.contains_key("plan") {
            map.insert("plan".to_string(), only(&document["plan"], &["plan", "identity"]));
        }
    }
    out
}

/// A decide document's `/audio` and `/decision/decision/audio`, each under its own path (call 5).
fn audio_parts(document: &Value) -> Value {
    let mut out = Map::new();
    if let Some(audio) = document.get("audio") {
        out.insert("audio".to_string(), audio.clone());
    }
    if let Value::Object(decision) = only(document, &["decision", "decision", "audio"]) {
        out.extend(decision);
    }
    Value::Object(out)
}

/// The command's name as typed.
const NAME: &str = "compare-job";
/// The usage line, under the command's name.
const USAGE: &str = "compare-job [-h] job --plan FILE [--decide KEY=FILE ...] [--plan-and-audio]";
/// What -h prints after the usage line.
const HELP: &str = "
Compares an engine job folder with the saved output of lazymkvctl plan and lazymkvctl decide.

positional arguments:
  job                the job folder (its state.json and <title key>/decide.json are read)

optional arguments:
  -h, --help         show this help message and exit
  --plan FILE        the saved output of lazymkvctl plan
  --decide KEY=FILE  the saved output of lazymkvctl decide for the title KEY (any number; each key once)
  --plan-and-audio   compare the plan and the two audio parts of each decide document only

exit status: 0 when nothing differs; 1 when something does; 2 for an argument error or a file that cannot be used.
";
/// The reason for a file that looks like UTF-16 (PowerShell's `>` writes it).
const UTF16_REASON: &str = "not UTF-8 JSON (it looks like UTF-16; save the output as bytes, not through PowerShell's >)";

/// The JSON file at `given` (relative to the context), or the line `compare-job: <given>: <reason>` (pull request 7's
/// notes, call 7).
fn read_json_file(ctx: &Ctx, given: &str) -> Result<Value, String> {
    let fail = |reason: String| format!("{NAME}: {given}: {reason}");
    let bytes = std::fs::read(ctx.abs(given)).map_err(|problem| fail(problem.to_string()))?;
    if bytes.is_empty() {
        return Err(fail("empty".to_string()));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) || bytes.contains(&0) {
        return Err(fail(UTF16_REASON.to_string()));
    }
    serde_json::from_slice(&bytes).map_err(|problem| fail(format!("not JSON: {problem}")))
}

/// What the command line asks for.
struct Request {
    job: String,
    plan: String,
    decide: Vec<(String, String)>,
    mode: Mode,
}

/// Reads the arguments (call 2); Err is the usage message.
fn parse(rest: &[String]) -> Result<Request, String> {
    let mut args = Args::new(rest, &["--plan", "--decide", "--plan-and-audio"]);
    let plans = args.flags("--plan")?;
    let decides = args.flags("--decide")?;
    let mode = if args.switch("--plan-and-audio") { Mode::PlanAndAudio } else { Mode::Whole };
    let words = args.positionals()?;
    let mut missing = Vec::new();
    if words.is_empty() {
        missing.push("job");
    }
    if plans.is_empty() {
        missing.push("--plan");
    }
    if !missing.is_empty() {
        return Err(format!("the following arguments are required: {}", missing.join(", ")));
    }
    if words.len() > 1 {
        return Err(format!("unrecognized arguments: {}", words[1..].join(" ")));
    }
    if plans.len() > 1 {
        return Err("argument --plan: given more than once".to_string());
    }
    let mut decide: Vec<(String, String)> = Vec::new();
    for text in decides {
        match text.split_once('=') {
            Some((key, file)) if !key.is_empty() && !file.is_empty() => {
                if decide.iter().any(|(seen, _)| seen == key) {
                    return Err(format!("argument --decide: {key} is given more than once"));
                }
                decide.push((key.to_string(), file.to_string()));
            }
            _ => return Err(format!("argument --decide: expected KEY=FILE, got {text}")),
        }
    }
    Ok(Request { job: words[0].clone(), plan: plans[0].clone(), decide, mode })
}

/// Reads every file the request names and compares them; Err is the whole line for stderr.
fn compare_request(ctx: &Ctx, request: &Request) -> Result<Report, String> {
    let state = read_json_file(ctx, &format!("{}/state.json", request.job))?;
    let mut own = Vec::new();
    if let Some(titles) = state.get("titles").and_then(Value::as_array) {
        for key in titles.iter().filter_map(|title| title.get("key").and_then(Value::as_str)) {
            if own.iter().any(|(seen, _): &(String, Value)| seen == key) {
                continue;
            }
            let given = format!("{}/{}/decide.json", request.job, key);
            if ctx.abs(&given).exists() {
                own.push((key.to_string(), read_json_file(ctx, &given)?));
            }
        }
    }
    let saved_plan = read_json_file(ctx, &request.plan)?;
    let mut saved = Vec::new();
    for (key, file) in &request.decide {
        saved.push((key.clone(), read_json_file(ctx, file)?));
    }
    compare(&state, &own, &saved_plan, &saved, request.mode).map_err(|text| format!("{NAME}: {text}"))
}

/// `lazymkv-dev compare-job <job folder> --plan FILE [--decide KEY=FILE ...] [--plan-and-audio]` (pull request 7's
/// notes, calls 2 and 7): the report on `out`; exit 0 for no difference, 1 for any, 2 for an argument or file problem
/// (its line on `err`).
pub fn run(ctx: &Ctx, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if args.iter().take_while(|word| word.as_str() != "--").any(|word| word == "-h" || word == "--help") {
        let _ = writeln!(out, "usage: lazymkv-dev {USAGE}");
        let _ = write!(out, "{HELP}");
        return 0;
    }
    let request = match parse(args) {
        Ok(request) => request,
        Err(message) => return usage_error(err, NAME, USAGE, &message),
    };
    match compare_request(ctx, &request) {
        Ok(report) => {
            for line in &report.lines {
                let _ = writeln!(out, "{line}");
            }
            i32::from(report.differences != 0)
        }
        Err(line) => {
            let _ = writeln!(err, "{line}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "t00-00800";

    /// The engine's decide.json: the CLI's document and the two extra keys.
    fn engine_doc(mut doc: Value) -> Value {
        let map = doc.as_object_mut().unwrap();
        map.insert("audio_rule_version".to_string(), json!(3));
        map.insert("lazymkv_version".to_string(), json!("0.1.0"));
        doc
    }

    fn state(plan: Value) -> Value {
        json!({"plan": plan, "titles": [{"key": KEY}]})
    }

    fn doc() -> Value {
        json!({
            "slug": "a", "label": "x",
            "plan": {"slug": "s", "inputs": {"i": 1}, "plan": {"identity": {"step": "none"}, "titles": [1]}},
            "audio": [{"x": 1}],
            "decision": {"decision": {"audio": {"y": 1}, "other": 1}}
        })
    }

    fn plan() -> Value {
        json!({"mode": "direct", "identity": {"step": "none"}, "titles": [1]})
    }

    fn pair(doc: Value) -> Vec<(String, Value)> {
        vec![(KEY.to_string(), doc)]
    }

    fn whole(state: &Value, own: &Value, saved_plan: &Value, saved: &Value) -> Result<Report, String> {
        compare(state, &pair(engine_doc(own.clone())), &json!({"plan": saved_plan}), &pair(saved.clone()), Mode::Whole)
    }

    fn refreshed() -> Value {
        let mut plan = plan();
        plan["evidence"] = json!({"sentences": [format!("{}step none, confidence 0.10", REFRESHED_IDENTITY)]});
        plan
    }

    #[test]
    fn equal_documents_give_no_difference() {
        let report = whole(&state(plan()), &doc(), &plan(), &doc()).unwrap();
        assert_eq!(report.lines, vec!["compare-job: no difference"]);
        assert_eq!(report.differences, 0);
    }

    #[test]
    fn a_plan_value_that_differs_is_one_line() {
        let mut saved = plan();
        saved["mode"] = json!("copy");
        let report = whole(&state(plan()), &doc(), &saved, &doc()).unwrap();
        assert_eq!(report.lines, vec!["plan: /mode: \"direct\" != \"copy\"", "compare-job: 1 difference(s)"]);
        assert_eq!(report.differences, 1);
    }

    #[test]
    fn a_decide_value_that_differs_names_its_title() {
        let mut saved = doc();
        saved["label"] = json!("y");
        let report = whole(&state(plan()), &doc(), &plan(), &saved).unwrap();
        assert_eq!(report.lines, vec!["decide t00-00800: /label: \"x\" != \"y\"", "compare-job: 1 difference(s)"]);
        assert_eq!(report.differences, 1);
    }

    #[test]
    fn the_two_extra_keys_are_dropped_and_required() {
        let report = whole(&state(plan()), &doc(), &plan(), &doc()).unwrap();
        assert_eq!(report.lines, vec!["compare-job: no difference"]);
        assert_eq!(report.differences, 0);
        for name in EXTRA_KEYS {
            let mut own = engine_doc(doc());
            own.as_object_mut().unwrap().remove(name);
            let result = compare(&state(plan()), &pair(own), &json!({"plan": plan()}), &pair(doc()), Mode::Whole);
            assert_eq!(result, Err(format!("t00-00800/decide.json lacks {}", name)));
        }
    }

    #[test]
    fn key_order_and_number_kind_count_as_the_diff_helper_reads_them() {
        let (a, b): (Value, Value) = (serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap(), serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap());
        let (mut left, mut right) = (doc(), doc());
        left["label"] = a;
        right["label"] = b;
        let report = whole(&state(plan()), &left, &plan(), &right).unwrap();
        assert_eq!(report.lines, vec!["compare-job: no difference"]);
        assert_eq!(report.differences, 0);
        left["label"] = serde_json::from_str("1").unwrap();
        right["label"] = serde_json::from_str("1.0").unwrap();
        let report = whole(&state(plan()), &left, &plan(), &right).unwrap();
        assert_eq!(report.lines, vec!["decide t00-00800: /label: 1 != 1.0", "compare-job: 1 difference(s)"]);
        assert_eq!(report.differences, 1);
    }

    #[test]
    fn a_refreshed_plan_compares_its_identity_only() {
        let mut saved_plan = plan();
        saved_plan["titles"] = json!([2]);
        let pending = PENDING_LINE.to_string();
        let mut own = doc();
        own["plan"]["slug"] = json!("own");
        own["plan"]["inputs"] = json!({"i": 2});
        own["plan"]["plan"]["titles"] = json!([2]);
        let report = whole(&state(refreshed()), &own, &saved_plan, &doc()).unwrap();
        assert_eq!(report.lines, vec![pending.clone(), "compare-job: no difference".to_string()]);
        assert_eq!(report.differences, 0);

        saved_plan["identity"]["step"] = json!("discdb_one");
        own["plan"]["plan"]["identity"]["step"] = json!("discdb_one");
        let report = whole(&state(refreshed()), &own, &saved_plan, &doc()).unwrap();
        assert_eq!(
            report.lines,
            vec![
                pending,
                "plan: /identity/step: \"none\" != \"discdb_one\"".to_string(),
                "decide t00-00800: /plan/plan/identity/step: \"discdb_one\" != \"none\"".to_string(),
                "compare-job: 2 difference(s)".to_string(),
            ]
        );
        assert_eq!(report.differences, 2);
    }

    #[test]
    fn plan_and_audio_compares_the_plan_and_the_two_audio_parts_only() {
        let mut own = doc();
        own["slug"] = json!("own");
        own["label"] = json!("own");
        own["subtitles"] = json!(["own"]);
        own["plan"]["slug"] = json!("own");
        own["decision"]["decision"]["other"] = json!(2);
        let run = |own: &Value| {
            compare(&state(plan()), &pair(engine_doc(own.clone())), &json!({"plan": plan()}), &pair(doc()), Mode::PlanAndAudio)
        };
        let report = run(&own).unwrap();
        assert_eq!(report.lines, vec![AUDIO_ONLY_LINE, "compare-job: no difference"]);
        assert_eq!(report.differences, 0);

        own["audio"][0]["x"] = json!(2);
        let report = run(&own).unwrap();
        assert_eq!(
            report.lines,
            vec!["decide t00-00800: /audio/0/x: 2 != 1".to_string(), AUDIO_ONLY_LINE.to_string(), "compare-job: 1 difference(s)".to_string()]
        );
        assert_eq!(report.differences, 1);
        own["audio"][0]["x"] = json!(1);
        own["decision"]["decision"]["audio"]["y"] = json!(2);
        let report = run(&own).unwrap();
        assert_eq!(
            report.lines,
            vec![
                "decide t00-00800: /decision/decision/audio/y: 2 != 1".to_string(),
                AUDIO_ONLY_LINE.to_string(),
                "compare-job: 1 difference(s)".to_string(),
            ]
        );
        assert_eq!(report.differences, 1);
    }

    #[test]
    fn plan_and_audio_compares_the_plan_too() {
        let mut saved_plan = plan();
        saved_plan["mode"] = json!("copy");
        let report = compare(
            &state(plan()),
            &pair(engine_doc(doc())),
            &json!({"plan": saved_plan}),
            &pair(doc()),
            Mode::PlanAndAudio,
        )
        .unwrap();
        assert_eq!(
            report.lines,
            vec!["plan: /mode: \"direct\" != \"copy\"".to_string(), AUDIO_ONLY_LINE.to_string(), "compare-job: 1 difference(s)".to_string()]
        );
        assert_eq!(report.differences, 1);
    }

    #[test]
    fn plan_and_audio_on_a_refreshed_plan_says_so_and_compares_its_identity_only() {
        let mut saved_plan = plan();
        saved_plan["titles"] = json!([2]);
        let report = compare(
            &state(refreshed()),
            &pair(engine_doc(doc())),
            &json!({"plan": saved_plan}),
            &pair(doc()),
            Mode::PlanAndAudio,
        )
        .unwrap();
        assert_eq!(report.lines, vec![PENDING_LINE, AUDIO_ONLY_LINE, "compare-job: no difference"]);
        assert_eq!(report.differences, 0);
    }

    #[test]
    fn plan_and_audio_names_a_title_it_was_given_no_pair_for() {
        let own = vec![(KEY.to_string(), engine_doc(doc())), ("t01-00801".to_string(), engine_doc(doc()))];
        let report = compare(
            &json!({"plan": plan(), "titles": [{"key": KEY}, {"key": "t01-00801"}]}),
            &own,
            &json!({"plan": plan()}),
            &pair(doc()),
            Mode::PlanAndAudio,
        )
        .unwrap();
        assert_eq!(report.lines, vec![AUDIO_ONLY_LINE, "not compared: title t01-00801", "compare-job: no difference"]);
        assert_eq!(report.differences, 0);
    }

    #[test]
    fn a_job_without_a_plan_and_a_pair_without_a_document_are_errors() {
        let run = |state: Value, saved_plan: Value, saved: Vec<(String, Value)>| {
            compare(&state, &pair(engine_doc(doc())), &saved_plan, &saved, Mode::Whole)
        };
        let plan_out = json!({"plan": plan()});
        assert_eq!(run(json!({"plan": null}), plan_out.clone(), pair(doc())), Err("the job has no plan".to_string()));
        assert_eq!(run(json!({}), plan_out.clone(), pair(doc())), Err("the job has no plan".to_string()));
        assert_eq!(
            run(state(plan()), json!({}), pair(doc())),
            Err("the saved plan output has no plan key".to_string())
        );
        let mut saved = pair(doc());
        saved.push(("t09-00999".to_string(), doc()));
        assert_eq!(run(state(plan()), plan_out.clone(), saved.clone()), Err("the job has no decide.json for t09-00999".to_string()));
        let report = compare(&state(plan()), &pair(engine_doc(doc())), &plan_out, &saved, Mode::PlanAndAudio);
        assert_eq!(report, Err("the job has no decide.json for t09-00999".to_string()));
    }

    #[test]
    fn whole_requires_a_pair_for_every_title_that_has_a_document() {
        let result = compare(&state(plan()), &pair(engine_doc(doc())), &json!({"plan": plan()}), &[], Mode::Whole);
        assert_eq!(result, Err("no --decide for t00-00800".to_string()));
    }

    #[test]
    fn a_title_of_state_json_without_a_document_gets_its_not_compared_line_in_both_modes() {
        let state = json!({"plan": plan(), "titles": [{"key": KEY}, {"key": "t05-00805"}]});
        let whole = compare(&state, &pair(engine_doc(doc())), &json!({"plan": plan()}), &pair(doc()), Mode::Whole).unwrap();
        assert_eq!(whole.lines, vec!["not compared: title t05-00805 has no decide.json", "compare-job: no difference"]);
        assert_eq!(whole.differences, 0);
        let audio = compare(&state, &pair(engine_doc(doc())), &json!({"plan": plan()}), &pair(doc()), Mode::PlanAndAudio).unwrap();
        assert_eq!(
            audio.lines,
            vec![AUDIO_ONLY_LINE, "not compared: title t05-00805 has no decide.json", "compare-job: no difference"]
        );
        assert_eq!(audio.differences, 0);
    }

    // The command (pull request 7's notes, calls 2 and 7).
    use crate::testutil::{call, Scratch, ScratchExt};

    const KEY2: &str = "t01-00801";

    fn compare_run(tmp: &Scratch, args: &[&str]) -> (i32, String, String) {
        call(run, &Ctx::for_test(&tmp.0), args)
    }

    /// A job folder `job` with two titles, KEY (a decide.json) and, when `second`, KEY2 too; the saved files `plan.json`
    /// and `KEY.json` (and `KEY2.json`).
    fn job(tag: &str, second: bool) -> Scratch {
        let tmp = Scratch::new(tag);
        let titles = if second { json!([{"key": KEY}, {"key": KEY2}]) } else { json!([{"key": KEY}]) };
        tmp.write("job/state.json", json!({"plan": plan(), "titles": titles}).to_string());
        tmp.write("plan.json", json!({"plan": plan()}).to_string());
        tmp.write(&format!("job/{KEY}/decide.json"), engine_doc(doc()).to_string());
        tmp.write(&format!("{KEY}.json"), doc().to_string());
        if second {
            tmp.write(&format!("job/{KEY2}/decide.json"), engine_doc(doc()).to_string());
            tmp.write(&format!("{KEY2}.json"), doc().to_string());
        }
        tmp
    }

    #[test]
    fn an_equal_job_exits_0_and_prints_no_difference() {
        let tmp = job("cj-equal", false);
        let (code, out, err) = compare_run(&tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json"]);
        assert_eq!((code, out.as_str(), err.as_str()), (0, "compare-job: no difference\n", ""));
    }

    #[test]
    fn a_difference_exits_1_with_its_line_on_stdout() {
        let tmp = job("cj-differ", false);
        let mut saved = doc();
        saved["label"] = json!("y");
        tmp.write(&format!("{KEY}.json"), saved.to_string());
        let (code, out, err) = compare_run(&tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json"]);
        assert_eq!(code, 1);
        assert_eq!(out, "decide t00-00800: /label: \"x\" != \"y\"\ncompare-job: 1 difference(s)\n");
        assert_eq!(err, "");
    }

    #[test]
    fn plan_and_audio_is_chosen_by_its_flag() {
        let tmp = job("cj-audio", false);
        let mut saved = doc();
        saved["label"] = json!("y");
        tmp.write(&format!("{KEY}.json"), saved.to_string());
        let (code, out, _) =
            compare_run(&tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json", "--plan-and-audio"]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("{AUDIO_ONLY_LINE}\ncompare-job: no difference\n"));
        // Without any --decide the plan-and-audio mode names the title it has no pair for.
        let (code, out, _) = compare_run(&tmp, &["job", "--plan", "plan.json", "--plan-and-audio"]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("{AUDIO_ONLY_LINE}\nnot compared: title t00-00800\ncompare-job: no difference\n"));
    }

    #[test]
    fn help_prints_the_usage_and_exits_0() {
        let tmp = Scratch::new("cj-help");
        for args in [&["-h"][..], &["--help"][..], &["job", "--plan", "p", "-h"][..]] {
            let (code, out, err) = compare_run(&tmp, args);
            assert_eq!((code, err.as_str()), (0, ""), "{args:?}");
            assert!(out.starts_with(&format!("usage: lazymkv-dev {USAGE}\n")), "{out}");
        }
    }

    #[test]
    fn each_argument_error_exits_2() {
        let tmp = job("cj-usage", false);
        for args in [
            &[][..],
            &["--plan", "plan.json"][..],
            &["job", "other", "--plan", "plan.json"][..],
            &["job"][..],
            &["job", "--plan", "plan.json", "--plan", "plan.json"][..],
            &["job", "--plan", "plan.json", "--decide", "t00-00800"][..],
            &["job", "--plan", "plan.json", "--decide", "=f.json"][..],
            &["job", "--plan", "plan.json", "--decide", "t00-00800="][..],
            &["job", "--plan", "plan.json", "--decide", "t00-00800=a.json", "--decide", "t00-00800=b.json"][..],
            &["job", "--plan", "plan.json", "--bogus"][..],
        ] {
            let (code, out, err) = compare_run(&tmp, args);
            assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
            assert!(err.starts_with(&format!("usage: lazymkv-dev {USAGE}\nlazymkv-dev compare-job: error: ")), "{args:?}: {err}");
        }
    }

    #[test]
    fn decide_splits_at_the_first_equals_sign() {
        let tmp = job("cj-equals", false);
        tmp.write("dir/a=b.json", doc().to_string());
        let (code, out, err) = compare_run(&tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=dir/a=b.json"]);
        assert_eq!((code, out.as_str(), err.as_str()), (0, "compare-job: no difference\n", ""));
    }

    #[test]
    fn an_unreadable_or_non_json_file_exits_2_and_names_it() {
        let tmp = job("cj-files", false);
        let go = |tmp: &Scratch| compare_run(tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json"]);
        tmp.write("plan.json", "no job x-1\n");
        let (code, out, err) = go(&tmp);
        assert_eq!((code, out.as_str()), (2, ""));
        assert!(err.starts_with("compare-job: plan.json: not JSON: "), "{err}");
        tmp.write("plan.json", "");
        assert_eq!(go(&tmp).2, "compare-job: plan.json: empty\n");
        let utf16: Vec<u8> = [0xFFu8, 0xFE].into_iter().chain("{}".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        tmp.write("plan.json", utf16);
        assert_eq!(
            go(&tmp).2,
            "compare-job: plan.json: not UTF-8 JSON (it looks like UTF-16; save the output as bytes, not through PowerShell's >)\n"
        );
        tmp.write("plan.json", json!({"plan": plan()}).to_string());
        std::fs::remove_file(tmp.0.join("t00-00800.json")).unwrap();
        let (code, _, err) = go(&tmp);
        assert_eq!(code, 2);
        assert!(err.starts_with("compare-job: t00-00800.json: "), "{err}");
        std::fs::remove_file(tmp.0.join("job/state.json")).unwrap();
        let (code, _, err) = go(&tmp);
        assert_eq!(code, 2);
        assert!(err.starts_with("compare-job: job/state.json: "), "{err}");
    }

    #[test]
    fn a_job_without_a_plan_exits_2() {
        let tmp = job("cj-noplan", false);
        tmp.write("job/state.json", json!({"titles": []}).to_string());
        let (code, out, err) = compare_run(&tmp, &["job", "--plan", "plan.json"]);
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", "compare-job: the job has no plan\n"));
    }

    #[test]
    fn a_title_without_decide_json_is_left_out_and_a_pair_for_it_is_an_error() {
        let tmp = job("cj-nodecide", true);
        std::fs::remove_file(tmp.0.join(format!("job/{KEY2}/decide.json"))).unwrap();
        let (code, out, err) = compare_run(&tmp, &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json"]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert_eq!(out, "not compared: title t01-00801 has no decide.json\ncompare-job: no difference\n");
        let (code, out, err) = compare_run(
            &tmp,
            &["job", "--plan", "plan.json", "--decide", "t00-00800=t00-00800.json", "--decide", "t01-00801=t01-00801.json"],
        );
        assert_eq!((code, out.as_str()), (2, ""));
        assert_eq!(err, "compare-job: the job has no decide.json for t01-00801\n");
    }
}
