"""English subtitle tracks by their OCR text (experiment 14; design: docs/phase0/experiments/EXP-14-design.md).

Reads, for one ripped title:
  REC/subs/index.json              the subtitle streams of evidence.mks (probe-title.ps1)
  REC/subs/sub-<n>.packets.json    ffprobe's packets of stream s:<n> (pts_time, duration_time, size)
  REC/subs-ocr/sub-<n>.frames.json line count of every OCR frame (ocr-subs.ps1; optional)
  TEXT/sub-<n>.ocr.jsonl           the OCR text of stream s:<n>, one record per frame (ocr-subs.ps1). The text
                                   lives in the tuning dataset on the share (decision D16); the Mac holds a
                                   temporary copy (tuning-fetch.ps1 -Text) and never commits it.
and prints numbers only (no subtitle text): one line per stream, one line per pair of streams read, the
verdict of the section 3 rule for every stream next to what spec 8.7 does today (counts and start times only) and
what the app does before experiment 14 answers yes, and one row per stream for the disc's notes.md. A text stream
without OCR text (the DVD closed captions) is set aside, as 8.7 does; another language's stream without OCR text is
used for timing only. Every threshold is in RULE and is PROVISIONAL.

  python3 scripts/phase0/subtitle_text.py --rec corpus/owner/SLUG/LABEL --text TEXTDIR [--orig-lang en]

Exit status 0 when the title was analysed, 2 when the input cannot be read, including an English or no-language
bitmap stream without OCR text.
Python 3.9 standard library. ASCII only: non-ASCII characters are built with chr().
"""
import argparse
import bisect
import json
import pathlib
import re
import sys
import unicodedata

RULE = {
    'CLEAR_MAX_BYTES': 64,      # sub_clear_max_bytes (spec 8.7): larger packets are displayed events
    'TOL_S': 0.300,             # subs_tolerance_ms (spec 8.7)
    'SAME_MIN': 0.70,           # subs_same_min: timing says same group
    'DIFF_MAX': 0.30,           # subs_diff_max: timing says different groups
    'FULL_SHARE': 0.10,         # a full track has at least this share of the largest English track's events
    'FORCED_MAX_EVENTS': 150,   # 8.7: a separately authored forced track has fewer events
    # readability
    'READ_LETTER_EVENTS': 0.90,  # share of events with 2 or more letters
    'READ_WORDLIKE': 0.85,       # share of word-like tokens (note stand-ins and '|' not counted)
    'READ_MIN_WORDS': 20,
    'EVENT_WORDLIKE': 0.50,      # a cue counts only in an event whose own tokens are at least this word-like
    # language (stopword shares on ASCII-folded words)
    'LANG_MIN_WORDS': 50,
    'EN_MIN': 0.20,              # English when the English share is at least this
    'EN_MAX_OTHER': 0.08,        # another language when English is at most this ...
    'OTHER_MIN': 0.12,           # ... and that language's share is at least this ...
    'OTHER_RATIO': 3.0,          # ... and at least this many times the English share
    # SDH evidence on one track
    'CAPS_STYLE': 0.50,          # share of capital lines at which the whole track is in capitals
    'SDH_M': 5,                  # marker events (cue, label or '>>')
    'SDH_RATE': 0.010,           # ... per event
    'SDH_SO': 3,                 # sound-only events (every letter inside a cue or label)
    'SDH_CS': 2,                 # events with a cue that names a sound
    'REG_M_MAX': 0,              # at most this many marker events: no SDH evidence
    'ODD_MAX': 2,                # more events with damaged brackets than this: not placed
    'CAPS_CUE_MAX': 2,           # more capital sound lines without brackets than this: not placed (UK style?)
    'UC_MIN': 3,                 # pair: capital-only events with no counterpart (reported; review only)
    # content of two tracks (time-free bigram containment)
    'CB_MIN_BIGRAMS': 20,
    'CB_SAME': 0.50,             # both directions at least this: same words
    'CB_DIFF': 0.20,             # both directions at most this: other words
    'CB_SUBSET_IN': 0.60,        # subset: the smaller track's bigrams found in the larger at least this ...
    'CB_SUBSET_BACK': 0.50,      # ... and the larger's found in the smaller under this ...
    'SUBSET_EVENTS': 0.70,       # ... and the smaller has at most this share of the larger's events
    'FORCED_CB': 0.60,           # forced candidate confirmed: its bigrams found in the dialogue track
    'NOTE_LINES': 0.30,          # forced candidate with this share of note-line events: lyrics, review
}

# ------------------------------------------------------------------ folding
# The music notes and the pilcrow become '#', the note stand-in, so that an engine that ever returns a note is
# read the same way as one that returns '#'. The inverted ? and ! are dropped: an ASCII-only engine cannot read
# them, and Tesseract's English model turns them into other characters anyway.
FOLD_MAP = {
    chr(0x2018): "'", chr(0x2019): "'", chr(0x201A): "'", chr(0x2032): "'", chr(0x00B4): "'", chr(0x0060): "'",
    chr(0x02BC): "'",
    chr(0x201C): '"', chr(0x201D): '"', chr(0x201E): '"', chr(0x00AB): '"', chr(0x00BB): '"', chr(0x2033): '"',
    chr(0x2010): '-', chr(0x2011): '-', chr(0x2012): '-', chr(0x2013): '-', chr(0x2014): '-', chr(0x2015): '-',
    chr(0x2212): '-', chr(0x2026): '...', chr(0x00A0): ' ',
    chr(0x2669): '#', chr(0x266A): '#', chr(0x266B): '#', chr(0x266C): '#', chr(0x266D): '#', chr(0x266F): '#',
    chr(0x00B6): '#',
    chr(0x00BF): '', chr(0x00A1): '',
    chr(0x00DF): 'ss', chr(0x00E6): 'ae', chr(0x00C6): 'AE', chr(0x0153): 'oe', chr(0x0152): 'OE',
    chr(0x00F8): 'o', chr(0x00D8): 'O', chr(0x0142): 'l', chr(0x0141): 'L',
}


def fold(s):
    """ASCII text: mapped punctuation, then NFKD without combining marks, then any other non-ASCII dropped."""
    s = ''.join(FOLD_MAP.get(c, c) for c in s)
    s = unicodedata.normalize('NFKD', s)
    return ''.join(c for c in s if not unicodedata.combining(c) and ord(c) < 128)


