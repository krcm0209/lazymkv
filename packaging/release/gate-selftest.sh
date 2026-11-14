#!/usr/bin/env bash
# The release gate's self-test (notes call 4, T6-R6, T6-R11): runs packaging/release/gate.sh over the six fixture
# manifests of packaging/release/fixtures with INPUTS_SHA256 set to 64 a digits, and stops at the first call whose
# output (stdout and stderr together) or exit status differs from the expected one, printing both. The fixtures
# hold made-up hashes only. The hashes cases use two made-up programs this script writes into a temporary folder
# with fixed text; unverified.json records those two files' SHA-256 (Secure Hash Algorithm 256) values, so no
# fixture changes at run time. Besides the matching and the mismatching cases, none.json (no program hash recorded:
# the values are printed and the exit status is 0) and half.json (only one recorded: a mismatch, exit 1) are run
# through the hashes branch too (the review of pull request #21 (X12)).
# Then packaging/ffmpeg/inputs-digest.sh runs twice over the real manifest and must
# print the same 64 hex digits both times.
# Prints "gate self-test: 17 passed" and exits 0 when every case passes; exits 1 at the first failure.
# Usage: bash packaging/release/gate-selftest.sh
set -euo pipefail
[ "$#" -eq 0 ] || { echo "usage: bash packaging/release/gate-selftest.sh" >&2; exit 2; }

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
gate="$here/gate.sh"
fixtures="$here/fixtures"
a64="$(printf '%064d' 0 | tr 0 a)"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# The two made-up programs; unverified.json records exactly these files' SHA-256 values.
printf 'lazymkv gate self-test: a made-up ffmpeg.exe\n' > "$work/ffmpeg.exe"
printf 'lazymkv gate self-test: a made-up ffprobe.exe\n' > "$work/ffprobe.exe"
made_up_ffmpeg=19aacc86ed74a59519d0f75c3fd96e2a19ca61b2968790c3bcb8f4acddb0f383
made_up_ffprobe=48878b3fef5c9a66a2cc19f70eb009de9eab445d2f75a7433d3a5f66e3df03e1
c64="$(printf '%064d' 0 | tr 0 c)"
d64="$(printf '%064d' 0 | tr 0 d)"

passed=0
# check <fixture> <expected output> <expected exit> <gate arguments...>
check() {
  local fixture="$1" want_out="$2" want_status="$3" out status
  shift 3
  if out="$(MANIFEST="$fixtures/$fixture" INPUTS_SHA256="$a64" bash "$gate" "$@" 2>&1 | tr -d '\r')"; then
    status=0
  else
    status=$?
  fi
  if [ "$out" != "$want_out" ] || [ "$status" -ne "$want_status" ]; then
    {
      echo "gate self-test: case $((passed + 1)) failed: gate.sh $* with $fixture"
      echo "expected exit $want_status and output:"
      printf '%s\n' "$want_out"
      echo "got exit $status and output:"
      printf '%s\n' "$out"
    } >&2
    exit 1
  fi
  passed=$((passed + 1))
}

check none.json "unverified: no build is recorded" 0 status
check half.json "unverified: no build is recorded" 0 status
check unverified.json "unverified: the build was never verified on the target machine" 0 status
check stale.json "unverified: the build's inputs changed after it was verified" 0 status
check verified.json "verified" 0 status
check verified.json "" 0 publish refs/tags/v1.2.3 1.2.3
check verified.json "refused: the workspace version is 0.0.0" 1 publish refs/tags/v0.0.0 0.0.0
check verified.json "refused: the tag is not v1.2.3" 1 publish refs/tags/v1.2.4 1.2.3
check verified.json "refused: the tag is not v1.2.3" 1 publish refs/heads/main 1.2.3
check unverified.json "refused: unverified: the build was never verified on the target machine" 1 \
  publish refs/tags/v1.2.3 1.2.3
check stale.json "refused: unverified: the build's inputs changed after it was verified" 1 \
  publish refs/tags/v1.2.3 1.2.3
check unverified.json "" 0 hashes "$work/ffmpeg.exe" "$work/ffprobe.exe"
check verified.json "$(printf '%s\n%s' \
  "ffmpeg.exe: the file's SHA-256 is $made_up_ffmpeg, the manifest records $c64" \
  "ffprobe.exe: the file's SHA-256 is $made_up_ffprobe, the manifest records $d64")" 1 \
  hashes "$work/ffmpeg.exe" "$work/ffprobe.exe"
# The review of pull request #21 (X12): a manifest that records no program hash prints the two files' values.
check none.json "$(printf '%s\n%s\n%s' "ffmpeg.exe $made_up_ffmpeg" "ffprobe.exe $made_up_ffprobe" \
  "not recorded yet")" 0 hashes "$work/ffmpeg.exe" "$work/ffprobe.exe"
# The review of pull request #21 (X12): a manifest that records only one program hash is a mismatch, not "not recorded".
check half.json "$(printf '%s\n%s' \
  "ffmpeg.exe: the file's SHA-256 is $made_up_ffmpeg, the manifest records $c64" \
  "ffprobe.exe: the file's SHA-256 is $made_up_ffprobe, the manifest records null")" 1 \
  hashes "$work/ffmpeg.exe" "$work/ffprobe.exe"
check rehashed.json "unverified: the recorded programs changed after they were verified" 0 status
check rehashed.json "refused: unverified: the recorded programs changed after they were verified" 1 \
  publish refs/tags/v1.2.3 1.2.3

first="$(MANIFEST="$here/../tools.json" bash "$here/../ffmpeg/inputs-digest.sh")"
second="$(MANIFEST="$here/../tools.json" bash "$here/../ffmpeg/inputs-digest.sh")"
case "$first" in
  *[!0-9a-f]*) echo "gate self-test: inputs-digest.sh printed '$first', not hex digits" >&2; exit 1 ;;
esac
if [ "${#first}" -ne 64 ]; then
  echo "gate self-test: inputs-digest.sh printed '$first', not 64 hex digits" >&2
  exit 1
fi
if [ "$first" != "$second" ]; then
  echo "gate self-test: inputs-digest.sh printed $first, then $second" >&2
  exit 1
fi

echo "gate self-test: $passed passed"
