//! The job state reducer (design spec 10.1, RP12): a job's states, the events the engine feeds back and the actions it
//! performs, as one pure function `reduce(state, event) -> (state, actions)`; the state each waiting state returns to;
//! and 10.1's "Resuming after a restart" (`resume`). Spec 10.2's failure classes and where each one leads
//! (`failure_outcome`, `classify_title_failure`); spec 12's notification events; spec 5's job ID (`job_id`) and spec
//! 6.1 step 2's job lookup (`job_lookup`); and `decision_done`, which combines a job's per-title decisions into its one
//! `DecisionDone` event (6.4 step 12). Pure (RP19): nothing here persists a state, runs a program or sends a
//! notification. Phase 2's engine persists every transition before it performs its actions and feeds their results
//! back as events. Written from the spec (RP20). The tests are in `job/tests.rs`.

use std::str::FromStr;

use crate::decision::{Disposition, FileDecision};
use crate::plan::Mode;
use crate::remembered::Overrides;
use crate::review::{sorted_unique, Note, ReviewReason};

/// Spec 10.1's fixed waits: AwaitingNetwork "retried every 10 minutes", AwaitingLookup "retried hourly". Def: T14. Use: T14.
pub const NETWORK_RETRY_MIN: u32 = 10;
/// Spec 10.1: AwaitingLookup is "retried hourly"; the wait in minutes. Def: T14. Use: T14.
pub const LOOKUP_RETRY_MIN: u32 = 60;

/// Spec 10.2's classes. Def: T14. Use: T14, phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FailureClass {
    /// MakeMKV reports the drive cannot read this disc type: Failed, no retry.
    DriveIncapable,
    /// Read errors with the staging share healthy: Failed, resumable; inserting the disc again resumes it.
    MediaReadError,
    /// Key or evaluation messages: AwaitingMakeMkv.
    Licence,
    /// MakeMKV reports the disc needs a newer version: AwaitingMakeMkv.
    NeedsMakeMkvUpdate,
    /// MakeMKV cannot fetch a key it needs for this disc: AwaitingNetwork.
    NeedsNetwork,
    /// Share unreachable, out of space, or a write error: AwaitingShare or AwaitingSpace.
    Output,
    /// TheDiscDB or TMDb (The Movie Database) unreachable: AwaitingLookup.
    LookupUnavailable,
    /// A required program is not found or too old: AwaitingTool.
    ToolMissing,
    /// Anything else: Failed, with the full log and the exit code attached.
    Unknown,
}

impl FailureClass {
    /// Every class, in spec 10.2's order.
    pub const ALL: [FailureClass; 9] = [
        FailureClass::DriveIncapable,
        FailureClass::MediaReadError,
        FailureClass::Licence,
        FailureClass::NeedsMakeMkvUpdate,
        FailureClass::NeedsNetwork,
        FailureClass::Output,
        FailureClass::LookupUnavailable,
        FailureClass::ToolMissing,
        FailureClass::Unknown,
    ];

    /// Spec 10.2's spelling: the serde spelling and the `class` of spec 12's `failed` notification.
    pub fn as_str(self) -> &'static str {
        match self {
            FailureClass::DriveIncapable => "DriveIncapable",
            FailureClass::MediaReadError => "MediaReadError",
            FailureClass::Licence => "Licence",
            FailureClass::NeedsMakeMkvUpdate => "NeedsMakeMkvUpdate",
            FailureClass::NeedsNetwork => "NeedsNetwork",
            FailureClass::Output => "Output",
            FailureClass::LookupUnavailable => "LookupUnavailable",
            FailureClass::ToolMissing => "ToolMissing",
            FailureClass::Unknown => "Unknown",
        }
    }
}

impl FromStr for FailureClass {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<FailureClass, String> {
        FailureClass::ALL
            .into_iter()
            .find(|class| class.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a failure class (spec 10.2)"))
    }
}

/// What an Output failure was (10.2's Output row: "share unreachable, out of space, or a write error"). Def: T14. Use: T14, phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputCause {
    /// The staging share or the library share cannot be reached.
    ShareUnreachable,
    /// The share is short of space.
    OutOfSpace,
    /// A write failed with the share reachable (treated as a share fault, 10.2's last paragraph).
    WriteError,
}

impl OutputCause {
    /// Every cause, in the order 10.2's Output row names them.
    pub const ALL: [OutputCause; 3] = [OutputCause::ShareUnreachable, OutputCause::OutOfSpace, OutputCause::WriteError];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            OutputCause::ShareUnreachable => "share_unreachable",
            OutputCause::OutOfSpace => "out_of_space",
            OutputCause::WriteError => "write_error",
        }
    }
}

impl FromStr for OutputCause {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<OutputCause, String> {
        OutputCause::ALL
            .into_iter()
            .find(|cause| cause.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not an output failure cause (spec 10.2)"))
    }
}

/// A failing step's class, and for Output its cause (None for every other class). Def: T14. Use: T14, phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Failure {
    /// The class of 10.2 the engine found.
    pub class: FailureClass,
    /// For Output, what it was; None for every other class.
    pub output: Option<OutputCause>,
}

/// The programs 10.2's ToolMissing row names (4.4, plus ffmpeg of 6.4 step 10a). Def: T14. Use: T14, phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    /// MakeMKV's command-line program: scan, rip, whole-disc copy (4.4).
    Makemkvcon,
    /// MKVToolNix's mkvmerge: header checks, subtitle extraction, remux (4.4).
    Mkvmerge,
    /// FFmpeg's ffprobe: audio profiles and subtitle packet timings (4.4).
    Ffprobe,
    /// FFmpeg itself: the decode of 6.4 step 10a.
    Ffmpeg,
    /// The Java runtime MakeMKV uses to analyse Blu-ray menus (4.4, 8.3 rule 2).
    Java,
}

impl Tool {
    /// Every tool, in 4.4's order with ffmpeg after ffprobe.
    pub const ALL: [Tool; 5] = [Tool::Makemkvcon, Tool::Mkvmerge, Tool::Ffprobe, Tool::Ffmpeg, Tool::Java];

    /// The serde spelling: the program's name.
    pub fn as_str(self) -> &'static str {
        match self {
            Tool::Makemkvcon => "makemkvcon",
            Tool::Mkvmerge => "mkvmerge",
            Tool::Ffprobe => "ffprobe",
            Tool::Ffmpeg => "ffmpeg",
            Tool::Java => "java",
        }
    }
}

impl FromStr for Tool {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<Tool, String> {
        Tool::ALL
            .into_iter()
            .find(|tool| tool.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a tool (spec 4.4, 10.2)"))
    }
}

/// Why a job is held (10.1's two NeedsReview rows). Def: T14. Use: T14.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Held {
    /// "held after a disc copy with titles not yet extracted" (6.3 step 7).
    AfterCopy,
    /// "held at the disposition gate" (6.4 step 12).
    DispositionGate,
}

impl Held {
    /// Both holds, in 10.1's order.
    pub const ALL: [Held; 2] = [Held::AfterCopy, Held::DispositionGate];

    /// The serde spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Held::AfterCopy => "after_copy",
            Held::DispositionGate => "disposition_gate",
        }
    }
}