# ------------------------------------------------------------------ lines and OCR corrections
def split_joined(text, nlines=None):
    """Lines of the proof of concept's text, whose lines are joined with ' | ' while Tesseract reads a capital I
    as '|'. With the frame's true line count, the extra bare '|' tokens are taken as I, preferring the text start,
    a '|' after another '|' and a '|' before a lower-case word; without it, every ' | ' is a line break."""
    toks = text.split(' ')
    bare = [i for i, t in enumerate(toks) if t == '|']
    n_i = 0 if not nlines else max(0, len(bare) - (nlines - 1))

    def score(i):
        nxt = toks[i + 1] if i + 1 < len(toks) else ''
        s = 3 if i == 0 else 0
        if i > 0 and toks[i - 1] == '|':
            s += 1
        if nxt[:1].islower() or nxt[:1] == "'":
            s += 2
        if nxt[:1].isupper() or nxt[:1] in '-([':
            s -= 1
        return s

    as_i = set(sorted(bare, key=lambda i: (-score(i), i))[:n_i])
    lines, cur = [], []
    for i, t in enumerate(toks):
        if t == '|' and i not in as_i:
            lines.append(' '.join(cur))
            cur = []
        else:
            cur.append(t)
    lines.append(' '.join(cur))
    return [x.strip() for x in lines if x.strip()]


# Corrections for English text from Tesseract (I read as '|', 'l' or '1'). Applied to English-tagged streams.
CORRECTIONS = (
    ('pipe_I', re.compile(r"(?<![A-Za-z0-9])\|(?=[\s'.,?!]|$)"), 'I'),     # "|", "|!", "|'m"
    ('pipe_word_I', re.compile(r"(?<![A-Za-z0-9])\|(?=[a-z])"), 'I'),      # "|t's"
    ('pipe_l', re.compile(r"(?<=[a-z])\|(?=[a-z])"), 'l'),                 # "he|lo"
    ('caps_I', re.compile(r"(?<=[A-Z])[l1|](?=[A-Z])"), 'I'),              # "BORlS", "WHlSTLlNG"
    ('caps_O', re.compile(r"(?<=[A-Z])0(?=[A-Z])"), 'O'),
    ('l1_pronoun', re.compile(r"(?<![A-Za-z0-9'])[l1](?='(?:m|ll|d|ve)\b)"), 'I'),
    ('i_am', re.compile(r"(?<![A-Za-z0-9])[|l1]am\b"), 'I am'),            # "lam", "1am"
)


def correct(line, counts):
    for name, rx, rep in CORRECTIONS:
        line, k = rx.subn(rep, line)
        counts[name] = counts.get(name, 0) + k
    k = line.count('|')
    counts['pipe_left'] = counts.get('pipe_left', 0) + k
    return line.replace('|', 'l')


# ------------------------------------------------------------------ markers (on folded, corrected text)
# A cue: a bracketed or parenthesised span of 2 to 80 characters, with at least 2 letters and only letters,
# digits, spaces and light punctuation inside (OCR junk does not make one). Either bracket may close the other.
RE_CUE = re.compile(r"[\[(]\s*([A-Za-z0-9][A-Za-z0-9 ',.!?&:;/\-]{0,78}[A-Za-z0-9.!?'])\s*[\])]")
# A capital speaker label at the start of a line, after an optional dash, followed by text on the same line.
RE_LABEL = re.compile(r"^\s*(?:-\s*)?([A-Z][A-Z0-9'.\-]*(?:\s+[A-Z0-9'.\-]+){0,2})\s*:\s+\S")
RE_CHEV = re.compile(r"^\s*>>")
# Lyrics (note stand-in, strict): '#' or '*' at either end of a line, after folding (real notes are '#').
RE_NOTE = re.compile(r"^\s*[#*]|[#*]\s*$")
# Lyrics (loose, ENGINE-DEPENDENT, reported only): a lone token reported as a Tesseract misread of a note.
RE_NOTE_LOOSE = re.compile(r"^\s*(?:J|JJ|Jf|Ji|Jl|P|D|d|f|\$|%)\s+\S|\S\s+(?:J|JJ|Jf|Ji|Jl|P|D|d|f|\$|%)\s*$")
NOTE_LOOSE_TOKENS = ('J', 'JJ', 'Jf', 'Ji', 'Jl', 'P', 'D', 'd', 'f', '$', '%')
# Damaged brackets, on the folded text before corrections: a line wrapped in '|' or '{' '}' (a misread bracket
# pair), counted per event together with unbalanced () and [].
RE_WRAP = re.compile(r"^\s*-?\s*[|{]\s*[A-Za-z][^|{}()\[\]]*[A-Za-z.!?]\s*[|}]\s*$")
RE_WORD = re.compile(r"[a-z]+(?:'[a-z]+)?")
RE_WORDLIKE = re.compile(r"[\"'(\[-]*(?:[A-Za-z]+(?:[-'][A-Za-z]+)*|\d+(?:[.,:]\d+)*)[\"'.,!?:;)\]-]*")

# Sound stems (prefixes, ASCII). Used only inside a cue or on a capital line, never on dialogue words: outside
# them they hit ordinary words (single, pantry) and names.
SOUND_STEMS = tuple("""laugh chuckl giggl sigh gasp groan grunt scream shout yell whisper cough sniff sob cries
crying music sing hum whistl ring buzz beep knock thud bang crash slam footstep applau cheer whoop breath pant mumbl
mutter murmur siren honk bark growl howl throat exhal inhal snor click thunder rumbl static chatter indistinct
inaudible explosion gunshot gunfire splash screech creak rustl clatter chim grumbl stammer stutter clears squeal
wheez moan whimper yelp shriek snicker scoff door phone telephone bell engine horn tire brake glass shatter thump
snap clap kiss wind rain alarm radio yawn weep wail whoosh typing tv television sneez hiccup splutter
spluttering playing plays""".split())

