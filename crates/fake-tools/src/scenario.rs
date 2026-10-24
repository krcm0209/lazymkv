//! The scenario file a stand-in executable replays.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub responses: Vec<Response>,
}

/// One reply of a stand-in executable (design part 12.2): which arguments it answers, what it prints to
/// standard output and standard error and when, how it ends, and which files it creates. A scenario with
/// several responses that match the same arguments hands them out one per run, in file order (see `select`).
/// Def: T10. Use: T11, T13, T14, T23.
#[derive(Debug, Deserialize)]
pub struct Response {
    /// Each string must be a substring of some argument.
    pub args_contain: Vec<String>,
    /// File in the scenario folder whose content is printed.
    #[serde(default)]
    pub stdout: Option<String>,
    /// File in the scenario folder with one `{"n":..,"t_ms":..}` record per printed line.
    #[serde(default)]
    pub timing: Option<String>,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub creates: Vec<Create>,
    /// File in the scenario folder replayed to standard error.
    #[serde(default)]
    pub stderr: Option<String>,
    /// The `{"n":..,"t_ms":..}` records of the standard error file, one per line.
    #[serde(default)]
    pub stderr_timing: Option<String>,
    /// Print this many standard output lines, then exit with `exit_code` without running `creates`.
    #[serde(default)]
    pub stop_after_line: Option<usize>,
    /// Print this many standard output lines, then sleep until killed.
    #[serde(default)]
    pub hang_after_line: Option<usize>,
}

/// A file a response creates (design part 12.2). Exactly one of `dir_arg`, `dir_arg_from_end` and `arg_after`
/// says where: the first two name the output folder (the file is `name` inside it) and `arg_after` names the
/// file itself, as the argument after `mkvmerge`'s `-o`.
/// Def: T10. Use: T11, T13, T14, T23.
#[derive(Debug, Deserialize)]
pub struct Create {
    /// Index of the argument that names the output directory.
    #[serde(default)]
    pub dir_arg: Option<usize>,
    /// Position of the argument that names the output directory, counted from the end (1 is the last argument).
    #[serde(default)]
    pub dir_arg_from_end: Option<usize>,
    /// The argument after the first argument equal to this flag is the file to create.
    #[serde(default)]
    pub arg_after: Option<String>,
    /// The file name inside the directory. Required with `dir_arg` and `dir_arg_from_end`, ignored with `arg_after`.
    #[serde(default)]
    pub name: Option<String>,
    /// File in the scenario folder whose bytes become the created file.
    pub copy_from: String,
}

impl Create {
    /// The path of the file to create for a run with `args` (design part 12.2), or the reason the locator does
    /// not fit them.
    /// Def: T10. Use: T11, T13, T14, T23.
    pub fn target(&self, args: &[String]) -> Result<PathBuf, String> {
        let locators = [self.dir_arg.is_some(), self.dir_arg_from_end.is_some(), self.arg_after.is_some()];
        if locators.iter().filter(|set| **set).count() != 1 {
            return Err("creates: give exactly one of dir_arg, dir_arg_from_end and arg_after".to_string());
        }
        if let Some(flag) = &self.arg_after {
            return args
                .iter()
                .position(|arg| arg == flag)
                .and_then(|at| args.get(at + 1))
                .map(PathBuf::from)
                .ok_or_else(|| format!("creates: there is no argument after {flag}"));
        }
        let name = self
            .name
            .as_deref()
            .ok_or_else(|| "creates: name is required with dir_arg and dir_arg_from_end".to_string())?;
        let dir = if let Some(at) = self.dir_arg {
            args.get(at).ok_or_else(|| format!("creates: there is no argument {at}"))?
        } else {
            let from_end = self.dir_arg_from_end.unwrap_or(0);
            let at = args.len().checked_sub(from_end).filter(|_| from_end > 0);
            at.map(|at| &args[at]).ok_or_else(|| format!("creates: there is no argument {from_end} from the end"))?
        };
        Ok(Path::new(dir).join(name))
    }
}

