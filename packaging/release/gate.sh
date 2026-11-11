#!/usr/bin/env bash
# The release gate over the tool manifest (notes call 4, T6-R6, T6-R11; design parts 11.3 and 11.5).
#
#   gate.sh status
#     Prints "verified" when ffmpeg.built records both programs' SHA-256 (Secure Hash Algorithm 256) values and
#     ffmpeg.built.verified.inputs_sha256 equals INPUTS_SHA256; else "unverified: <reason>", the reason one of
#     "no build is recorded", "the build was never verified on the target machine" or "the build's inputs changed
#     after it was verified". Exits 0 either way: the caller decides what an unverified build means.
#   gate.sh hashes <ffmpeg.exe> <ffprobe.exe>
#     Exits 0 when both files' SHA-256 values equal the manifest's. While the manifest records neither value, it
#     prints the two files' values and "not recorded yet" and exits 0, so the first build's values can be recorded.
#     Else it prints, for each program, the file's value and the manifest's, and exits 1 (release step 3).
#   gate.sh publish <ref> <version>
#     Exits 0 only when <ref> is refs/tags/v<version>, <version> is not 0.0.0 and status is verified; else prints
#     "refused: <reason>" and exits 1 (T6-R11, D1).
#
# MANIFEST names the manifest (default packaging/tools.json). INPUTS_SHA256 is the digest of the build's inputs
# (default: what packaging/ffmpeg/inputs-digest.sh prints for MANIFEST). Every jq output goes through tr -d '\r', since
# jq on Windows (Git Bash) ends its lines with CRLF (T6-R6). A manifest whose recorded value is neither null nor 64
# lower-case hex digits, or a missing file, is an error: exit 2 with what is wrong. Works with GNU sha256sum or with
# shasum -a 256.
# Usage: bash packaging/release/gate.sh status | hashes <ffmpeg.exe> <ffprobe.exe> | publish <ref> <version>
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="${MANIFEST:-$here/../tools.json}"

usage() {
  echo "usage: bash packaging/release/gate.sh status | hashes <ffmpeg.exe> <ffprobe.exe> | publish <ref> <version>" >&2
  exit 2
}
error() { echo "gate: $*" >&2; exit 2; }

command -v jq >/dev/null 2>&1 || error "jq is not on PATH"
[ -f "$manifest" ] || error "the manifest $manifest is not a file"

# Prints the jq filter's raw value from the manifest, CR removed; "null" for a null or missing value.
field() {
  local value
  value="$(jq -r "$1" "$manifest" | tr -d '\r')" || error "jq could not read $1 from $manifest"
  printf '%s' "$value"
}

is_hex64() {
  case "$1" in
    *[!0-9a-f]*) return 1 ;;
  esac
  [ "${#1}" -eq 64 ]
}

# A recorded value: null, or 64 lower-case hex digits; anything else is a broken manifest, not an unverified build.
recorded() {
  local value
  value="$(field "$1")"
  if [ "$value" != null ] && ! is_hex64 "$value"; then
    error "$1 in $manifest is '$value', neither null nor 64 lower-case hex digits"
  fi
  printf '%s' "$value"
}

file_sha256() {
  local line
  [ -f "$1" ] || error "$1 is not a file"
  if command -v sha256sum >/dev/null 2>&1; then
    line="$(sha256sum < "$1" | tr -d '\r')" || error "sha256sum failed on $1"
  elif command -v shasum >/dev/null 2>&1; then
    line="$(shasum -a 256 < "$1" | tr -d '\r')" || error "shasum failed on $1"
  else
    error "neither sha256sum nor shasum is on PATH"
  fi
  line="${line%% *}"
  is_hex64 "$line" || error "the hash tool printed '$line' for $1, not a digest"
  printf '%s' "$line"
}

# Sets the global reason to the empty string when the build is verified, else to why not.
reason=""
judge() {
  local ffmpeg_sha ffprobe_sha verified inputs
  ffmpeg_sha="$(recorded .ffmpeg.built.ffmpeg_exe_sha256)"
  ffprobe_sha="$(recorded .ffmpeg.built.ffprobe_exe_sha256)"
  verified="$(field '.ffmpeg.built.verified')"
  if [ -n "${INPUTS_SHA256:-}" ]; then
    inputs="$INPUTS_SHA256"
  else
    inputs="$(MANIFEST="$manifest" bash "$here/../ffmpeg/inputs-digest.sh")" || error "inputs-digest.sh failed"
  fi
  is_hex64 "$inputs" || error "INPUTS_SHA256 is '$inputs', not 64 lower-case hex digits"
  if [ "$ffmpeg_sha" = null ] || [ "$ffprobe_sha" = null ]; then
    reason="no build is recorded"
  elif [ "$verified" = null ]; then
    reason="the build was never verified on the target machine"
  else
    local verified_inputs
    verified_inputs="$(recorded .ffmpeg.built.verified.inputs_sha256)"
    [ "$verified_inputs" != null ] || error "ffmpeg.built.verified in $manifest is set but holds no inputs_sha256"
    if [ "$verified_inputs" = "$inputs" ]; then
      reason=""
    else
      reason="the build's inputs changed after it was verified"
    fi
  fi
}

[ "$#" -ge 1 ] || usage
case "$1" in
  status)
    [ "$#" -eq 1 ] || usage
    judge
    if [ -z "$reason" ]; then
      echo verified
    else
      echo "unverified: $reason"
    fi
    ;;
  hashes)
    [ "$#" -eq 3 ] || usage
    built_ffmpeg="$(file_sha256 "$2")"
    built_ffprobe="$(file_sha256 "$3")"
    want_ffmpeg="$(recorded .ffmpeg.built.ffmpeg_exe_sha256)"
    want_ffprobe="$(recorded .ffmpeg.built.ffprobe_exe_sha256)"
    if [ "$want_ffmpeg" = null ] && [ "$want_ffprobe" = null ]; then
      echo "ffmpeg.exe $built_ffmpeg"
      echo "ffprobe.exe $built_ffprobe"
      echo "not recorded yet"
    elif [ "$built_ffmpeg" != "$want_ffmpeg" ] || [ "$built_ffprobe" != "$want_ffprobe" ]; then
      echo "ffmpeg.exe: the file's SHA-256 is $built_ffmpeg, the manifest records $want_ffmpeg" >&2
      echo "ffprobe.exe: the file's SHA-256 is $built_ffprobe, the manifest records $want_ffprobe" >&2
      exit 1
    fi
    ;;
  publish)
    [ "$#" -eq 3 ] || usage
    ref="$2"
    version="$3"
    [ -n "$version" ] || error "the version is empty"
    if [ "$version" = 0.0.0 ]; then
      echo "refused: the workspace version is 0.0.0" >&2
      exit 1
    fi
    if [ "$ref" != "refs/tags/v$version" ]; then
      echo "refused: the tag is not v$version" >&2
      exit 1
    fi
    judge
    if [ -n "$reason" ]; then
      echo "refused: unverified: $reason" >&2
      exit 1
    fi
    ;;
  *)
    usage
    ;;
esac
