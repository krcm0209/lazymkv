"""Parser for makemkvcon robot-mode output (spec 9.4)."""
from dataclasses import dataclass, field


def split_fields(text):
    """Split the text after 'KIND:' into fields.

    Quoted fields may contain commas, backslash escapes and doubled quotes.
    """
    fields, current, quoted = [], [], False
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if quoted:
            if c == "\\" and i + 1 < n:
                current.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                if i + 1 < n and text[i + 1] == '"':
                    current.append('"')
                    i += 2
                    continue
                quoted = False
            else:
                current.append(c)
        elif c == '"':
            quoted = True
        elif c == ",":
            fields.append("".join(current))
            current = []
        else:
            current.append(c)
        i += 1
    fields.append("".join(current))
    return fields


@dataclass
class Scan:
    messages: list = field(default_factory=list)  # (code, text, params)
    drives: list = field(default_factory=list)    # one field list per DRV line
    disc: dict = field(default_factory=dict)      # attribute -> value
    titles: dict = field(default_factory=dict)    # title -> {attribute: value}
    streams: dict = field(default_factory=dict)   # (title, stream) -> {attribute: value}


def parse(text):
    scan = Scan()
    for raw in text.splitlines():
        kind, separator, rest = raw.lstrip("﻿").partition(":")
        if not separator:
            continue
        f = split_fields(rest)
        try:
            if kind == "MSG":
                scan.messages.append((int(f[0]), f[3], f[5:]))
            elif kind == "DRV":
                scan.drives.append(f)
            elif kind == "CINFO":
                scan.disc[int(f[0])] = f[2]
            elif kind == "TINFO":
                scan.titles.setdefault(int(f[0]), {})[int(f[1])] = f[3]
            elif kind == "SINFO":
                scan.streams.setdefault((int(f[0]), int(f[1])), {})[int(f[2])] = f[4]
        except (ValueError, IndexError):
            continue
    return scan


def duration_seconds(text):
    hours, minutes, seconds = (int(part) for part in text.split(":"))
    return hours * 3600 + minutes * 60 + seconds
