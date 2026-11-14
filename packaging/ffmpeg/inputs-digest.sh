#!/usr/bin/env bash
# Prints the digest of the minimal FFmpeg build's inputs (T6-R6, notes call 4) as 64 lower-case hex digits and a
# newline: the SHA-256 (Secure Hash Algorithm 256) of the line
#   jq -cS '[.ffmpeg.version, .ffmpeg.source, .ffmpeg.zlib, .ffmpeg.runner, .ffmpeg.apt]'
# prints for the manifest, followed by the bytes of packaging/ffmpeg/build.sh. So the FFmpeg pin, the zlib pin, the
# runner label, the apt pins and the build script are the inputs, and a change to any one of them changes the digest,
# which un-verifies a recorded build (gate.sh status). The manifest's other parts (the built hashes, MKVToolNix, the
# models) are not inputs.
# The digest must be the same on Linux, macOS and Windows (Git Bash): .gitattributes keeps packaging/** at LF on every
# checkout, and jq's output goes through tr -d '\r', since jq on Windows ends its lines with CRLF.
# MANIFEST names the manifest (default packaging/tools.json); the build script is always packaging/ffmpeg/build.sh.
# Works with GNU sha256sum or with shasum -a 256.
# Exit 0 with the digest printed, 1 on any failure (with what failed on stderr), 2 on a usage error.
# Usage: bash packaging/ffmpeg/inputs-digest.sh
set -euo pipefail
[ "$#" -eq 0 ] || { echo "usage: bash packaging/ffmpeg/inputs-digest.sh" >&2; exit 2; }

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="${MANIFEST:-$here/../tools.json}"
build_script="$here/build.sh"

fail() { echo "inputs-digest: $*" >&2; exit 1; }

command -v jq >/dev/null 2>&1 || fail "jq is not on PATH"
[ -f "$manifest" ] || fail "the manifest $manifest is not a file"
[ -f "$build_script" ] || fail "the build script $build_script is not a file"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256; }
else
  fail "neither sha256sum nor shasum is on PATH"
fi

# Every input must be present: a missing key would make jq print null and hash a manifest that names no build.
jq -e '.ffmpeg | has("version") and has("source") and has("zlib") and has("runner") and has("apt")' \
  "$manifest" >/dev/null || fail "the manifest $manifest lacks one of ffmpeg.version, source, zlib, runner or apt"

inputs="$(jq -cS '[.ffmpeg.version, .ffmpeg.source, .ffmpeg.zlib, .ffmpeg.runner, .ffmpeg.apt]' "$manifest" \
  | tr -d '\r')" || fail "jq could not read $manifest"
# The command substitution drops jq's newline; it is put back, so the hashed bytes are jq's line as jq prints it.
line="$( { printf '%s\n' "$inputs"; cat "$build_script"; } | sha256 | tr -d '\r')" || fail "hashing failed"
digest="${line%% *}"
case "$digest" in
  *[!0-9a-f]*) fail "the hash tool printed '$line', not a digest" ;;
esac
[ "${#digest}" -eq 64 ] || fail "the hash tool printed '$line', not a 64-digit digest"
printf '%s\n' "$digest"
