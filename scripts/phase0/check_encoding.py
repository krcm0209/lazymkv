"""Experiment 8: which encoding does makemkvcon use for robot output?"""
import pathlib
import sys


def main(path):
    data = pathlib.Path(path).read_bytes()
    try:
        data.decode("utf-8")
        print("valid UTF-8: yes")
    except UnicodeDecodeError as error:
        print(f"valid UTF-8: NO ({error})")
    shown = 0
    for raw in data.splitlines():
        if any(byte > 127 for byte in raw):
            print("utf-8 :", raw.decode("utf-8", errors="replace")[:160])
            print("cp1252:", raw.decode("cp1252", errors="replace")[:160])
            shown += 1
            if shown == 5:
                break
    print(f"lines with non-ASCII bytes shown: {shown}")
    quoted = [raw for raw in data.splitlines() if b'\\"' in raw]
    print(f"lines with a backslash-escaped quote: {len(quoted)}")
    for raw in quoted[:3]:
        print("  ", raw.decode("utf-8", errors="replace")[:160])


if __name__ == "__main__":
    main(sys.argv[1])
