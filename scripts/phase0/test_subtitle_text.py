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
    sounds = ['(DOOR SLAMS)', '(PHONE RINGING)', '(GLASS SHATTERS)', '[sighs]', '(SIREN WAILING)', '(KNOCKING)',
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


def junk(count, seed=3, start=10.0, step=1.0):
    """Events of OCR junk: an unreadable track."""
    rnd = random.Random(seed)
    return [{'start': start + i * step, 'dur': 0.8, 'lines': [''.join(rnd.choice('xq#%}{3|') for _ in range(12))]}
            for i in range(count)]


def unclear(count=100, seed=6, start=11.7, step=4.0):
    """Readable made-up text with one English stopword in nine words: its language is 'unknown' (T1b)."""
    rnd = random.Random(seed)
    return [{'start': start + i * step, 'dur': 2.5,
             'lines': [('the ' + ' '.join(rnd.choice(OTHER) for _ in range(8))).capitalize() + '.']}
            for i in range(count)]


def garbled(events):
    """The same events with OCR junk after every line: the words stay, but the track is unreadable."""
    return [dict(e, lines=[l + ' x#%}{ q|}{# 3%}{x }{#q' for l in e['lines']]) for e in events]


def unread(n, events, tag):
    """Another language's stream without OCR text: its packets only (T4's timing)."""
    return {'n': n, 'tag': tag, 'forced': False, 'events': [dict(start=e['start'], dur=e['dur']) for e in events],
            'text': False}


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

    def test_note_only_events_do_not_make_a_track_unreadable(self):
        ev = sdh_version(dialogue())
        gaps = [e['start'] + 3.3 for e in ev[5:115:5]]
        ev += [{'start': s, 'dur': 0.5, 'lines': ['#' if k % 2 else 'J']} for k, s in enumerate(gaps)]
        ev.sort(key=lambda e: e['start'])
        t, = measured(track(0, ev))
        self.assertEqual(t['m']['note_only'], 22)
        self.assertEqual(t['m']['events'], 150)
        self.assertTrue(t['m']['readable'])
        self.assertEqual(t['m']['class'], 'sdh')

    def test_garbage_is_unreadable(self):
        t, = measured(track(0, junk(60)))
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
    def decide(self, tracks, orig_lang):
        """st.decide, checking the owner's 'ambiguous = keep': no track is dropped with a review."""
        v = st.decide(tracks, orig_lang)
        for n, r in v.items():
            self.assertFalse(r['role'] == 'drop' and r['review'], 's%d is dropped with a review' % n)
        return v

    def test_single_tracks(self):
        tracks = measured(track(0, sdh_version(dialogue())))
        self.assertEqual(self.decide(tracks, 'en')[0]['role'], 'sdh')
        tracks = measured(track(0, dialogue()))
        self.assertEqual(self.decide(tracks, 'en')[0], {'role': 'regular', 'review': None, 'note': None})

    def test_sdh_and_regular_by_text_not_by_count(self):
        base = dialogue()
        split = []
        for e in base:
            split.append(dict(e, dur=1.2))
            split.append(dict(e, start=e['start'] + 1.3, dur=1.2))
        tracks = measured(track(0, sdh_version(base)), track(1, split))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[0]['role'], v[1]['role']), ('sdh', 'regular'))
        self.assertIsNone(v[0]['review'])
        self.assertEqual((v[0]['note'], v[1]['note']), ('subtitle_count_disagrees', 'subtitle_count_disagrees'))
        self.assertEqual(st.spec87(tracks)[1], 'sdh')

    def test_retimed_regular_stays_in_the_dialogue_group(self):
        base = dialogue()
        late = [dict(e, start=e['start'] + 10.0) for e in base]
        tracks = measured(track(0, sdh_version(base)), track(1, late))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[0]['role'], v[1]['role']), ('sdh', 'regular'))

    def test_two_regular_tracks_go_to_review(self):
        tracks = measured(track(0, dialogue()), track(1, [dict(e, start=e['start'] + 0.1) for e in dialogue()]))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[0]['review'], v[0]['note']), ('subtitle_ambiguous', 'two regular tracks'))
        self.assertEqual(v[1]['review'], 'subtitle_ambiguous')

    def test_two_sdh_tracks_go_to_review(self):
        ev = sdh_version(dialogue())
        tracks = measured(track(0, ev), track(1, [dict(e, start=e['start'] + 0.1) for e in ev]))
        v = self.decide(tracks, 'en')
        for n in (0, 1):
            self.assertEqual(v[n], {'role': 'sdh', 'review': 'subtitle_ambiguous', 'note': 'two SDH tracks'})

    def test_unclear_class_is_reviewed_with_suggested_roles(self):
        def asides():
            ev = dialogue()
            for k in range(9):
                ev[4 + 10 * k]['lines'][0] += ' (FBI)'
            return ev
        why = 'some SDH markers (9 events, 0 sound-only, 0 naming a sound)'
        tracks = measured(track(0, asides()))
        self.assertEqual(tracks[0]['m']['class'], 'unclear')
        self.assertEqual(self.decide(tracks, 'en')[0], {'role': 'regular', 'review': 'subtitle_ambiguous', 'note': why})
        tracks = measured(track(0, sdh_version(dialogue())), track(1, asides()))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[0], {'role': 'sdh', 'review': 'subtitle_ambiguous', 'note': why})
        self.assertEqual(v[1], {'role': 'regular', 'review': 'subtitle_ambiguous', 'note': why})
        tracks = measured(track(0, dialogue()), track(1, asides()))
        v = self.decide(tracks, 'en')
        for n in (0, 1):
            self.assertEqual(v[n], {'role': 'keep', 'review': 'subtitle_ambiguous', 'note': why})

    def test_capital_only_events_in_one_track_name_the_review(self):
        base = dialogue()
        caps = list(base) + [{'start': e['start'] + 2.9, 'dur': 0.4, 'lines': ['KALON BIDUN']} for e in base[10:50:10]]
        caps.sort(key=lambda e: e['start'])
        tracks = measured(track(0, base), track(1, caps))
        self.assertEqual(tracks[1]['m']['class'], 'regular')
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1]['note'], 'capital-only events in one track only (UK-style SDH?)')
        self.assertEqual(v[0]['review'], 'subtitle_ambiguous')

    def test_an_unreadable_pair_falls_back_to_the_event_count(self):
        base = dialogue()
        tracks = measured(track(0, sdh_version(base)), track(1, garbled(base)))
        self.assertEqual(tracks[1]['m']['class'], 'unreadable')
        v = self.decide(tracks, 'en')
        self.assertEqual((v[0]['role'], v[0]['review']), ('sdh', 'subtitle_ambiguous'))
        self.assertTrue(v[0]['note'].startswith('subtitle_text_unreadable'))
        self.assertEqual((v[1]['role'], v[1]['review']), ('regular', 'subtitle_ambiguous'))

    def test_more_than_two_tracks_go_to_review(self):
        base = dialogue()
        tracks = measured(*[track(n, [dict(e, start=e['start'] + 0.05 * n) for e in base]) for n in range(3)])
        v = self.decide(tracks, 'en')
        for n in range(3):
            self.assertEqual(v[n], {'role': 'keep', 'review': 'subtitle_ambiguous',
                                    'note': 'more than two tracks in the dialogue group'})

    def test_forced_subset_with_many_events(self):
        base = dialogue()
        tracks = measured(track(0, sdh_version(base)), track(1, base[40:70]))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1]['role'], 'forced')
        self.assertEqual(v[0]['role'], 'sdh')

    def test_forced_candidates_in_8_7_order_before_subsets(self):
        base = dialogue()
        tracks = measured(track(0, base), track(1, base[0:36]), track(2, base[60:70]))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[2]['role'], v[2]['review']), ('forced', None))
        self.assertEqual(v[1], {'role': 'keep', 'review': 'subtitle_ambiguous', 'note': 'a second forced candidate'})

    def test_second_confirmed_forced_candidate_is_kept_for_review(self):
        base = dialogue()
        tracks = measured(track(0, base), track(1, base[40:50]), track(2, base[60:70]))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1], {'role': 'forced', 'review': None, 'note': 'words found in the dialogue track'})
        self.assertEqual(v[2], {'role': 'keep', 'review': 'subtitle_ambiguous', 'note': 'a second forced candidate'})

    def test_forced_candidate_that_looks_like_lyrics_is_kept(self):
        base = dialogue()
        song = [dict(e, lines=['# ' + l + ' #' for l in e['lines']]) for e in base[40:50]]
        tracks = measured(track(0, base), track(1, song))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[1]['role'], v[1]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertTrue(v[1]['note'].startswith('forced candidate looks like lyrics'))

    def test_forced_candidate_that_partly_matches_is_kept(self):
        base = dialogue()
        half = base[40:45] + dialogue(5, seed=7, nouns=OTHER, start=300.0)
        tracks = measured(track(0, base), track(1, half))
        x = st.contained(tracks[1], tracks[0])
        self.assertTrue(st.RULE['CB_DIFF'] < x < st.RULE['FORCED_CB'], x)
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1], {'role': 'keep', 'review': 'subtitle_ambiguous',
                                'note': 'forced candidate partly matches the dialogue track'})

    def test_forced_candidate_in_another_language_is_kept(self):
        rnd = random.Random(2)
        ev = [{'start': 30.0 + 7 * i, 'dur': 2.0,
               'lines': [' '.join(rnd.choice(SPANISH) for _ in range(8)).capitalize()]} for i in range(10)]
        tracks = measured(track(0, dialogue()), track(1, ev, forced=True))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1], {'role': 'keep', 'review': 'subtitle_ambiguous', 'note': 'tagged eng, text reads es'})
        self.assertEqual(st.spec87(tracks)[1], 'forced')

    def test_unrelated_short_track_is_not_forced(self):
        other = dialogue(11, seed=9, nouns=OTHER, start=13.0, step=40.0)
        tracks = measured(track(0, dialogue()), track(1, other))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1]['role'], 'keep')
        self.assertEqual(v[1]['review'], 'subtitle_ambiguous')
        self.assertEqual(st.spec87(tracks)[1], 'forced')

    def test_commentary_dropped_only_when_words_and_timing_differ(self):
        base = dialogue()
        comm = dialogue(100, seed=5, nouns=OTHER, start=11.7, step=4.0)
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, comm))
        self.assertEqual(self.decide(tracks, 'en')[2]['role'], 'drop')
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, comm))
        v = self.decide(tracks, 'es')
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))
        near = [dict(e, start=e['start'] + (0.1 if i % 2 else 0.6))
                for i, e in enumerate(dialogue(100, seed=5, nouns=OTHER))]
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, near))
        v = self.decide(tracks, 'en')
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))

    def test_unreadable_track_outside_the_dialogue_group_is_kept(self):
        base = dialogue()
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, junk(60, start=11.7, step=4.0)))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[2], {'role': 'keep', 'review': 'subtitle_ambiguous',
                                'note': 'subtitle_text_unreadable (8.7 would drop it by timing)'})
        self.assertEqual(st.spec87(tracks)[2], 'drop')

    def test_other_language_track_picks_the_dialogue_group(self):
        base = dialogue()
        sdh = sdh_version(base)
        comm = dialogue(100, seed=5, nouns=OTHER, start=11.7, step=4.0)
        tracks = measured(track(0, sdh), track(1, comm)) + [unread(2, sdh, 'fre')]
        v = self.decide(tracks, 'en')
        self.assertEqual(v[0], {'role': 'sdh', 'review': None, 'note': None})
        self.assertEqual(v[1]['role'], 'drop')
        self.assertEqual(v[2]['note'], 'not English (tag fre)')
        self.assertEqual((st.spec87(tracks)[0], st.spec87(tracks)[1]), ('keep (review)', 'keep (review)'))
        tracks = measured(track(0, sdh), track(1, comm))
        v = self.decide(tracks, 'en')
        for n in (0, 1):
            self.assertEqual(v[n], {'role': 'keep', 'review': 'subtitle_ambiguous', 'note': 'no dialogue group'})

    def test_mistagged_language_is_kept_for_review(self):
        rnd = random.Random(2)
        ev = [{'start': 10.0 + 3 * i, 'dur': 2.0,
               'lines': [' '.join(rnd.choice(SPANISH) for _ in range(8)).capitalize()]} for i in range(80)]
        tracks = measured(track(0, ev, tag='eng'))
        v = self.decide(tracks, 'es')
        self.assertEqual((v[0]['role'], v[0]['review']), ('keep', 'subtitle_ambiguous'))

    def test_unclear_language_is_reviewed_and_never_dropped(self):
        base = dialogue()
        tracks = measured(track(0, sdh_version(base)), track(1, list(base)), track(2, unclear()))
        self.assertEqual(tracks[2]['m']['lang'], 'unknown')
        v = self.decide(tracks, 'en')
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertTrue(v[2]['note'].startswith('language of the text unclear'))
        tracks = measured(track(0, dialogue(1700)), track(1, unclear(160, step=9.0)))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1], {'role': 'keep', 'review': 'subtitle_ambiguous',
                                'note': 'language of the text unclear (8.7 would drop it as partial)'})
        tracks = measured(track(0, unclear(start=10.0)))
        self.assertEqual(self.decide(tracks, 'en')[0], {'role': 'regular', 'review': 'subtitle_ambiguous',
                                                        'note': 'language of the text unclear'})

    def test_no_language_track_is_read_not_dropped(self):
        tracks = measured(track(0, dialogue()), track(1, junk(60, seed=4, start=12.0, step=4.0), tag='und'))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[0]['role'], 'regular')
        self.assertEqual((v[1]['role'], v[1]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertEqual(st.spec87(tracks)[1], 'drop')
        tracks = measured(track(0, dialogue(), tag='und'))
        self.assertEqual(self.decide(tracks, 'en')[0], {'role': 'regular', 'review': None, 'note': None})

    def test_no_language_track_in_another_language_or_too_short(self):
        rnd = random.Random(2)
        spanish = [{'start': 10.0 + 3 * i, 'dur': 2.0,
                    'lines': [' '.join(rnd.choice(SPANISH) for _ in range(8)).capitalize()]} for i in range(80)]
        tracks = measured(track(0, dialogue()), track(1, spanish, tag='und'))
        self.assertEqual(self.decide(tracks, 'en')[1], {'role': 'drop', 'review': None, 'note': 'not English (tag und)'})
        tracks = measured(track(0, dialogue(3), tag='und'))
        self.assertEqual(tracks[0]['m']['lang'], 'short')
        self.assertEqual(self.decide(tracks, 'en')[0], {
            'role': 'keep', 'review': 'subtitle_ambiguous',
            'note': 'no language tag and the text does not show its language'})

    def test_partial_track_is_a_forced_candidate_or_kept_when_unreadable(self):
        base = dialogue(1700)
        tracks = measured(track(0, base), track(1, base[100:260]), track(2, junk(160, seed=5, start=11.0, step=9.0)))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[0]['role'], 'regular')
        self.assertEqual((v[1]['role'], v[1]['review']), ('forced', None))
        self.assertEqual((v[2]['role'], v[2]['review']), ('keep', 'subtitle_ambiguous'))
        self.assertEqual((st.spec87(tracks)[1], st.spec87(tracks)[2]), ('drop', 'drop'))

    def test_readable_partial_track_is_dropped(self):
        other = dialogue(160, seed=8, nouns=OTHER, start=11.7, step=9.0)
        tracks = measured(track(0, dialogue(1700)), track(1, other))
        self.assertEqual(self.decide(tracks, 'en')[1], {'role': 'drop', 'review': None,
                                                        'note': 'partial_subtitle_dropped'})

    def test_unreadable_tracks_are_not_judged_by_their_words(self):
        base = dialogue()
        # An unreadable full track whose words are a subset: 8.7 decides it (two aligned tracks), not T2.
        tracks = measured(track(0, base), track(1, garbled(base[40:70])))
        self.assertFalse(tracks[1]['m']['readable'])
        v = self.decide(tracks, 'en')
        self.assertEqual((v[1]['role'], v[1]['review']), ('regular', 'subtitle_ambiguous'))
        self.assertEqual(st.spec87(tracks)[1], 'regular')
        # A readable candidate beside an unreadable dialogue track: forced as 8.7 says, but reviewed.
        tracks = measured(track(0, garbled(base)), track(1, base[40:50]))
        v = self.decide(tracks, 'en')
        self.assertEqual(v[1], {'role': 'forced', 'review': 'subtitle_ambiguous', 'note': 'dialogue track unreadable'})

    def test_before_yes(self):
        self.assertEqual(st.before_yes({'role': 'sdh', 'review': None}, 'regular'),
                         'regular (review: subtitle_text_disagrees)')
        self.assertEqual(st.before_yes({'role': 'regular', 'review': None}, 'regular'), 'regular')
        self.assertEqual(st.before_yes({'role': 'regular', 'review': 'subtitle_ambiguous'}, 'regular'),
                         'regular (review: subtitle_text_disagrees)')
        self.assertEqual(st.before_yes({'role': 'keep', 'review': 'subtitle_ambiguous'}, 'keep (review)'),
                         'keep (review)')
        self.assertEqual(st.before_yes({'role': 'sdh', 'review': None}, 'sdh (review)'), 'sdh (review)')


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

    def add_stream(self, rec, n, language, codec, packets):
        index = json.loads((rec / 'subs' / 'index.json').read_text(encoding='utf-8'))
        index.append({'n': n, 'language': language, 'codec': codec, 'forced': False})
        (rec / 'subs' / 'index.json').write_text(json.dumps(index), encoding='utf-8')
        (rec / 'subs' / ('sub-%d.packets.json' % n)).write_text(json.dumps({'packets': packets}), encoding='utf-8')

    def test_closed_captions_are_set_aside_and_unread_streams_reported(self):
        with tempfile.TemporaryDirectory() as d:
            rec, text, n = self.write_title(pathlib.Path(d), True)
            # The DVD closed captions as MakeMKV writes them: SubRip, every packet an event (spec 8.7), no OCR text.
            cc = [{'pts_time': '%.3f' % (20.0 + 3 * i), 'duration_time': '2.000', 'size': '40'} for i in range(5)]
            self.add_stream(rec, 1, 'eng', 'SubRip/SRT', cc)
            pgs = json.loads((rec / 'subs' / 'sub-0.packets.json').read_text(encoding='utf-8'))['packets']
            self.add_stream(rec, 2, 'spa', 'HDMV PGS', pgs)
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                code = st.main(['--rec', str(rec), '--text', str(text), '--orig-lang', 'en'])
            out = buf.getvalue()
            self.assertEqual(code, 0)
            self.assertIn('1 subtitle streams with OCR text, 1 not read (another language), 1 set aside', out)
            self.assertIn('stream s1 eng: set aside (closed captions: SubRip/SRT is a text stream; 8.7 sets it aside), '
                          '5 events', out)
            self.assertIn('| s1 | eng | 5 | set aside | set aside | - |', out)
            self.assertIn('| set aside | set aside |\n', out)
            self.assertIn('stream s2 spa: not read (no OCR text), %d events, used for timing only' % n, out)
            self.assertIn('| s2 | spa | %d | not read | drop | - | not English (tag spa) | drop | drop |' % n, out)
            self.assertIn('| s0 | eng | %d | sdh | sdh | - | - | regular | regular (review: subtitle_text_disagrees) |'
                          % n, out)
            # An English bitmap stream without OCR text is an input error, never left out silently.
            self.add_stream(rec, 3, 'eng', 'HDMV PGS', pgs)
            err = io.StringIO()
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
                self.assertEqual(st.main(['--rec', str(rec), '--text', str(text)]), 2)
            self.assertIn('stream s3 eng: no OCR text', err.getvalue())

    def test_missing_text_is_an_input_error(self):
        with tempfile.TemporaryDirectory() as d:
            rec, text, n = self.write_title(pathlib.Path(d), True)
            (text / 'sub-0.ocr.jsonl').unlink()
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                self.assertEqual(st.main(['--rec', str(rec), '--text', str(text)]), 2)


if __name__ == '__main__':
    unittest.main()