impl FromStr for Held {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<Held, String> {
        Held::ALL
            .into_iter()
            .find(|held| held.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a hold (spec 10.1)"))
    }
}

/// 10.1's states. `Planned` carries the mode the plan chose; `RipVerified` and `Ejected` whether they follow a whole-disc
/// copy (then the job is held next, 6.3 step 7) or a Direct rip (then it waits for the file-phase slot, spec 6); waiting
/// states carry the state they return to; `Decided` whether the owner approved the job (8.8); Failed carries its class
/// and the state that failed. Disc-phase states: Detected, Fingerprinted, Scanned, Planned, Ripping, Copying,
/// RipVerified. File-phase states: Ejected, Extracting, Inspecting, Decided, Remuxing, Verified.
/// Def: T14. Use: T14, phase 2.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state")]
pub enum State {
    /// 10.1 main line: media arrived and the job exists; the fingerprint runs (6.1 steps 1 and 2).
    Detected,
    /// 10.1 main line: the fingerprint is known; the scan and the database lookups run (6.1 steps 3 and 4).
    Fingerprinted,
    /// 10.1 main line: the scan finished; the plan is being made (6.1 step 5).
    Scanned,
    /// 10.1 main line: the plan is fixed with its mode (6.2 or 6.3); section 7's space check runs.
    Planned { mode: Mode },
    /// 10.1 main line: the disc's titles are being ripped (6.2 step 6).
    Ripping,
    /// 10.1 main line: the whole disc is being copied and the copy scanned (6.3 steps 6 and 6a).
    Copying,
    /// 10.1 main line ("Ripping (or Copying) → RipVerified → Ejected"): every planned title is ripped and verified
    /// (`after_copy` false, 6.2 steps 7 and 8), or the whole-disc copy is done and its scan lists titles (`after_copy`
    /// true, 6.3 steps 6, 6a and 7); the disc is ejected. Resuming ejects only, so a finished copy is never emptied and
    /// copied again.
    RipVerified { after_copy: bool },
    /// 10.1 main line: the disc is out; a Direct job waits for the file-phase slot, a copy job to be held (6.2 step 8,
    /// 6.3 step 7).
    Ejected { after_copy: bool },
    /// 10.1 main line: the staged files are probed and the decision made (6.4 steps 9 to 11).
    Inspecting,
    /// 10.1 main line: the decision is made, or is being made again after approval or re-finalise (6.4 steps 11 and 12).
    /// `approved` holds the review reasons the owner approved when entered by approval at the disposition gate (the
    /// reasons shown there) or by re-finalise (the filed decision's reasons; 8.8: the owner approved), and is None when
    /// entered from Inspecting with an AutoFile decision; an unapproved job whose decision, made again, needs review goes
    /// to the disposition gate (6.4 step 12). An approved job goes on whatever confidences its decision made again carries
    /// and whatever review reasons the owner approved (spec 11 Approve: continue with the reviewed decisions), but a
    /// reason the owner did not approve (one the decision made again with the overrides raised first, such as
    /// original_language_missing after Set identity; 6.4 step 12, 8.6 "The review reasons above hold the job
    /// regardless"), `path_collision` (spec 7: a file the job did not create is never overwritten) and a decision with no
    /// final path (`has_path` false, Open point 38) hold the job at the disposition gate again (Open point 39).
    Decided { approved: Option<Vec<ReviewReason>> },
    /// 10.1 main line: the partial final file is written and verified (6.4 steps 13 and 14).
    Remuxing,
    /// 10.1 main line: the partial file matched the decision; it is renamed into its final path (6.4 step 14).
    Verified,
    /// 10.1 main line's end: the file is in the library (6.4 step 15).
    Filed,
    /// 10.1: held for the owner (spec 11), after a disc copy or at the disposition gate.
    NeedsReview { held: Held },
    /// 10.1: titles are being extracted from a disc copy after approval (6.3 step 8).
    Extracting,
    /// 10.1: the staging or library share is unreachable (section 7, 10.2 Output).
    AwaitingShare { return_to: Box<State> },
    /// 10.1: a share is short of space (section 7, 10.2 Output).
    AwaitingSpace { return_to: Box<State> },
    /// 10.1: a licence or version message; back when MakeMKV's version or key changes (10.2 Licence, NeedsMakeMkvUpdate).
    AwaitingMakeMkv { return_to: Box<State> },
    /// 10.1: the disc needs a key from the internet; retried every 10 minutes (10.2 NeedsNetwork).
    AwaitingNetwork { return_to: Box<State> },
    /// 10.1: a database answer the decision needs is pending; retried hourly (10.2 LookupUnavailable).
    AwaitingLookup { return_to: Box<State> },
    /// 10.1: a required program is missing or too old; back when the self-test finds it (10.2 ToolMissing).
    AwaitingTool { return_to: Box<State> },
    /// 10.1's Failed(class): `from` is the state whose step failed (10.2's Result column).
    Failed { class: FailureClass, from: Box<State> },
    /// 10.1: the owner discarded the job and its staging data (spec 11 Discard).
    Discarded,
}

impl State {
    /// Spec 10.1's name of the state, which is also its serde tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Detected => "Detected",
            State::Fingerprinted => "Fingerprinted",
            State::Scanned => "Scanned",
            State::Planned { .. } => "Planned",
            State::Ripping => "Ripping",
            State::Copying => "Copying",
            State::RipVerified { .. } => "RipVerified",
            State::Ejected { .. } => "Ejected",
            State::Inspecting => "Inspecting",
            State::Decided { .. } => "Decided",
            State::Remuxing => "Remuxing",
            State::Verified => "Verified",
            State::Filed => "Filed",
            State::NeedsReview { .. } => "NeedsReview",
            State::Extracting => "Extracting",
            State::AwaitingShare { .. } => "AwaitingShare",
            State::AwaitingSpace { .. } => "AwaitingSpace",
            State::AwaitingMakeMkv { .. } => "AwaitingMakeMkv",
            State::AwaitingNetwork { .. } => "AwaitingNetwork",
            State::AwaitingLookup { .. } => "AwaitingLookup",
            State::AwaitingTool { .. } => "AwaitingTool",
            State::Failed { .. } => "Failed",
            State::Discarded => "Discarded",
        }
    }

    /// Detected, Fingerprinted, Scanned, Planned, Ripping, Copying, RipVerified: the states that need the disc (6.1 to 6.3).
    pub fn is_disc_phase(&self) -> bool {
        matches!(
            self,
            State::Detected
                | State::Fingerprinted
                | State::Scanned
                | State::Planned { .. }
                | State::Ripping
                | State::Copying
                | State::RipVerified { .. }
        )
    }

    /// Ejected, Extracting, Inspecting, Decided, Remuxing, Verified: the states of 6.4 and 6.3 step 8, which work from
    /// staging and no longer need the disc.
    pub fn is_file_phase(&self) -> bool {
        matches!(
            self,
            State::Ejected { .. }
                | State::Extracting
                | State::Inspecting
                | State::Decided { .. }
                | State::Remuxing
                | State::Verified
        )
    }

    /// The six Awaiting states of 10.1, each of which stores the state it returns to.
    pub fn is_waiting(&self) -> bool {
        matches!(
            self,
            State::AwaitingShare { .. }
                | State::AwaitingSpace { .. }
                | State::AwaitingMakeMkv { .. }
                | State::AwaitingNetwork { .. }
                | State::AwaitingLookup { .. }
                | State::AwaitingTool { .. }
        )
    }
}

/// The `reason` of spec 12's `waiting` event. Def: T14. Use: T14, phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitReason {
    /// AwaitingShare: a share is unreachable.
    Share,
    /// AwaitingSpace: a share is short of space.
    Space,
    /// AwaitingMakeMkv: a licence or version message.
    Makemkv,
    /// AwaitingNetwork: the disc needs a key from the internet.
    Network,
    /// AwaitingLookup: a database answer is pending.
    Lookup,
    /// AwaitingTool: a required program is missing.
    Tool,
}

impl WaitReason {
    /// Every reason, in spec 12's order.
    pub const ALL: [WaitReason; 6] = [
        WaitReason::Share,
        WaitReason::Space,
        WaitReason::Makemkv,
        WaitReason::Network,
        WaitReason::Lookup,
        WaitReason::Tool,
    ];