STOP = {
    'en': """the and is are was were be been to of in on at for with that this it you i he she we they my your me
him her not do don't what have has will just so if there here can all get go know it's i'm you're that's would
could did didn't can't from about out up yes oh well right okay""".split(),
    'es': """el la los las de del que y en un una es se lo le por con para su al te mi tu yo pero como mas esta
este eso esto si ya hay muy bien nos os usted senor ella ellos aqui nada todo cuando donde porque tengo tiene
puede vamos""".split(),
    'fr': """le les des du et est je tu il elle nous vous ils que qui ne pas pour dans sur avec ce cette mais ou mon
ma oui non c'est j'ai tout bien ca moi toi lui suis sont fait rien ici""".split(),
    'de': """der die das und ist ich du er sie wir ihr nicht ein eine zu mit auf fur den dem des es ja nein was wie
aber auch noch mir mich dich hier sein haben bin bist wird kann""".split(),
}
_where = {}
for _lang, _words in STOP.items():
    for _w in _words:
        _where.setdefault(_w, set()).add(_lang)
# A word in two lists counts for neither; English words that are common Spanish words are taken out of English.
STOP = {l: {w for w in ws if len(_where[w]) == 1} for l, ws in STOP.items()}
STOP['en'] -= {'me', 'he', 'has', 'a', 'no', 'son', 'come', 'sin', 'con', 'pan'}


def letters(s):
    return sum(1 for c in s if c.isalpha())


def sound_named(s):
    return any(w.startswith(SOUND_STEMS) for w in RE_WORD.findall(s.lower()))


def wordlike(tok):
    return bool(RE_WORDLIKE.fullmatch(tok))


def is_note_token(tok):
    return tok in ('#', '##', '*', '**', '|') or bool(re.fullmatch(r"[#*]+", tok))


# ------------------------------------------------------------------ loading
def read_json(path):
    return json.loads(pathlib.Path(path).read_text(encoding='utf-8-sig'))


def packet_events(packets, clear_max_bytes):
    """Displayed events (packets larger than clear_max_bytes): start and duration in seconds."""
    rows = packets['packets'] if isinstance(packets, dict) else packets
    out = []
    for p in rows:
        if 'pts_time' not in p or int(p.get('size', 0)) <= clear_max_bytes:
            continue
        d = p.get('duration_time')
        out.append({'start': float(p['pts_time']), 'dur': float(d) if d not in (None, '', 'N/A') else 0.0})
    out.sort(key=lambda e: e['start'])
    return out


def ocr_records(jsonl_path, frames_path=None):
    """(t, lines) for every OCR frame with text. A record with a 'lines' array (the final ocr-subs.ps1) is taken as
    it is; the proof of concept's ' | '-joined text is split with the frame's line count from frames.json."""
    rows = [json.loads(l) for l in pathlib.Path(jsonl_path).read_text(encoding='utf-8-sig').splitlines() if l.strip()]
    counts = {}
    if frames_path and pathlib.Path(frames_path).exists():
        for f in read_json(frames_path):
            counts[f['frame']] = f.get('lines')
    out = []
    for r in rows:
        if isinstance(r.get('lines'), list):
            lines = [str(x).strip() for x in r['lines'] if str(x).strip()]
        else:
            text = (r.get('text') or '').strip()
            lines = split_joined(text, counts.get(r.get('frame'))) if text else []
        if lines:
            out.append({'t': float(r.get('pts', r.get('t', 0.0))), 'pts': 'pts' in r, 'lines': lines})
    return out


def pair_text(events, recs):
    """Gives each displayed event its OCR lines. Equal counts: by order (the k-th frame with text is the k-th event;
    true on every real track so far). Otherwise by time: the proof of concept's 't' is the packet time minus the
    file's first packet time, on a 0.25 s grid. Returns the method and the number of frames left unmatched."""
    if len(recs) == len(events):
        for e, r in zip(events, recs):
            e['lines'] = r['lines']
        return 'order', 0
    for e in events:
        e['lines'] = []
    if not events:
        return 'time', len(recs)
    starts = [e['start'] for e in events]
    off = 0.0 if (recs and recs[0]['pts']) else events[0]['start'] - (recs[0]['t'] if recs else 0.0)
    left = 0
    for r in recs:
        t = r['t'] + off
        i = bisect.bisect_left(starts, t)
        best = min((j for j in (i - 1, i) if 0 <= j < len(starts)), key=lambda j: abs(starts[j] - t))
        if abs(starts[best] - t) <= 0.30:
            events[best]['lines'] = events[best]['lines'] + r['lines']
        else:
            left += 1
    return 'time', left


# ------------------------------------------------------------------ per-event and per-track measures
def event_features(lines_raw, english, counts):
    f = {}
    folded = [fold(l) for l in lines_raw]
    joined_raw = ' '.join(folded)
    f['odd'] = (joined_raw.count('(') + joined_raw.count('[') != joined_raw.count(')') + joined_raw.count(']')
                or any(RE_WRAP.match(l) for l in folded))
    lines = [correct(l, counts) if english else l.replace('|', ' ') for l in folded]
    lines = [l.strip() for l in lines if l.strip()]
    joined = ' '.join(lines)
    toks = joined.split()
    # A note-only event (a music cue read as note stand-ins, or as one stray letter; ENGINE-DEPENDENT) is left out
    # of the readability share of events with letters.
    f['note_only'] = bool(toks) and (all(is_note_token(t) for t in toks)
                                     or (len(toks) == 1 and toks[0] in NOTE_LOOSE_TOKENS))
    real = [t for t in toks if not is_note_token(t)]
    f['tokens'] = len(real)
    f['wordlike'] = sum(1 for t in real if wordlike(t))
    ok_event = not real or f['wordlike'] / len(real) >= RULE['EVENT_WORDLIKE']
    cues = []
    for m in RE_CUE.finditer(joined):
        inner = m.group(1)
        if letters(inner) >= 2 and ok_event:
            cues.append(inner)
    f['cue'] = bool(cues)
    f['cue_sound'] = any(sound_named(c) for c in cues)
    labels = []
    rest = []
    for l in lines:
        m = RE_LABEL.match(l)
        if m and letters(m.group(1)) >= 2:
            labels.append(m.group(1))
            l = l[m.end(1):].lstrip(' :')
        rest.append(RE_CUE.sub(' ', l))
    f['label'] = bool(labels)
    f['chev'] = any(RE_CHEV.match(l) for l in lines)
    remainder = RE_CUE.sub(' ', ' '.join(rest))
    remainder = re.sub(r"[#*>]", ' ', remainder)
    f['letters'] = letters(joined)
    f['sound_only'] = f['cue'] and letters(remainder) < 2
    f['note'] = any(RE_NOTE.search(l) for l in lines)
    f['note_loose'] = any(RE_NOTE_LOOSE.search(l) for l in lines)
    caps = 0
    caps_cue = False
    for l, r in zip(lines, rest):
        if letters(r) >= 4 and not re.search(r"[a-z]", r):
            caps += 1
            if sound_named(r):
                caps_cue = True
    f['caps_lines'] = caps
    f['caps_cue'] = caps_cue
    f['all_caps'] = letters(joined) >= 3 and not re.search(r"[a-z]", joined)
    f['lines'] = len(lines)
    f['words'] = RE_WORD.findall(remainder.lower())
    return f


