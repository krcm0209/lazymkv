#!/usr/bin/env bash
# Copy a recording from the mini PC into corpus/owner/<slug>.
# corpus/owner is the private repository lazymkv-evidence, cloned in place and ignored by this repository: the
# recording is committed there, never here, and the last line printed says how. Without that clone (no
# corpus/owner/.git) the script stops with exit 2 before it touches the mini PC, and nothing is changed.
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
if [ ! -d corpus/owner/.git ] && [ ! -f corpus/owner/.git ]; then
  echo "corpus/owner is not the lazymkv-evidence clone; clone https://github.com/krcm0209/lazymkv-evidence at corpus/owner first" >&2
  exit 2
fi
TMP="$(mktemp -d "${TMPDIR:-/tmp}/lazymkv-pull.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
scp -q -r -o BatchMode=yes "$HOST:C:/lazymkv/phase0/recordings/$SLUG" "$TMP/"
[ -f "$TMP/$SLUG/meta.json" ] || { echo "recording $SLUG on the mini PC has no meta.json; nothing was changed in corpus/owner" >&2; exit 1; }
# The copies of the disc's navigation files may arrive read-only; make them replaceable by the next pull.
chmod -R u+w "$TMP/$SLUG"
# The mini PC's Windows user name never enters a repository (this one is public; the evidence repository keeps the
# same rule). MakeMKV's debug lines and the experiment logs name the user's folder (C:\Users\<name>, or
# C:\\Users\\<name> inside JSON); in every text file the name is replaced by USER there, before anything reaches
# corpus/owner. The name is the user part of the ssh target. The files that still name it are listed below:
# scripts/phase0/check-names.sh checks this repository's files only, not the evidence repository's.
BOX_USER="${HOST%%@*}"
if [ "$BOX_USER" = "$HOST" ] || [ -z "$BOX_USER" ] || [ "$(printf '%s' "$BOX_USER" | tr '[:upper:]' '[:lower:]')" = user ]; then
  echo "note: the ssh target names no user (or only the placeholder USER), so no user name was scrubbed; check corpus/owner/$SLUG for the mini PC's user name before committing it" >&2
else
  named="$(grep -rIil -F -e "$BOX_USER" "$TMP/$SLUG" || true)"
  if [ -n "$named" ]; then
    while IFS= read -r f; do
      LAZYMKV_SCRUB_NAME="$BOX_USER" perl -0777 -i -pe 'BEGIN { $n = quotemeta $ENV{LAZYMKV_SCRUB_NAME} }
        s/(C:(?:\\\\|\\)Users(?:\\\\|\\))$n(?![A-Za-z0-9._\$-])/${1}USER/gi' "$f"
    done <<< "$named"
  fi
  left="$(grep -rail -F -e "$BOX_USER" "$TMP/$SLUG" | sed "s|^$TMP/$SLUG/||" || true)"  # binary files too
  echo "user name: in $(printf '%s' "$named" | grep -c . || true) text file(s), replaced by USER in their C:\Users paths;" \
       "$(printf '%s' "$left" | grep -c . || true) file(s) still name it"
  if [ -n "$left" ]; then
    echo "warning: these files still contain the mini PC's user name (outside a C:\Users path, or in a binary file); edit them before committing:" >&2
    echo "$left" >&2
  fi
fi
mkdir -p "corpus/owner/$SLUG"
cp -Rf "$TMP/$SLUG/." "corpus/owner/$SLUG/"
copied="$(find "$TMP/$SLUG" -type f | wc -l | tr -d ' ')"
total="$(find "corpus/owner/$SLUG" -type f | wc -l | tr -d ' ')"
echo "pulled corpus/owner/$SLUG ($copied files copied, $total files there now)"
echo "commit it there: git -C corpus/owner add $SLUG && git -C corpus/owner commit"