    /// Spec 12's spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            WaitReason::Share => "share",
            WaitReason::Space => "space",
            WaitReason::Makemkv => "makemkv",
            WaitReason::Network => "network",
            WaitReason::Lookup => "lookup",
            WaitReason::Tool => "tool",
        }
    }
}

impl FromStr for WaitReason {
    type Err = String;

    /// Only the exact spellings of `as_str` are accepted.
    fn from_str(text: &str) -> Result<WaitReason, String> {
        WaitReason::ALL
            .into_iter()
            .find(|reason| reason.as_str() == text)
            .ok_or_else(|| format!("\"{text}\" is not a waiting reason (spec 12)"))
    }
}

/// Spec 12's notification events, a closed set (the engine adds job_id, fingerprint, title, drive, message, time). Def: T14. Use: T14, phase 2.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum NotifyEvent {
    /// The disc phase succeeded and the disc was ejected; processing continues.
    DiscDone,
    /// A known disc was inserted and ejected.
    AlreadyRipped,
    /// A file was placed in the library.
    Filed,
    /// A job was held, with its review reasons.
    NeedsReview { reasons: Vec<ReviewReason> },
    /// A job entered an Awaiting state, with the reason.
    Waiting { reason: WaitReason },
    /// A waiting job continued.
    Resumed,
    /// A job failed, with its class.
    Failed { class: FailureClass },
    /// The disc should be removed by hand.
    EjectFailed,
}

impl NotifyEvent {
    /// Spec 12's `event` field: the serde tag.
    pub fn as_str(&self) -> &'static str {
        match self {
            NotifyEvent::DiscDone => "disc_done",
            NotifyEvent::AlreadyRipped => "already_ripped",
            NotifyEvent::Filed => "filed",
            NotifyEvent::NeedsReview { .. } => "needs_review",
            NotifyEvent::Waiting { .. } => "waiting",
            NotifyEvent::Resumed => "resumed",
            NotifyEvent::Failed { .. } => "failed",
            NotifyEvent::EjectFailed => "eject_failed",
        }
    }
}

/// 10.1's events, plus the engine's answers the reducer needs to walk the main line (SpaceOk for the space checks of 7,
/// FilePhaseSlot for spec 6's one-job file phase, which phase 2 also grants by delivering ReviewApproved, ReFinalise, or
/// Retry of a job that failed in its file phase; HoldForReview for 6.3 step 7's hold after a copy; FileDone for the rename
/// of 6.4 step 14). Def: T14. Use: T14, phase 2.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event")]
pub enum Event {
    /// Media arrived in a drive (6.1 step 1).
    MediaArrived,
    /// The disc was removed.
    MediaRemoved,
    /// 6.1 step 2's fingerprint finished.
    FingerprintDone,
    /// 6.1 step 2's fingerprint failed, with its 10.2 class.
    FingerprintFailed { failure: Failure },
    /// 6.1 step 4's scan finished.
    ScanDone,
    /// 6.1 step 4's scan failed, with its 10.2 class.
    ScanFailed { failure: Failure },
    /// 6.1 step 5's plan is fixed, with its mode (6.2 or 6.3).
    PlanDone { mode: Mode },
    /// 6.1 step 5's plan failed, with its 10.2 class.
    PlanFailed { failure: Failure },
    /// Section 7's space check passed.
    SpaceOk,
    /// Section 7's space check failed, or space ran out.
    SpaceShort,
    /// Section 7: space was freed; AwaitingSpace returns.
    SpaceFreed,
    /// 6.2 step 6: a title's rip finished.
    TitleRipped,
    /// 6.2 step 6: a title's rip failed, with its 10.2 class (`classify_title_failure`).
    TitleRipFailed { failure: Failure },
    /// `last`: no planned (or assigned) title is left to rip or extract.
    TitleVerified { last: bool },
    /// 6.2 step 7: a mismatch; the first is re-ripped, the second goes on (the engine records verify_failed for the decision).
    TitleVerifyFailed { second: bool, last: bool },
    /// `hash_errors`: MSG:5079 (6.3 step 6); the engine carries copy_hash_errors into HoldForReview's reasons.
    CopyDone { scan_listed_titles: bool, hash_errors: bool },
    /// 6.3 step 6: the copy failed, with its 10.2 class; `second`: this job's copy failed once before (6.3's last paragraph).
    CopyFailed { failure: Failure, second: bool },
    /// 6.3 step 8: a title's extraction from the copy finished.
    TitleExtracted,
    /// 6.3 step 8: a title's extraction failed, with its 10.2 class.
    ExtractFailed { failure: Failure },
    /// 6.2 step 8 and 6.3 step 7: the disc was ejected.
    Ejected,
    /// 6.2 step 8 and 6.3 step 7: the eject failed; the disc is removed by hand (10.3, 12).
    EjectFailed,
    /// 6.3 step 7: the copy job enters NeedsReview with these reasons.
    HoldForReview { reasons: Vec<ReviewReason> },
    /// Spec 6: the one file-phase slot is free for this Direct job in Ejected. Phase 2 also grants the slot by delivering
    /// ReviewApproved, ReFinalise, or Retry of a job that failed in its file phase.
    FilePhaseSlot,
    /// 6.4 step 9's probe finished.
    ProbeDone,
    /// 6.4 step 9's probe failed, with its 10.2 class.
    ProbeFailed { failure: Failure },
    /// 6.4 step 11's decision for the whole job, with 8.8's disposition and review reasons: phase 2's engine builds it
    /// with `decision_done` over every FileDecision of the job (after `decision::same_path_reviews`), never from one
    /// title's decision. `has_path` is true only when every title built a final path; false is Task 13's Open point 38
    /// case, where `decide` gives NeedsReview whenever a path is None, and a job with no final path for a title is never
    /// filed (it is held at the disposition gate whatever its disposition).
    DecisionDone { disposition: Disposition, reasons: Vec<ReviewReason>, has_path: bool },
    /// 6.4 step 11's decision failed, with its 10.2 class.
    DecisionFailed { failure: Failure },
    /// 6.4 step 13's remux finished.
    RemuxDone,
    /// 6.4 step 13's remux failed, with its 10.2 class.
    RemuxFailed { failure: Failure },
    /// 6.4 step 14: the partial file matched the decision.
    FileVerified,
    /// 6.4 step 14: the partial file's verification failed, with its 10.2 class.
    FileVerifyFailed { failure: Failure },
    /// 6.4 step 14: the partial file was renamed into its final path.
    FileDone,
    /// 6.4 step 14: the rename into the final path failed, with its 10.2 class.
    FileFailed { failure: Failure },
    /// Spec 10.1 "share unreachable"; section 7.
    ShareLost,
    /// Spec 10.1: the share is reachable again; AwaitingShare returns (section 7).
    ShareBack,
    /// Spec 10.1 "licence message" (10.2 Licence). Taken only in Scanned, Ripping and Copying; a message printed by the scan
    /// itself (the job is then Fingerprinted) is reported as ScanFailed with class Licence, NeedsMakeMkvUpdate or
    /// NeedsNetwork, which waits the same way.
    LicenceMessage,
    /// Spec 10.1 "version message" (10.2 NeedsMakeMkvUpdate). Taken only in Scanned, Ripping and Copying; a message printed
    /// by the scan itself (the job is then Fingerprinted) is reported as ScanFailed with class Licence, NeedsMakeMkvUpdate or
    /// NeedsNetwork, which waits the same way.
    VersionMessage,
    /// The installed MakeMKV version or key changed (checked at engine start and at each disc insertion).
    MakeMkvChanged,
    /// The disc needs a key from the internet. Taken only in Scanned, Ripping and Copying; a message printed by the scan
    /// itself (the job is then Fingerprinted) is reported as ScanFailed with class Licence, NeedsMakeMkvUpdate or
    /// NeedsNetwork, which waits the same way.
    NetworkNeeded,
    /// AwaitingNetwork's 10-minute timer ran out.
    NetworkRetry,
    /// A database answer the decision needs is still pending (or, in AwaitingLookup, still pending at the hourly retry).
    LookupPending,
    /// The pending database answer arrived; AwaitingLookup returns.
    LookupAnswered,
    /// 10.2 ToolMissing: the self-test or a step found `tool` missing or too old.
    ToolMissing { tool: Tool },
    /// The self-test found `tool`; AwaitingTool returns.
    ToolFound { tool: Tool },
    /// Spec 11 Approve, with the owner's review decisions and the review reasons shown at the gate when the owner
    /// approved (`approved_reasons`: the held decision's reasons, which the decision made again may carry without
    /// holding the job; 6.4 step 12). Phase 2 delivers it only once spec 6's file-phase slot is free for this job, as
    /// FilePhaseSlot is for a Direct job in Ejected; the job holds the slot until it reaches Filed, NeedsReview, Failed or
    /// Discarded.
    ReviewApproved { overrides: Overrides, approved_reasons: Vec<ReviewReason> },
    /// Spec 11 Discard: accepted from NeedsReview and from Failed of every class (spec 11's Discard deletes the job's
    /// staging data, 4.5 `discard`, and spec 7 never deletes a held or failed job's data automatically, so a failed job's
    /// data is freed only this way), to Discarded with DeleteJobData.
    ReviewDiscarded,
    /// Spec 11 Retry, for a failed job. For a job that failed in its file phase, phase 2 delivers it only once spec 6's
    /// file-phase slot is free for this job, as for ReviewApproved.
    Retry,
    /// Spec 11 Rip again: arm a new job for a disc that was already ripped.
    RipAgain,
    /// Spec 11 Re-finalise, from history, with the review reasons of the decision the job was filed with
    /// (`approved_reasons`, the engine's job record): filing approved them, and a reason the decision made again raises
    /// beyond them holds the job (6.4 step 12). Phase 2 delivers it only once spec 6's file-phase slot is free for this
    /// job, as FilePhaseSlot is for a Direct job in Ejected; the job holds the slot until it reaches Filed, NeedsReview,
    /// Failed or Discarded.
    ReFinalise { approved_reasons: Vec<ReviewReason> },
    /// The engine started again; `disc_present` says whether the job's disc is in a drive.
    EngineRestart { disc_present: bool },
}