def lang_of(words):
    n = len(words)
    shares = {l: (sum(1 for w in words if w in ws) / n if n else 0.0) for l, ws in STOP.items()}
    if n < RULE['LANG_MIN_WORDS']:
        return 'short', shares
    en = shares['en']
    other, share = max(((l, s) for l, s in shares.items() if l != 'en'), key=lambda x: x[1])
    if en >= RULE['EN_MIN']:
        return 'en', shares
    if en <= RULE['EN_MAX_OTHER'] and share >= RULE['OTHER_MIN'] and share >= RULE['OTHER_RATIO'] * en:
        return other, shares
    return 'unknown', shares


def bigrams(fs):
    """Consecutive word pairs inside each event (dialogue words, at least 2 letters), leaving out pairs made of
    two English stopwords, which any two English texts share."""
    out = set()
    for f in fs:
        ws = [w for w in f['words'] if len(w) >= 2]
        for a, b in zip(ws, ws[1:]):
            if a in STOP['en'] and b in STOP['en']:
                continue
            out.add((a, b))
    return out


def measure(track):
    """Measures of one track: track = {'n', 'tag', 'events': [{'start', 'dur', 'lines'}], ...}."""
    english = track.get('tag') in ('eng', 'en')
    counts = {}
    fs = [event_features(e.get('lines', []), english, counts) for e in track['events']]
    n = len(fs)
    nlines = sum(f['lines'] for f in fs)
    toks = sum(f['tokens'] for f in fs)
    words = [w for f in fs for w in f['words']]
    m = {'events': n, 'text_events': sum(1 for f in fs if f['letters'] > 0), 'lines': nlines, 'words': len(words)}
    scored = [f for f in fs if not f['note_only']]
    m['note_only'] = n - len(scored)
    m['letter_events'] = round(sum(1 for f in scored if f['letters'] >= 2) / len(scored), 3) if scored else 0.0
    m['wordlike'] = round(sum(f['wordlike'] for f in fs) / toks, 3) if toks else 0.0
    m['readable'] = (m['letter_events'] >= RULE['READ_LETTER_EVENTS'] and m['wordlike'] >= RULE['READ_WORDLIKE']
                     and len(words) >= RULE['READ_MIN_WORDS'])
    m['lang'], shares = lang_of(words)
    m['shares'] = {k: round(v, 3) for k, v in shares.items()}
    m['caps_share'] = round(sum(f['caps_lines'] for f in fs) / nlines, 3) if nlines else 0.0
    m['caps_style'] = m['caps_share'] >= RULE['CAPS_STYLE']
    cs = m['caps_style']
    m['CUE'] = sum(f['cue'] for f in fs)
    m['LABEL'] = sum(f['label'] for f in fs) if not cs else 0
    m['CHEV'] = sum(f['chev'] for f in fs)
    m['M'] = sum(1 for f in fs if f['cue'] or f['chev'] or (f['label'] and not cs))
    m['SO'] = sum(f['sound_only'] for f in fs)
    m['CS'] = sum(f['cue_sound'] for f in fs)
    m['rate'] = round(m['M'] / n, 4) if n else 0.0
    m['CAPS_CUE'] = sum(f['caps_cue'] for f in fs) if not cs else 0
    m['NOTE'] = sum(f['note'] for f in fs)
    m['NOTE_LOOSE'] = sum(f['note_loose'] for f in fs)
    m['ODD'] = sum(f['odd'] for f in fs)
    m['corrections'] = counts
    m['class'], m['why'] = classify(m)
    track['m'] = m
    track['fs'] = fs
    track['bigrams'] = bigrams(fs)
    return m


def classify(m):
    """sdh, regular, unclear (kept, review) or unreadable (8.7 decides, review)."""
    if not m['readable']:
        return 'unreadable', 'text not readable (letters %.2f, word-like %.2f, %d words)' % (
            m['letter_events'], m['wordlike'], m['words'])
    if (m['M'] >= RULE['SDH_M'] and m['rate'] >= RULE['SDH_RATE'] and m['SO'] >= RULE['SDH_SO']
            and m['CS'] >= RULE['SDH_CS']):
        return 'sdh', 'cues and labels in %d events, %d sound-only' % (m['M'], m['SO'])
    if m['M'] <= RULE['REG_M_MAX']:
        if m['ODD'] > RULE['ODD_MAX']:
            return 'unclear', '%d events with damaged brackets' % m['ODD']
        if m['CAPS_CUE'] > RULE['CAPS_CUE_MAX']:
            return 'unclear', '%d capital sound lines without brackets (UK-style SDH?)' % m['CAPS_CUE']
        return 'regular', 'no SDH markers'
    return 'unclear', 'some SDH markers (%d events, %d sound-only, %d naming a sound)' % (m['M'], m['SO'], m['CS'])


def aligned(a, b, tol=None):
    """Share of a's events that start within tol of some event of b (spec 8.7)."""
    tol = RULE['TOL_S'] if tol is None else tol
    bs = [e['start'] for e in b['events']]
    if not a['events']:
        return 0.0
    hit = 0
    for e in a['events']:
        i = bisect.bisect_left(bs, e['start'] - tol)
        if i < len(bs) and bs[i] <= e['start'] + tol:
            hit += 1
    return round(hit / len(a['events']), 3)


def contained(a, b):
    """Share of a's bigrams that also occur in b, anywhere in the title (time-free); None when a has too few."""
    if len(a['bigrams']) < RULE['CB_MIN_BIGRAMS']:
        return None
    return round(len(a['bigrams'] & b['bigrams']) / len(a['bigrams']), 3)


