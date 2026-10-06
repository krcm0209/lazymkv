"""Tests for ocr_compare.py (the ocrs trial). Constructed lines only: no line of any real subtitle track."""
import contextlib
import io
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import ocr_compare as oc  # noqa: E402

NOTE = chr(0x266A)
E_ACUTE = chr(0x00E9)


def frames(texts):
    return [{'frame': k, 'file': 'f-%08d.png' % k, 'lines': lines} for k, lines in enumerate(texts)]


def write_jsonl(path, rows):
    path.write_text(''.join(json.dumps(r) + '\n' for r in rows), encoding='utf-8')


class Distance(unittest.TestCase):
    def test_levenshtein(self):
        self.assertEqual(oc.levenshtein('', ''), 0)
        self.assertEqual(oc.levenshtein('abc', ''), 3)
        self.assertEqual(oc.levenshtein('', 'abc'), 3)
        self.assertEqual(oc.levenshtein('kitten', 'sitting'), 3)
        self.assertEqual(oc.levenshtein('sitting', 'kitten'), 3)
        self.assertEqual(oc.levenshtein('flaw', 'lawn'), 2)

    def test_cer_is_over_the_longer_text(self):
        self.assertEqual(oc.cer('', ''), 0.0)
        self.assertEqual(oc.cer('abcd', ''), 1.0)
        self.assertEqual(oc.cer('', 'abcd'), 1.0)
        self.assertEqual(oc.cer('abcd', 'abcd'), 0.0)
        self.assertAlmostEqual(oc.cer('abcd', 'abxd'), 0.25)
        self.assertAlmostEqual(oc.cer('ab', 'abcd'), 0.5)


class Normalise(unittest.TestCase):
    def test_fold_and_english_corrections(self):
        # An accent and the music notes fold as subtitle_text reads them; Tesseract's '|' for I is corrected.
        got = oc.norm_lines(['caf' + E_ACUTE + '  bleu', '  ', NOTE + ' la la ' + NOTE, '| wonder'])
        self.assertEqual(got, ['cafe bleu', '# la la #', 'I wonder'])

    def test_other_language_takes_the_pipe_as_a_space(self):
        self.assertEqual(oc.norm_lines(['a|b'], english=False), ['a b'])
        self.assertEqual(oc.norm_lines(['a|b']), ['alb'])

    def test_a_different_line_split_costs_nothing(self):
        self.assertEqual(oc.norm_text(['one two', 'three']), oc.norm_text(['one', 'two three']))