/// 10.1's actions, plus Fingerprint and Plan (the main line implies them: 10.1's list omits them), RefreshIdentity (8.1:
/// the answered lookups refresh the plan's identity before a file-phase step), CheckSpace (section 7), the resume
/// housekeeping of "Resuming after a restart", and ArmRipAgain (6.1 step 2: the engine marks the job so the next
/// insertion of its fingerprint starts a new job). RunScan also starts 6.1 step 3's database lookups beside the scan.
/// Def: T14. Use: T14, phase 2.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action")]
pub enum Action {
    /// 6.1 step 2: the fingerprint, the disc ID, the names and the navigation files.
    Fingerprint,
    /// 6.1 step 4, with 6.1 step 3's lookups beside it.
    RunScan,
    /// 6.1 step 5.
    Plan,
    /// 8.1 after AwaitingLookup: the answered lookups refresh the plan's identity (`plan::refresh_identity`, which keeps
    /// the ripped titles); phase 2 persists the refreshed plan and passes it to the next Decide
    /// (`decision::DecisionInputs.plan`), so the answers the job waited for reach the decision.
    RefreshIdentity,
    /// Section 7's space check for the next step.
    CheckSpace,
    /// 6.2 step 6, the next planned title (or the same one again after a first verification failure).
    RipTitle,
    /// 6.3 step 6 and 6a.
    CopyDisc,
    /// 6.3 step 8, the next assigned title.
    ExtractTitle,
    /// 6.2 step 7 (a rip or an extraction) or 6.4 step 14 (the partial final file).
    VerifyFile,
    /// 6.2 step 8, 6.3 step 7.
    Eject,
    /// 6.4 step 9.
    Probe,
    /// 6.4 step 10 and 10a.
    ExtractSubtitleEvidence,
    /// 6.4 step 11.
    Decide,
    /// 6.4 step 13.
    Remux,
    /// 6.4 step 14's rename into the final path.
    File,
    /// Spec 12.
    Notify { event: NotifyEvent },
    /// The retry timer of AwaitingNetwork or AwaitingLookup.
    StartTimer { minutes: u32 },
    /// Spec 11 Discard: the job's staging data.
    DeleteJobData,
    /// Before a rip or an extraction runs again (10.1 "Resuming after a restart").
    EmptyTitleDirectory,
    /// Before a whole-disc copy runs again.
    EmptyCopyDirectory,
    /// Before a remux runs again.
    DeletePartialFile,
    /// A disc-phase job whose disc is not in a drive waits for it. Phase 2 performs a disc-phase action only with the job's
    /// disc in a drive; when the disc is gone it performs none and feeds MediaRemoved, which gives [WaitForDisc]. It does not
    /// feed the media removal its own Eject causes.
    WaitForDisc,
    /// A note on the job (6.3 step 6a's switch to Direct mode, 10.2's Java note).
    RecordNote { note: Note },
    /// 6.1 step 2: the next insertion of this fingerprint starts a new job.
    ArmRipAgain,
}

/// An event the state does not take. Def: T14. Use: T14.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    /// The state the event reached.
    pub state: State,
    /// The event no row of the table holds for that state.
    pub event: Event,
}

/// 10.2's Result column: DriveIncapable, MediaReadError and Unknown give Failed { class, from }; Licence and
/// NeedsMakeMkvUpdate AwaitingMakeMkv; NeedsNetwork AwaitingNetwork; Output with OutOfSpace AwaitingSpace, with
/// ShareUnreachable, WriteError or None AwaitingShare (a write error is treated as a share fault, 10.2's last paragraph);
/// LookupUnavailable AwaitingLookup; ToolMissing AwaitingTool; each waiting state returning to `from`. Def: T14. Use: T14.
pub fn failure_outcome(failure: Failure, from: &State) -> State {
    let back = Box::new(from.clone());
    match failure.class {
        FailureClass::DriveIncapable | FailureClass::MediaReadError | FailureClass::Unknown => {
            State::Failed { class: failure.class, from: back }
        }
        FailureClass::Licence | FailureClass::NeedsMakeMkvUpdate => State::AwaitingMakeMkv { return_to: back },
        FailureClass::NeedsNetwork => State::AwaitingNetwork { return_to: back },
        FailureClass::Output => match failure.output {
            Some(OutputCause::OutOfSpace) => State::AwaitingSpace { return_to: back },
            Some(OutputCause::ShareUnreachable) | Some(OutputCause::WriteError) | None => {
                State::AwaitingShare { return_to: back }
            }
        },
        FailureClass::LookupUnavailable => State::AwaitingLookup { return_to: back },
        FailureClass::ToolMissing => State::AwaitingTool { return_to: back },
    }
}

