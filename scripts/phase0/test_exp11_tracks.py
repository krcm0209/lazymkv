"""exp11_tracks.py: what a title can test of the eng-only selection, and how a rip's tracks are paired with the scan."""
import pathlib
import unittest

import exp11_tracks

MOVIES = pathlib.Path(__file__).resolve().parents[2] / "corpus" / "thediscdb" / "movie"
JOHN_WICK = MOVIES / "John Wick (2014)" / "4k-ultra-hd--blu-ray" / "disc01.txt"
KNOWING = MOVIES / "Knowing (2009)" / "2010-dvd" / "disc01.txt"


def ident(tracks):
    """A stand-in for `mkvmerge -J`: tracks are (type, language, audio channels or None)."""
    return {"tracks": [
        {"id": i, "type": kind, "codec": "x",
         "properties": dict({"language": lang}, **({"audio_channels": ch} if ch else {}))}
        for i, (kind, lang, ch) in enumerate(tracks)
    ]}


def tags(rows):
    return [(tag, r["stream"] if r else None) for tag, r, _ in rows]


class Coverage(unittest.TestCase):
    def test_a_uhd_with_only_english_audio_tests_neither_special_nor_nolang(self):
        n = exp11_tracks.coverage(exp11_tracks.load_scan(JOHN_WICK), 0)
        self.assertEqual(n, {"audio-lang": 0, "nolang": 0, "sub-lang": 2, "core": 1, "special": 0,
                             "sub-flagged": 0, "audio-512": 0})

    def test_a_dvd_with_a_flagged_commentary_tests_special(self):
        n = exp11_tracks.coverage(exp11_tracks.load_scan(KNOWING), 0)
        self.assertEqual(n, {"audio-lang": 1, "nolang": 0, "sub-lang": 2, "core": 0, "special": 1,
                             "sub-flagged": 2, "audio-512": 0})


class Match(unittest.TestCase):
    # Knowing, title 0: 0 video; audio 1 eng 6ch, 2 spa 6ch, 3 eng 2ch flag 1; subtitles 4 eng, 5 spa,
    # 6 eng flag 1, 7 spa flag 1. The eng prediction keeps 0, 1, 4 and 6.
    def setUp(self):
        self.scan = exp11_tracks.load_scan(KNOWING)

    def test_a_file_as_predicted_has_no_difference(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("audio", "eng", 6), ("subtitles", "eng", None), ("subtitles", "eng", None)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("predicted", 1), ("predicted", 4), ("predicted", 6)])

    def test_a_kept_commentary_and_a_dropped_subtitle_are_named(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("audio", "eng", 6), ("audio", "eng", 2), ("subtitles", "eng", None)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("predicted", 1), ("EXTRA", 3), ("predicted", 4), ("MISSING", 6)])

    def test_a_track_out_of_scan_order_is_unmatched(self):
        rows = exp11_tracks.match(self.scan, 0, ident(
            [("video", "und", None), ("subtitles", "eng", None), ("audio", "eng", 6)]))
        self.assertEqual(tags(rows), [("predicted", 0), ("MISSING", 1), ("predicted", 4), ("UNMATCHED", None), ("MISSING", 6)])


if __name__ == "__main__":
    unittest.main()
