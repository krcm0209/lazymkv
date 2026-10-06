"""Tests for subtitle_text.py (experiment 14). Constructed text only: no line of any real subtitle track."""
import contextlib
import io
import json
import pathlib
import random
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import subtitle_text as st  # noqa: E402

def made_up(consonants, vowels, count):
    """Made-up words (letters only), so that no real subtitle text is needed."""
    words = []
    for a in consonants:
        for v in vowels:
            for b in consonants:
                for w in vowels:
                    words.append(a + v + b + w + 'n')
    return words[:count]


NOUNS = made_up('bdgkl', 'aeiou', 400)
VERBS = made_up('mprst', 'aeiou', 200)
OTHER = made_up('fhvwz', 'aeiou', 400)
OTHER_VERBS = made_up('cjnqx', 'aeiou', 200)
SPANISH = ('el perro de la casa y los ninos que no tienen nada para comer pero ella dice que esta bien con '
           'su madre aqui en el pueblo cuando llueve').split()


def sentence(rnd, nouns=NOUNS):
    verbs = VERBS if nouns is NOUNS else OTHER_VERBS
    return 'we %s the %s and the %s with the %s' % (rnd.choice(verbs), rnd.choice(nouns), rnd.choice(nouns),
                                                    rnd.choice(nouns))


def track(n, events, tag='eng', forced=False):
    return {'n': n, 'tag': tag, 'forced': forced, 'events': events}


def dialogue(count=120, seed=1, nouns=NOUNS, start=10.0, step=4.0):
    rnd = random.Random(seed)
    return [{'start': start + i * step, 'dur': 2.5, 'lines': [sentence(rnd, nouns).capitalize() + '.']}
            for i in range(count)]


def sdh_version(events, cues=8, labels=6):
    """Adds stand-alone sound cues between events and capital speaker labels to some events."""
    out = [dict(e, lines=list(e['lines'])) for e in events]
    step = len(events) // (max(cues, labels) + 1)
    for k in range(labels):
        out[2 + step * k]['lines'][0] = 'ANNA: ' + out[2 + step * k]['lines'][0]
    sounds = ['(DOOR SLAMS)', '(PHONE RINGING)', '(LAUGHS)', '[sighs]', '(SIREN WAILING)', '(KNOCKING)',
              '(DOG BARKING)', '(WIND HOWLING)', '(COUGHS)', '(THUNDER RUMBLING)']
    for k in range(cues):
        e = events[3 + step * k]
        out.append({'start': e['start'] + 2.8, 'dur': 0.9, 'lines': [sounds[k % len(sounds)]]})
    out.sort(key=lambda e: e['start'])
    return out


def measured(*tracks):
    for t in tracks:
        st.measure(t)
    return list(tracks)


class FoldAndLines(unittest.TestCase):
    def test_fold_notes_accents_and_punctuation(self):
        s = (chr(0x266A) + ' La ni' + chr(0x00F1) + 'a ' + chr(0x00BF) + 'qu' + chr(0x00E9) + '? '
             + chr(0x2019) + chr(0x266B))
        self.assertEqual(st.fold(s), "# La nina que? '#")

    def test_split_joined_uses_the_line_count(self):
        self.assertEqual(st.split_joined('| almost fell. | | went home', 2), ['| almost fell.', '| went home'])
        self.assertEqual(st.split_joined('It is fine if | go', 1), ['It is fine if | go'])
        self.assertEqual(st.split_joined('First line | Second line', None), ['First line', 'Second line'])

    def test_corrections(self):
        c = {}
        self.assertEqual(st.correct(st.fold('| almost fell.'), c), 'I almost fell.')
        self.assertEqual(st.correct('lam here, |!', c), 'I am here, I!')
        self.assertEqual(st.correct('(WHlSTLlNG) BORlS', c), '(WHISTLING) BORIS')
        self.assertEqual(st.correct("l'm sure", c), "I'm sure")
        self.assertGreaterEqual(c['pipe_I'], 2)