/// 10.2's last paragraph, checked in this order: the share unreachable gives Output { ShareUnreachable }; out of space
/// Output { OutOfSpace }; a write error Output { WriteError }; read errors seen, a stalled read, or a process that did not
/// exit (10.2's table, 10.4) with the share healthy MediaReadError: pass `read_errors` true for each; else Unknown.
/// Def: T14. Use: T14, phase 2.
pub fn classify_title_failure(
    share_healthy: bool,
    out_of_space: bool,
    write_error: bool,
    read_errors: bool,
) -> Failure {
    let output = |cause| Failure { class: FailureClass::Output, output: Some(cause) };
    if !share_healthy {
        output(OutputCause::ShareUnreachable)
    } else if out_of_space {
        output(OutputCause::OutOfSpace)
    } else if write_error {
        output(OutputCause::WriteError)
    } else if read_errors {
        Failure { class: FailureClass::MediaReadError, output: None }
    } else {
        Failure { class: FailureClass::Unknown, output: None }
    }
}

/// The job's `DecisionDone` from the FileDecisions of all its titles (6.4 steps 11 and 12, 8.8: the disposition is
/// the job's): NeedsReview when any title's disposition is NeedsReview, or when there is no decision at all, else
/// AutoFile; the reasons are the sorted union of every decision's reviews (`review::sorted_unique`); `has_path` is true
/// only when there is a decision and every one built a final path. Phase 2's engine builds the event with this function
/// after `decision::same_path_reviews` has compared the titles' paths, so a multi-title job with one pathless title is
/// held at the disposition gate, never filed (Open points 38 and 39, Task 13's pointer that the job's disposition is
/// combined from its titles'). Spec 6.4 step 12 ("A job with any review reason, or any confidence below
/// `auto_threshold`, enters NeedsReview"). Def: final review (H5). Use: phase 2.
pub fn decision_done(decisions: &[FileDecision]) -> Event {
    let held = decisions.is_empty() || decisions.iter().any(|decision| decision.disposition == Disposition::NeedsReview);
    Event::DecisionDone {
        disposition: if held { Disposition::NeedsReview } else { Disposition::AutoFile },
        reasons: sorted_unique(decisions.iter().flat_map(|decision| decision.reviews.iter().copied()).collect()),
        has_path: !decisions.is_empty() && decisions.iter().all(|decision| decision.path.is_some()),
    }
}

/// A new job's first state and actions: (Detected, [Fingerprint]) (6.1 steps 1 and 2; the engine creates it on MediaArrived
/// when `job_lookup` gives NewJob, or before the fingerprint is known). Def: T14. Use: phase 2.
pub fn new_job() -> (State, Vec<Action>) {
    (State::Detected, vec![Action::Fingerprint])
}

/// "Resuming after a restart" (and the actions of a waiting state's return): Detected [Fingerprint]; Fingerprinted
/// [RunScan]; Scanned [Plan]; Planned [CheckSpace]; Decided { .. } [Decide] (an approved Decided may be entered with
/// Decide pending, after approval or re-finalise; header-only steps simply run again, 10.1, and a job whose decision,
/// made again, needs review is gated again unless the owner approved it: an approved job is held again only for a
/// reason the owner did not approve, `path_collision` or a decision with no final path (`has_path` false), spec 7, 6.4
/// step 12, Open point 39); Ripping
/// [EmptyTitleDirectory, RipTitle]; Extracting [EmptyTitleDirectory, ExtractTitle]; Copying [EmptyCopyDirectory,
/// CopyDisc]; RipVerified (either after_copy) [Eject]; Inspecting [Probe]; Remuxing [DeletePartialFile, Remux]; Verified [File]; Ejected nothing (it waits for FilePhaseSlot or
/// HoldForReview); a disc-phase state with `disc_present` false [WaitForDisc]; waiting, held, failed, filed and discarded
/// states nothing, except AwaitingNetwork [StartTimer { minutes: NETWORK_RETRY_MIN }] and AwaitingLookup
/// [StartTimer { minutes: LOOKUP_RETRY_MIN }] (10.1's "retried every 10 minutes" and "retried hourly": a restart loses
/// the engine's timers). The state is returned unchanged. Def: T14. Use: T14, phase 2.
pub fn resume(state: &State, disc_present: bool) -> (State, Vec<Action>) {
    if state.is_disc_phase() && !disc_present {
        return (state.clone(), vec![Action::WaitForDisc]);
    }
    let actions = match state {
        State::Detected => vec![Action::Fingerprint],
        State::Fingerprinted => vec![Action::RunScan],
        State::Scanned => vec![Action::Plan],
        State::Planned { .. } => vec![Action::CheckSpace],
        State::Decided { .. } => vec![Action::Decide],
        State::Ripping => vec![Action::EmptyTitleDirectory, Action::RipTitle],
        State::Extracting => vec![Action::EmptyTitleDirectory, Action::ExtractTitle],
        State::Copying => vec![Action::EmptyCopyDirectory, Action::CopyDisc],
        State::RipVerified { .. } => vec![Action::Eject],
        State::Inspecting => vec![Action::Probe],
        State::Remuxing => vec![Action::DeletePartialFile, Action::Remux],
        State::Verified => vec![Action::File],
        State::AwaitingNetwork { .. } => vec![Action::StartTimer { minutes: NETWORK_RETRY_MIN }],
        State::AwaitingLookup { .. } => vec![Action::StartTimer { minutes: LOOKUP_RETRY_MIN }],
        State::Ejected { .. }
        | State::Filed
        | State::NeedsReview { .. }
        | State::AwaitingShare { .. }
        | State::AwaitingSpace { .. }
        | State::AwaitingMakeMkv { .. }
        | State::AwaitingTool { .. }
        | State::Failed { .. }
        | State::Discarded => Vec::new(),
    };
    (state.clone(), actions)
}

/// (new state, actions) for every row of the reducer table (spec 10.1 and 10.2, as the plan's Shared interfaces list it
/// under `core::job`); every other pair is Rejected. Def: T14. Use: phase 2.
///
/// The table's rows are grouped into the rules of `RULES`; each pair of a state and an event is held by at most one rule
/// (`no_state_and_event_pair_is_held_by_two_rules`), so the order of `RULES` is no precedence.
pub fn reduce(state: &State, event: &Event) -> Result<(State, Vec<Action>), Rejected> {
    RULES
        .iter()
        .find_map(|rule| rule(state, event))
        .ok_or_else(|| Rejected { state: state.clone(), event: event.clone() })
}

/// One group of rows of the reducer table: Some((new state, actions)) for a pair the group holds, None otherwise.
type Rule = fn(&State, &Event) -> Option<(State, Vec<Action>)>;

/// The reducer table, group by group.
const RULES: &[Rule] = &[any_state, direct_line, copy_line, review_line, waits, failures, disc_presence];

/// A notification action.
fn notify(event: NotifyEvent) -> Action {
    Action::Notify { event }
}

/// The rows for any state: EngineRestart gives `resume(state, disc_present)`; ToolMissing { Java } leaves the state and
/// records the note that label rule 8.3.2 does not apply (10.2's ToolMissing row).
fn any_state(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    match event {
        Event::EngineRestart { disc_present } => Some(resume(state, *disc_present)),
        Event::ToolMissing { tool: Tool::Java } => {
            Some((state.clone(), vec![Action::RecordNote { note: Note::LabelRuleUnavailable }]))
        }
        _ => None,
    }
}

