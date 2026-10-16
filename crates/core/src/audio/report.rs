//! The per-folder text report of the audio content rule (spec 8.6, EXP-13), line for line as
//! `audio_similarity.py <folder>` printed it, and the folder's results as data for the audio-<folder>.json golden
//! and decide's "audio". Py: audio_similarity.fmt, reverse_line, peer_line, report.

use crate::audio::levels::{Fields, FRAME_S};
use crate::audio::measures::{ok, reverse_measures, swapped, Constants, Measures, Reverse};
use crate::audio::rule::{
    meaning, not_judged, results, reverse_pattern, verdict, Dialogue, FolderResults, Peer, Plain, Rule, Verdict, Veto,
    VETO_MEANING,
};
use crate::recording::{py_fixed, PathKey, Tree};

/// The dialogue reference when the main track's centre channel is the dialogue curve.
const CENTRE_CHANNEL: &str = "its centre channel";
/// ... and when it is not (no main-fc.txt, or one at or below SILENT_DB throughout).
const MONO_MIXDOWN: &str = "its mono mixdown (no centre channel)";
/// The line of a folder whose manifest names no candidate.
const NO_CANDIDATE: &str = "  no candidate stream: nothing to judge (measured for the tuning dataset only)";
/// What a holding veto adds after its reason.
const VETO_HOLDS: &str = "; no commentary is confirmed on this title: every candidate is reference-unclear";
/// The reverse line's ending when the reverse run has the commentary pattern.
const REVERSE_PATTERN: &str = "commentary pattern (the main track looks like a commentary over this one)";
/// The reverse line's ending when it has not and may be judged.
const NO_COMMENTARY_PATTERN: &str = "no commentary pattern";
/// not_judged's subject, and what the reverse line says in its place.
const THE_MAIN_TRACK: &str = "the main track";
const REVERSE_REFERENCE: &str = "this track (the reverse run's reference)";
/// The peer line's ending when the peer run says commentary.
const PEER_PATTERN: &str = "commentary pattern";

/// "n/a" for NaN, else '%.<digits>f'. Py: audio_similarity.fmt.
pub fn fmt(v: f64, digits: usize) -> String {
    if ok(v) {
        py_fixed(v, digits)
    } else {
        "n/a".to_string()
    }
}

/// `fields.get(key, default)` of a manifest line.
fn field<'a>(fields: &'a Fields, key: &str, default: &'a str) -> &'a str {
    fields.get(key).map(String::as_str).unwrap_or(default)
}

/// The report's veto line: holds (True), none (False), or not checked (None). Py: the veto line of report.
pub fn veto_line(veto: &Veto) -> String {
    match veto.holds {
        Some(true) => format!("  veto: holds (row 0, D21): {}{}", veto.why, VETO_HOLDS),
        Some(false) => format!("  veto: none (row 0, D21: {})", veto.why),
        None => format!("  veto: {} (row 0, D21)", veto.why),
    }
}

/// The report's reverse line: the reverse run's measures and whether it has the commentary pattern, judged with the
/// default rule. Py: reverse_line.
pub fn reverse_line(m: &Measures) -> String {
    let rule = Rule::default();
    let Some(back) = reverse_measures(m) else {
        // No reverse block: the earlier keys alone (measures written before 2026-10-06). The Python raised KeyError
        // for measures without them; measures from `results` always have them (analyse runs the reverse run), so the
        // defaults verdict() reads stand in: rev_gap_s 0.0 and rev_ag NaN.
        let (gap_s, ag) = match m.run().map(|run| &run.rev) {
            Some(Reverse::Earlier { rev_gap_s, rev_ag }) => (*rev_gap_s, *rev_ag),
            _ => (0.0, f64::NAN),
        };
        return format!("    reverse: {} s of gaps, AG {}", py_fixed(gap_s, 0), fmt(ag, 2));
    };
    let r = match &back {
        Measures::Error { error, .. } => return format!("    reverse: not judged ({error})"),
        Measures::Run(r) => r,
    };
    let why = not_judged(&back, &rule).replace(THE_MAIN_TRACK, REVERSE_REFERENCE);
    let ending = if reverse_pattern(m, &rule) {
        REVERSE_PATTERN.to_string()
    } else if !why.is_empty() {
        format!("not judged ({why})")
    } else {
        NO_COMMENTARY_PATTERN.to_string()
    };
    format!(
        "    reverse: {} s of gaps, DC {}, AG {}, tenths {}/{}, SMI {}, LS {} dB, added {} s: {}",
        py_fixed(r.gap_s, 0),
        fmt(r.dc, 2),
        fmt(r.ag, 2),
        r.tenths_comm,
        r.tenths,
        fmt(r.smi, 2),
        fmt(r.ls_db, 1),
        py_fixed(r.added_s, 0),
        ending
    )
}

/// The report's peer line: a cross-language candidate against the first stream of its own language. Py: peer_line.
pub fn peer_line(peer: &Peer) -> String {
    let m = match &peer.measures {
        Measures::Error { error, .. } => {
            return format!("    peer: against stream {}: not judged ({error})", peer.stream);
        }
        Measures::Run(m) => m,
    };
    let ending = if peer.verdict == Verdict::Commentary {
        PEER_PATTERN.to_string()
    } else {
        format!("{}, so not confirmed", peer.verdict.as_str())
    };
    format!(
        "    peer: against stream {} (placed first in the candidate's language): DC {}, AG {}, tenths {}/{}, SMI {}, LS {} dB: {}",
        peer.stream,
        fmt(m.dc, 2),
        fmt(m.ag, 2),
        m.tenths_comm,
        m.tenths,
        fmt(m.smi, 2),
        fmt(m.ls_db, 1),
        ending
    )
}