class Classes(unittest.TestCase):
    def test_sdh_track(self):
        t, = measured(track(0, sdh_version(dialogue())))
        self.assertEqual(t['m']['class'], 'sdh')
        self.assertEqual(t['m']['SO'], 8)
        self.assertEqual(t['m']['LABEL'], 6)
        self.assertEqual(t['m']['lang'], 'en')

    def test_regular_track(self):
        t, = measured(track(0, dialogue()))
        self.assertEqual(t['m']['class'], 'regular')
        self.assertEqual(t['m']['M'], 0)

    def test_inline_asides_are_not_sdh(self):
        ev = dialogue()
        for k in range(9):
            ev[4 + 10 * k]['lines'][0] += ' (FBI)'
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['M'], 9)
        self.assertEqual(t['m']['SO'], 0)
        self.assertEqual(t['m']['class'], 'unclear')

    def test_bracketed_notes_without_sounds_are_not_sdh(self):
        ev = dialogue()
        for k in range(8):
            ev.append({'start': ev[3 + 12 * k]['start'] + 2.8, 'dur': 0.9, 'lines': ['[SIGN]']})
        ev.sort(key=lambda e: e['start'])
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['SO'], 8)
        self.assertEqual(t['m']['CS'], 0)
        self.assertEqual(t['m']['class'], 'unclear')

    def test_capital_sound_lines_without_brackets_are_unclear(self):
        ev = dialogue()
        for k in range(5):
            ev.append({'start': ev[3 + 12 * k]['start'] + 2.8, 'dur': 0.9, 'lines': ['DOOR SLAMS']})
        ev.sort(key=lambda e: e['start'])
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['CAPS_CUE'], 5)
        self.assertEqual(t['m']['class'], 'unclear')

    def test_all_capital_track_keeps_bracketed_cues_only(self):
        ev = sdh_version(dialogue())
        for e in ev:
            e['lines'] = [l.upper() for l in e['lines']]
        t, = measured(track(0, ev))
        self.assertTrue(t['m']['caps_style'])
        self.assertEqual(t['m']['LABEL'], 0)
        self.assertEqual(t['m']['class'], 'sdh')
        r, = measured(track(0, [dict(e, lines=[l.upper() for l in e['lines']]) for e in dialogue()]))
        self.assertEqual(r['m']['class'], 'regular')

    def test_damaged_brackets_are_unclear(self):
        ev = dialogue()
        for k in range(4):
            ev.append({'start': ev[3 + 12 * k]['start'] + 2.8, 'dur': 0.9, 'lines': ['|door closes|']})
        ev.sort(key=lambda e: e['start'])
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['ODD'], 4)
        self.assertEqual(t['m']['class'], 'unclear')

    def test_note_stand_ins_do_not_make_a_track_unreadable(self):
        ev = sdh_version(dialogue())
        for e in ev[::2]:
            e['lines'] = ['# ' + l + ' #' for l in e['lines']]
        t, = measured(track(0, ev))
        self.assertTrue(t['m']['readable'])
        self.assertEqual(t['m']['class'], 'sdh')
        self.assertGreater(t['m']['NOTE'], 50)

    def test_garbage_is_unreadable(self):
        rnd = random.Random(3)
        ev = [{'start': 10.0 + i, 'dur': 0.8, 'lines': [''.join(rnd.choice('xq#%}{3|') for _ in range(12))]}
              for i in range(60)]
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['class'], 'unreadable')
        # T0: an unreadable track is always reviewed, also a flagged forced track with no dialogue track beside it.
        self.assertEqual(st.decide([dict(t, forced=True)], 'en')[0]['review'], 'subtitle_ambiguous')

    def test_language(self):
        rnd = random.Random(2)
        ev = [{'start': 10.0 + 3 * i, 'dur': 2.0,
               'lines': [' '.join(rnd.choice(SPANISH) for _ in range(8)).capitalize()]} for i in range(80)]
        t, = measured(track(0, ev, tag='spa'))
        self.assertEqual(t['m']['lang'], 'es')
        e, = measured(track(0, dialogue()))
        self.assertEqual(e['m']['lang'], 'en')