/// The main line in Direct mode (10.1's first line; 6.1, 6.2 and 6.4): fingerprint, scan, plan, space, rip and verify each
/// planned title (a first verification failure re-rips the same title, 6.2 step 7), eject, the file-phase slot, probe,
/// subtitle evidence and decide, the disposition gate, space, remux, verify, file.
fn direct_line(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    let next = match (state, event) {
        (State::Detected, Event::FingerprintDone) => (State::Fingerprinted, vec![Action::RunScan]),
        (State::Fingerprinted, Event::ScanDone) => (State::Scanned, vec![Action::Plan]),
        (State::Scanned, Event::PlanDone { mode }) => (State::Planned { mode: *mode }, vec![Action::CheckSpace]),
        (State::Planned { mode: Mode::Direct }, Event::SpaceOk) => (State::Ripping, vec![Action::RipTitle]),
        (State::Ripping, Event::TitleRipped) => (State::Ripping, vec![Action::VerifyFile]),
        (State::Ripping, Event::TitleVerified { last: false })
        | (State::Ripping, Event::TitleVerifyFailed { second: true, last: false })
        | (State::Ripping, Event::TitleVerifyFailed { second: false, .. }) => (State::Ripping, vec![Action::RipTitle]),
        (State::Ripping, Event::TitleVerified { last: true })
        | (State::Ripping, Event::TitleVerifyFailed { second: true, last: true }) => {
            (State::RipVerified { after_copy: false }, vec![Action::Eject])
        }
        (State::RipVerified { after_copy: false }, Event::Ejected) => {
            (State::Ejected { after_copy: false }, vec![notify(NotifyEvent::DiscDone)])
        }
        (State::RipVerified { after_copy: false }, Event::EjectFailed) => {
            (State::Ejected { after_copy: false }, vec![notify(NotifyEvent::EjectFailed)])
        }
        (State::Ejected { after_copy: false }, Event::FilePhaseSlot) => (State::Inspecting, vec![Action::Probe]),
        (State::Inspecting, Event::ProbeDone) => {
            (State::Inspecting, vec![Action::ExtractSubtitleEvidence, Action::Decide])
        }
        (State::Inspecting, Event::DecisionDone { disposition: Disposition::AutoFile, has_path: true, .. }) => {
            (State::Decided { approved: None }, vec![Action::CheckSpace])
        }
        // A decision with no final path is never filed (Open point 38): held at the gate whatever its disposition.
        (
            State::Inspecting,
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons, .. }
            | Event::DecisionDone { has_path: false, reasons, .. },
        ) => (
            State::NeedsReview { held: Held::DispositionGate },
            vec![notify(NotifyEvent::NeedsReview { reasons: reasons.clone() })],
        ),
        (State::Decided { .. }, Event::SpaceOk) => (State::Remuxing, vec![Action::Remux]),
        (State::Remuxing, Event::RemuxDone) => (State::Remuxing, vec![Action::VerifyFile]),
        (State::Remuxing, Event::FileVerified) => (State::Verified, vec![Action::File]),
        (State::Verified, Event::FileDone) => (State::Filed, vec![notify(NotifyEvent::Filed)]),
        _ => return None,
    };
    Some(next)
}

/// The `reason` of spec 12's `waiting` notification for a waiting state; None for every other state.
fn wait_reason(state: &State) -> Option<WaitReason> {
    match state {
        State::AwaitingShare { .. } => Some(WaitReason::Share),
        State::AwaitingSpace { .. } => Some(WaitReason::Space),
        State::AwaitingMakeMkv { .. } => Some(WaitReason::Makemkv),
        State::AwaitingNetwork { .. } => Some(WaitReason::Network),
        State::AwaitingLookup { .. } => Some(WaitReason::Lookup),
        State::AwaitingTool { .. } => Some(WaitReason::Tool),
        _ => None,
    }
}

/// Entering a waiting state: StartTimer { NETWORK_RETRY_MIN } for AwaitingNetwork or StartTimer { LOOKUP_RETRY_MIN } for
/// AwaitingLookup (10.1's retries), then Notify { Waiting { reason } } (spec 12 "a job entered an Awaiting state").
fn entering(waiting: State) -> (State, Vec<Action>) {
    let mut actions = Vec::new();
    match waiting {
        State::AwaitingNetwork { .. } => actions.push(Action::StartTimer { minutes: NETWORK_RETRY_MIN }),
        State::AwaitingLookup { .. } => actions.push(Action::StartTimer { minutes: LOOKUP_RETRY_MIN }),
        _ => {}
    }
    if let Some(reason) = wait_reason(&waiting) {
        actions.push(notify(NotifyEvent::Waiting { reason }));
    }
    (waiting, actions)
}

/// The table's "Outcome": `failure_outcome(failure, from)`, with [Notify { Failed { class } }] for a Failed state and the
/// actions of `entering` for a waiting state.
fn outcome(failure: Failure, from: &State) -> (State, Vec<Action>) {
    match failure_outcome(failure, from) {
        State::Failed { class, from } => {
            (State::Failed { class, from }, vec![notify(NotifyEvent::Failed { class })])
        }
        waiting => entering(waiting),
    }
}

/// Whole-disc copy mode (6.3, 10.1 and D18): a Copy plan copies the disc; a copy whose scan lists titles enters
/// RipVerified { after_copy: true } and ejects (10.1's main line "Ripping (or Copying) → RipVerified → Ejected", so a
/// restart or a Return before the eject ejects and never empties the finished copy to copy again), and is held for
/// review; a copy whose scan failed, or a second read failure of the copy, switches to Direct mode with a note and checks
/// space for the titles (6.3 step 6a); a first copy failure is an Outcome (a read failure: Failed { MediaReadError }).
/// After approval each assigned title is extracted and verified as in 6.2 step 7 (6.3 step 8: a first verification failure
/// extracts the same title again), and the last one goes on to Inspecting.
fn copy_line(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    let direct_fallback = || {
        (
            State::Planned { mode: Mode::Direct },
            vec![Action::RecordNote { note: Note::DirectModeFallback }, Action::CheckSpace],
        )
    };
    let next = match (state, event) {
        (State::Planned { mode: Mode::Copy }, Event::SpaceOk) => (State::Copying, vec![Action::CopyDisc]),
        (State::Copying, Event::CopyDone { scan_listed_titles: true, .. }) => {
            (State::RipVerified { after_copy: true }, vec![Action::Eject])
        }
        (State::Copying, Event::CopyDone { scan_listed_titles: false, .. }) => direct_fallback(),
        (State::Copying, Event::CopyFailed { failure, second: true })
            if failure.class == FailureClass::MediaReadError =>
        {
            direct_fallback()
        }
        (State::Copying, Event::CopyFailed { failure, .. }) => outcome(*failure, state),
        (State::RipVerified { after_copy: true }, Event::Ejected) => {
            (State::Ejected { after_copy: true }, vec![notify(NotifyEvent::DiscDone)])
        }
        (State::RipVerified { after_copy: true }, Event::EjectFailed) => {
            (State::Ejected { after_copy: true }, vec![notify(NotifyEvent::EjectFailed)])
        }
        (State::Ejected { after_copy: true }, Event::HoldForReview { reasons }) => (
            State::NeedsReview { held: Held::AfterCopy },
            vec![notify(NotifyEvent::NeedsReview { reasons: reasons.clone() })],
        ),
        (State::NeedsReview { held: Held::AfterCopy }, Event::ReviewApproved { .. }) => {
            (State::Extracting, vec![Action::CheckSpace])
        }
        (State::Extracting, Event::SpaceOk) => (State::Extracting, vec![Action::ExtractTitle]),
        (State::Extracting, Event::TitleExtracted) => (State::Extracting, vec![Action::VerifyFile]),
        (State::Extracting, Event::TitleVerified { last: false })
        | (State::Extracting, Event::TitleVerifyFailed { second: true, last: false })
        | (State::Extracting, Event::TitleVerifyFailed { second: false, .. }) => {
            (State::Extracting, vec![Action::ExtractTitle])
        }
        (State::Extracting, Event::TitleVerified { last: true })
        | (State::Extracting, Event::TitleVerifyFailed { second: true, last: true }) => {
            (State::Inspecting, vec![Action::Probe])
        }
        _ => return None,
    };
    Some(next)
}

