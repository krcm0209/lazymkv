import pathlib
import unittest

import robot
import subtitle_alignment

CORPUS = pathlib.Path(__file__).resolve().parents[2] / "corpus" / "thediscdb"
JOHN_WICK = CORPUS / "movie" / "John Wick (2014)" / "4k-ultra-hd--blu-ray" / "disc01.txt"


class SplitFields(unittest.TestCase):
    def test_quoted_fields_keep_commas_and_unescape_backslashes(self):
        line = (
            '3344,0,1,"Using Java runtime from C:\\\\Program Files\\\\Java\\\\bin\\\\java.exe",'
            '"Using Java runtime from %1","C:\\\\Program Files\\\\Java\\\\bin\\\\java.exe"'
        )
        fields = robot.split_fields(line)
        self.assertEqual(fields[0:3], ["3344", "0", "1"])
        self.assertEqual(fields[5], "C:\\Program Files\\Java\\bin\\java.exe")

    def test_both_quote_escapes_are_accepted(self):
        self.assertEqual(robot.split_fields('1,"a \\"b\\", c","d ""e"""'), ["1", 'a "b", c', 'd "e"'])


class Parse(unittest.TestCase):
    def setUp(self):
        self.scan = robot.parse(JOHN_WICK.read_text(encoding="utf-8", errors="replace"))

    def test_titles_and_streams(self):
        self.assertEqual(self.scan.titles[0][16], "00001.mpls")
        self.assertEqual(self.scan.titles[0][9], "1:41:13")
        self.assertEqual(self.scan.titles[0][8], "16")
        self.assertEqual(self.scan.streams[(0, 1)][1], "Audio")
        self.assertEqual(self.scan.streams[(0, 1)][6], "TrueHD")
        self.assertEqual(self.scan.streams[(0, 1)][3], "eng")

    def test_messages_carry_their_parameters(self):
        added = [params for code, _, params in self.scan.messages if code == 3307]
        self.assertEqual(added[0], ["00001.mpls", "0"])
        version = [params for code, _, params in self.scan.messages if code == 1005]
        self.assertTrue(version[0][0].startswith("MakeMKV v"))

    def test_drives(self):
        self.assertEqual(len(self.scan.drives), 16)
        self.assertEqual(self.scan.drives[0][1], "2")

    def test_duration_seconds(self):
        self.assertEqual(robot.duration_seconds("1:41:13"), 6073)
        self.assertEqual(robot.duration_seconds("0:06:00"), 360)


class Alignment(unittest.TestCase):
    def test_events_drop_small_clear_packets(self):
        packets = [
            {"pts_time": "1.000", "size": "5000"},
            {"pts_time": "3.000", "size": "30"},
            {"pts_time": "9.500", "size": "4000"},
            {"size": "4000"},
        ]
        self.assertEqual(subtitle_alignment.events(packets, 64), [1.0, 9.5])

    def test_aligned_is_the_share_of_events_with_a_near_start(self):
        a = [1.0, 2.0, 3.0]
        b = [1.1, 2.5, 3.0]
        self.assertAlmostEqual(subtitle_alignment.aligned(a, b, 0.3), 2 / 3)
        self.assertEqual(subtitle_alignment.aligned([], b, 0.3), 0.0)
        self.assertEqual(subtitle_alignment.aligned(a, [], 0.3), 0.0)


if __name__ == "__main__":
    unittest.main()