def uc(a, b, tol=None):
    """a's all-capital events that overlap no event of b (padded by tol)."""
    tol = RULE['TOL_S'] if tol is None else tol
    spans = sorted((e['start'], e['start'] + max(e['dur'], 0.0)) for e in b['events'])
    starts = [s for s, _ in spans]
    k = 0
    for e, f in zip(a['events'], a['fs']):
        if not f['all_caps']:
            continue
        lo, hi = e['start'] - tol, e['start'] + max(e['dur'], 0.0) + tol
        i = bisect.bisect_right(starts, hi)
        if not any(s <= hi and t >= lo for s, t in spans[max(0, i - 50):i]):
            k += 1
    return k


def pair(a, b):
    p = {'aligned': (aligned(a, b), aligned(b, a)), 'cb': (contained(a, b), contained(b, a)),
         'uc': (uc(a, b), uc(b, a))}
    t = max(p['aligned'])
    p['timing'] = 'same' if t >= RULE['SAME_MIN'] else 'diff' if t <= RULE['DIFF_MAX'] else 'between'
    x, y = p['cb']
    if not (a['m']['readable'] and b['m']['readable']) or x is None or y is None:
        p['content'] = 'unknown'
    elif min(x, y) >= RULE['CB_SAME']:
        p['content'] = 'same'
    elif max(x, y) <= RULE['CB_DIFF']:
        p['content'] = 'different'
    else:
        p['content'] = 'partial'
    return p


def subset_of(a, b, p):
    """True when a's words are a one-way subset of b's: forced (or partial) content, whatever 8.7's 'full' says.
    Never for an unreadable track: 8.7 decides those (T0)."""
    if not (a['m']['readable'] and b['m']['readable']):
        return False
    x, y = p['cb']
    if x is None or y is None:
        return False
    return (x >= RULE['CB_SUBSET_IN'] and y < RULE['CB_SUBSET_BACK']
            and len(a['events']) <= RULE['SUBSET_EVENTS'] * len(b['events']))


