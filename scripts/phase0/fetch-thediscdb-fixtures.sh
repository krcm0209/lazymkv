#!/usr/bin/env bash
# Download a pinned set of TheDiscDB scan logs (MIT licence) into corpus/thediscdb.
set -euo pipefail
COMMIT=b54d1922080fb1947912500db126cf95dcdfaa45
BASE="https://raw.githubusercontent.com/TheDiscDb/data/$COMMIT"
DEST="$(git rev-parse --show-toplevel)/corpus/thediscdb"

urlencode() { python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$1"; }

# fetch <release folder under data/> <disc name>...
fetch() {
  local dir="$1"; shift
  mkdir -p "$DEST/$dir"
  for disc in "$@"; do
    for suffix in .txt .json -summary.txt; do
      curl -fsS -m 60 "$BASE/data/$(urlencode "$dir")/$disc$suffix" -o "$DEST/$dir/$disc$suffix"
    done
  done
}

fetch "movie/John Wick (2014)/4k-ultra-hd--blu-ray" disc01 disc02
fetch "movie/The Expendables 3 (2014)/the-expendables-3-film-collection" disc01
fetch "movie/Smile (2022)/2022-4k" disc01
fetch "movie/Knowing (2009)/2010-dvd" disc01
fetch "series/Chernobyl (2019)/2020-4k-US" disc01 disc02
fetch "series/Firefly (2002)/2003-dvd-the-complete-series" disc01 disc02
curl -fsS -m 60 "$BASE/LICENSE" -o "$DEST/LICENSE"

# Save the live API's answer for each pinned disc, by content hash and by disc ID (spec 8.1).
API="$(git rev-parse --show-toplevel)/corpus/thediscdb-api"
mkdir -p "$API"
find "$DEST" -name 'disc*.json' | while read -r json; do
  hash="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8-sig")).get("ContentHash") or "")' "$json")"
  id="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8-sig")).get("GlobalDiscId") or "")' "$json")"
  [ -n "$hash" ] && curl -fsS -m 60 "https://thediscdb.com/api/dischash/$hash" -o "$API/dischash-$hash.json"
  [ -n "$id" ] && curl -fsS -m 60 "https://thediscdb.com/api/discid/$id" -o "$API/discid-$id.json"
  sleep 1
done
echo "saved $(find "$API" -name '*.json' | wc -l | tr -d ' ') API responses"
echo "fetched $(find "$DEST" -name 'disc*.txt' ! -name '*-summary.txt' | wc -l | tr -d ' ') logs"
