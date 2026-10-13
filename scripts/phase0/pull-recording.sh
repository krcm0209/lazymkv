#!/usr/bin/env bash
# Copy a recording from the mini PC into corpus/owner/<slug>.
# Nothing in corpus/owner is ever deleted: the recording is first copied into a fresh temporary
# folder, and only then are its files copied over corpus/owner/<slug>/, replacing files of the same
# name and keeping files that exist only on the Mac (such as a hand-written notes.md).
# A failed copy from the mini PC changes nothing in corpus/owner.
set -euo pipefail
[ "$#" -eq 1 ] || { echo "usage: scripts/phase0/pull-recording.sh <slug>" >&2; exit 2; }
SLUG="$1"
# A slug is lowercase letters, digits and hyphens; it names a folder on both machines.
[[ "$SLUG" =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "bad slug: '$SLUG'" >&2; exit 2; }
# The mini PC's ssh target comes from box.env beside this script (git-ignored: LAZYMKV_BOX=user@host) or the environment.
[ -f "$(dirname "$0")/box.env" ] && . "$(dirname "$0")/box.env"
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/lazymkv-pull.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
scp -q -r -o BatchMode=yes "$HOST:C:/lazymkv/phase0/recordings/$SLUG" "$TMP/"
[ -f "$TMP/$SLUG/meta.json" ] || { echo "recording $SLUG on the mini PC has no meta.json; nothing was changed in corpus/owner" >&2; exit 1; }
# The copies of the disc's navigation files may arrive read-only; make them replaceable by the next pull.
chmod -R u+w "$TMP/$SLUG"
mkdir -p "corpus/owner/$SLUG"
cp -Rf "$TMP/$SLUG/." "corpus/owner/$SLUG/"
copied="$(find "$TMP/$SLUG" -type f | wc -l | tr -d ' ')"
total="$(find "corpus/owner/$SLUG" -type f | wc -l | tr -d ' ')"
echo "pulled corpus/owner/$SLUG ($copied files copied, $total files there now)"
