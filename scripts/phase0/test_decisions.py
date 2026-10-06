"""The scripts that answer an experiment with yes or no must not say yes on empty or broken input."""
import contextlib
import io
import pathlib
import tempfile
import unittest

import compare_scans
import compare_tracks
import exp02_check
import hit_rate
import robot

CORPUS = pathlib.Path(__file__).resolve().parents[2] / "corpus" / "thediscdb"
JOHN_WICK = CORPUS / "movie" / "John Wick (2014)" / "4k-ultra-hd--blu-ray" / "disc01.txt"

ADDED = 'MSG:3307,0,2,"File {f} was added as title #{n}","File %1 was added as title #%2","{f}","{n}"\n'
SAVING = 'MSG:5014,0,2,"Saving 1 titles into directory x","Saving %1 titles into directory %2","1","x"\n'
SCAN = 'TINFO:0,16,0,"00001.mpls"\nTINFO:1,16,0,"00002.mpls"\n'
BOTH_ADDED = ADDED.format(f="00001.mpls", n=0) + ADDED.format(f="00002.mpls", n=1) + SAVING

# One title: video, then two audio tracks of the same language with 8 and 2 channels.
TRACK_SCAN = (
    'TINFO:0,8,0,"2"\nTINFO:0,9,0,"0:10:00"\n'
    'SINFO:0,0,1,6201,"Video"\nSINFO:0,0,6,0,"MpegH"\n'
    'SINFO:0,1,1,6202,"Audio"\nSINFO:0,1,3,0,"{lang}"\nSINFO:0,1,6,0,"TrueHD"\nSINFO:0,1,14,0,"8"\n'
    'SINFO:0,2,1,6202,"Audio"\nSINFO:0,2,3,0,"{lang}"\nSINFO:0,2,6,0,"DD"\nSINFO:0,2,14,0,"2"\n'
)


