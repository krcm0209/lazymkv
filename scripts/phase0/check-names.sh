#!/usr/bin/env bash
# Commit check: no tracked or staged file names the mini PC's Windows user or this Mac's user (the repository is
# public). The names are never written here: the mini PC's is the user part of LAZYMKV_BOX (user@host) from the
# git-ignored scripts/phase0/box.env or the environment, and the Mac's is `id -un`. Every line that names either
# (case-insensitive) is printed, as git grep prints it, from the working tree's tracked files and from the index;
# binary files are searched too (git grep -a).
# Exit 0 when nothing is printed, 1 when a line was, 2 on a usage error.
# Usage: scripts/phase0/check-names.sh
set -euo pipefail
[ "$#" -eq 0 ] || { echo "usage: scripts/phase0/check-names.sh" >&2; exit 2; }
cd "$(git rev-parse --show-toplevel)"
[ -f scripts/phase0/box.env ] && . scripts/phase0/box.env
names=()
box="${LAZYMKV_BOX:-}"
box_user="${box%%@*}"
if [ -n "$box" ] && [ "$box_user" != "$box" ] && [ -n "$box_user" ]; then
  names+=("$box_user")
else
  echo "note: LAZYMKV_BOX (scripts/phase0/box.env) names no user, so only this Mac's user name is checked" >&2
fi
names+=("$(id -un)")
found=0
for name in "${names[@]}"; do
  # USER is the placeholder the scrub writes; it is not a name to look for.
  [ "$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]')" = user ] && continue
  hits="$( { git grep -nia -F -e "$name" || true; git grep --cached -nia -F -e "$name" | sed 's/^/index: /' || true; } )"
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits"
    found=1
  fi
done
exit "$found"
