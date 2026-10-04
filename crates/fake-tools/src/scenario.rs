//! The scenario file a stand-in executable replays.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub responses: Vec<Response>,
}

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
}

#[derive(Debug, Deserialize)]
pub struct Create {
    /// Index of the argument that names the output directory.
    pub dir_arg: usize,
    pub name: String,
    pub copy_from: String,
}

pub fn select<'a>(scenario: &'a Scenario, args: &[String]) -> Option<&'a Response> {
    scenario.responses.iter().find(|response| {
        response
            .args_contain
            .iter()
            .all(|needle| args.iter().any(|arg| arg.contains(needle.as_str())))
    })
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

        let drives = select(&scenario, &args(&["-r", "--cache=1", "info", "disc:9999"])).unwrap();
        assert_eq!(drives.stdout.as_deref(), Some("drives.txt"));
        assert_eq!(drives.exit_code, 0);

        let scan = select(&scenario, &args(&["-r", "info", "disc:0"])).unwrap();
        assert_eq!(scan.stdout.as_deref(), Some("scan.txt"));
        assert_eq!(scan.exit_code, 1);

        let rip = select(&scenario, &args(&["-r", "mkv", "disc:0", "0", "out"])).unwrap();
        assert_eq!(rip.creates[0].dir_arg, 4);
        assert!(rip.stdout.is_none());

        assert!(select(&scenario, &args(&["backup"])).is_none());
    }

    #[test]
    fn timing_lines_give_each_line_its_arrival_time() {
        assert_eq!(
            parse_timing("{\"n\":0,\"t_ms\":5}\n{\"n\":1,\"t_ms\":250}\nnot json\n"),
            vec![5, 250]
        );
    }
}