def quiet(function, *args):
    """Call a script's main and return (result, what it printed)."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        result = function(*args)
    return result, out.getvalue()


def ident(tracks, chapters=2, seconds=600):
    """A stand-in for `mkvmerge -J`: tracks are (type, language, audio channels or None)."""
    return {
        "tracks": [
            {"id": i, "type": kind, "codec": "x",
             "properties": dict({"language": lang}, **({"audio_channels": ch} if ch else {}))}
            for i, (kind, lang, ch) in enumerate(tracks)
        ],
        "chapters": [{"num_entries": chapters}] if chapters else [],
        "container": {"properties": {"duration": seconds * 10**9}},
    }


class TempDir(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        self.addCleanup(tmp.cleanup)
        self.dir = pathlib.Path(tmp.name)

    def write(self, name, text):
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path


class Exp02(TempDir):
    def test_yes_when_every_run_maps_its_title_to_the_scans_source(self):
        self.write("scan.txt", SCAN)
        self.write("exp02/title-0.txt", BOTH_ADDED)
        self.write("exp02/title-1.txt", BOTH_ADDED)
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertTrue(ok)
        self.assertIn("EXPERIMENT 2: yes (2 titles)", out)

    def test_the_saving_message_of_a_profile_rip_counts(self):
        # With --profile, MakeMKV 2.0.0 prints MSG:5015 where a plain rip prints MSG:5014.
        self.write("scan.txt", SCAN)
        self.write("exp02/title-0.txt", BOTH_ADDED.replace("MSG:5014,", "MSG:5015,"))
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertTrue(ok)
        self.assertIn("5014=yes added-before-5014=yes", out)

    def test_no_when_the_scan_does_not_list_the_title(self):
        self.write("scan.txt", "")
        self.write("exp02/title-3.txt", SAVING)
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertFalse(ok)
        self.assertIn("match=NO", out)
        self.assertIn("added-before-5014=NO", out)
        self.assertIn("EXPERIMENT 2: no (1 titles)", out)

    def test_no_when_the_rip_maps_the_title_to_another_file(self):
        self.write("scan.txt", SCAN)
        self.write("exp02/title-0.txt", ADDED.format(f="00002.mpls", n=0) + SAVING)
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertFalse(ok)
        self.assertIn("match=NO", out)

    def test_no_when_a_title_is_added_after_5014(self):
        self.write("scan.txt", SCAN)
        self.write("exp02/title-0.txt", SAVING + ADDED.format(f="00001.mpls", n=0))
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertFalse(ok)
        self.assertIn("match=yes", out)
        self.assertIn("added-before-5014=NO", out)

    def test_no_when_a_title_number_is_added_for_two_files(self):
        self.write("scan.txt", SCAN)
        run = ADDED.format(f="00002.mpls", n=0) + ADDED.format(f="00001.mpls", n=0) + SAVING
        self.write("exp02/title-0.txt", run)
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertFalse(ok)
        self.assertIn("rip #0=00001.mpls/00002.mpls match=NO", out)

    def test_no_without_runs(self):
        self.write("scan.txt", SCAN)
        ok, out = quiet(exp02_check.main, self.dir)
        self.assertFalse(ok)
        self.assertIn("EXPERIMENT 2: no (0 titles)", out)


class CompareScans(TempDir):
    def test_a_scan_equals_itself(self):
        code, out = quiet(compare_scans.main, JOHN_WICK, JOHN_WICK)
        self.assertEqual(code, 0)
        self.assertIn("only in A: 0", out)
        self.assertIn("only in B: 0", out)

    def test_empty_scans_do_not_compare_equal(self):
        a, b = self.write("a.txt", ""), self.write("b.txt", "garbage\n")
        code, out = quiet(compare_scans.main, a, b)
        self.assertEqual(code, 1)
        self.assertIn("EMPTY SCAN", out)

    def test_a_title_listed_twice_on_one_side_is_a_difference(self):
        one = 'TINFO:{n},16,0,"00001.mpls"\nTINFO:{n},26,0,"1"\nTINFO:{n},9,0,"1:00:00"\n'
        a = self.write("a.txt", one.format(n=0) + one.format(n=1))
        b = self.write("b.txt", one.format(n=0))
        code, out = quiet(compare_scans.main, a, b)
        self.assertEqual(code, 1)
        self.assertIn("titles: A=2 B=1 common=1", out)
        self.assertIn("only in A: 1", out)


class CompareTracks(unittest.TestCase):
    def verdicts(self, lang, tracks, **file_facts):
        scan = robot.parse(TRACK_SCAN.format(lang=lang))
        file = ident(tracks, **file_facts)
        want = compare_tracks.expected_rows(scan, 0, "all")
        verdicts, _ = compare_tracks.checks(want, compare_tracks.file_rows(file), scan.titles[0], file)
        return verdicts

    def test_a_matching_file_passes_every_check(self):
        verdicts = self.verdicts("eng", [("video", "und", None), ("audio", "eng", 8), ("audio", "eng", 2)])
        self.assertEqual(
            verdicts, {"count": True, "order": True, "video": True, "chapters": True, "duration": True}
        )

    def test_same_language_audio_tracks_in_the_wrong_order_fail(self):
        verdicts = self.verdicts("eng", [("video", "und", None), ("audio", "eng", 2), ("audio", "eng", 8)])
        self.assertTrue(verdicts["count"])
        self.assertFalse(verdicts["order"])

    def test_both_iso_639_2_forms_name_the_same_language(self):
        verdicts = self.verdicts("fra", [("video", "und", None), ("audio", "fre", 8), ("audio", "fre", 2)])
        self.assertTrue(verdicts["order"])

    def test_the_language_of_a_video_track_is_not_compared(self):
        verdicts = self.verdicts("eng", [("video", "eng", None), ("audio", "eng", 8), ("audio", "eng", 2)])
        self.assertTrue(verdicts["order"])

    def test_a_stream_makemkv_removed_as_empty_is_read_from_the_rip_log(self):
        removed = 'Forced subtitles track #2 turned out to be empty and was removed from output file'
        rip = robot.parse(
            f'MSG:4047,16,3,"AV sync issue in stream 2 at 0:00:00 : {removed}",'
            f'"AV sync issue in stream %1 at %2 : %3","2","0:00:00","{removed}"\n'
            'MSG:4047,16,3,"AV sync issue in stream 1 at 0:01:00 : audio gap",'
            '"AV sync issue in stream %1 at %2 : %3","1","0:01:00","audio gap"\n'
        )
        self.assertEqual(compare_tracks.removed_streams(rip), {2})

    def test_a_removed_stream_is_not_expected_in_the_file(self):
        scan = robot.parse(TRACK_SCAN.format(lang="eng"))
        file = ident([("video", "und", None), ("audio", "eng", 8)])
        want, dropped = compare_tracks.without_removed(compare_tracks.expected_rows(scan, 0, "all"), {2})
        self.assertEqual([r["stream"] for r in dropped], [2])
        # MSG:4047 counts the selected streams, not the scan's: with the first stream deselected,
        # output track 1 is scan stream 2.
        rows = compare_tracks.expected_rows(scan, 0, "all")[1:]
        want2, dropped2 = compare_tracks.without_removed(rows, {1})
        self.assertEqual([r["stream"] for r in dropped2], [2])
        self.assertEqual([r["stream"] for r in want2], [r["stream"] for r in rows if r["stream"] != 2])
        verdicts, _ = compare_tracks.checks(want, compare_tracks.file_rows(file), scan.titles[0], file)
        self.assertTrue(verdicts["count"])
        self.assertTrue(verdicts["order"])

    def test_chapter_count_and_duration_are_checked(self):
        verdicts = self.verdicts(
            "eng", [("video", "und", None), ("audio", "eng", 8), ("audio", "eng", 2)], chapters=1, seconds=500
        )
        self.assertTrue(verdicts["order"])
        self.assertFalse(verdicts["chapters"])
        self.assertFalse(verdicts["duration"])

    def test_a_stream_without_a_language_code_is_und_not_the_metadata_language(self):
        # Attribute 28 is the disc's metadata language ("eng" even on Chinese subtitles); only
        # attribute 3 names a stream's language, and a stream without one has no language (spec 8.6).
        scan = robot.parse(
            'TINFO:0,8,0,"2"\nTINFO:0,9,0,"0:10:00"\n'
            'SINFO:0,0,1,6201,"Video"\nSINFO:0,0,6,0,"MpegH"\nSINFO:0,0,28,0,"eng"\n'
            'SINFO:0,1,1,6202,"Audio"\nSINFO:0,1,6,0,"DD"\nSINFO:0,1,14,0,"2"\nSINFO:0,1,28,0,"eng"\n'
            'SINFO:0,2,1,6203,"Subtitles"\nSINFO:0,2,3,0,"zho"\nSINFO:0,2,6,0,"PGS"\nSINFO:0,2,28,0,"eng"\n'
            'SINFO:0,3,1,6203,"Subtitles"\nSINFO:0,3,6,0,"PGS"\nSINFO:0,3,28,0,"eng"\n'
        )
        want = compare_tracks.expected_rows(scan, 0, "all")
        self.assertEqual([r["lang"] for r in want], ["eng", "und", "zho", "und"])
        eng = compare_tracks.expected_rows(scan, 0, "eng")
        self.assertEqual([r["stream"] for r in eng], [0, 1])
        file = ident([("video", "und", None), ("audio", "und", 2)])
        verdicts, _ = compare_tracks.checks(eng, compare_tracks.file_rows(file), scan.titles[0], file)
        self.assertTrue(verdicts["order"])

    def test_an_empty_file_against_an_empty_title_fails(self):
        scan = robot.parse('TINFO:0,9,0,"0:10:00"\n')
        file = ident([], chapters=0, seconds=0)
        verdicts, _ = compare_tracks.checks([], compare_tracks.file_rows(file), scan.titles[0], file)
        self.assertEqual(
            verdicts, {"count": False, "order": False, "video": False, "chapters": True, "duration": False}
        )


class HitRate(TempDir):
    def disc(self, name, status, hash_body=None, id_body=None):
        self.write(f"{name}/meta.json", '{"format": "UHD"}')
        self.write(f"{name}/lookup-status.txt", status)
        if hash_body is not None:
            self.write(f"{name}/thediscdb-hash.json", hash_body)
        if id_body is not None:
            self.write(f"{name}/thediscdb-discid.json", id_body)

    def test_hits_and_misses_are_counted_when_every_lookup_answered(self):
        self.disc("a", "dischash 200\ndiscid 200\n", '{"titles": [{"index": 0}]}', '[{"titles": []}]')
        self.disc("b", "dischash 404\ndiscid 200\n", '{"title": "Not Found"}', "[]")
        code, out = quiet(hit_rate.main, self.dir)
        self.assertEqual(code, 0)
        self.assertIn("TheDiscDB knows 1 of 2 discs by fingerprint", out)
        self.assertIn("by fingerprint or disc ID: 1 of 2 discs", out)

    def test_a_failed_lookup_is_not_a_miss(self):
        self.disc("a", "dischash 500\n", '{"errors": "x"}')
        self.disc("b", "dischash 200\n", "not json")
        self.disc("c", "")
        code, out = quiet(hit_rate.main, self.dir)
        self.assertEqual(code, 1)
        self.assertNotIn("hash=miss", out)
        self.assertIn("TheDiscDB knows 0 of 0 discs by fingerprint", out)
        self.assertIn("3 fingerprint lookups failed or were not made", out)

    def test_a_failed_disc_id_lookup_is_reported(self):
        self.disc("a", "dischash 404\ndiscid 429\n", '{"title": "Not Found"}', "rate limited")
        code, out = quiet(hit_rate.main, self.dir)
        self.assertEqual(code, 1)
        self.assertIn("hash=miss", out)
        self.assertIn("discid=failed", out)
        self.assertIn("1 disc ID lookups failed", out)

    def test_a_200_answer_that_is_not_a_disc_record_is_a_failure(self):
        self.disc("a", "dischash 200\n", '{"errors": "x"}')
        code, out = quiet(hit_rate.main, self.dir)
        self.assertEqual(code, 1)
        self.assertIn("hash=failed", out)

    def test_a_recording_without_a_status_file_is_not_a_miss(self):
        self.write("a/meta.json", '{"format": "DVD"}')
        self.write("a/thediscdb-hash.json", '{"titles": [{"index": 0}]}')
        code, out = quiet(hit_rate.main, self.dir)
        self.assertEqual(code, 1)
        self.assertIn("hash=n/a", out)
        self.assertIn("TheDiscDB knows 0 of 0 discs by fingerprint", out)


class ParseLines(unittest.TestCase):
    def test_only_a_newline_ends_a_line(self):
        scan = robot.parse('\ufeffTINFO:0,2,0,"A\u2028B"\r\nTINFO:0,9,0,"0:10:00"\r\n')
        self.assertEqual(scan.titles[0][2], "A\u2028B")
        self.assertEqual(scan.titles[0][9], "0:10:00")


if __name__ == "__main__":
    unittest.main()