# ------------------------------------------------------------------ the rule (section 3 of the design)
def decide(tracks, orig_lang=None):
    """Role of every stream: tracks = [{'n', 'tag', 'forced', 'events', 'm', ...}], each English or no-language
    stream measured first (another language's stream may be unread, 'text': False; it then serves T4's timing only;
    a closed-caption text stream is 'set_aside').
    Returns {n: {'role', 'review', 'note'}}. role: sdh, regular, forced, drop, keep (kept with no role) or
    set aside. A track with a review is never dropped (ambiguous = keep)."""
    out = {}

    def put(t, role, review=None, note=None):
        out[t['n']] = {'role': role, 'review': review, 'note': note}

    eng = []
    for t in tracks:
        if t.get('set_aside'):
            put(t, 'set aside', None, 'closed captions, a text stream (8.7: set aside first)')
            continue
        if t['tag'] not in ('eng', 'en', 'und', '', None):
            put(t, 'drop', None, 'not English (tag %s)' % t['tag'])
            continue
        m = t['m']
        if t['tag'] in ('eng', 'en'):
            if m['readable'] and m['lang'] not in ('en', 'unknown', 'short'):
                put(t, 'keep', 'subtitle_ambiguous', 'tagged eng, text reads %s' % m['lang'])
                continue
            eng.append(t)
        elif t['tag'] in ('und', '', None) and m['readable'] and m['lang'] == 'en':
            eng.append(t)
        elif t['tag'] in ('und', '', None) and (not m['readable'] or m['lang'] in ('short', 'unknown')):
            # No tag and no language in the text: it may be English, so it is never dropped (T1c).
            put(t, 'keep', 'subtitle_ambiguous', 'no language tag and the text does not show its language')
        else:
            put(t, 'drop', None, 'not English (tag %s)' % t['tag'])
    if not eng:
        return out
    unclear_lang = {t['n'] for t in eng if t['m']['readable'] and t['m']['lang'] == 'unknown'}
    pairs = {}
    for i, a in enumerate(eng):
        for b in eng[i + 1:]:
            p = pair(a, b)
            pairs[(a['n'], b['n'])] = p
            pairs[(b['n'], a['n'])] = dict(p, aligned=p['aligned'][::-1], cb=p['cb'][::-1], uc=p['uc'][::-1])
    largest = max(len(t['events']) for t in eng)
    # rank: T7's order of the forced candidates (8.7's flagged ones, then 8.7's not full with < 150 events, then T2's).
    full, cands, partial, rank = [], [], [], {}
    for t in eng:
        if t.get('forced'):
            cands.append(t)
            rank[t['n']] = 0
        elif len(t['events']) >= RULE['FULL_SHARE'] * largest:
            full.append(t)
        elif len(t['events']) < RULE['FORCED_MAX_EVENTS']:
            cands.append(t)
            rank[t['n']] = 1
        else:
            partial.append(t)
    # A full track whose words are a one-way subset of a larger full track is a forced candidate.
    for t in list(full):
        if any(u is not t and subset_of(t, u, pairs[(t['n'], u['n'])]) for u in full):
            full.remove(t)
            cands.append(t)
            rank[t['n']] = 2
    # A not-full track with 150 events or more (8.7: partial_subtitle_dropped) is a forced candidate first when its
    # words are a one-way subset of a full track; when its text cannot be read, or its language is unclear, it is
    # kept for review (T0, T1b, T7).
    for t in partial:
        if any(subset_of(t, u, pairs[(t['n'], u['n'])]) for u in full):
            cands.append(t)
            rank[t['n']] = 2
        elif not t['m']['readable']:
            put(t, 'keep', 'subtitle_ambiguous', 'subtitle_text_unreadable (8.7 would drop it as partial)')
        elif t['n'] in unclear_lang:
            put(t, 'keep', 'subtitle_ambiguous', 'language of the text unclear (8.7 would drop it as partial)')
        else:
            put(t, 'drop', None, 'partial_subtitle_dropped')
    # Groups: same words join (whatever the timing); timing joins when the text cannot be read.
    group = {t['n']: {t['n']} for t in full}
    unsure = set()
    for i, a in enumerate(full):
        for b in full[i + 1:]:
            p = pairs[(a['n'], b['n'])]
            c, tm = p['content'], p['timing']
            join = c == 'same' or (c == 'unknown' and tm == 'same')
            if not join and not (c == 'different' and tm == 'diff') and not (c == 'unknown' and tm == 'diff'):
                unsure |= {a['n'], b['n']}
            if join:
                g = group[a['n']] | group[b['n']]
                for k in g:
                    group[k] = g
    groups = []
    for t in full:
        g = group[t['n']]
        if g not in groups:
            groups.append(g)
    by_n = {t['n']: t for t in eng}
    dialogue = None
    if len(groups) == 1:
        dialogue = groups[0]
    elif groups:
        big = sorted(groups, key=len, reverse=True)
        if len(big[0]) >= 2 and len(big[1]) < len(big[0]):
            dialogue = big[0]
        elif len(big[0]) == 1:
            # Only single-track groups: the one whose track follows a full subtitle track of another language
            # (that track's timing is the dialogue's), when exactly one does. Timing, not SDH markers, decides.
            # Another language's stream counts here whether its text was read or not (only its packets are used).
            others = [t for t in tracks if t not in eng and t['tag'] not in ('und', '', None) and not t.get('set_aside')
                      and len(t['events']) >= RULE['FULL_SHARE'] * largest]
            hits = [g for g in groups if any(max(aligned(by_n[k], o), aligned(o, by_n[k])) >= RULE['SAME_MIN']
                                             for k in g for o in others)]
            if len(hits) == 1:
                dialogue = hits[0]
    if full and dialogue is None:
        for t in full:
            put(t, 'keep', 'subtitle_ambiguous', 'no dialogue group')
    elif full:
        members = [by_n[k] for k in sorted(dialogue)]
        roles(members, pairs, put)
        for t in full:
            if t['n'] in dialogue:
                continue
            m = t['m']
            vs = [pairs[(t['n'], k)] for k in dialogue]
            other_words = all(p['content'] == 'different' for p in vs)
            other_time = all(p['timing'] == 'diff' for p in vs)
            if t['n'] in unsure:
                put(t, 'keep', 'subtitle_ambiguous', 'text and timing disagree with the dialogue group')
            elif not m['readable']:
                put(t, 'keep', 'subtitle_ambiguous', 'subtitle_text_unreadable (8.7 would drop it by timing)')
            elif (other_words and other_time and m['SO'] == 0 and m['class'] != 'sdh' and len(group[t['n']]) == 1
                  and m['lang'] == 'en' and orig_lang == 'en'):
                put(t, 'drop', None, 'commentary or trivia: other words and other timing')
            else:
                why = 'outside the dialogue group'
                if orig_lang != 'en':
                    why = 'original language %s' % (orig_lang or 'unknown')
                if t['n'] in unclear_lang:
                    why = 'language of the text unclear; ' + why
                put(t, 'keep', 'subtitle_ambiguous', why)
    # Forced candidates in T7's order, each confirmed by its words. A candidate whose text reads another language
    # never gets here: T1a keeps it for review.
    ref = None
    if dialogue:
        regs = [by_n[k] for k in sorted(dialogue) if out.get(k, {}).get('role') == 'regular']
        sdhs = [by_n[k] for k in sorted(dialogue) if out.get(k, {}).get('role') == 'sdh']
        ref = (regs or sdhs or [by_n[k] for k in sorted(dialogue)])[0]
    kept = False
    for t in sorted(cands, key=lambda t: (rank[t['n']], t['n'])):
        m = t['m']
        x = None
        if ref is not None:
            x = contained(t, ref) if m['readable'] and ref['m']['readable'] else None
        lyrics = m['events'] and m['NOTE'] / m['events'] >= RULE['NOTE_LINES']
        if lyrics:
            put(t, 'keep', 'subtitle_ambiguous', 'forced candidate looks like lyrics (%d note lines)' % m['NOTE'])
        elif x is not None and x <= RULE['CB_DIFF']:
            put(t, 'keep', 'subtitle_ambiguous', 'forced candidate shares no words with the dialogue track')
        elif x is not None and x < RULE['FORCED_CB']:
            put(t, 'keep', 'subtitle_ambiguous', 'forced candidate partly matches the dialogue track')
        elif kept:
            put(t, 'keep', 'subtitle_ambiguous', 'a second forced candidate')
        elif not m['readable']:
            put(t, 'forced', 'subtitle_ambiguous', 'subtitle_text_unreadable')
            kept = True
        elif ref is not None and not ref['m']['readable']:
            put(t, 'forced', 'subtitle_ambiguous', 'dialogue track unreadable')
            kept = True
        else:
            note = ('words found in the dialogue track' if x is not None else
                    'no dialogue track to compare with' if ref is None else 'too short to compare')
            put(t, 'forced', None, note)
            kept = True
    for k in unclear_lang:
        if k in out and out[k]['role'] == 'drop':
            out[k] = {'role': 'keep', 'review': 'subtitle_ambiguous',
                      'note': 'language of the text unclear (the rule would drop it: %s)' % out[k]['note']}
        elif k in out and not out[k]['review']:
            out[k] = dict(out[k], review='subtitle_ambiguous', note='language of the text unclear')
    # Ambiguous = keep: a track held for review is never dropped, whatever step reviewed it.
    # Unreachable today, like the drop branch just above: a guard so that no later step can bring back a reviewed drop.
    for k, v in out.items():
        if v['role'] == 'drop' and v['review']:
            out[k] = dict(v, role='keep')
    return out