/// The report's lines for a folder whose results are in hand. Py: report's body.
pub fn report_lines(tree: &dyn Tree, folder: &PathKey, res: &FolderResults) -> Vec<String> {
    let info = &res.manifest.info;
    let main = &res.main;
    let dialogue = match res.dialogue {
        Dialogue::Fc => CENTRE_CHANNEL,
        Dialogue::Mix => MONO_MIXDOWN,
    };
    let mut lines = vec![
        format!("{} ({}, measured in {} s)", tree.display(folder), field(info, "file", "?"), field(info, "seconds", "?")),
        format!(
            "  main stream {}: {} {} ch {} {}; dialogue reference: {}; {} frames ({} min)",
            field(main, "main", "?"),
            field(main, "codec", "?"),
            field(main, "channels", "?"),
            field(main, "layout", ""),
            field(main, "language", ""),
            dialogue,
            res.frames,
            py_fixed(res.frames as f64 * FRAME_S / 60.0, 1)
        ),
        veto_line(&res.veto),
    ];
    if res.candidates.is_empty() {
        lines.push(NO_CANDIDATE.to_string());
    }
    let file = match info.get("file") {
        Some(file) => file.clone(),
        None => tree.resolved_name(folder),
    };
    let label = format!("{}/{}", tree.resolved_name(&folder.parent()), file);
    for r in &res.candidates {
        let c = &r.info;
        let sid = &r.stream;
        let mut head = format!(
            "  stream {}: {} {} ch {} {}",
            sid,
            field(c, "codec", "?"),
            field(c, "channels", "?"),
            field(c, "layout", ""),
            field(c, "language", "")
        );
        if !r.cross_language.is_empty() {
            head.push_str(&format!(" (cross-language: {})", r.cross_language));
        }
        lines.push(head);
        if let Some(original) = &r.duplicate_of {
            lines.push(format!("    verdict: duplicate of stream {} ({})", original, meaning(Verdict::Duplicate)));
            lines.push(format!("    row: | {} | {} | same packets as stream {} | duplicate |", label, sid, original));
            continue;
        }
        // results gives measures to every candidate that is not a duplicate
        let Some(measures) = r.measures.as_ref() else { continue };
        let m = match measures {
            Measures::Error { error, .. } => {
                lines.push(format!("    verdict: not-judged ({error})"));
                continue;
            }
            Measures::Run(m) => m,
        };
        // analyse always gives lag_ms and r_global; "None" and "n/a" stand in for measures without them
        let lag = m.lag_ms.map_or_else(|| "None".to_string(), |lag| lag.to_string());
        lines.push(format!(
            "    lag {} ms (envelope r {}), overlap {}, main silent {}",
            lag,
            fmt(m.r_global.unwrap_or(f64::NAN), 2),
            fmt(m.overlap, 2),
            fmt(m.main_silent, 2)
        ));
        lines.push(format!(
            "    DC {}  AG {} ({} s of gaps, {} s added)  tenths {}/{}  SMI {}  LS {} dB  ADL {}  added {} s",
            fmt(m.dc, 2),
            fmt(m.ag, 2),
            py_fixed(m.gap_s, 0),
            py_fixed(m.ag_s, 0),
            m.tenths_comm,
            m.tenths,
            fmt(m.smi, 2),
            fmt(m.ls_db, 1),
            fmt(m.adl, 2),
            py_fixed(m.added_s, 0)
        ));
        lines.push(reverse_line(measures));
        if let Some(peer) = &r.peer {
            lines.push(peer_line(peer));
        }
        let v = r.verdict.as_str();
        let because = match r.before_veto {
            Some(before) => format!("{}; rows 3 to 9 alone: {}", VETO_MEANING, before.as_str()),
            None => meaning(r.verdict).to_string(),
        };
        let why = if r.not_judged.is_empty() { String::new() } else { format!("; not judged: {}", r.not_judged) };
        lines.push(format!("    verdict: {} ({}){}", v, because, why));
        lines.push(format!(
            "    row: | {} | {} | DC {}, AG {}, tenths {}/{}, SMI {}, LS {} | {} |",
            label,
            sid,
            fmt(m.dc, 2),
            fmt(m.ag, 2),
            m.tenths_comm,
            m.tenths,
            fmt(m.smi, 2),
            fmt(m.ls_db, 1),
            v
        ));
    }
    lines
}

/// The report of one level folder: results with the default rule and constants and no cache, then its lines; Err
/// with the refusal's reason (or a read error). Py: audio_similarity.report.
pub fn report(tree: &dyn Tree, folder: &PathKey) -> Result<Vec<String>, String> {
    let rule = Rule::default();
    let consts = Constants::default();
    let res = results(tree, folder, &rule, &consts, &mut Plain { consts: &consts })?;
    Ok(report_lines(tree, folder, &res))
}

/// <label>/audio-<folder>.json (spec section 5) and an entry of decide's "audio". Field order is the JSON order.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FolderJson {
    /// The level folder's name.
    pub folder: String,
    pub main: Fields,
    pub frames: usize,
    pub dialogue: Dialogue,
    pub veto: Veto,
    /// veto_line(&veto).
    pub veto_line: String,
    pub candidates: Vec<CandidateJson>,
}

