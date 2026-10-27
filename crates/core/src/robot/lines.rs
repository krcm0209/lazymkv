//! The per-line reader of makemkvcon output (PIPE-10, design part 5.3, spec 9.4). The whole-text `robot::parse` still
//! reads the finished scan for the planner. No I/O (RP19).

use super::{message, split_fields, Drive, Message, Scan};
use crate::disc::TitleSource;

/// Which makemkvcon call the lines come from (design part 5.3, PIPE-10). Def: T5. Use: T13.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// The drive listing, `info disc:9999`.
    DriveList,
    /// A scan of one disc, `info disc:N`.
    Scan,
    /// A rip of one planned title: its index in the scan and how the index guard recognises it again.
    Rip { index: u32, source: TitleSource },
}

/// What one line tells the driver at once (design part 5.3). Def: T5. Use: T13.
#[derive(Clone, Debug, PartialEq)]
pub enum LineEvent {
    /// A `PRGV` line: the share of the work done, `total / max`.
    Progress { fraction: f64 },
    /// The index guard failed at the saving message (spec 6.2 step 6).
    IndexMismatch,
    /// One of spec 9.4's licence and key messages.
    Licence { code: u32, text: String },
    /// `MSG:1009`: MakeMKV rejected the profile.
    ProfileRejected { text: String },
}

/// What a whole run said (design part 5.3, spec 9.4). Def: T5. Use: T13, T23.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunSummary {
    /// The `N.N.N` after "MakeMKV v" in the first `MSG:1005`.
    pub version: Option<String>,
    /// `MSG:3344` appeared: MakeMKV used a Java runtime.
    pub java: bool,
    /// The text of the first `MSG:1009`.
    pub profile_rejected: Option<String>,
    /// The first licence or key message.
    pub licence: Option<Message>,
    /// The index guard's verdict: None until a rip's saving message, then whether the planned title kept its index.
    pub index_guard: Option<bool>,
    /// `MSG:2003` appeared.
    pub read_errors: bool,
    /// `MSG:2019` appeared.
    pub write_error: bool,
    /// Every `MSG:4047`, a forced-track removal.
    pub removed: Vec<Message>,
    /// A `MSG:5005` or `MSG:5036` reported exactly one title saved.
    pub title_saved: bool,
    /// A `MSG:5003`, `MSG:5004` or `MSG:5037` appeared.
    pub title_failed: bool,
    /// `MSG:5011` was followed by a `TCOUNT` line.
    pub scan_complete: bool,
    /// Every `DRV` line's fields, in order.
    pub drives: Vec<Drive>,
}

/// Spec 9.4's licence and key messages, which `LineReader` reports as `LineEvent::Licence`. Def: T5. Use: T5.
pub const LICENCE_CODES: [u32; 5] = [5021, 5050, 5052, 5055, 5095];

/// The per-line reader (PIPE-10); holds a scratch `Scan` so `robot::message` builds each Message. Def: T5. Use: T13.
pub struct LineReader {
    call: Call,
    scratch: Scan,
    summary: RunSummary,
    seen_version: bool,
    seen_5011: bool,
    /// Lines of the planned title's kind seen before the verdict: matching `3307` or `3308` lines on a Blu-ray, every
    /// `3028` line on a DVD.
    counted: u32,
    /// The index (Blu-ray) or DVD title number the guard read from the line it trusts.
    recorded: Option<u32>,
    verdict_fixed: bool,
}

impl LineReader {
    /// A reader for one `makemkvcon` call, with nothing read yet (design part 5.3, PIPE-10). Def: T5. Use: T13.
    pub fn new(call: Call) -> LineReader {
        LineReader {
            call,
            scratch: Scan::default(),
            summary: RunSummary::default(),
            seen_version: false,
            seen_5011: false,
            counted: 0,
            recorded: None,
            verdict_fixed: false,
        }
    }

