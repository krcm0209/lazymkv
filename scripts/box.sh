#!/usr/bin/env bash
# Mirror the working tree to the mini PC and optionally run a command there.
# Usage: scripts/box.sh                 (sync only)
#        scripts/box.sh cargo test -p lazymkv-core
set -euo pipefail
# The mini PC's ssh target comes from scripts/phase0/box.env (git-ignored: LAZYMKV_BOX=user@host) or the environment.
[ -f "$(git rev-parse --show-toplevel)/scripts/phase0/box.env" ] && . "$(git rev-parse --show-toplevel)/scripts/phase0/box.env"
HOST="${LAZYMKV_BOX:-USER@makemkv-box}"
cd "$(git rev-parse --show-toplevel)"
ARCHIVE="$(mktemp -t lazymkv-src)"
trap 'rm -f "$ARCHIVE"' EXIT
git ls-files -co --exclude-standard -z | COPYFILE_DISABLE=1 tar --null -T - -czf "$ARCHIVE"
ssh -o BatchMode=yes "$HOST" '(if exist C:\lazymkv\src rmdir /s /q C:\lazymkv\src) & mkdir C:\lazymkv\src'
scp -q -o BatchMode=yes "$ARCHIVE" "$HOST:C:/lazymkv/src.tgz"
ssh -o BatchMode=yes "$HOST" 'tar -xzf C:\lazymkv\src.tgz -C C:\lazymkv\src && del C:\lazymkv\src.tgz'
if [ "$#" -gt 0 ]; then
  status=0
  ssh -o BatchMode=yes "$HOST" "cd /d C:\\lazymkv\\src && set CARGO_TARGET_DIR=C:\\lazymkv\\target&& set PATH=%USERPROFILE%\\.cargo\\bin;%PATH%&& $*" || status=$?
  # cargo writes Cargo.lock on the mini PC; bring it back so it can be committed.
  scp -q -o BatchMode=yes "$HOST:C:/lazymkv/src/Cargo.lock" Cargo.lock 2>/dev/null || true
  exit "$status"
fi
