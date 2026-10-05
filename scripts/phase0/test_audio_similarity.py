import pathlib
import tempfile
import unittest

import audio_similarity as sim


def curve(pattern, repeats=50):
    """A loudness curve built from (level in dB, frames) pairs."""
    out = []
    for _ in range(repeats):
        for level, frames in pattern:
            out.extend([float(level)] * frames)
    return out


# A programme that is loud for 3 s and quiet for 2 s, over and over.
PROGRAMME = curve([(-20, 30), (-70, 20)])


class Levels(unittest.TestCase):
    def test_levels_are_read_and_silence_is_the_floor(self):
        with tempfile.TemporaryDirectory(prefix="lazymkv-test-") as tmp:
            path = pathlib.Path(tmp) / "x.txt"
            path.write_text(
                "frame:0    pts:0       pts_time:0\n"
                "lavfi.astats.Overall.RMS_level=-inf\n"
                "frame:1    pts:800     pts_time:0.1\n"
                "lavfi.astats.Overall.RMS_level=-23.5\n",
                encoding="utf-8",
            )
            self.assertEqual(sim.levels(path), [sim.FLOOR, -23.5])


class Measures(unittest.TestCase):
    def test_a_copy_of_the_programme_is_the_same_programme_and_fills_nothing(self):
        m = sim.measures(PROGRAMME, [], PROGRAMME, PROGRAMME, [v - 3 for v in PROGRAMME])
        self.assertAlmostEqual(m["same_programme"], 1.0, places=6)
        self.assertEqual(m["fills_quiet"], 0.0)
        self.assertAlmostEqual(m["side_minus_mid_db"], -3.0)

    def test_a_quieter_mix_of_the_same_programme_is_still_the_same_programme(self):
        quieter = [v - 12 for v in PROGRAMME]
        m = sim.measures(PROGRAMME, [], quieter, quieter, quieter)
        self.assertAlmostEqual(m["same_programme"], 1.0, places=6)
        self.assertEqual(m["fills_quiet"], 0.0)

    def test_a_voice_that_never_pauses_fills_the_quiet_moments(self):
        voice = curve([(-25, 37), (-28, 6)], repeats=58)[:len(PROGRAMME)]  # steady speech with its own rhythm
        m = sim.measures(PROGRAMME, [], voice, voice, [v - 40 for v in voice])
        self.assertLess(abs(m["same_programme"]), 0.3)
        self.assertEqual(m["fills_quiet"], 1.0)
        self.assertGreater(m["cand_active"], m["main_active"])
        self.assertLess(m["cand_range_db"], m["main_range_db"])
        self.assertAlmostEqual(m["side_minus_mid_db"], -40.0)

    def test_the_dialogue_channel_is_used_when_the_main_track_has_one(self):
        dialogue = curve([(-20, 10), (-70, 40)])  # dialogue only in the first second of every five
        narrator = curve([(-70, 30), (-25, 20)])  # speaks only where the programme is quiet
        m = sim.measures(PROGRAMME, dialogue, narrator, narrator, narrator)
        self.assertEqual(m["fills_quiet"], 1.0)
        self.assertEqual(m["fills_no_dialogue"], 0.5)

    def test_empty_input_gives_no_evidence(self):
        m = sim.measures([], [], [], [], [])
        self.assertEqual((m["frames"], m["same_programme"], m["fills_quiet"]), (0, 0.0, 0.0))


if __name__ == "__main__":
    unittest.main()