/// The response for a run with `args` (design part 12.2). Responses whose `args_contain` strings all appear in
/// the arguments match; the first match in file order answers the first run, the second match the second run,
/// and the last match every run after that. `calls` counts the runs by the file index of a group's first
/// match, as decimal strings, and is updated here. Nothing matching gives `None` and leaves `calls` alone.
/// Def: T10. Use: T11, T13, T14, T23.
pub fn select<'a>(
    scenario: &'a Scenario,
    args: &[String],
    calls: &mut BTreeMap<String, u64>,
) -> Option<&'a Response> {
    let matching: Vec<(usize, &Response)> = scenario
        .responses
        .iter()
        .enumerate()
        .filter(|(_, response)| {
            response
                .args_contain
                .iter()
                .all(|needle| args.iter().any(|arg| arg.contains(needle.as_str())))
        })
        .collect();
    let key = matching.first()?.0.to_string();
    let runs = calls.get(&key).copied().unwrap_or(0);
    let chosen = matching[runs.min(matching.len() as u64 - 1) as usize].1;
    calls.insert(key, runs + 1);
    Some(chosen)
}

/// The output stream a scheduled line belongs to (design part 12.2).
/// Def: T10. Use: T11, T13, T14, T23.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    /// Standard output.
    Out,
    /// Standard error.
    Err,
}

/// Both streams' lines in the order they are printed (design part 12.2), as `(stream, line index, time in
/// milliseconds)`. A line takes its time from `out_ms` or `err_ms`; with no record it takes the time of the
/// line before it in its own stream, or 0 for the first. A stream's own order is kept even if its times are
/// not increasing. The streams are merged by time, with standard output first when two times are equal.
/// Def: T10. Use: T11, T13, T14, T23.
pub fn schedule(out_ms: &[u64], out_lines: usize, err_ms: &[u64], err_lines: usize) -> Vec<(Stream, usize, u64)> {
    let times = |stream: Stream, ms: &[u64], lines: usize| {
        let mut last = 0;
        (0..lines)
            .map(|index| {
                last = ms.get(index).copied().unwrap_or(last).max(last);
                (stream, index, last)
            })
            .collect::<Vec<_>>()
    };
    let mut items = times(Stream::Out, out_ms, out_lines);
    items.extend(times(Stream::Err, err_ms, err_lines));
    items.sort_by_key(|(_, _, t_ms)| *t_ms);
    items
}

#[derive(Deserialize)]
struct TimingRecord {
    t_ms: u64,
}