def roles(members, pairs, put):
    """Regular and SDH inside the dialogue group (replaces 8.7's 'the one with more events is SDH')."""
    if len(members) == 1:
        t = members[0]
        c = t['m']['class']
        if c == 'sdh':
            put(t, 'sdh')
        elif c == 'regular':
            put(t, 'regular')
        elif c == 'unreadable':
            put(t, 'regular', 'subtitle_ambiguous', 'subtitle_text_unreadable')
        else:
            put(t, 'regular', 'subtitle_ambiguous', t['m']['why'])
        return
    if len(members) > 2:
        for t in members:
            put(t, 'keep', 'subtitle_ambiguous', 'more than two tracks in the dialogue group')
        return
    a, b = members
    ca, cb = a['m']['class'], b['m']['class']
    if 'unreadable' in (ca, cb):
        hi, lo = (a, b) if len(a['events']) >= len(b['events']) else (b, a)
        put(hi, 'sdh', 'subtitle_ambiguous', 'subtitle_text_unreadable: SDH by event count (8.7)')
        put(lo, 'regular', 'subtitle_ambiguous', 'subtitle_text_unreadable')
    elif {ca, cb} == {'sdh', 'regular'}:
        s, r = (a, b) if ca == 'sdh' else (b, a)
        note = None if len(s['events']) >= len(r['events']) else 'subtitle_count_disagrees'
        put(s, 'sdh', None, note)
        put(r, 'regular', None, note)
    elif ca == cb == 'regular':
        p = pairs[(a['n'], b['n'])]
        why = 'two regular tracks'
        if max(p['uc']) >= RULE['UC_MIN'] and min(p['uc']) <= 1:
            why = 'capital-only events in one track only (UK-style SDH?)'
        put(a, 'regular', 'subtitle_ambiguous', why)
        put(b, 'regular', 'subtitle_ambiguous', why)
    elif ca == cb == 'sdh':
        put(a, 'sdh', 'subtitle_ambiguous', 'two SDH tracks')
        put(b, 'sdh', 'subtitle_ambiguous', 'two SDH tracks')
    else:
        # One or both unclear: kept for review. When the other one is SDH, that is the suggestion shown.
        why = '; '.join(t['m']['why'] for t in (a, b) if t['m']['class'] == 'unclear')
        for t, u in ((a, b), (b, a)):
            role = 'keep'
            if t['m']['class'] == 'sdh':
                role = 'sdh'
            elif u['m']['class'] == 'sdh':
                role = 'regular'
            put(t, role, 'subtitle_ambiguous', why)


def spec87(tracks):
    """What spec 8.7 does today, from event counts and start times alone (for comparison): no TheDiscDB and no
    forced-only parent. A role with ' (review)' raised subtitle_ambiguous."""
    out = {}
    eng = [t for t in tracks if t['tag'] in ('eng', 'en') and not t.get('set_aside')]
    for t in tracks:
        if t.get('set_aside'):
            out[t['n']] = 'set aside'
        elif t not in eng:
            out[t['n']] = 'drop'
    if not eng:
        return out
    largest = max(len(t['events']) for t in eng)
    full = [t for t in eng if not t.get('forced') and len(t['events']) >= RULE['FULL_SHARE'] * largest]
    rest = [t for t in eng if t not in full]
    group = {t['n']: {t['n']} for t in full}
    review = False
    for i, a in enumerate(full):
        for b in full[i + 1:]:
            s = max(aligned(a, b), aligned(b, a))
            if s >= RULE['SAME_MIN']:
                g = group[a['n']] | group[b['n']]
                for k in g:
                    group[k] = g
            elif s > RULE['DIFF_MAX']:
                review = True
    groups = []
    for t in full:
        if group[t['n']] not in groups:
            groups.append(group[t['n']])
    dialogue = None
    if len(groups) == 1:
        dialogue = groups[0]
    elif groups:
        big = sorted(groups, key=len, reverse=True)
        if len(big[0]) >= 2 and len(big[1]) < len(big[0]):
            dialogue = big[0]
    tag = ' (review)' if review else ''
    by_n = {t['n']: t for t in full}
    if dialogue is None:
        for t in full:
            out[t['n']] = 'keep (review)'
    else:
        ms = sorted((by_n[k] for k in dialogue), key=lambda t: -len(t['events']))
        if len(ms) == 1:
            out[ms[0]['n']] = 'regular' + tag
        elif len(ms) == 2:
            out[ms[0]['n']] = 'sdh' + tag
            out[ms[1]['n']] = 'regular' + tag
        else:
            for t in ms:
                out[t['n']] = 'keep (review)'
        for t in full:
            if t['n'] not in dialogue:
                out[t['n']] = 'drop' + tag
    kept = False
    for t in sorted(rest, key=lambda t: (not t.get('forced'), t['n'])):
        if t.get('forced') or len(t['events']) < RULE['FORCED_MAX_EVENTS']:
            out[t['n']] = 'drop' if kept else 'forced'
            kept = True
        else:
            out[t['n']] = 'drop'
    return out


# ------------------------------------------------------------------ printing
def track_line(t):
    m = t['m']
    sh = ' '.join('%s=%.3f' % (k, v) for k, v in m['shares'].items())
    return ('stream s%d %s: events=%d lines=%d words=%d letters=%.3f wordlike=%.3f readable=%s lang=%s (%s) '
            'CUE=%d LABEL=%d CHEV=%d M=%d rate=%.4f SO=%d CS=%d caps_share=%.3f CAPS_CUE=%d NOTE=%d NOTE_LOOSE=%d '
            'ODD=%d class=%s' % (
                t['n'], t['tag'], m['events'], m['lines'], m['words'], m['letter_events'], m['wordlike'],
                'yes' if m['readable'] else 'no', m['lang'], sh, m['CUE'], m['LABEL'], m['CHEV'], m['M'], m['rate'],
                m['SO'], m['CS'], m['caps_share'], m['CAPS_CUE'], m['NOTE'], m['NOTE_LOOSE'], m['ODD'], m['class']))


def fmt(x):
    return 'n/a' if x is None else '%.3f' % x


def before_yes(v, today):
    """The outcome until experiment 14 answers yes: 8.7's role (today), and subtitle_ambiguous with the note
    subtitle_text_disagrees when the rule differs: another role, or a review that 8.7 does not raise."""
    role = today.replace(' (review)', '')
    review = today.endswith(' (review)')
    differs = v['role'] != role or (bool(v['review']) and not review)
    if differs:
        return '%s (review: subtitle_text_disagrees)' % role
    return today


def has_text(t):
    return 'pairing' in t


