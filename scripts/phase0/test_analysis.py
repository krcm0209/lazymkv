import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

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


class SubsDir(unittest.TestCase):
    """--subs-dir reads a subs folder written by probe-title.ps1; text streams count every packet (spec 8.7)."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="lazymkv-test-")
        self.addCleanup(tmp.cleanup)
        self.dir = pathlib.Path(tmp.name)

    def write_subs(self, index, packets):
        """index.json as probe-title.ps1 writes it (UTF-8 with a BOM), and one packets file per stream."""
        (self.dir / "index.json").write_text(json.dumps(index), encoding="utf-8-sig")
        for n, items in packets.items():
            (self.dir / f"sub-{n}.packets.json").write_text(json.dumps({"packets": items}), encoding="utf-8")

    def run_main(self, *args):
        out = io.StringIO()
        with mock.patch("sys.argv", ["subtitle_alignment.py", *args]), contextlib.redirect_stdout(out):
            subtitle_alignment.main()
        return out.getvalue()

    def test_text_codecs_are_told_from_image_codecs(self):
        for codec in ("SubRip/SRT", "SubStationAlpha", "WebVTT", "HDMV TextST"):
            self.assertTrue(subtitle_alignment.is_text(codec), codec)
        for codec in ("HDMV PGS", "VobSub", "", None):
            self.assertFalse(subtitle_alignment.is_text(codec), codec)

    def test_only_english_and_no_language_streams_are_kept(self):
        self.write_subs(
            [
                {"n": 0, "language": "eng", "codec": "HDMV PGS", "track_name": None, "forced": False},
                {"n": 1, "language": "fre", "codec": "HDMV PGS", "track_name": None, "forced": False},
                {"n": 2, "language": "eng", "codec": "HDMV PGS", "track_name": None, "forced": True},
                {"n": 3, "language": None, "codec": "SubRip/SRT", "track_name": None, "forced": None},
            ],
            {},
        )
        names = [name for name, _, _, _ in subtitle_alignment.index_streams(self.dir)]
        self.assertEqual(names, ["s0-eng-image", "s2-eng-image-forced", "s3-und-text"])

    def test_an_index_in_another_encoding_is_still_read(self):
        text = '[{"n": 0, "language": "eng", "codec": "VobSub", "track_name": "Fran_ais", "forced": false}]'
        (self.dir / "index.json").write_bytes(text.encode("ascii").replace(b"_", b"\xe7"))
        names = [name for name, _, _, _ in subtitle_alignment.index_streams(self.dir)]
        self.assertEqual(names, ["s0-eng-image"])

    def test_text_streams_count_every_packet_and_image_streams_drop_clear_packets(self):
        starts = [10.0 + 4 * i for i in range(20)]
        image = []
        for s in starts:
            image.append({"pts_time": f"{s:.3f}", "size": "5000"})
            image.append({"pts_time": f"{s + 2:.3f}", "size": "30"})
        text = [{"pts_time": f"{s:.3f}", "size": str(20 + (i % 3) * 20)} for i, s in enumerate(starts)]
        self.write_subs(
            [
                {"n": 0, "language": "eng", "codec": "VobSub", "track_name": None, "forced": False},
                {"n": 1, "language": "eng", "codec": "SubRip/SRT", "track_name": "CC", "forced": False},
            ],
            {0: image, 1: text},
        )
        out = self.run_main("--subs-dir", str(self.dir), "--clear-max-bytes", "64")
        self.assertIn("s0-eng-image: events=20", out)
        self.assertIn("s1-eng-text: events=20", out)
        pairs = [line.split() for line in out.splitlines() if line.startswith("s0-eng-image") and "s1-eng-text" in line]
        self.assertEqual(pairs, [["s0-eng-image", "s1-eng-text", "1.00"]])

    def test_inputs_and_a_subs_dir_are_not_mixed(self):
        for args in ((), ("a=x.json", "--subs-dir", str(self.dir))):
            err = io.StringIO()
            with contextlib.redirect_stderr(err), self.assertRaises(SystemExit):
                self.run_main(*args)
            self.assertIn("give either name=path inputs or --subs-dir", err.getvalue())


if __name__ == "__main__":
    unittest.main()