/// Arrival time in milliseconds of each line, from a `.timing.jsonl` file.
pub fn parse_timing(text: &str) -> Vec<u64> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<TimingRecord>(line).ok())
        .map(|record| record.t_ms)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn the_first_response_whose_needles_all_appear_is_selected() {
        let scenario: Scenario = serde_json::from_str(
            r#"{"responses":[
                {"args_contain":["info","disc:9999"],"stdout":"drives.txt"},
                {"args_contain":["info"],"stdout":"scan.txt","exit_code":1},
                {"args_contain":["mkv"],"creates":[{"dir_arg":4,"name":"t.mkv","copy_from":"s.mkv"}]}
            ]}"#,
        )
        .unwrap();
        let mut calls = BTreeMap::new();

        let drives = select(&scenario, &args(&["-r", "--cache=1", "info", "disc:9999"]), &mut calls).unwrap();
        assert_eq!(drives.stdout.as_deref(), Some("drives.txt"));
        assert_eq!(drives.exit_code, 0);

        let scan = select(&scenario, &args(&["-r", "info", "disc:0"]), &mut calls).unwrap();
        assert_eq!(scan.stdout.as_deref(), Some("scan.txt"));
        assert_eq!(scan.exit_code, 1);

        let rip = select(&scenario, &args(&["-r", "mkv", "disc:0", "0", "out"]), &mut calls).unwrap();
        assert_eq!(rip.creates[0].dir_arg, Some(4));
        assert!(rip.stdout.is_none());

        assert!(select(&scenario, &args(&["backup"]), &mut calls).is_none());
    }

    #[test]
    fn timing_lines_give_each_line_its_arrival_time() {
        assert_eq!(
            parse_timing("{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":250}\nnot json\n"),
            vec![5, 250]
        );
    }

    #[test]
    fn repeated_arguments_take_the_next_matching_response_then_the_last_repeats() {
        let scenario: Scenario = serde_json::from_str(
            r#"{"responses":[
                {"args_contain":["mkv"],"exit_code":1},
                {"args_contain":["info"],"exit_code":5},
                {"args_contain":["mkv","disc:0"],"exit_code":0,"stdout":"rip.txt"}
            ]}"#,
        )
        .unwrap();
        let mut calls = BTreeMap::new();
        let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
        let codes: Vec<i32> = (0..3).map(|_| select(&scenario, &rip, &mut calls).unwrap().exit_code).collect();
        assert_eq!(codes, vec![1, 0, 0]);
        assert_eq!(select(&scenario, &args(&["info", "disc:0"]), &mut calls).unwrap().exit_code, 5);
        assert_eq!(calls, BTreeMap::from([("0".to_string(), 3), ("1".to_string(), 1)]));
    }

    #[test]
    fn each_locator_names_its_target() {
        let create = |json: &str| serde_json::from_str::<Create>(json).unwrap();
        let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
        assert_eq!(create(r#"{"dir_arg":4,"name":"t.mkv","copy_from":"s"}"#).target(&rip), Ok(Path::new("out").join("t.mkv")));
        assert_eq!(
            create(r#"{"dir_arg_from_end":1,"name":"t.mkv","copy_from":"s"}"#).target(&rip),
            Ok(Path::new("out").join("t.mkv"))
        );
        let remux = args(&["--gui-mode", "-o", "lib/x.mkv.partial", "raw.mkv"]);
        assert_eq!(
            create(r#"{"arg_after":"-o","name":"ignored","copy_from":"s"}"#).target(&remux),
            Ok(PathBuf::from("lib/x.mkv.partial"))
        );
    }

    #[test]
    fn a_create_with_two_locators_or_none_or_no_name_is_refused() {
        let create = |json: &str| serde_json::from_str::<Create>(json).unwrap();
        let rip = args(&["-r", "mkv", "disc:0", "0", "out"]);
        let both = "creates: give exactly one of dir_arg, dir_arg_from_end and arg_after";
        assert_eq!(create(r#"{"dir_arg":4,"arg_after":"-o","name":"t","copy_from":"s"}"#).target(&rip), Err(both.to_string()));
        assert_eq!(create(r#"{"name":"t","copy_from":"s"}"#).target(&rip), Err(both.to_string()));
        assert_eq!(
            create(r#"{"dir_arg":4,"copy_from":"s"}"#).target(&rip),
            Err("creates: name is required with dir_arg and dir_arg_from_end".to_string())
        );
        assert_eq!(
            create(r#"{"dir_arg_from_end":9,"name":"t","copy_from":"s"}"#).target(&rip),
            Err("creates: there is no argument 9 from the end".to_string())
        );
        assert_eq!(
            create(r#"{"arg_after":"-o","copy_from":"s"}"#).target(&rip),
            Err("creates: there is no argument after -o".to_string())
        );
    }

    #[test]
    fn stdout_and_stderr_lines_are_merged_by_their_times() {
        assert_eq!(
            schedule(&[0, 10, 30], 3, &[5, 30], 2),
            vec![(Stream::Out, 0, 0), (Stream::Err, 0, 5), (Stream::Out, 1, 10), (Stream::Out, 2, 30), (Stream::Err, 1, 30)]
        );
    }

    #[test]
    fn a_line_without_timing_keeps_the_time_of_the_line_before_it() {
        assert_eq!(
            schedule(&[0, 20], 3, &[], 1),
            vec![(Stream::Out, 0, 0), (Stream::Err, 0, 0), (Stream::Out, 1, 20), (Stream::Out, 2, 20)]
        );
    }
}