class Rule(unittest.TestCase):
    def test_single_tracks(self):
        tracks = measured(track(0, sdh_version(dialogue())))
        self.assertEqual(st.decide(tracks, 'en')[0]['role'], 'sdh')
        tracks = measured(track(0, dialogue()))
        self.assertEqual(st.decide(tracks, 'en')[0], {'role': 'regular', 'review': None, 'note': None})

    def test_sdh_and_regular_by_text_not_by_count(self):
        base = dialogue()
        split = []
        for e in base:
            split.append(dict(e, dur=1.2))
            split.append(dict(e, start=e['start'] + 1.3, dur=1.2))
        tracks = measured(track(0, sdh_version(base)), track(1, split))
        v = st.decide(tracks, 'en')
        self.assertEqual((v[0]['role'], v[1]['role']), ('sdh', 'regular'))
        self.assertIsNone(v[0]['review'])
        self.assertEqual(st.spec87(tracks)[1], 'sdh')

    def test_retimed_regular_stays_in_the_dialogue_group(self):
        base = dialogue()
        late = [dict(e, start=e['start'] + 10.0) for e in base]
        tracks = measured(track(0, sdh_version(base)), track(1, late))
        v = st.decide(tracks, 'en')
        self.assertEqual((v[0]['role'], v[1]['role']), ('sdh', 'regular'))

    def test_two_regular_tracks_go_to_review(self):
        tracks = measured(track(0, dialogue()), track(1, [dict(e, start=e['start'] + 0.1) for e in dialogue()]))
        v = st.decide(tracks, 'en')
        self.assertEqual(v[0]['review'], 'subtitle_ambiguous')
        self.assertEqual(v[1]['review'], 'subtitle_ambiguous')

    def test_forced_subset_with_many_events(self):
        base = dialogue()
        tracks = measured(track(0, sdh_version(base)), track(1, base[40:70]))
        v = st.decide(tracks, 'en')
        self.assertEqual(v[1]['role'], 'forced')
        self.assertEqual(v[0]['role'], 'sdh')

    def test_unrelated_short_track_is_not_forced(self):
        other = dialogue(11, seed=9, nouns=OTHER, start=13.0, step=40.0)
        tracks = measured(track(0, dialogue()), track(1, other))
        v = st.decide(tracks, 'en')
        self.assertEqual(v[1]['role'], 'keep')
        self.assertEqual(v[1]['review'], 'subtitle_ambiguous')
        self.assertEqual(st.spec87(tracks)[1], 'forced')

    def test_commentary_dropped_only_when_words_and_timing_differ(self):
        base = dialogue()
        comm = dialogue(100, seed=5, nouns=OTHER, start=11.7, step=4.0)
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, comm))
        self.assertEqual(st.decide(tracks, 'en')[2]['role'], 'drop')
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, comm))
        v = st.decide(tracks, 'es')
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))
        near = [dict(e, start=e['start'] + (0.1 if i % 2 else 0.6))
                for i, e in enumerate(dialogue(100, seed=5, nouns=OTHER))]
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, near))
        v = st.decide(tracks, 'en')
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))

    def test_mistagged_language_is_kept_for_review(self):
        rnd = random.Random(2)
        ev = [{'start': 10.0 + 3 * i, 'dur': 2.0,
               'lines': [' '.join(rnd.choice(SPANISH) for _ in range(8)).capitalize()]} for i in range(80)]
        tracks = measured(track(0, ev, tag='eng'))
        v = st.decide(tracks, 'es')
        self.assertEqual((v[0]['role'], v[0]['review']), ('keep', 'subtitle_ambiguous'))

    def test_no_language_track_is_read_not_dropped(self):
        rnd = random.Random(4)
        junk = [{'start': 12.0 + 4 * i, 'dur': 0.8, 'lines': [''.join(rnd.choice('xq#%}{3|') for _ in range(12))]}
                for i in range(60)]
        tracks = measured(track(0, dialogue()), track(1, junk, tag='und'))
        v = st.decide(tracks, 'en')
        self.assertEqual(v[0]['role'], 'regular')
        self.assertEqual((v[1]['role'], v[1]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertEqual(st.spec87(tracks)[1], 'drop')
        tracks = measured(track(0, dialogue(), tag='und'))
        self.assertEqual(st.decide(tracks, 'en')[0], {'role': 'regular', 'review': None, 'note': None})

    def test_partial_track_is_a_forced_candidate_or_kept_when_unreadable(self):
        base = dialogue(1700)
        rnd = random.Random(5)
        junk = [{'start': 11.0 + 9 * i, 'dur': 0.8, 'lines': [''.join(rnd.choice('xq#%}{3|') for _ in range(12))]}
                for i in range(160)]
        tracks = measured(track(0, base), track(1, base[100:260]), track(2, junk))
        v = st.decide(tracks, 'en')
        self.assertEqual(v[0]['role'], 'regular')
        self.assertEqual((v[1]['role'], v[1]['review']), ('forced', None))
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertEqual((st.spec87(tracks)[1], st.spec87(tracks)[2]), ('drop', 'drop'))


class Files(unittest.TestCase):
    def write_title(self, root, lines_format):
        rec = root / 'rec'
        (rec / 'subs').mkdir(parents=True)
        (rec / 'subs-ocr').mkdir()
        text = root / 'text'
        text.mkdir()
        (rec / 'subs' / 'index.json').write_text(json.dumps(
            [{'n': 0, 'language': 'eng', 'codec': 'HDMV PGS', 'forced': False}]), encoding='utf-8')
        ev = sdh_version(dialogue(60))
        packets, frames, recs = [], [], []
        for k, e in enumerate(ev):
            packets.append({'pts_time': '%.3f' % e['start'], 'duration_time': '%.3f' % e['dur'], 'size': '4000'})
            packets.append({'pts_time': '%.3f' % (e['start'] + e['dur']), 'duration_time': '0.021', 'size': '30'})
            t = round(e['start'] - ev[0]['start'], 2)
            frames.append({'frame': 2 * k, 'lines': 0})
            frames.append({'frame': 2 * k + 1, 'lines': len(e['lines'])})
            recs.append({'frame': 2 * k, 'file': 'f.png', 't': max(0.0, t - 0.5), 'text': ''})
            if lines_format:
                recs.append({'frame': 2 * k + 1, 'file': 'f.png', 't': t, 'lines': e['lines']})
            else:
                recs.append({'frame': 2 * k + 1, 'file': 'f.png', 't': t, 'text': ' | '.join(e['lines'])})
        (rec / 'subs' / 'sub-0.packets.json').write_text(json.dumps({'packets': packets}), encoding='utf-8')
        (rec / 'subs-ocr' / 'sub-0.frames.json').write_text(json.dumps(frames), encoding='utf-8')
        (text / 'sub-0.ocr.jsonl').write_text('\n'.join(json.dumps(r) for r in recs), encoding='utf-8')
        return rec, text, len(ev)

    def test_main_prints_numbers_and_a_verdict(self):
        for lines_format in (False, True):
            with tempfile.TemporaryDirectory() as d:
                rec, text, n = self.write_title(pathlib.Path(d), lines_format)
                buf = io.StringIO()
                with contextlib.redirect_stdout(buf):
                    code = st.main(['--rec', str(rec), '--text', str(text), '--orig-lang', 'en'])
                out = buf.getvalue()
                self.assertEqual(code, 0)
                self.assertIn('stream s0 eng: events=%d' % n, out)
                self.assertIn('class=sdh', out)
                self.assertIn('text by order', out)
                self.assertIn('| s0 | eng | %d | sdh | sdh |' % n, out)
                for word in NOUNS + VERBS:
                    self.assertNotIn(word, out)

    def test_pairing_by_time_when_counts_differ(self):
        with tempfile.TemporaryDirectory() as d:
            rec, text, n = self.write_title(pathlib.Path(d), True)
            rows = (text / 'sub-0.ocr.jsonl').read_text(encoding='utf-8').splitlines()
            (text / 'sub-0.ocr.jsonl').write_text('\n'.join(rows[:-4]), encoding='utf-8')
            tracks = st.load_title(rec, text, 64)
            self.assertIn('text by time', tracks[0]['pairing'])
            self.assertEqual(sum(1 for e in tracks[0]['events'] if e['lines']), n - 2)

    def test_missing_text_is_an_input_error(self):
        with tempfile.TemporaryDirectory() as d:
            rec, text, n = self.write_title(pathlib.Path(d), True)
            (text / 'sub-0.ocr.jsonl').unlink()
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                self.assertEqual(st.main(['--rec', str(rec), '--text', str(text)]), 2)


if __name__ == '__main__':
    unittest.main()
