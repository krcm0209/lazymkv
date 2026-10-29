//! The pure half of design part 4.2's drive watcher (Ruling W8), compiled and tested on every system: the drive id,
//! the scan's filter, whether a volume answers, and the queue of arrivals and removals that `next` and `present` read.
//! `win::drives` makes the Windows calls.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use crate::DriveEvent;

/// `GetDriveTypeW`'s answer for an optical drive.
#[cfg_attr(not(windows), allow(dead_code))]
const DRIVE_CDROM: u32 = 5;

/// The errors after which a volume keeps the last scan's answer: `ERROR_ACCESS_DENIED` (5) and
/// `ERROR_SHARING_VIOLATION` (32), what a volume lock gives (Ruling T3-16-a).
#[cfg_attr(not(windows), allow(dead_code))]
const LOCKED: [u32; 2] = [5, 32];

/// The drive id of `letter` in MakeMKV's `D:` form: the letter upper-cased, then a colon (design part 5.4 step 1;
/// engine.md:575). The same id keys `drives.json` and is what `Ejector::eject` takes. Def: T3-16. Use: T3-16.
pub fn drive_id(letter: char) -> String {
    format!("{}:", letter.to_ascii_uppercase())
}

/// How often the watcher scans the drives with no notification (Ruling W8). Def: T3-16. Use: T3-16.
pub const DRIVE_POLL_EVERY: Duration = Duration::from_secs(2);