def analyse(tracks, orig_lang=None, label='', out=print):
    for t in tracks:
        if t.get('set_aside'):
            out('stream s%d %s: set aside (closed captions: %s is a text stream; 8.7 sets it aside), %d events' % (
                t['n'], t['tag'], t.get('codec') or '?', len(t['events'])))
            continue
        if not has_text(t):
            out('stream s%d %s: not read (no OCR text), %d events, used for timing only' % (
                t['n'], t['tag'], len(t['events'])))
            continue
        measure(t)
        out(track_line(t) + '  [%s]' % t['pairing'])
        c = {k: v for k, v in sorted(t['m']['corrections'].items()) if v}
        if c:
            out('  corrections: ' + ' '.join('%s=%d' % kv for kv in c.items()))
        if t['m']['note_only']:
            out('  note-only events: %d (left out of letters)' % t['m']['note_only'])
    read = [t for t in tracks if has_text(t)]
    for i, a in enumerate(read):
        for b in read[i + 1:]:
            p = pair(a, b)
            out('pair s%d s%d: aligned=%.3f/%.3f timing=%s CB=%s/%s content=%s UC=%d/%d subset=%s' % (
                a['n'], b['n'], p['aligned'][0], p['aligned'][1], p['timing'], fmt(p['cb'][0]), fmt(p['cb'][1]),
                p['content'], p['uc'][0], p['uc'][1],
                's%d in s%d' % (a['n'], b['n']) if subset_of(a, b, p) else
                's%d in s%d' % (b['n'], a['n']) if subset_of(b, a, {'cb': p['cb'][::-1]}) else 'no'))
    verdict = decide(tracks, orig_lang)
    today = spec87(tracks)
    out('original language: %s' % (orig_lang or 'unknown'))
    out('| Stream | Tag | Events | Class | Rule | Review | Note | 8.7 today | Before yes |')
    out('|---|---|---|---|---|---|---|---|---|')
    for t in tracks:
        v = verdict.get(t['n'], {'role': '?', 'review': None, 'note': None})
        cls = t['m']['class'] if has_text(t) else 'set aside' if t.get('set_aside') else 'not read'
        out('| s%d | %s | %d | %s | %s | %s | %s | %s | %s |' % (
            t['n'], t['tag'], len(t['events']), cls, v['role'], v['review'] or '-', v['note'] or '-',
            today.get(t['n'], '?'), before_yes(v, today.get(t['n'], '?'))))
    for t in tracks:
        v = verdict.get(t['n'], {})
        if has_text(t):
            m = t['m']
            what = 'M %d, SO %d, CS %d, lang %s, %s' % (m['M'], m['SO'], m['CS'], m['lang'], m['class'])
        else:
            what = 'set aside (closed captions)' if t.get('set_aside') else 'not read (timing only)'
        out('row: | %s/evidence.mks | s%d: %s %s, %d events | %s: %s%s | |' % (
            label, t['n'], t.get('codec') or '?', t['tag'], len(t['events']), what, v.get('role'),
            ' (review: %s)' % v['note'] if v.get('review') else ''))
    return verdict


# Words in mkvmerge's codec names of text subtitles (as subtitle_alignment.py): "SubRip/SRT", "SubStationAlpha",
# "WebVTT", "HDMV TextST". The DVD closed-caption stream is one: MakeMKV converts it to SubRip.
TEXT_CODECS = ('SubRip', 'SRT', 'SubStation', 'WebVTT', 'Text')


def is_text_codec(codec):
    return any(word in (codec or '') for word in TEXT_CODECS)


def load_title(rec, text, clear_max_bytes):
    """Every stream of index.json with its displayed events (a text stream: every packet, spec 8.7). A stream with
    OCR text gets its lines; a text stream without OCR text is set aside (closed captions, 8.7); any other stream
    without OCR text is marked unread ('text': False)."""
    rec, text = pathlib.Path(rec), pathlib.Path(text)
    index = read_json(rec / 'subs' / 'index.json')
    tracks = []
    for entry in index:
        n = int(entry['n'])
        codec = entry.get('codec')
        txt = is_text_codec(codec)
        events = packet_events(read_json(rec / 'subs' / ('sub-%d.packets.json' % n)), 0 if txt else clear_max_bytes)
        t = {'n': n, 'tag': entry.get('language') or 'und', 'codec': codec, 'forced': bool(entry.get('forced')),
             'events': events}
        jsonl = text / ('sub-%d.ocr.jsonl' % n)
        if jsonl.exists():
            recs = ocr_records(jsonl, rec / 'subs-ocr' / ('sub-%d.frames.json' % n))
            method, left = pair_text(events, recs)
            t['pairing'] = 'text by %s, %d frames with text, %d unmatched' % (method, len(recs), left)
        elif txt:
            t['set_aside'] = True
        else:
            t['text'] = False
        tracks.append(t)
    return tracks


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--rec', required=True, help='corpus/owner/<slug>/<label>')
    ap.add_argument('--text', required=True, help='folder holding sub-<n>.ocr.jsonl (a temporary copy)')
    ap.add_argument('--orig-lang', default=None, help="the title's original language (two letters); default "
                    'from the disc folder\'s tmdb.json')
    ap.add_argument('--clear-max-bytes', type=int, default=RULE['CLEAR_MAX_BYTES'])
    a = ap.parse_args(argv)
    rec = pathlib.Path(a.rec)
    try:
        tracks = load_title(rec, a.text, a.clear_max_bytes)
    except (OSError, ValueError, KeyError) as e:
        print('cannot read the input: %s' % e, file=sys.stderr)
        return 2
    if not any(has_text(t) for t in tracks):
        print('no sub-<n>.ocr.jsonl in %s for the streams of %s/subs/index.json' % (a.text, rec), file=sys.stderr)
        return 2
    # An English or no-language bitmap stream without text would leave the rule judging fewer tracks than the title
    # has: an input error, never a silent skip.
    missing = [t for t in tracks if t.get('text') is False and t['tag'] in ('eng', 'en', 'und')]
    for t in missing:
        print('stream s%d %s: no OCR text (no sub-%d.ocr.jsonl in %s); read it with ocr-subs.ps1' % (
            t['n'], t['tag'], t['n'], a.text), file=sys.stderr)
    if missing:
        return 2
    orig = a.orig_lang
    tmdb = rec.parent / 'tmdb.json'
    if orig is None and tmdb.exists():
        orig = read_json(tmdb).get('original_language')
    unread = sum(1 for t in tracks if t.get('text') is False)
    aside = sum(1 for t in tracks if t.get('set_aside'))
    extra = ''.join([', %d not read (another language)' % unread if unread else '',
                     ', %d set aside (closed captions)' % aside if aside else ''])
    print('%s: %d subtitle streams with OCR text%s; rule thresholds are provisional' % (
        rec, sum(1 for t in tracks if has_text(t)), extra))
    analyse(tracks, orig, rec.name)
    return 0


if __name__ == '__main__':
    sys.exit(main())