    /// Reads one line, with or without its "\n" or "\r\n" and a leading byte order mark, and returns what it tells the
    /// driver at once (design part 5.3, PIPE-10). A malformed line and a kind not named there are skipped. Def: T5.
    /// Use: T13.
    pub fn feed(&mut self, line: &str) -> Vec<LineEvent> {
        let line = line.strip_suffix('\n').unwrap_or(line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let line = line.strip_prefix('\u{feff}').unwrap_or(line);
        let Some((kind, rest)) = line.split_once(':') else {
            return Vec::new();
        };
        match kind {
            "DRV" => {
                self.summary.drives.push(split_fields(rest));
                Vec::new()
            }
            "TCOUNT" => {
                self.summary.scan_complete = self.seen_5011;
                Vec::new()
            }
            "PRGV" => progress(rest).into_iter().collect(),
            "MSG" => {
                if message(&mut self.scratch, &split_fields(rest)).is_err() {
                    return Vec::new();
                }
                match self.scratch.messages.pop() {
                    Some(message) => self.on_message(message),
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    /// Ends the run and returns what it said (design part 5.3, spec 9.4). Def: T5. Use: T13.
    pub fn finish(self) -> RunSummary {
        self.summary
    }

    /// One `MSG` line, by its code. A code not named here has no effect (PIPE-14: `5010` on the drive listing among
    /// them).
    fn on_message(&mut self, message: Message) -> Vec<LineEvent> {
        let mut events = Vec::new();
        match message.code {
            1005 => {
                if !self.seen_version {
                    self.seen_version = true;
                    self.summary.version = version_of(&message);
                }
            }
            3344 => self.summary.java = true,
            1009 => {
                self.summary.profile_rejected.get_or_insert_with(|| message.text.clone());
                events.push(LineEvent::ProfileRejected { text: message.text });
            }
            code if LICENCE_CODES.contains(&code) => {
                events.push(LineEvent::Licence { code, text: message.text.clone() });
                self.summary.licence.get_or_insert(message);
            }
            2003 => self.summary.read_errors = true,
            2019 => self.summary.write_error = true,
            4047 => self.summary.removed.push(message),
            5005 | 5036 => {
                if message.params.first().map(String::as_str) == Some("1") {
                    self.summary.title_saved = true;
                }
            }
            5003 | 5004 | 5037 => self.summary.title_failed = true,
            5011 => self.seen_5011 = true,
            3307 | 3308 => self.note_file_title(&message),
            3028 => self.note_dvd_title(&message),
            5014 | 5015 => events.extend(self.fix_verdict()),
            _ => {}
        }
        events
    }

    /// `3307` or `3308` on a Blu-ray rip: the first line naming the planned file records the index it was added as.
    fn note_file_title(&mut self, message: &Message) {
        let Call::Rip { source: TitleSource::File { name }, .. } = &self.call else {
            return;
        };
        if self.verdict_fixed || message.params.first() != Some(name) {
            return;
        }
        if self.counted == 0 {
            self.recorded = message.params.get(1).and_then(|index| index.parse().ok());
        }
        self.counted = self.counted.saturating_add(1);
    }

    /// `3028` on a DVD rip: the (index + 1)-th line records the DVD title number it added.
    fn note_dvd_title(&mut self, message: &Message) {
        let Call::Rip { index, source: TitleSource::DvdTitle { .. } } = &self.call else {
            return;
        };
        if self.verdict_fixed {
            return;
        }
        if self.counted == *index {
            self.recorded = message.params.first().and_then(|number| number.trim().parse().ok());
        }
        self.counted = self.counted.saturating_add(1);
    }

    /// The first saving message, `5015` or `5014`, fixes the index guard's verdict (spec 6.2 step 6); later ones change
    /// nothing. Only a rip has a verdict. A mismatch is reported once, here.
    fn fix_verdict(&mut self) -> Option<LineEvent> {
        let Call::Rip { index, source } = &self.call else {
            return None;
        };
        if self.verdict_fixed {
            return None;
        }
        self.verdict_fixed = true;
        let expected = match source {
            TitleSource::File { .. } => Some(*index),
            TitleSource::DvdTitle { number } => Some(*number),
            TitleSource::Unknown => None,
        };
        let kept = expected.is_some_and(|expected| self.recorded == Some(expected));
        self.summary.index_guard = Some(kept);
        (!kept).then_some(LineEvent::IndexMismatch)
    }
}

/// A `PRGV` line's `[current, total, max]` as `total / max`, only when all three are numbers and `max` is above 0.
fn progress(rest: &str) -> Option<LineEvent> {
    let fields = split_fields(rest);
    let [current, total, max, ..] = fields.as_slice() else {
        return None;
    };
    current.parse::<u64>().ok()?;
    let (total, max) = (total.parse::<u64>().ok()?, max.parse::<u64>().ok()?);
    (max > 0).then(|| LineEvent::Progress { fraction: total as f64 / max as f64 })
}

/// The `N.N.N` of a `MSG:1005`: the run of non-space characters after "MakeMKV v" in the first parameter, else in the
/// text.
fn version_of(message: &Message) -> Option<String> {
    let first = message.params.first().map(String::as_str);
    first.into_iter().chain([message.text.as_str()]).find_map(version_after_prefix)
}

fn version_after_prefix(text: &str) -> Option<String> {
    let (_, rest) = text.split_once("MakeMKV v")?;
    let version: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disc::TitleSource;
    use crate::robot::split_fields;

    /// The public Pan's Labyrinth UHD scan of `corpus/scenarios/disc-a-scan` (CRLF line ends).
    const DISC_A: &[u8] = include_bytes!("../../../../corpus/scenarios/disc-a-scan/scan.txt");

    /// A MSG line as makemkvcon writes it: code, flags 0, the parameter count, the text, the text again as the format,
    /// then each parameter quoted.
    fn msg(code: u32, text: &str, params: &[&str]) -> String {
        let quoted: String = params.iter().map(|p| format!(",\"{p}\"")).collect();
        format!("MSG:{code},0,{},\"{text}\",\"{text}\"{quoted}", params.len())
    }

    fn read<S: AsRef<str>>(call: Call, lines: &[S]) -> (Vec<LineEvent>, RunSummary) {
        let mut reader = LineReader::new(call);
        let mut events = Vec::new();
        for line in lines {
            events.extend(reader.feed(line.as_ref()));
        }
        (events, reader.finish())
    }

    fn rip_file(index: u32) -> Call {
        Call::Rip { index, source: TitleSource::File { name: "00800.mpls".to_string() } }
    }

    fn added(code: u32, file: &str, n: &str) -> String {
        msg(code, "File %1 was added as title #%2", &[file, n])
    }

    fn saving() -> String {
        msg(5015, "Saving all titles to MKV files", &[])
    }

    #[test]
    fn a_scan_is_complete_only_with_5011_followed_by_a_tcount_line() {
        let text = String::from_utf8_lossy(DISC_A);
        let lines: Vec<&str> = text.lines().collect();
        let (_, summary) = read(Call::Scan, &lines);
        assert!(summary.scan_complete);
        assert_eq!(summary.version.as_deref(), Some("2.0.0"));
        assert_eq!(summary.drives.len(), 16);
        assert_eq!(summary.drives[0][6], "D:");
        let without: Vec<&str> = lines.iter().copied().filter(|line| !line.starts_with("TCOUNT")).collect();
        assert!(!read(Call::Scan, &without).1.scan_complete);
        let reversed = ["TCOUNT:1".to_string(), msg(5011, "Operation successfully completed", &[])];
        assert!(!read(Call::Scan, &reversed).1.scan_complete);
    }

    #[test]
    fn the_version_comes_from_msg_1005() {
        let line = msg(1005, "MakeMKV v1.17.0 win(x64-release) started", &["MakeMKV v1.17.0 win(x64-release)"]);
        assert_eq!(read(Call::DriveList, &[line]).1.version.as_deref(), Some("1.17.0"));
        assert_eq!(read(Call::DriveList, &[msg(1005, "started", &["something else"])]).1.version, None);
    }

    #[test]
    fn java_in_use_is_msg_3344() {
        let line = msg(3344, "Using Java runtime from %1", &["/opt/java/bin/java"]);
        assert!(read(Call::Scan, &[line]).1.java);
        assert!(!read(Call::Scan, &[msg(1005, "x", &[])]).1.java);
    }

    #[test]
    fn msg_1009_is_a_rejected_profile() {
        let (events, summary) = read(Call::Scan, &[msg(1009, "Profile parsing error: bad selection", &[])]);
        assert_eq!(events, vec![LineEvent::ProfileRejected { text: "Profile parsing error: bad selection".to_string() }]);
        assert_eq!(summary.profile_rejected.as_deref(), Some("Profile parsing error: bad selection"));
    }

    #[test]
    fn each_licence_code_is_reported() {
        assert_eq!(LICENCE_CODES, [5021, 5050, 5052, 5055, 5095]);
        for code in LICENCE_CODES {
            let (events, summary) = read(Call::Scan, &[msg(code, "key problem", &[])]);
            assert_eq!(events, vec![LineEvent::Licence { code, text: "key problem".to_string() }]);
            assert_eq!(summary.licence.map(|message| message.code), Some(code));
        }
    }

    #[test]
    fn the_index_guard_passes_when_the_planned_file_keeps_its_index() {
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "0"), saving()]);
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        assert_eq!(read(Call::Scan, &[added(3307, "00800.mpls", "0"), saving()]).1.index_guard, None);
    }

    #[test]
    fn the_index_guard_fails_when_3307_moves_the_planned_file() {
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "1"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_angle_form_3308_counts_as_an_added_title() {
        assert_eq!(read(rip_file(0), &[added(3308, "00800.mpls", "0"), saving()]).1.index_guard, Some(true));
        let (events, summary) = read(rip_file(0), &[added(3308, "00800.mpls", "1"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_dvd_guard_compares_the_3028_title_number_as_a_number() {
        let call = Call::Rip { index: 1, source: TitleSource::DvdTitle { number: 3 } };
        let dvd = |second: &str| {
            vec![
                msg(3028, "Title #%1 was added (%2 cell(s), %3)", &["1", "19", "2:01:14"]),
                msg(3028, "Title #%1 was added (%2 cell(s), %3)", &[second, "25", "1:26:26"]),
                saving(),
            ]
        };
        let (events, summary) = read(call.clone(), &dvd("3"));
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        assert_eq!(read(call.clone(), &dvd("03")).1.index_guard, Some(true));
        let (events, summary) = read(call, &dvd("4"));
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn a_missing_3307_before_the_saving_message_is_a_mismatch() {
        let (events, summary) = read(rip_file(0), &[saving(), added(3307, "00800.mpls", "0")]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn the_guard_verdict_is_fixed_at_the_first_saving_message_5015_or_5014() {
        let plain = msg(5014, "Saving %1 titles into directory %2", &["1", "x"]);
        let (events, summary) = read(rip_file(0), &[added(3307, "00800.mpls", "0"), plain.clone(), saving()]);
        assert!(events.is_empty());
        assert_eq!(summary.index_guard, Some(true));
        let (events, summary) = read(rip_file(0), &[plain, added(3307, "00800.mpls", "0"), saving()]);
        assert_eq!(events, vec![LineEvent::IndexMismatch]);
        assert_eq!(summary.index_guard, Some(false));
    }

    #[test]
    fn a_5036_with_one_title_is_a_saved_title() {
        let saved = |code: u32, n: &str| read(rip_file(0), &[msg(code, "Copy complete. %1 titles saved.", &[n])]).1.title_saved;
        assert!(saved(5036, "1"));
        assert!(saved(5005, "1"));
        assert!(!saved(5036, "2"));
        assert!(!saved(5005, "0"));
    }

    #[test]
    fn msg_5003_or_5004_with_5037_is_a_failed_title() {
        let failed = |codes: &[u32]| {
            let lines: Vec<String> = codes.iter().map(|code| msg(*code, "failed", &["1"])).collect();
            read(rip_file(0), &lines).1.title_failed
        };
        assert!(failed(&[5003]));
        assert!(failed(&[5004, 5037]));
        assert!(!failed(&[5036]));
    }

    #[test]
    fn msgs_2003_and_2019_set_read_and_write_errors() {
        let read_error = msg(2003, "Error '%1' occurred while reading '%2' at offset '%3'", &["Scsi error", "BD-RE", "1048576"]);
        let summary = read(rip_file(0), &[read_error]).1;
        assert!(summary.read_errors && !summary.write_error);
        let summary = read(rip_file(0), &[msg(2019, "Error writing", &[])]).1;
        assert!(!summary.read_errors && summary.write_error);
    }

    #[test]
    fn msg_4047_is_kept_as_a_removal() {
        let line = msg(
            4047,
            "AV sync issue in stream %1 at %2 : %3",
            &["2", "0:00:00", "Forced subtitles track #2 turned out to be empty and was removed from output file"],
        );
        let summary = read(rip_file(0), &[line]).1;
        assert_eq!(summary.removed.len(), 1);
        assert_eq!(summary.removed[0].params[0], "2");
    }

    #[test]
    fn prgv_gives_total_over_max() {
        let (events, _) = read(rip_file(0), &["PRGV:10,32768,65536", "PRGV:0,0,0", "PRGV:x,1,2"]);
        assert_eq!(events, vec![LineEvent::Progress { fraction: 0.5 }]);
    }

    #[test]
    fn msg_5010_on_the_drive_listing_is_ignored() {
        let drv = r#"DRV:0,2,999,12,"BD-RE SYNTHETIC DRIVE 1.00 SERIAL","SYNTHETIC_MOVIE","tok0""#;
        let (events, summary) = read(Call::DriveList, &[drv.to_string(), msg(5010, "Failed to open disc", &[])]);
        assert!(events.is_empty());
        let fields = ["0", "2", "999", "12", "BD-RE SYNTHETIC DRIVE 1.00 SERIAL", "SYNTHETIC_MOVIE", "tok0"];
        assert_eq!(summary, RunSummary { drives: vec![fields.map(String::from).to_vec()], ..RunSummary::default() });
        assert_eq!(split_fields(&drv[4..]), summary.drives[0]);
    }

    #[test]
    fn a_line_with_its_newline_and_a_byte_order_mark_is_read() {
        let line = format!("\u{feff}{}\r\n", msg(1005, "x", &["MakeMKV v2.0.0 win(x64-release)"]));
        assert_eq!(read(Call::Scan, &[line]).1.version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn a_malformed_line_is_skipped() {
        let (events, summary) = read(Call::Scan, &["MSG:x,0,0", "no colon", "PRGV:1"]);
        assert!(events.is_empty());
        assert_eq!(summary, RunSummary::default());
    }
}
