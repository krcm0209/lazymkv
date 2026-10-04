#!/usr/bin/env bash
# Copy a recording from the mini PC into corpus/owner/<slug>.
set -euo pipefail
SLUG="$1"
# A slug is lowercase letters, digits and hyphens; anything else must never reach rm -rf below.
[[ "$SLUG" =~ ^[a-z0-9][a-z0-9-]*$ ]] || { echo "bad slug: '$SLUG'" >&2; exit 2; }
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
mkdir -p corpus/owner
rm -rf "corpus/owner/$SLUG"
scp -q -r -o BatchMode=yes "$HOST:C:/lazymkv/phase0/recordings/$SLUG" corpus/owner/
echo "pulled corpus/owner/$SLUG ($(find "corpus/owner/$SLUG" -type f | wc -l | tr -d ' ') files)"