/// The review queue's rows (10.1, spec 11): approving at the disposition gate decides again with the owner's overrides
/// (the engine passes them to Decide) and records the reasons the owner approved, and a decision after approval goes on
/// whatever its disposition and confidences and whatever reasons the owner approved (spec 11 Approve: continue with the
/// reviewed decisions; 8.8: only a review decision raises a confidence), except that it is held at the disposition gate
/// again, with the same notification, when it carries a reason the owner did not approve (6.4 step 12), raises
/// `path_collision` (spec 7: the job never overwrites a file it did not create, and a re-finalise whose recorded file
/// no longer matches raises it) or built no final path (`has_path` false, Open points 38 and 39); a decision made again in an
/// unapproved job (an auto-filed job after a restart, a Return or a Retry) goes on only when it files automatically with
/// a final path and is otherwise held at the disposition gate (6.4 step 12; a job with no final path is never AutoFile);
/// Discard deletes the job's data, of a held job (10.1's table) or of a failed one of any class (spec 11's Discard, the
/// 10.1 row extended by the final review's ruling: spec 7 never deletes a failed job's data automatically, so this is
/// how the owner frees it); Re-finalise takes a filed job back to a Decided that approves the filed decision's reasons;
/// Rip again arms a new job for a
/// filed, held or file-phase job, one that failed in its file phase, or a waiting job returning to a file-phase state
/// (6.1 step 2 counts each as past the disc phase), and leaves its state.
fn review_line(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    let next = match (state, event) {
        (State::NeedsReview { held: Held::DispositionGate }, Event::ReviewApproved { approved_reasons, .. }) => {
            (State::Decided { approved: Some(approved_reasons.clone()) }, vec![Action::Decide])
        }
        (State::Decided { approved: Some(approved) }, Event::DecisionDone { reasons, has_path, .. })
            if !blocks_filing(reasons, *has_path, approved) =>
        {
            (state.clone(), vec![Action::CheckSpace])
        }
        (
            State::Decided { approved: None },
            Event::DecisionDone { disposition: Disposition::AutoFile, has_path: true, .. },
        ) => (state.clone(), vec![Action::CheckSpace]),
        (State::Decided { approved: Some(approved) }, Event::DecisionDone { reasons, has_path, .. })
            if blocks_filing(reasons, *has_path, approved) =>
        {
            (
                State::NeedsReview { held: Held::DispositionGate },
                vec![notify(NotifyEvent::NeedsReview { reasons: reasons.clone() })],
            )
        }
        (
            State::Decided { approved: None },
            Event::DecisionDone { disposition: Disposition::NeedsReview, reasons, .. }
            | Event::DecisionDone { has_path: false, reasons, .. },
        ) => (
            State::NeedsReview { held: Held::DispositionGate },
            vec![notify(NotifyEvent::NeedsReview { reasons: reasons.clone() })],
        ),
        (State::NeedsReview { .. } | State::Failed { .. }, Event::ReviewDiscarded) => {
            (State::Discarded, vec![Action::DeleteJobData])
        }
        (State::Filed, Event::ReFinalise { approved_reasons }) => {
            (State::Decided { approved: Some(approved_reasons.clone()) }, vec![Action::Decide])
        }
        (State::Filed | State::NeedsReview { .. }, Event::RipAgain) => (state.clone(), vec![Action::ArmRipAgain]),
        (State::Failed { from, .. }, Event::RipAgain) if from.is_file_phase() => {
            (state.clone(), vec![Action::ArmRipAgain])
        }
        (_, Event::RipAgain) if state.is_file_phase() => (state.clone(), vec![Action::ArmRipAgain]),
        (_, Event::RipAgain) if waiting_return(state).is_some_and(State::is_file_phase) => {
            (state.clone(), vec![Action::ArmRipAgain])
        }
        _ => return None,
    };
    Some(next)
}

/// Whether a decision made again in an approved job cannot be filed: it carries a review reason not in `approved` (the
/// reasons the owner approved at the gate, or the filed decision's for a re-finalise), so the owner never saw it (6.4
/// step 12; 8.6 "The review reasons above hold the job regardless": Set identity to a Japanese film raises
/// original_language_missing on a disc without Japanese audio); it raises `path_collision`, approved or not (spec 7: a
/// file the job did not create is never overwritten, and a re-finalise whose recorded file no longer matches raises
/// it); or it built no final path (`has_path` false, Open point 38). Open point 39: the engine does not file such a job;
/// 6.4 step 12 holds it at the disposition gate again. A confidence below `auto_threshold` is no reason to stop after
/// spec 11's Approve ("continue with the reviewed decisions"; 8.8: only a review decision raises a confidence).
fn blocks_filing(reasons: &[ReviewReason], has_path: bool, approved: &[ReviewReason]) -> bool {
    !has_path
        || reasons.contains(&ReviewReason::PathCollision)
        || reasons.iter().any(|reason| !approved.contains(reason))
}

/// The state a waiting state returns to; None for every other state.
fn waiting_return(state: &State) -> Option<&State> {
    match state {
        State::AwaitingShare { return_to }
        | State::AwaitingSpace { return_to }
        | State::AwaitingMakeMkv { return_to }
        | State::AwaitingNetwork { return_to }
        | State::AwaitingLookup { return_to }
        | State::AwaitingTool { return_to } => Some(&**return_to),
        _ => None,
    }
}

/// The table's "Return": the waiting state's `return_to`, with `resume(return_to, true)`'s actions followed by
/// Notify { Resumed } (spec 12 "a waiting job continued"). Phase 2 performs a disc-phase action only with the job's disc
/// in a drive; when the disc is gone it performs none and feeds MediaRemoved, which gives [WaitForDisc]. It does not feed
/// the media removal its own Eject causes.
fn back_to(return_to: &State) -> (State, Vec<Action>) {
    let (state, mut actions) = resume(return_to, true);
    actions.push(notify(NotifyEvent::Resumed));
    (state, actions)
}

/// 10.2's ToolMissing row for every tool but Java (which `any_state` holds): without makemkvcon the disc phase and an
/// extraction wait; without mkvmerge every disc- and file-phase state waits ("no disc is started"); without ffprobe or
/// ffmpeg the file phase waits and the disc phase runs on, unchanged ("the disc phase runs and the file phase waits").
fn tool_missing(state: &State, tool: Tool) -> Option<(State, Vec<Action>)> {
    let waits = match tool {
        Tool::Makemkvcon => state.is_disc_phase() || matches!(state, State::Extracting),
        Tool::Mkvmerge => state.is_disc_phase() || state.is_file_phase(),
        Tool::Ffprobe | Tool::Ffmpeg => {
            if state.is_disc_phase() {
                return Some((state.clone(), Vec::new()));
            }
            state.is_file_phase()
        }
        Tool::Java => false,
    };
    waits.then(|| entering(State::AwaitingTool { return_to: Box::new(state.clone()) }))
}