/// One candidate of a FolderJson.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CandidateJson {
    pub stream: String,
    pub duplicate_of: Option<String>,
    pub verdict: Verdict,
    pub not_judged: String,
    pub cross_language: String,
    pub before_veto: Option<Verdict>,
    /// The forward run (None for a duplicate).
    pub measures: Option<Measures>,
    /// reverse_measures(measures).
    pub reverse: Option<Measures>,
    /// swapped(measures), and verdict(swapped) with the rule given.
    pub swapped: Option<Measures>,
    pub swapped_verdict: Option<Verdict>,
    pub peer: Option<PeerJson>,
}

/// The peer run of a cross-language candidate.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PeerJson {
    pub stream: String,
    pub measures: Measures,
    pub verdict: Verdict,
}

/// FolderJson from results already computed (no measure is run again).
pub fn folder_json(res: &FolderResults, folder_name: &str, rule: &Rule) -> FolderJson {
    let candidates = res
        .candidates
        .iter()
        .map(|c| {
            let back = c.measures.as_ref().and_then(swapped);
            let swapped_verdict = back.as_ref().map(|m| verdict(m, rule));
            CandidateJson {
                stream: c.stream.clone(),
                duplicate_of: c.duplicate_of.clone(),
                verdict: c.verdict,
                not_judged: c.not_judged.clone(),
                cross_language: c.cross_language.clone(),
                before_veto: c.before_veto,
                measures: c.measures.clone(),
                reverse: c.measures.as_ref().and_then(reverse_measures),
                swapped: back,
                swapped_verdict,
                peer: c.peer.as_ref().map(|p| PeerJson {
                    stream: p.stream.clone(),
                    measures: p.measures.clone(),
                    verdict: p.verdict,
                }),
            }
        })
        .collect();
    FolderJson {
        folder: folder_name.to_string(),
        main: res.main.clone(),
        frames: res.frames,
        dialogue: res.dialogue,
        veto: res.veto.clone(),
        veto_line: veto_line(&res.veto),
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::levels::refusal;
    use crate::audio::measures::{RevBlock, Run};
    use crate::recording::{MemTree, PathKey};
    use crate::testsupport::{commentary, level_file_text, manifest, programme, raw_file_text};

    /// The Python's report of <tmp>/audio-levels-20ms-1-3: MIX[:2000] as main and as candidate 3, one md5.
    const DUPLICATE: &[&str] = &[
        "tmp/audio-levels-20ms-1-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) eng; dialogue reference: its mono mixdown (no centre channel); 2000 frames (0.7 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 3: ac3 2 ch stereo eng",
        "    verdict: duplicate of stream 1 (exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed)",
        "    row: | tmp/x.mkv | 3 | same packets as stream 1 | duplicate |",
    ];

    /// The Python's report of <tmp>/nc/audio-levels-20ms-1: a main stream measured for the dataset only, with a
    /// streams.json in which it is the first English stream.
    const NO_CANDIDATE_FOLDER: &[&str] = &[
        "tmp/nc/audio-levels-20ms-1 (y.mkv, measured in 12 s)",
        "  main stream 1: truehd 8 ch 7.1 eng; dialogue reference: its mono mixdown (no centre channel); 2000 frames (0.7 min)",
        "  veto: none (row 0, D21: the reference, stream 1 (truehd 8 ch), is the first eng stream of the file)",
        "  no candidate stream: nothing to judge (measured for the tuning dataset only)",
    ];

    /// The Python's report of <tmp>/ts/audio-levels-20ms-1-3: 500 frames, too short to measure.
    const TOO_SHORT: &[&str] = &[
        "tmp/ts/audio-levels-20ms-1-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) eng; dialogue reference: its mono mixdown (no centre channel); 500 frames (0.2 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 3: ac3 2 ch stereo eng",
        "    verdict: not-judged (too short)",
    ];

    /// The Python's report of <tmp>/analysed/audio-levels-20ms-1-3: MIX, CENTRE, and commentary(MIX) written as raw
    /// ametadata output at 20 ms (the folder of Report::test_a_folder_written_by_the_new_script_is_analysed).
    const ANALYSED: &[&str] = &[
        "tmp/analysed/audio-levels-20ms-1-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) eng; dialogue reference: its centre channel; 21000 frames (7.0 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 3: ac3 2 ch stereo eng",
        "    lag 0 ms (envelope r 0.06), overlap 1.00, main silent 0.00",
        "    DC 0.02  AG 0.87 (162 s of gaps, 140 s added)  tenths 10/10  SMI 0.77  LS 9.0 dB  ADL 0.08  added 173 s",
        "    reverse: 11 s of gaps, DC 0.00, AG 0.74, tenths 0/0, SMI 0.76, LS 11.1 dB, added 30 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)",
        "    verdict: commentary (commentary pattern: kept and flagged for review until experiment 13 answers yes, then dropped)",
        "    row: | analysed/x.mkv | 3 | DC 0.02, AG 0.87, tenths 10/10, SMI 0.77, LS 9.0 | commentary |",
    ];

    /// The Python's report of <tmp>/c1/audio-levels-20ms-1-3: a Spanish main stream and an English commentary.
    const CROSS_LANGUAGE: &[&str] = &[
        "tmp/c1/audio-levels-20ms-1-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) spa; dialogue reference: its centre channel; 21000 frames (7.0 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 3: ac3 2 ch stereo eng (cross-language: eng against spa)",
        "    lag 0 ms (envelope r 0.06), overlap 1.00, main silent 0.00",
        "    DC 0.02  AG 0.87 (162 s of gaps, 140 s added)  tenths 10/10  SMI 0.77  LS 9.1 dB  ADL 0.08  added 174 s",
        "    reverse: 11 s of gaps, DC 0.00, AG 0.75, tenths 0/0, SMI 0.76, LS 11.1 dB, added 31 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)",
        "    verdict: commentary-cross-language (commentary pattern on a track in another language than the reference: keep and flag for review (commentary_detected); never dropped by content until real dubs in a wanted language have been measured (EXP-13 addendum 9.2))",
        "    row: | c1/x.mkv | 3 | DC 0.02, AG 0.87, tenths 10/10, SMI 0.77, LS 9.1 | commentary-cross-language |",
    ];

    /// The Python's report of <tmp>/c2/audio-levels-20ms-1-2-3: a Spanish film, an English dub stand-in (2) with the
    /// commentary pattern, and its 2.0 companion (3), the same mix 60 ms late and 6 dB down.
    const COMPANION: &[&str] = &[
        "tmp/c2/audio-levels-20ms-1-2-3 (x.mkv, measured in 1 s)",
        "  main stream 1: dts 6 ch 5.1(side) spa; dialogue reference: its centre channel; 21000 frames (7.0 min)",
        "  veto: not checked: the folder has no streams.json (row 0, D21)",
        "  stream 2: ac3 6 ch x eng (cross-language: eng against spa)",
        "    lag 0 ms (envelope r 0.06), overlap 1.00, main silent 0.00",
        "    DC 0.02  AG 0.87 (162 s of gaps, 140 s added)  tenths 10/10  SMI 0.77  LS 9.1 dB  ADL 0.08  added 174 s",
        "    reverse: 11 s of gaps, DC 0.00, AG 0.75, tenths 0/0, SMI 0.76, LS 11.1 dB, added 31 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)",
        "    verdict: commentary-cross-language (commentary pattern on a track in another language than the reference: keep and flag for review (commentary_detected); never dropped by content until real dubs in a wanted language have been measured (EXP-13 addendum 9.2))",
        "    row: | c2/x.mkv | 2 | DC 0.02, AG 0.87, tenths 10/10, SMI 0.77, LS 9.1 | commentary-cross-language |",
        "  stream 3: ac3 2 ch x eng (cross-language: eng against spa)",
        "    lag 0 ms (envelope r 0.06), overlap 1.00, main silent 0.00",
        "    DC 0.02  AG 0.87 (162 s of gaps, 141 s added)  tenths 10/10  SMI 0.77  LS 9.1 dB  ADL 0.08  added 175 s",
        "    reverse: 8 s of gaps, DC 0.01, AG 0.98, tenths 0/0, SMI 0.76, LS 11.1 dB, added 31 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)",
        "    peer: against stream 2 (placed first in the candidate's language): DC 1.00, AG 0.00, tenths 0/0, SMI n/a, LS 9.1 dB: keep, so not confirmed",
        "    verdict: note-commentary (keep, note possible_commentary)",
        "    row: | c2/x.mkv | 3 | DC 0.02, AG 0.87, tenths 10/10, SMI 0.77, LS 9.1 | note-commentary |",
    ];

    /// The Python's report of <tmp>/pv/audio-levels-20ms-2-1-3-4 with its streams.json: the veto holds.
    const VETOED: &[&str] = &[
        "tmp/pv/audio-levels-20ms-2-1-3-4 (x.mkv, measured in 1 s)",
        "  main stream 2: dts 6 ch 5.1(side) eng; dialogue reference: its centre channel; 21000 frames (7.0 min)",
        "  veto: holds (row 0, D21): the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng stream of the file: stream 1 (dts DTS 6 ch) comes first and is lossless or has more than two channels, so the reference choice may be inverted; no commentary is confirmed on this title: every candidate is reference-unclear",
        "  stream 1: dts 6 ch 5.1(side) eng",
        "    lag 60 ms (envelope r 1.00), overlap 1.00, main silent 0.00",
        "    DC 1.00  AG 0.00 (162 s of gaps, 0 s added)  tenths 0/10  SMI n/a  LS 11.1 dB  ADL 0.00  added 0 s",
        "    reverse: 162 s of gaps, DC 1.00, AG 0.00, tenths 0/10, SMI n/a, LS 11.1 dB, added 0 s: no commentary pattern",
        "    verdict: reference-unclear (the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear); rows 3 to 9 alone: keep)",
        "    row: | pv/x.mkv | 1 | DC 1.00, AG 0.00, tenths 0/10, SMI n/a, LS 11.1 | reference-unclear |",
        "  stream 3: ac3 2 ch stereo eng",
        "    lag 0 ms (envelope r 0.06), overlap 1.00, main silent 0.00",
        "    DC 0.02  AG 0.87 (162 s of gaps, 140 s added)  tenths 10/10  SMI 0.77  LS 9.1 dB  ADL 0.08  added 174 s",
        "    reverse: 11 s of gaps, DC 0.00, AG 0.75, tenths 0/0, SMI 0.76, LS 11.1 dB, added 31 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)",
        "    verdict: reference-unclear (the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear); rows 3 to 9 alone: commentary)",
        "    row: | pv/x.mkv | 3 | DC 0.02, AG 0.87, tenths 10/10, SMI 0.77, LS 9.1 | reference-unclear |",
        "  stream 4: dts 6 ch 5.1(side) eng",
        "    verdict: duplicate of stream 2 (exact duplicate (same packets): keep the first copy, drop this one; no audio analysis needed)",
        "    row: | pv/x.mkv | 4 | same packets as stream 2 | duplicate |",
    ];

    /// CrossLanguage.folder of test_audio_similarity.py for [(2, dub, 6), (3, companion, 2)].
    const COMPANION_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n",
        "main=1 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=spa fc=yes frames=1 md5=aa\n",
        "cand=2 codec=ac3 profile=- channels=6 layout=x language=eng frames=1 md5=m2\n",
        "cand=3 codec=ac3 profile=- channels=2 layout=x language=eng frames=1 md5=m3\n",
    );

    /// PositionVeto::test_it_never_turns_a_keep_into_a_drop's manifest: main 2, candidates 1, 3 and 4 (4 a duplicate).
    const VETO_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=x.mkv bytes=1 seconds=1 frame_ms=20 band_hz=300-3400\n",
        "main=2 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng fc=yes frames=1 md5=aa\n",
        "cand=1 codec=dts profile=DTS channels=6 layout=5.1(side) language=eng frames=1 md5=bb\n",
        "cand=3 codec=ac3 profile=- channels=2 layout=stereo language=eng frames=1 md5=cc\n",
        "cand=4 codec=dts profile=DTS-HD_MA channels=6 layout=5.1(side) language=eng frames=1 md5=aa\n",
    );

    /// streams_json(d, [(1, "dts", "DTS", 6, "eng"), (2, "dts", "DTS-HD MA", 6, "eng"), (3, "ac3", None, 2, "eng"),
    /// (4, "dts", "DTS-HD MA", 6, "eng")]) of test_audio_similarity.py, as json.dumps wrote it.
    const VETO_STREAMS: &str = r#"{"streams": [{"index": 0, "codec_type": "video"}, {"index": 1, "codec_type": "audio", "codec_name": "dts", "profile": "DTS", "channels": 6, "tags": {"language": "eng"}}, {"index": 2, "codec_type": "audio", "codec_name": "dts", "profile": "DTS-HD MA", "channels": 6, "tags": {"language": "eng"}}, {"index": 3, "codec_type": "audio", "codec_name": "ac3", "profile": null, "channels": 2, "tags": {"language": "eng"}}, {"index": 4, "codec_type": "audio", "codec_name": "dts", "profile": "DTS-HD MA", "channels": 6, "tags": {"language": "eng"}}]}"#;

    /// A main stream measured for the dataset only: TrueHD 7.1, no candidate.
    const NO_CANDIDATE_MANIFEST: &str = concat!(
        "# lazymkv audio-levels v2 (experiment 13)\n",
        "file=y.mkv bytes=1 seconds=12 frame_ms=20 band_hz=300-3400\n",
        "main=1 codec=truehd profile=- channels=8 layout=7.1 language=eng fc=no frames=1 md5=aa\n",
    );

    /// Its streams.json: the TrueHD stream first, an AC-3 stereo stream after it.
    const NO_CANDIDATE_STREAMS: &str = r#"{"streams": [{"index": 0, "codec_type": "video"}, {"index": 1, "codec_type": "audio", "codec_name": "truehd", "profile": null, "channels": 8, "tags": {"language": "eng"}}, {"index": 2, "codec_type": "audio", "codec_name": "ac3", "profile": null, "channels": 2, "tags": {"language": "eng"}}]}"#;

    /// [-100.0] * 3 + [v - 6 for v in values[:-3]]: the same sound 60 ms late and 6 dB down.
    fn late_copy(values: &[f64]) -> Vec<f64> {
        let mut out = vec![-100.0; 3];
        out.extend(values[..values.len() - 3].iter().map(|v| v - 6.0));
        out
    }

    /// results with the default rule and constants and Plain, as report() runs it.
    fn measured(tree: &dyn Tree, folder: &PathKey) -> FolderResults {
        let consts = Constants::default();
        results(tree, folder, &Rule::default(), &consts, &mut Plain { consts: &consts }).unwrap()
    }

    /// ReverseRun.PANS of test_audio_similarity.py: the English commentary of Pan's Labyrinth against the Spanish
    /// main track, as measured, with the earlier reverse keys (rev_gap_s, rev_ag) alone.
    fn pans() -> Run {
        Run {
            overlap: 1.0,
            main_silent: 0.0,
            gap_s: 929.0,
            dc: 0.08,
            ag: 0.54,
            ag_s: 505.0,
            adl: 0.18,
            tenths: 10,
            tenths_comm: 9,
            tenths_ad: 10,
            smi: 0.51,
            ls_db: 10.8,
            added_s: 2016.0,
            rev: Reverse::Earlier { rev_gap_s: 470.0, rev_ag: 0.31 },
            ..Run::blank()
        }
    }

    /// ReverseRun.REV with PANS's rev_gap_s and rev_ag: the reverse block as measured.
    fn rev() -> RevBlock {
        RevBlock {
            rev_error: String::new(),
            rev_overlap: 1.0,
            rev_main_silent: 0.001,
            rev_gap_s: 470.0,
            rev_dc: 0.06,
            rev_ag: 0.31,
            rev_ag_s: 144.0,
            rev_adl: 0.04,
            rev_tenths: Some(10),
            rev_tenths_comm: Some(1),
            rev_tenths_ad: Some(10),
            rev_smi: 0.39,
            rev_smi_windows: Some(526),
            rev_ls_db: 17.1,
            rev_added_s: 796.0,
            rev_gain_db: -6.7,
        }
    }

    fn with_reverse(block: RevBlock) -> Measures {
        Measures::Run(Run { rev: Reverse::Block(block), ..pans() })
    }

    #[test]
    fn an_identical_stream_is_reported_as_a_duplicate_without_analysis() {
        let (mix, _) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "audio-levels-20ms-1-3";
        for name in ["main-mix.txt", "cand-3.txt"] {
            tree.insert_file(&format!("{d}/{name}"), level_file_text(&mix[..2000]));
        }
        tree.insert_file(&format!("{d}/manifest.txt"), manifest("no", "43ae", "43ae"));
        let text = report(&tree, &PathKey::new(d)).unwrap().join("\n");
        assert!(text.contains("verdict: duplicate of stream 1"), "{text}");
        assert!(text.contains("its mono mixdown (no centre channel)"), "{text}");
    }

    #[test]
    fn level_files_of_100_ms_frames_are_refused_even_with_a_manifest() {
        let (mix, _) = programme(420, 1);
        let coarse: Vec<f64> = mix[..3000].iter().step_by(5).copied().collect(); // MIX[:3000:5]
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "audio-levels-20ms-1-3";
        for name in ["main-mix.txt", "cand-3.txt"] {
            tree.insert_file(&format!("{d}/{name}"), raw_file_text(&coarse, 0.1));
        }
        tree.insert_file(&format!("{d}/manifest.txt"), manifest("no", "aa", "bb"));
        let why = refusal(&tree, &PathKey::new(d)).unwrap();
        assert!(why.contains("main-mix.txt holds 100 ms frames, not 20 ms"), "{why}");
        assert!(why.contains("measure the title again"), "{why}");
        assert_eq!(
            why,
            "tmp/audio-levels-20ms-1-3: main-mix.txt holds 100 ms frames, not 20 ms; this folder cannot be analysed: measure the title again with the new audio-levels.ps1, which writes audio-levels-20ms-<main>-<candidates>"
        );
        assert_eq!(report(&tree, &PathKey::new(d)).unwrap_err(), why);
    }

    #[test]
    fn fmt_is_na_for_nan_and_fixed_otherwise() {
        // Each case as the Python's fmt() gave it ('%.Nf' rounds the exact binary value half to even).
        let cases = [
            (f64::NAN, 2, "n/a"),
            (0.125, 2, "0.12"),
            (2.675, 2, "2.67"),
            (0.375, 2, "0.38"),
            (17.05, 1, "17.1"),
            (-0.004, 2, "-0.00"),
            (1.0, 1, "1.0"),
            (f64::INFINITY, 2, "inf"),
        ];
        for (v, digits, want) in cases {
            assert_eq!(fmt(v, digits), want, "fmt({v}, {digits})");
        }
    }

    #[test]
    fn veto_line_has_the_pythons_three_forms() {
        let line = |holds, why: &str| veto_line(&Veto { holds, why: why.to_string() });
        assert_eq!(
            line(Some(true), "the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng stream of the file: stream 1 (dts DTS 6 ch) comes first and is lossless or has more than two channels, so the reference choice may be inverted"),
            "  veto: holds (row 0, D21): the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng stream of the file: stream 1 (dts DTS 6 ch) comes first and is lossless or has more than two channels, so the reference choice may be inverted; no commentary is confirmed on this title: every candidate is reference-unclear"
        );
        assert_eq!(
            line(Some(false), "the reference, stream 1 (truehd 8 ch), is the first eng stream of the file"),
            "  veto: none (row 0, D21: the reference, stream 1 (truehd 8 ch), is the first eng stream of the file)"
        );
        assert_eq!(
            line(None, "not checked: the folder has no streams.json"),
            "  veto: not checked: the folder has no streams.json (row 0, D21)"
        );
    }

    #[test]
    fn reverse_line_has_the_pythons_forms() {
        // The earlier reverse keys alone (measures written before 2026-10-06).
        assert_eq!(reverse_line(&Measures::Run(pans())), "    reverse: 470 s of gaps, AG 0.31");
        assert_eq!(
            reverse_line(&with_reverse(rev())),
            "    reverse: 470 s of gaps, DC 0.06, AG 0.31, tenths 1/10, SMI 0.39, LS 17.1 dB, added 796 s: no commentary pattern"
        );
        let pattern = RevBlock {
            rev_dc: 0.03,
            rev_ag: 0.66,
            rev_tenths_comm: Some(10),
            rev_smi: 0.58,
            rev_ls_db: 10.1,
            rev_added_s: 517.0,
            ..rev()
        };
        assert_eq!(
            reverse_line(&with_reverse(pattern)),
            "    reverse: 470 s of gaps, DC 0.03, AG 0.66, tenths 10/10, SMI 0.58, LS 10.1 dB, added 517 s: commentary pattern (the main track looks like a commentary over this one)"
        );
        assert_eq!(
            reverse_line(&with_reverse(RevBlock { rev_error: "too short".to_string(), ..rev() })),
            "    reverse: not judged (too short)"
        );
        assert_eq!(
            reverse_line(&with_reverse(RevBlock { rev_gap_s: 29.0, ..rev() })),
            "    reverse: 29 s of gaps, DC 0.06, AG 0.31, tenths 1/10, SMI 0.39, LS 17.1 dB, added 796 s: not judged (this track (the reverse run's reference) has less than 90 s of dialogue gaps)"
        );
        assert_eq!(
            reverse_line(&with_reverse(RevBlock { rev_main_silent: 0.5, ..rev() })),
            "    reverse: 470 s of gaps, DC 0.06, AG 0.31, tenths 1/10, SMI 0.39, LS 17.1 dB, added 796 s: not judged (this track (the reverse run's reference) is digitally silent for more than 10% of the time (not continuous programme sound))"
        );
    }

    #[test]
    fn peer_line_has_the_pythons_forms() {
        let error = Peer {
            stream: "2".to_string(),
            measures: Measures::Error { frames: None, error: "too short".to_string() },
            key: None,
            verdict: Verdict::NotJudged,
        };
        assert_eq!(peer_line(&error), "    peer: against stream 2: not judged (too short)");
        let pattern = Peer { stream: "5".to_string(), measures: Measures::Run(pans()), key: None, verdict: Verdict::Commentary };
        assert_eq!(
            peer_line(&pattern),
            "    peer: against stream 5 (placed first in the candidate's language): DC 0.08, AG 0.54, tenths 9/10, SMI 0.51, LS 10.8 dB: commentary pattern"
        );
        let note = Peer {
            stream: "5".to_string(),
            measures: Measures::Run(Run { smi: f64::NAN, ..pans() }),
            key: None,
            verdict: Verdict::NoteCommentary,
        };
        assert_eq!(
            peer_line(&note),
            "    peer: against stream 5 (placed first in the candidate's language): DC 0.08, AG 0.54, tenths 9/10, SMI n/a, LS 10.8 dB: note-commentary, so not confirmed"
        );
    }

    #[test]
    fn a_duplicate_folder_reports_the_pythons_lines_and_its_json() {
        let (mix, _) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "audio-levels-20ms-1-3";
        for name in ["main-mix.txt", "cand-3.txt"] {
            tree.insert_file(&format!("{d}/{name}"), level_file_text(&mix[..2000]));
        }
        tree.insert_file(&format!("{d}/manifest.txt"), manifest("no", "43ae", "43ae"));
        let folder = PathKey::new(d);
        let rule = Rule::default();
        let res = measured(&tree, &folder);
        assert_eq!(report_lines(&tree, &folder, &res), DUPLICATE);
        let json = serde_json::to_value(folder_json(&res, d, &rule)).unwrap();
        let keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["folder", "main", "frames", "dialogue", "veto", "veto_line", "candidates"]);
        let cand = &json["candidates"][0];
        let keys: Vec<&str> = cand.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "stream", "duplicate_of", "verdict", "not_judged", "cross_language", "before_veto", "measures", "reverse",
                "swapped", "swapped_verdict", "peer"
            ]
        );
        assert_eq!(json["folder"], "audio-levels-20ms-1-3");
        assert_eq!(json["frames"], 2000);
        assert_eq!(json["dialogue"], "mix");
        assert_eq!(json["main"]["md5"], "43ae");
        assert_eq!(json["veto"], serde_json::json!({"holds": null, "why": "not checked: the folder has no streams.json"}));
        assert_eq!(json["veto_line"], DUPLICATE[2]);
        assert_eq!(cand["stream"], "3");
        assert_eq!(cand["duplicate_of"], "1");
        assert_eq!(cand["verdict"], "duplicate");
        for key in ["before_veto", "measures", "reverse", "swapped", "swapped_verdict", "peer"] {
            assert!(cand[key].is_null(), "{key}: {}", cand[key]);
        }
        let back: FolderJson = serde_json::from_value(json).unwrap();
        assert_eq!(back, folder_json(&res, d, &rule));
    }

    #[test]
    fn a_folder_without_candidates_reports_its_veto_and_nothing_to_judge() {
        let (mix, _) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "nc/audio-levels-20ms-1";
        tree.insert_file(&format!("{d}/main-mix.txt"), level_file_text(&mix[..2000]));
        tree.insert_file(&format!("{d}/manifest.txt"), NO_CANDIDATE_MANIFEST);
        tree.insert_file(&format!("{d}/streams.json"), NO_CANDIDATE_STREAMS);
        assert_eq!(report(&tree, &PathKey::new(d)).unwrap(), NO_CANDIDATE_FOLDER);
    }

    #[test]
    fn a_too_short_candidate_is_reported_not_judged() {
        let (mix, _) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "ts/audio-levels-20ms-1-3";
        for name in ["main-mix.txt", "cand-3.txt"] {
            tree.insert_file(&format!("{d}/{name}"), level_file_text(&mix[..500]));
        }
        tree.insert_file(&format!("{d}/manifest.txt"), manifest("no", "aa", "bb"));
        assert_eq!(report(&tree, &PathKey::new(d)).unwrap(), TOO_SHORT);
    }

    #[test]
    fn an_analysed_folder_reports_the_pythons_lines_and_its_json_carries_both_runs() {
        let (mix, centre) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "analysed/audio-levels-20ms-1-3";
        tree.insert_file(&format!("{d}/main-mix.txt"), level_file_text(&mix));
        tree.insert_file(&format!("{d}/main-fc.txt"), level_file_text(&centre));
        tree.insert_file(&format!("{d}/cand-3.txt"), raw_file_text(&commentary(&mix, 7), 0.02));
        tree.insert_file(&format!("{d}/manifest.txt"), manifest("yes", "aa", "bb"));
        let folder = PathKey::new(d);
        let rule = Rule::default();
        let res = measured(&tree, &folder);
        assert_eq!(report_lines(&tree, &folder, &res), ANALYSED);
        let json = folder_json(&res, "audio-levels-20ms-1-3", &rule);
        let cand = &json.candidates[0];
        let m = res.candidates[0].measures.as_ref().unwrap();
        let back = swapped(m).unwrap();
        // NaN is written as null, so measures are compared in their serde form
        let value = |x: &Option<Measures>| serde_json::to_value(x).unwrap();
        assert_eq!(value(&cand.measures), value(&Some(m.clone())));
        assert!(cand.reverse.is_some());
        assert_eq!(value(&cand.reverse), value(&reverse_measures(m)));
        assert_eq!(value(&cand.swapped), value(&Some(back.clone())));
        assert_eq!(cand.swapped_verdict, Some(verdict(&back, &rule)));
        // the programme judged against the commentary as its reference: the reverse run's pattern
        assert_eq!(cand.swapped_verdict, Some(Verdict::ReferenceUnclear));
        assert_eq!((cand.verdict, cand.peer.is_none(), cand.before_veto), (Verdict::Commentary, true, None));
        assert_eq!(json.veto_line, ANALYSED[2]);
        assert_eq!(json.dialogue, Dialogue::Fc);
    }

    #[test]
    fn report_of_a_cross_language_commentary_names_both_languages() {
        // The report half of CrossLanguage::test_a_commentary_in_another_language_is_judged_against_the_programme_reference.
        let (mix, centre) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "c1/audio-levels-20ms-1-3";
        tree.insert_file(&format!("{d}/main-mix.txt"), level_file_text(&mix));
        tree.insert_file(&format!("{d}/main-fc.txt"), level_file_text(&centre));
        tree.insert_file(&format!("{d}/cand-3.txt"), level_file_text(&commentary(&mix, 7)));
        tree.insert_file(
            &format!("{d}/manifest.txt"),
            manifest("yes", "aa", "bb").replace("layout=5.1(side) language=eng", "layout=5.1(side) language=spa"),
        );
        let lines = report(&tree, &PathKey::new(d)).unwrap();
        let text = lines.join("\n");
        assert!(text.contains("stream 3: ac3 2 ch stereo eng (cross-language: eng against spa)"), "{text}");
        assert_eq!(lines, CROSS_LANGUAGE);
    }

    #[test]
    fn report_of_a_dub_companion_prints_its_peer_run() {
        // The report half of CrossLanguage::test_a_companion_of_a_dub_follows_the_dub_and_gets_only_a_note.
        let (mix, centre) = programme(420, 1);
        let dub = commentary(&mix, 7);
        let companion = late_copy(&dub);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "c2/audio-levels-20ms-1-2-3";
        tree.insert_file(&format!("{d}/main-mix.txt"), level_file_text(&mix));
        tree.insert_file(&format!("{d}/main-fc.txt"), level_file_text(&centre));
        tree.insert_file(&format!("{d}/cand-2.txt"), level_file_text(&dub));
        tree.insert_file(&format!("{d}/cand-3.txt"), level_file_text(&companion));
        tree.insert_file(&format!("{d}/manifest.txt"), COMPANION_MANIFEST);
        let lines = report(&tree, &PathKey::new(d)).unwrap();
        let text = lines.join("\n");
        assert!(text.contains("    peer: against stream 2 (placed first in the candidate's language): DC 1.00"), "{text}");
        assert!(text.contains("keep, so not confirmed"), "{text}");
        assert_eq!(lines, COMPANION);
    }

    #[test]
    fn report_of_a_vetoed_folder_prints_the_veto_and_what_rows_3_to_9_said() {
        // The report half of PositionVeto::test_it_never_turns_a_keep_into_a_drop.
        let (mix, centre) = programme(420, 1);
        let mut tree = MemTree::new("tmp", "tmp");
        let d = "pv/audio-levels-20ms-2-1-3-4";
        tree.insert_file(&format!("{d}/main-mix.txt"), level_file_text(&mix));
        tree.insert_file(&format!("{d}/main-fc.txt"), level_file_text(&centre));
        tree.insert_file(&format!("{d}/cand-1.txt"), level_file_text(&late_copy(&mix)));
        tree.insert_file(&format!("{d}/cand-3.txt"), level_file_text(&commentary(&mix, 7)));
        tree.insert_file(&format!("{d}/cand-4.txt"), level_file_text(&mix));
        tree.insert_file(&format!("{d}/manifest.txt"), VETO_MANIFEST);
        tree.insert_file(&format!("{d}/streams.json"), VETO_STREAMS);
        let lines = report(&tree, &PathKey::new(d)).unwrap();
        let text = lines.join("\n");
        assert!(
            text.contains("  veto: holds (row 0, D21): the reference, stream 2 (dts DTS-HD MA 6 ch), is not the first eng stream of the file: stream 1 (dts DTS 6 ch) comes first"),
            "{text}"
        );
        assert!(
            text.contains("verdict: reference-unclear (the position veto holds (row 0, D21): the reference may not be the programme, so no commentary is confirmed on this title: keep, review (audio_reference_unclear); rows 3 to 9 alone: commentary)"),
            "{text}"
        );
        assert_eq!(lines, VETOED);
    }
}