/// The scan's filter (Ruling W8): each letter of `mask` (`GetLogicalDrives`' bits, bit 0 `A` to bit 25 `Z`, higher bits
/// ignored) whose `kind` is `DRIVE_CDROM` (5) and whose volume `answers`, as `drive_id` -> `X:\`. `answers` is asked
/// only for an optical drive, so the volume of a fixed or network drive is never touched. Def: T3-16. Use: T3-16.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn ready_drives(
    mask: u32,
    kind: impl Fn(char) -> u32,
    answers: impl Fn(char) -> bool,
) -> BTreeMap<String, PathBuf> {
    (0..26u8)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| char::from(b'A' + bit))
        .filter(|&letter| kind(letter) == DRIVE_CDROM && answers(letter))
        .map(|letter| (drive_id(letter), PathBuf::from(format!(r"{letter}:\"))))
        .collect()
}

/// Whether a volume answers after `GetVolumeInformationW` gave `read` (Ruling W8): Ok answers; ERROR_ACCESS_DENIED (5)
/// and ERROR_SHARING_VIOLATION (32), which a volume lock held by the `Ejector` or another program gives, keep the last
/// scan's answer `before`, so a lock is never reported as a removal; every other code, ERROR_NOT_READY (21) for an
/// empty tray and ERROR_UNRECOGNIZED_VOLUME (1005) among them, does not answer. Keeping the answer on 5 and 32 widens
/// W8's "a successful GetVolumeInformationW" (Ruling T3-16-a). Def: T3-16. Use: T3-16.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn volume_answers(read: Result<(), u32>, before: bool) -> bool {
    match read {
        Ok(()) => true,
        Err(code) => LOCKED.contains(&code) && before,
    }
}

/// The watcher's state (design part 5.4, L1a, Ruling W8): the discs of the last scan and the events not yet handed
/// out. Def: T3-16. Use: T3-16.
pub struct DriveQueue {
    /// The discs of the last scan, drive id -> root.
    now: BTreeMap<String, PathBuf>,
    /// The events not yet handed out, oldest first.
    events: VecDeque<DriveEvent>,
}

impl DriveQueue {
    /// A queue whose current discs are `now`, with no event: the discs at start are `present`'s, never `next`'s (L1a).
    /// Def: T3-16. Use: T3-16.
    pub fn new(now: BTreeMap<String, PathBuf>) -> DriveQueue {
        DriveQueue { now, events: VecDeque::new() }
    }

    /// Takes one scan: queues `Removed` for each id of the last scan missing from `now`, in id order, then `Arrived`
    /// with its root for each id of `now` missing from the last scan, in id order; an id in both queues nothing, which
    /// is the debounce (W8). Def: T3-16. Use: T3-16.
    pub fn observe(&mut self, now: BTreeMap<String, PathBuf>) {
        for drive in self.now.keys().filter(|drive| !now.contains_key(*drive)) {
            self.events.push_back(DriveEvent::Removed { drive: drive.clone() });
        }
        for (drive, root) in now.iter().filter(|(drive, _)| !self.now.contains_key(*drive)) {
            self.events.push_back(DriveEvent::Arrived { drive: drive.clone(), root: root.clone() });
        }
        self.now = now;
    }

    /// The current discs as (id, root) in id order, and every queued event dropped, since the list holds their outcome
    /// (L1a). Def: T3-16. Use: T3-16.
    pub fn present(&mut self) -> Vec<(String, PathBuf)> {
        self.events.clear();
        self.now.iter().map(|(drive, root)| (drive.clone(), root.clone())).collect()
    }

    /// The oldest queued event. Def: T3-16. Use: T3-16.
    pub fn pop(&mut self) -> Option<DriveEvent> {
        self.events.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::DriveEvent;

    fn discs(ids: &[&str]) -> BTreeMap<String, PathBuf> {
        ids.iter().map(|id| (id.to_string(), PathBuf::from(format!(r"{id}\")))).collect()
    }

    fn arrived(id: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Arrived { drive: id.to_string(), root: PathBuf::from(format!(r"{id}\")) })
    }

    fn removed(id: &str) -> Option<DriveEvent> {
        Some(DriveEvent::Removed { drive: id.to_string() })
    }

    #[test]
    fn a_lower_case_letter_is_reported_upper_case_with_a_colon() {
        assert_eq!(drive_id('d'), "D:");
        assert_eq!(drive_id('D'), "D:");
    }

    #[test]
    fn an_arrival_and_a_removal_are_queued_once_each() {
        let mut queue = DriveQueue::new(discs(&[]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
        assert_eq!(queue.pop(), None);
        queue.observe(discs(&[]));
        assert_eq!(queue.pop(), removed("D:"));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn a_disc_seen_twice_is_reported_once() {
        let mut queue = DriveQueue::new(discs(&[]));
        queue.observe(discs(&["D:"]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn removals_come_before_arrivals_in_letter_order() {
        let mut queue = DriveQueue::new(discs(&["D:", "F:"]));
        queue.observe(discs(&["E:", "G:"]));
        let order: Vec<Option<DriveEvent>> = (0..5).map(|_| queue.pop()).collect();
        assert_eq!(order, vec![removed("D:"), removed("F:"), arrived("E:"), arrived("G:"), None]);
    }

    /// L1a: what `present` lists, `next` never reports again.
    #[test]
    fn present_drops_events_queued_before_it() {
        let mut queue = DriveQueue::new(discs(&["E:"]));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.present(), vec![("D:".to_string(), PathBuf::from(r"D:\"))]);
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn an_event_after_present_is_reported() {
        let mut queue = DriveQueue::new(discs(&["D:"]));
        assert_eq!(queue.present().len(), 1);
        queue.observe(discs(&[]));
        assert_eq!(queue.pop(), removed("D:"));
        queue.observe(discs(&["D:"]));
        assert_eq!(queue.pop(), arrived("D:"));
    }

    /// engine.md:582, Ruling W8: a disc whose volume does not answer is not listed, so it arrives through `next` once
    /// it answers and gets the readiness wait of any arrival.
    #[test]
    fn a_cd_drive_whose_volume_does_not_answer_is_not_listed() {
        // C: is fixed (3); D: and E: are optical (5); only D:'s volume answers.
        let mask = (1 << 2) | (1 << 3) | (1 << 4);
        let kind = |letter: char| if letter == 'C' { 3 } else { 5 };
        assert_eq!(ready_drives(mask, kind, |letter| letter == 'D'), discs(&["D:"]));
    }

    #[test]
    fn only_cd_drives_are_asked_whether_their_volume_answers() {
        let asked = RefCell::new(Vec::new());
        // A:, C:, D: and a bit past Z:.
        let mask = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 30);
        let kind = |letter: char| if letter == 'D' { 5 } else { 3 };
        let found = ready_drives(mask, kind, |letter| {
            asked.borrow_mut().push(letter);
            true
        });
        assert_eq!(found, discs(&["D:"]));
        assert_eq!(*asked.borrow(), vec!['D']);
    }

    #[test]
    fn a_locked_volume_keeps_its_last_answer() {
        assert!(volume_answers(Ok(()), false));
        assert!(!volume_answers(Err(21), true));
        assert!(!volume_answers(Err(1005), true));
        assert!(volume_answers(Err(5), true));
        assert!(!volume_answers(Err(5), false));
        assert!(volume_answers(Err(32), true));
        assert!(!volume_answers(Err(32), false));
    }
}
