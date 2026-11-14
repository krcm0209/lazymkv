#!/usr/bin/env bash
# Builds lazymkv's minimal LGPL (GNU Lesser General Public License) FFmpeg for 64-bit Windows: ffmpeg.exe and
# ffprobe.exe, cross-compiled on the hosted Linux runner the manifest names (design parts 11.2 and 11.3, P16, K1, K4;
# notes calls 2 and 3; T6-R6, T6-R7). This script is the authority for the configure line (design part 11.2).
#
# It reads every pin from packaging/tools.json with jq: installs the apt packages at their pinned versions, fetches the
# FFmpeg tarball and the zlib source archive and checks each SHA-256 (Secure Hash Algorithm 256) before unpacking,
# builds zlib as a static library with the mingw-w64 cross-compiler, then configures FFmpeg in its own tree and builds
# it. It writes into <out dir>, which must not exist or be empty:
#   ffmpeg.exe, ffprobe.exe          the two programs
#   ffmpeg-build-settings.txt        this script's text, the configure line as run, the versions of GCC (the GNU
#                                    Compiler Collection), binutils, nasm and zlib, and FFmpeg's configure summary
#   mingw-w64-runtime-copyright.txt  /usr/share/doc/mingw-w64-common/copyright, the notice of the runtime parts the
#                                    static link takes in
#   COPYING.LGPLv2.1, LICENSE.md     from the FFmpeg tarball
#   LICENSE                          zlib's, from the zlib archive
#   inputs-sha256.txt                packaging/ffmpeg/inputs-digest.sh's line, the digest of this build's inputs
#
# Reproducibility (K4): two runs of this script with the same inputs must give byte-identical programs, and the
# workflow fails otherwise. What could make two builds differ is held fixed here:
#   - Paths. FFmpeg writes its configure arguments into both programs, so every path in them must be the same on every
#     run: the build folder is always /tmp/lazymkv-ffmpeg, removed first, and zlib is always installed under
#     /tmp/lazymkv-ffmpeg/zlib (T6-R7). Nothing of the checkout's or the runner's temporary folder reaches the build.
#   - Time. -Wl,--no-insert-timestamp keeps GNU ld from stamping the link time into the PE (Portable Executable)
#     header, and SOURCE_DATE_EPOCH is the modification time of the tarball's RELEASE file (T6-R7), which binutils
#     uses for any PE time stamp it still writes, strip's rewrite of the programs included. TZ and the locale are
#     fixed too.
#   - The version string. FFmpeg's ffbuild/version.sh asks git for a revision before it reads RELEASE;
#     GIT_CEILING_DIRECTORIES stops git above the build folder, so the version is RELEASE's on every run, and the
#     script checks it.
#   - Archives. zlib's static library is made with ar's deterministic mode (D), which stores no time, owner or mode;
#     FFmpeg's configure picks rcD itself when ar has it.
#   - Parallel make. make -j changes only the order in which objects are compiled; each library's members and the
#     link line keep the Makefile's order, so the bytes do not depend on it.
#   - Stripping. FFmpeg's Makefile links ffmpeg_g.exe and ffprobe_g.exe and writes ffmpeg.exe and ffprobe.exe from
#     them with the cross strip, so the shipped programs carry no debug information and no build paths in it.
#   - Libraries. --disable-autodetect keeps out every library the runner happens to have; zlib is the only one linked,
#     and pkg-config is pointed at an empty folder so it can name no other.
#
# Usage: bash packaging/ffmpeg/build.sh <out dir>
set -euo pipefail

fail() { echo "build.sh: $*" >&2; exit 1; }

if [ "$#" -ne 1 ] || [ -z "$1" ]; then
  echo "usage: bash packaging/ffmpeg/build.sh <out dir>" >&2
  exit 2
fi

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script="$here/build.sh"
manifest="$here/../tools.json"
root=/tmp/lazymkv-ffmpeg
zlib_prefix="$root/zlib"
cross=x86_64-w64-mingw32-

command -v jq >/dev/null 2>&1 || fail "jq is not on PATH"
[ -f "$manifest" ] || fail "the manifest $manifest is not a file"

# One value from the manifest, CR removed (T6-R6); a null or missing value fails.
pin() {
  local value
  value="$(jq -er "$1" "$manifest" | tr -d '\r')" || fail "the manifest has no value at $1"
  printf '%s' "$value"
}