/// The waiting states (10.1, section 7, 10.2): entering each from the states its row names, the hourly retry of a lookup
/// still pending, and the Return of each when its condition clears; AwaitingLookup's Return to a file-phase state starts
/// with RefreshIdentity (8.1: the answered lookups reach the decision).
fn waits(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    let working = state.is_disc_phase() || state.is_file_phase();
    let here = || Box::new(state.clone());
    match (state, event) {
        (_, Event::ShareLost) if working => Some(entering(State::AwaitingShare { return_to: here() })),
        (_, Event::SpaceShort) if working => Some(entering(State::AwaitingSpace { return_to: here() })),
        (State::Scanned | State::Ripping | State::Copying, Event::LicenceMessage | Event::VersionMessage) => {
            Some(entering(State::AwaitingMakeMkv { return_to: here() }))
        }
        (State::Scanned | State::Ripping | State::Copying, Event::NetworkNeeded) => {
            Some(entering(State::AwaitingNetwork { return_to: here() }))
        }
        (State::Ejected { .. } | State::Inspecting, Event::LookupPending) => {
            Some(entering(State::AwaitingLookup { return_to: here() }))
        }
        (State::AwaitingLookup { .. }, Event::LookupPending) => {
            Some((state.clone(), vec![Action::StartTimer { minutes: LOOKUP_RETRY_MIN }]))
        }
        (_, Event::ToolMissing { tool }) => tool_missing(state, *tool),
        (State::AwaitingShare { return_to }, Event::ShareBack)
        | (State::AwaitingSpace { return_to }, Event::SpaceFreed)
        | (State::AwaitingMakeMkv { return_to }, Event::MakeMkvChanged)
        | (State::AwaitingNetwork { return_to }, Event::NetworkRetry)
        | (State::AwaitingTool { return_to }, Event::ToolFound { .. }) => Some(back_to(return_to)),
        // 8.1: the answers the job waited for refresh the plan's identity before the file phase's next action, so the
        // Decide that follows uses them (`plan::refresh_identity`). A disc-phase step that waited (a lookup failure
        // before the plan is fixed) plans with the answers itself.
        (State::AwaitingLookup { return_to }, Event::LookupAnswered) => {
            let (state, mut actions) = back_to(return_to);
            if return_to.is_file_phase() {
                actions.insert(0, Action::RefreshIdentity);
            }
            Some((state, actions))
        }
        _ => None,
    }
}

/// The failure an event reports when `state` is the state whose step it ran (10.1's "each finishing or failing (with a
/// failure class)"): the fingerprint in Detected, the scan in Fingerprinted, the plan in Scanned, a title's rip in Ripping,
/// an extraction in Extracting, the probe in Inspecting, the decision in Inspecting or (after approval or re-finalise)
/// Decided, the remux and the partial file's verification in Remuxing, the rename in Verified. A copy's failure is
/// `copy_line`'s.
fn step_failure(state: &State, event: &Event) -> Option<Failure> {
    match (state, event) {
        (State::Detected, Event::FingerprintFailed { failure })
        | (State::Fingerprinted, Event::ScanFailed { failure })
        | (State::Scanned, Event::PlanFailed { failure })
        | (State::Ripping, Event::TitleRipFailed { failure })
        | (State::Extracting, Event::ExtractFailed { failure })
        | (State::Inspecting, Event::ProbeFailed { failure })
        | (State::Inspecting | State::Decided { .. }, Event::DecisionFailed { failure })
        | (State::Remuxing, Event::RemuxFailed { failure })
        | (State::Remuxing, Event::FileVerifyFailed { failure })
        | (State::Verified, Event::FileFailed { failure }) => Some(*failure),
        _ => None,
    }
}

/// Failures and their way back (10.1, 10.2): a step's failure is the table's Outcome; Retry takes a failed job back to
/// the state that failed with `resume(from, true)`'s actions, except a DriveIncapable failure (10.2 "no retry"); the disc
/// inserted again (either drive) resumes a MediaReadError failure as a Return. Phase 2 performs a disc-phase action only
/// with the job's disc in a drive; when the disc is gone it performs none and feeds MediaRemoved, which gives
/// [WaitForDisc].
fn failures(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    if let Some(failure) = step_failure(state, event) {
        return Some(outcome(failure, state));
    }
    match (state, event) {
        (State::Failed { class, from }, Event::Retry) if *class != FailureClass::DriveIncapable => {
            Some(resume(from, true))
        }
        (State::Failed { class: FailureClass::MediaReadError, from }, Event::MediaArrived) => Some(back_to(from)),
        _ => None,
    }
}

/// The disc leaving or coming back during the disc phase (10.1 "A disc-phase state whose disc is no longer in the drive
/// waits for the disc"): MediaRemoved leaves the state and waits; MediaArrived leaves it and resumes it.
fn disc_presence(state: &State, event: &Event) -> Option<(State, Vec<Action>)> {
    if !state.is_disc_phase() {
        return None;
    }
    match event {
        Event::MediaRemoved => Some((state.clone(), vec![Action::WaitForDisc])),
        Event::MediaArrived => Some(resume(state, true)),
        _ => None,
    }
}

/// Spec 5 and K3: `<fingerprint in lower case>-<n>`, n one more than the highest n among the `earlier` IDs that
/// start with the lower-cased fingerprint and '-' (an upper-case ID, an ID of another fingerprint and a malformed ID
/// are ignored), 1 for the first. Lower case keeps the ID a valid slug (`audio::labels::is_slug`), so a job folder
/// reads as a recording (design part 7.1); the fingerprint itself stays upper case in meta.json. Def: T14 (phase 1),
/// K3 in T2. Use: T22.
pub fn job_id(fingerprint: &str, earlier: &[&str]) -> String {
    let fp = fingerprint.to_ascii_lowercase();
    let highest = earlier
        .iter()
        .filter_map(|id| id.strip_prefix(fp.as_str())?.strip_prefix('-'))
        .filter_map(job_number)
        .max()
        .unwrap_or(0);
    format!("{fp}-{}", u64::from(highest) + 1)
}

/// The n of a job ID's last part: one or more ASCII digits, at most u32::MAX; None for anything else ("", "x", "3-2",
/// "+4", " 4", a number above u32::MAX).
fn job_number(digits: &str) -> Option<u32> {
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Spec 6.1 step 2's three outcomes. Def: T14. Use: phase 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobLookup {
    /// The job with this ID resumes (only titles not yet verified are ripped).
    Resume(String),
    /// The job with this ID is past its disc phase: send `already_ripped` and eject.
    AlreadyRipped(String),
    /// A new job starts (and applies any remembered decisions for the fingerprint).
    NewJob,
}

/// Spec 6.1 step 2 over the earlier jobs of this fingerprint as (job id, state, rip again armed); the one with the highest n
/// decides: Resume when it has not finished its disc phase (a disc-phase state, or a waiting state returning to one) or is
/// Failed { MediaReadError } from a disc-phase state; AlreadyRipped when it is past the disc phase (a file-phase state,
/// NeedsReview, Filed, a waiting state returning to a file-phase state, or Failed of any class from a file-phase state)
/// and not armed; NewJob otherwise (armed, so an armed Rip again of a file-phase failure starts a new rip; Discarded;
/// Failed with another class from a disc-phase state; or no earlier job). A job that failed in its disc phase with
/// DriveIncapable or Unknown is left for the owner to Retry (Unknown only) or Discard (spec 11): the disc inserted again
/// starts a new job beside it. An ID whose last part after '-' is not a job number is skipped. Def: T14. Use: phase 2.
pub fn job_lookup(earlier: &[(String, State, bool)]) -> JobLookup {
    let latest = earlier
        .iter()
        .filter_map(|(id, state, armed)| {
            let n = job_number(id.rsplit_once('-')?.1)?;
            Some((n, id, state, *armed))
        })
        .max_by_key(|(n, ..)| *n);
    let Some((_, id, state, armed)) = latest else {
        return JobLookup::NewJob;
    };
    let returns_to = waiting_return(state);
    let unfinished = state.is_disc_phase()
        || returns_to.is_some_and(State::is_disc_phase)
        || matches!(state, State::Failed { class: FailureClass::MediaReadError, from } if from.is_disc_phase());
    let past = state.is_file_phase()
        || matches!(state, State::NeedsReview { .. } | State::Filed)
        || returns_to.is_some_and(State::is_file_phase)
        || matches!(state, State::Failed { from, .. } if from.is_file_phase());
    if unfinished {
        JobLookup::Resume(id.clone())
    } else if past && !armed {
        JobLookup::AlreadyRipped(id.clone())
    } else {
        JobLookup::NewJob
    }
}

#[cfg(test)]
mod tests;