class Reading(unittest.TestCase):
    def test_lines_and_the_summary_line(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'ocrs-s0.jsonl'
            write_jsonl(p, [{'frame': 0, 'file': 'f-1.png', 'lines': [' Zabo kelu ', ''], 'ms': 5.0},
                            {'summary': {'frames': 1, 'ms_per_frame': 5.0}}])
            got, summary = oc.read_frames(p)
        self.assertEqual(got, [{'frame': 0, 'file': 'f-1.png', 'lines': ['Zabo kelu'], 'ms': 5.0}])
        self.assertEqual(summary['frames'], 1)

    def test_joined_text_is_split_with_the_line_count(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'sub-0.ocr.jsonl'
            write_jsonl(p, [{'frame': 0, 'file': 'f-1.png', 'text': 'Zabo kelu | Mira tosa'},
                            {'frame': 1, 'file': 'f-2.png', 'text': ''}])
            got, summary = oc.read_frames(p, {0: 2})
        self.assertEqual([f['lines'] for f in got], [['Zabo kelu', 'Mira tosa'], []])
        self.assertIsNone(summary)

    def test_stream_number_from_either_name(self):
        self.assertEqual(oc.stream_number('a/sub-12.ocr.jsonl', 'b/ocrs-s3.jsonl'), 12)
        self.assertEqual(oc.stream_number('a/copy.jsonl', 'b/ocrs-s3.jsonl'), 3)
        self.assertEqual(oc.stream_number('a/copy.jsonl', 'b/ocrs-s4-crop16.jsonl'), 4)
        self.assertIsNone(oc.stream_number('a/copy.jsonl', 'b/other.jsonl'))

    def test_crop_number(self):
        self.assertEqual(oc.crop_number('b/ocrs-s4-crop16.jsonl'), 16)
        self.assertEqual(oc.crop_number('b/ocrs-s4.jsonl'), 0)

    def test_stream_entry_by_crop(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / 'ocrs-summary.json'
            # The first entry predates -Crop: it has no crop and counts as the full frame.
            streams = [{'n': 0, 'ms_per_frame': 1.0}, {'n': 0, 'crop': 16, 'ms_per_frame': 2.0},
                       {'n': 1, 'crop': 0, 'ms_per_frame': 3.0}]
            p.write_text(json.dumps({'streams': streams}), encoding='utf-8')
            self.assertEqual(oc.stream_entry(p, 0, 0)['ms_per_frame'], 1.0)
            self.assertEqual(oc.stream_entry(p, 0, 16)['ms_per_frame'], 2.0)
            self.assertEqual(oc.stream_entry(p, 0)['ms_per_frame'], 1.0)
            self.assertIsNone(oc.stream_entry(p, 1, 16))


class Pairing(unittest.TestCase):
    def test_by_file_name_with_a_limited_ocrs_run(self):
        pairs, method, only_t, only_o = oc.pair_frames(frames([[]] * 5), frames([[]] * 3))
        self.assertEqual((len(pairs), method, only_t, only_o), (3, 'file name', 2, 0))

    def test_by_file_name_not_by_position(self):
        tess = [{'frame': 0, 'file': 'f-a.png', 'lines': ['x']}, {'frame': 1, 'file': 'f-b.png', 'lines': ['y']}]
        ocrs = [{'frame': 0, 'file': 'f-b.png', 'lines': ['y']}]
        pairs = oc.pair_frames(tess, ocrs)[0]
        self.assertEqual([(t['frame'], o['frame']) for t, o in pairs], [(1, 0)])

    def test_by_frame_number_when_a_name_is_missing(self):
        tess = [{'frame': 0, 'file': None, 'lines': []}, {'frame': 1, 'file': None, 'lines': []}]
        ocrs = [{'frame': 1, 'file': 'f-b.png', 'lines': []}, {'frame': 7, 'file': 'f-z.png', 'lines': []}]
        pairs, method, only_t, only_o = oc.pair_frames(tess, ocrs)
        self.assertEqual((len(pairs), method, only_t, only_o), (1, 'frame number', 1, 1))


class Compare(unittest.TestCase):
    def test_counts_rates_and_markers(self):
        tess = frames([['[DOOR KNOCKS]'], ['BOLA: Zabo kelu mira.'], [], ['caf' + E_ACUTE + ' tosa'], ['| wonder']])
        ocrs = frames([['[DOOR KNOCKS]'], ['BOLA: Zabo kelu mlra.'], [], ['cafe tosa'], []])
        c = oc.compare(tess, ocrs)
        self.assertEqual((c['pairs'], c['method'], c['only_tess'], c['only_ocrs']), (5, 'file name', 0, 0))
        self.assertEqual((c['text_tess'], c['text_ocrs'], c['both'], c['either'], c['neither']), (4, 3, 3, 4, 1))
        self.assertEqual(c['exact'], 2)  # the cue, and the accent once folded
        self.assertAlmostEqual(c['cer_mean'], (0 + 1 / 21 + 0 + 1) / 4)
        self.assertEqual(c['cer_high'], 1)  # the frame ocrs left empty
        self.assertEqual(c['same_lines'], 3)
        self.assertEqual([r['frame'] for r in c['worst']], [4, 1])
        for m in (c['m_tess'], c['m_ocrs']):
            self.assertEqual((m['CUE'], m['LABEL'], m['CHEV'], m['NOTE']), (1, 1, 0, 0))
        self.assertEqual((c['m_tess']['events'], c['m_ocrs']['events']), (4, 3))

    def test_report_shows_no_text(self):
        c = oc.compare(frames([['Quorbel fen.']]), frames([['Quorbe1 fen.']]))
        lines = []
        oc.report(c, out=lines.append)
        text = '\n'.join(lines)
        self.assertIn('frame 0 f-00000000.png chars 12/12 lines 1/1 CER 0.083', text)
        self.assertNotIn('Quorbe', text)


class Main(unittest.TestCase):
    def run_main(self, argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = oc.main(argv)
        return code, out.getvalue(), err.getvalue()

    def write_pair(self, d):
        tess = [dict(f, t=k * 0.25, pts=k * 0.25, text=' | '.join(f['lines']))
                for k, f in enumerate(frames([['Vorpal zindle quax.'], ['[ZORBLE HUMS]'], []]))]
        ocrs = [dict(f, ms=900.0) for f in frames([['Vorpa1 zindle quax.'], ['[ZORBLE HUMS]'], []])]
        ocrs.append({'summary': {'frames': 3, 'ms_total': 2700.0, 'ms_per_frame': 900.0, 'load_ms': 800.0,
                                 'ocrs': '0.13.1', 'models': {'detection': 'aa', 'recognition': 'bb'}}})
        write_jsonl(d / 'sub-2.ocr.jsonl', tess)
        write_jsonl(d / 'ocrs-s2.jsonl', ocrs)
        return [str(d / 'sub-2.ocr.jsonl'), str(d / 'ocrs-s2.jsonl')]

    def test_numbers_and_speed_from_the_recording(self):
        with tempfile.TemporaryDirectory() as d:
            d = pathlib.Path(d)
            rec = d / 'rec'
            rec.mkdir()
            streams = [{'n': 1, 'frames': 9, 'tesseract_s': 9.0}, {'n': 2, 'frames': 3, 'tesseract_s': 1.5}]
            (rec / 'summary.json').write_text(json.dumps({'streams': streams}), encoding='utf-8')
            (rec / 'ocrs-summary.json').write_text(json.dumps({'streams': [
                {'n': 2, 'frames': 3, 'ms_per_frame': 910.0, 'load_ms': 750.0}]}), encoding='utf-8')
            # The ocrs file's own summary line is the run compared, so it wins over ocrs-summary.json.
            code, out, err = self.run_main(self.write_pair(d) + ['--rec', str(rec)])
        self.assertEqual(code, 0, err)
        self.assertIn('paired by file name: 3 frames; only in tesseract 0, only in ocrs 0', out)
        self.assertIn('frames with text: tesseract 2, ocrs 2, both 2, either 2, neither 1', out)
        self.assertIn('exact match after folding: 1 of 2 frames with text in either (0.500)', out)
        self.assertEqual(out.count('CUE=1 '), 2)
        self.assertIn('speed: tesseract 500.0 ms/frame (summary.json: 1.5 s for 3 frames, one call); '
                      'ocrs 900.0 ms/frame over 3 frames (+ 800.0 ms model load)', out)
        # An output from before --crop: its ms was decode + OCR, and it says so.
        self.assertIn('ocrs per frame: decode + OCR (an output from before --crop) 900.0 ms mean over 3 frames read',
                      out)
        for word in ('Vorpa', 'zindle', 'quax', 'ZORBLE'):
            self.assertNotIn(word, out + err)

    def test_speed_from_the_summary_line_without_a_recording(self):
        with tempfile.TemporaryDirectory() as d:
            code, out, err = self.run_main(self.write_pair(pathlib.Path(d)))
        self.assertEqual(code, 0, err)
        self.assertIn('speed: tesseract n/a (no --rec summary.json entry); '
                      'ocrs 900.0 ms/frame over 3 frames (+ 800.0 ms model load)', out)

    def write_cropped(self, d, summary=True):
        tess = frames([['Vorpal zindle quax.'], ['[ZORBLE HUMS]'], []])
        ocrs = [dict(f, ms=ms, decode_ms=20.0, detect_ms=det, rec_ms=ms - det, crop=box, crop_ms=cms)
                for f, ms, det, box, cms in zip(frames([['Vorpal zindle quax.'], ['[ZORBLE HUMS]'], []]),
                                                (300.0, 200.0, 0.0), (250.0, 150.0, 0.0),
                                                ([500, 900, 900, 120], [600, 950, 700, 60], None), (4.0, 3.0, 2.0))]
        if summary:
            ocrs.append({'summary': {'frames': 3, 'ms_total': 500.0, 'ms_per_frame': 166.7, 'load_ms': 800.0,
                                     'crop_pad': 16, 'read_frames': 2, 'ms_per_read_frame': 250.0}})
        write_jsonl(d / 'sub-2.ocr.jsonl', tess)
        write_jsonl(d / 'ocrs-s2-crop16.jsonl', ocrs)
        return [str(d / 'sub-2.ocr.jsonl'), str(d / 'ocrs-s2-crop16.jsonl')]

    def test_a_cropped_reading(self):
        with tempfile.TemporaryDirectory() as d:
            code, out, err = self.run_main(self.write_cropped(pathlib.Path(d)))
        self.assertEqual(code, 0, err)
        self.assertIn('paired by file name: 3 frames; only in tesseract 0, only in ocrs 0', out)
        self.assertIn('exact match after folding: 2 of 2 frames with text in either (1.000)', out)
        self.assertIn('ocrs 166.7 ms/frame over 3 frames, 250.0 ms per frame read (2 read), crop pad 16 '
                      '(+ 800.0 ms model load)', out)
        self.assertIn('ocrs per frame: OCR 250.0 ms mean over 2 frames read; detection 200.0 ms, recognition 50.0 ms; '
                      'crop: 2 boxes, mean 800 x 90 px, 1 frames without a dark pixel (not read), crop_ms mean 3.00',
                      out)
        for word in ('Vorpa', 'zindle', 'ZORBLE'):
            self.assertNotIn(word, out + err)

    def test_a_cropped_reading_without_a_summary_line_takes_its_own_entry(self):
        with tempfile.TemporaryDirectory() as d:
            d = pathlib.Path(d)
            rec = d / 'rec'
            rec.mkdir()
            (rec / 'ocrs-summary.json').write_text(json.dumps({'streams': [
                {'n': 2, 'frames': 3, 'ms_per_frame': 910.0, 'load_ms': 750.0},
                {'n': 2, 'crop': 16, 'frames': 3, 'ms_per_frame': 170.0, 'load_ms': 700.0}]}), encoding='utf-8')
            code, out, err = self.run_main(self.write_cropped(d, summary=False) + ['--rec', str(rec)])
        self.assertEqual(code, 0, err)
        self.assertIn('ocrs 170.0 ms/frame over 3 frames (+ 700.0 ms model load)', out)

    def test_unreadable_or_disjoint_input_exits_2(self):
        with tempfile.TemporaryDirectory() as d:
            d = pathlib.Path(d)
            argv = self.write_pair(d)
            self.assertEqual(self.run_main([argv[0], str(d / 'missing.jsonl')])[0], 2)
            (d / 'bad.jsonl').write_text('{not json\n', encoding='utf-8')
            self.assertEqual(self.run_main([argv[0], str(d / 'bad.jsonl')])[0], 2)
            write_jsonl(d / 'other.jsonl', [{'frame': 0, 'file': 'f-elsewhere.png', 'lines': []}])
            code, out, err = self.run_main([argv[0], str(d / 'other.jsonl')])
        self.assertEqual(code, 2)
        self.assertIn('no frame is in both files', err)


if __name__ == '__main__':
    unittest.main()