ffmpeg_version="$(pin .ffmpeg.version)"
ffmpeg_url="$(pin .ffmpeg.source.url)"
ffmpeg_sha256="$(pin .ffmpeg.source.sha256)"
zlib_version="$(pin .ffmpeg.zlib.version)"
zlib_url="$(pin .ffmpeg.zlib.url)"
zlib_sha256="$(pin .ffmpeg.zlib.sha256)"
runner="$(pin .ffmpeg.runner)"
apt_pins=()
while IFS= read -r line; do
  apt_pins+=("$line")
done < <(jq -er '.ffmpeg.apt[]' "$manifest" | tr -d '\r')
[ "${#apt_pins[@]}" -gt 0 ] || fail "the manifest's ffmpeg.apt lists no package"

# The out folder: made absolute, outside the build folder (which is removed below), and empty, so the artifact holds
# exactly the files listed above.
mkdir -p "$1" || fail "cannot make the out folder $1"
out="$(cd "$1" && pwd)"
case "$out/" in
  "$root"/*) fail "the out folder $out is inside the build folder $root, which this script removes" ;;
esac
[ -z "$(ls -A "$out")" ] || fail "the out folder $out is not empty"

# The runner the manifest names (notes call 2): the image label is ubuntu-24.04, so the system must be Ubuntu 24.04.
[ -r /etc/os-release ] || fail "/etc/os-release is missing, so the system cannot be checked against $runner"
# /etc/os-release is the runner's own file, read only for ID and VERSION_ID; shellcheck cannot follow it.
# shellcheck disable=SC1091
system="$(. /etc/os-release && printf '%s-%s' "$ID" "$VERSION_ID")"
[ "$system" = "$runner" ] || fail "this system is $system, not the manifest's runner $runner"

export LC_ALL=C TZ=UTC
umask 022

echo "== installing the apt pins: ${apt_pins[*]}"
if [ "$(id -u)" -eq 0 ]; then sudo=(); else sudo=(sudo); fi
"${sudo[@]}" apt-get update || fail "apt-get update failed"
"${sudo[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${apt_pins[@]}" \
  || fail "apt-get could not install the pinned packages ${apt_pins[*]}"
for spec in "${apt_pins[@]}"; do
  name="${spec%%=*}"
  want="${spec#*=}"
  # dpkg-query's format strings are literal; the single quotes are meant.
  # shellcheck disable=SC2016
  got="$(dpkg-query -W -f='${Version}' "$name")" || fail "$name is not installed after apt-get install"
  [ "$got" = "$want" ] || fail "$name is at version $got, not the pinned $want"
done

for tool in gcc ld ar strip windres; do
  command -v "$cross$tool" >/dev/null 2>&1 || fail "$cross$tool is not on PATH after the install"
done
command -v nasm >/dev/null 2>&1 || fail "nasm is not on PATH after the install"
# The win32 variant of the cross-compiler, whose thread model is win32: --enable-w32threads links no pthreads library.
gcc_v="$("${cross}gcc" -v 2>&1)" || fail "${cross}gcc -v failed"
thread_model="$(printf '%s\n' "$gcc_v" | sed -n 's/^Thread model: //p')"
[ "$thread_model" = win32 ] || fail "${cross}gcc has the thread model '$thread_model', not win32"

echo "== preparing $root"
rm -rf "$root"
mkdir -p "$root/downloads" "$zlib_prefix/include" "$zlib_prefix/lib" "$root/pkgconfig-empty"

# fetch <url> <sha256> <file>: downloads, then checks the SHA-256 before anything unpacks it.
fetch() {
  local got
  echo "== fetching $1"
  curl --fail --location --silent --show-error --retry 3 --output "$3" "$1" || fail "could not download $1"
  got="$(sha256sum "$3")"
  got="${got%% *}"
  [ "$got" = "$2" ] || fail "$1 has SHA-256 $got, not the pinned $2"
}

ffmpeg_tar="$root/downloads/ffmpeg-$ffmpeg_version.tar.xz"
zlib_tar="$root/downloads/zlib-$zlib_version.tar.xz"
fetch "$ffmpeg_url" "$ffmpeg_sha256" "$ffmpeg_tar"
fetch "$zlib_url" "$zlib_sha256" "$zlib_tar"

tar -xJf "$ffmpeg_tar" -C "$root" || fail "could not unpack $ffmpeg_tar"
tar -xJf "$zlib_tar" -C "$root" || fail "could not unpack $zlib_tar"
ffmpeg_src="$root/ffmpeg-$ffmpeg_version"
zlib_src="$root/zlib-$zlib_version"
[ -d "$ffmpeg_src" ] || fail "the FFmpeg tarball did not unpack into $ffmpeg_src"
[ -d "$zlib_src" ] || fail "the zlib archive did not unpack into $zlib_src"

# SOURCE_DATE_EPOCH (T6-R7): tar keeps the archive's modification times, so RELEASE's is the release's own.
[ -f "$ffmpeg_src/RELEASE" ] || fail "the FFmpeg tarball has no RELEASE file"
release="$(tr -d '\r\n' < "$ffmpeg_src/RELEASE")"
[ "$release" = "$ffmpeg_version" ] || fail "the tarball's RELEASE says '$release', not the pinned $ffmpeg_version"
SOURCE_DATE_EPOCH="$(stat -c %Y "$ffmpeg_src/RELEASE")" || fail "cannot read RELEASE's modification time"
export SOURCE_DATE_EPOCH
export GIT_CEILING_DIRECTORIES="$root"
unset GIT_DIR GIT_WORK_TREE
# pkg-config sees only an empty folder, so it can find no library; FFmpeg's zlib check then falls back to -lz, which
# the --extra-cflags and --extra-ldflags below point at this build's zlib.
export PKG_CONFIG_LIBDIR="$root/pkgconfig-empty" PKG_CONFIG_PATH=""

# The notes' zlib pin: zlib 1.4's fix is in inflateGetDictionary, which FFmpeg's Matroska demuxer must not call,
# or the pin goes back to the owner. The file is checked first, since grep's status 2 for a missing file would read
# as "not there".
matroskadec="$ffmpeg_src/libavformat/matroskadec.c"
[ -f "$matroskadec" ] || fail "$matroskadec is missing from the FFmpeg tarball"
if grep -n inflateGetDictionary "$matroskadec"; then
  fail "$matroskadec calls inflateGetDictionary, so the zlib $zlib_version pin goes back to the owner"
fi

echo "== building zlib $zlib_version"
[ -f "$zlib_src/win32/Makefile.gcc" ] || fail "the zlib archive has no win32/Makefile.gcc"
make -C "$zlib_src" -f win32/Makefile.gcc PREFIX="$cross" ARFLAGS=rcsD libz.a || fail "the zlib build failed"
for header in zlib.h zconf.h; do
  [ -f "$zlib_src/$header" ] || fail "the zlib archive has no $header"
  cp "$zlib_src/$header" "$zlib_prefix/include/"
done
cp "$zlib_src/libz.a" "$zlib_prefix/lib/"
zlib_header_version="$(sed -n 's/^#define ZLIB_VERSION "\(.*\)"$/\1/p' "$zlib_prefix/include/zlib.h")"
[ "$zlib_header_version" = "$zlib_version" ] \
  || fail "zlib.h says version '$zlib_header_version', not the pinned $zlib_version"

# Design part 11.2's line, then only the two flags that point the build at this zlib (notes call 3). The commas are
# FFmpeg's own list separators inside one argument, not array separators.
# shellcheck disable=SC2054
configure_args=(
  --enable-cross-compile --target-os=mingw32 --arch=x86_64
  --cross-prefix="$cross"
  --extra-ldflags="-static -Wl,--no-insert-timestamp"
  --disable-autodetect --enable-w32threads --enable-zlib
  --disable-everything --disable-doc --disable-network
  --disable-avdevice --disable-swscale
  --disable-programs --enable-ffmpeg --enable-ffprobe
  --enable-demuxer=matroska
  --enable-decoder=truehd,mlp,dca,ac3,eac3,aac,mp2,pcm_bluray,pcm_dvd,pcm_s16le,pcm_s24le,pcm_s32le,flac
  --enable-parser=mlp,dca,ac3,aac,mpegaudio,flac
  --enable-encoder=flac,pcm_s16le
  --enable-muxer=matroska,null,streamhash
  --enable-filter=pan,aresample,highpass,lowpass,asetnsamples,astats,ametadata,aformat,anull,asplit,volume
  --enable-protocol=file
  --extra-cflags="-I$zlib_prefix/include"
  --extra-ldflags="-L$zlib_prefix/lib"
)
configure_line="./configure"
for arg in "${configure_args[@]}"; do
  case "$arg" in
    *" "*) configure_line+=" \\"$'\n'"  \"$arg\"" ;;
    *) configure_line+=" \\"$'\n'"  $arg" ;;
  esac
done

echo "== configuring FFmpeg $ffmpeg_version (SOURCE_DATE_EPOCH=$SOURCE_DATE_EPOCH)"
printf '%s\n' "$configure_line"
cd "$ffmpeg_src"
if ! ./configure "${configure_args[@]}" > "$root/configure-summary.txt" 2>&1; then
  cat "$root/configure-summary.txt" >&2
  tail -n 50 ffbuild/config.log >&2 || true
  fail "FFmpeg's configure failed"
fi
cat "$root/configure-summary.txt"

# The build is LGPL with zlib linked (design part 11.2, P30): nothing GPL, version 3 or nonfree, and zlib really on.
grep -qx 'License: LGPL version 2.1 or later' "$root/configure-summary.txt" \
  || fail "configure did not report the licence LGPL version 2.1 or later"
for define in 'CONFIG_ZLIB 1' 'CONFIG_GPL 0' 'CONFIG_VERSION3 0' 'CONFIG_NONFREE 0' 'HAVE_W32THREADS 1'; do
  grep -qx "#define $define" config.h || fail "config.h lacks '#define $define'"
done

echo "== building FFmpeg $ffmpeg_version"
make -j"$(nproc)" || fail "the FFmpeg build failed"
grep -q "FFMPEG_VERSION \"$ffmpeg_version\"" libavutil/ffversion.h \
  || fail "libavutil/ffversion.h does not give the version $ffmpeg_version: $(cat libavutil/ffversion.h)"
for program in ffmpeg.exe ffprobe.exe; do
  [ -f "$program" ] || fail "the build made no $program"
  [ "$(head -c 2 "$program")" = MZ ] || fail "$program is not a Windows executable"
done

echo "== writing $out"
cp ffmpeg.exe ffprobe.exe "$out/"
for licence in COPYING.LGPLv2.1 LICENSE.md; do
  [ -f "$licence" ] || fail "the FFmpeg tarball has no $licence"
  cp "$licence" "$out/"
done
[ -f "$zlib_src/LICENSE" ] || fail "the zlib archive has no LICENSE"
cp "$zlib_src/LICENSE" "$out/LICENSE"
mingw_copyright=/usr/share/doc/mingw-w64-common/copyright
[ -f "$mingw_copyright" ] || fail "$mingw_copyright is missing"
cp "$mingw_copyright" "$out/mingw-w64-runtime-copyright.txt"
MANIFEST="$manifest" bash "$here/inputs-digest.sh" > "$out/inputs-sha256.txt" || fail "inputs-digest.sh failed"

# The settings file holds nothing that varies between runs: no date, no runner path.
{
  echo "lazymkv's minimal FFmpeg build (design parts 11.2 and 11.3)"
  echo "FFmpeg $ffmpeg_version from $ffmpeg_url, SHA-256 $ffmpeg_sha256"
  echo "zlib $zlib_version from $zlib_url, SHA-256 $zlib_sha256"
  echo "Runner: $runner. SOURCE_DATE_EPOCH: $SOURCE_DATE_EPOCH. Inputs' SHA-256: $(cat "$out/inputs-sha256.txt")"
  echo
  echo "== The build script, packaging/ffmpeg/build.sh"
  cat "$script"
  echo
  echo "== The configure line as run, in $ffmpeg_src"
  printf '%s\n' "$configure_line"
  echo
  echo "== Versions"
  echo "GCC: $("${cross}gcc" --version | sed -n 1p)"
  echo "binutils: $("${cross}ld" --version | sed -n 1p)"
  echo "nasm: $(nasm -v)"
  echo "zlib: $zlib_header_version"
  for spec in "${apt_pins[@]}"; do
    echo "apt: $spec"
  done
  echo
  echo "== FFmpeg's configure summary"
  cat "$root/configure-summary.txt"
} > "$out/ffmpeg-build-settings.txt"

echo "== done; $out holds:"
ls -l "$out"
sha256sum "$out/ffmpeg.exe" "$out/ffprobe.exe"
