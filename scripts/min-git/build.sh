#!/usr/bin/env bash
# hostのgitに求める最小versionのgitをsourceからbuildし、その`bin`のpathを出す。
#
# gitは公式のbinaryを配らず、OSやpackage managerが入れるのは新しい版だけである。
# 開発を始めるsandboxから届くのはGitHubだけのため、gitとzlibのsourceをGitHubから取り、
# Linuxではmiseが入れるzigで、macOSではCommand Line Toolsのccでbuildする。
#
# versionとchecksumを固定する。versionは隣の`version`に置き、sbxmがhostに求めるversionと
# 同じであることをtestが確かめる。build済みなら何もせずpathだけを出す。途中で止まった
# buildは、完了の印が無いため、次の実行で作り直す。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
GIT_VERSION="$(cat "$here/version")"
GIT_SHA256=59dbc158dce293798570166fe7acfe225514f2868bc2d6e25c1a5a00c4ac0888
ZLIB_VERSION=1.3.1
ZLIB_SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23

root="$(cd "$here/../.." && pwd)"
prefix="$root/target/min-git/$GIT_VERSION"
complete="$prefix/.complete"

if [[ -f $complete ]]; then
  echo "$prefix/bin"
  exit 0
fi

# 途中までのものを残さない。buildの出力は、pathを受け取る呼び出し側の標準出力へ混ぜない。
rm -rf "$prefix"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if command -v sha256sum >/dev/null; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

fetch() {
  local url=$1 expected=$2 file=$3
  curl --fail --silent --show-error --location --retry 3 --output "$file" "$url"
  local actual
  actual="$(sha256 "$file")"
  if [[ $actual != "$expected" ]]; then
    echo "checksum mismatch for $url: expected $expected, got $actual" >&2
    exit 1
  fi
  tar -xzf "$file" -C "$work"
}

fetch "https://github.com/madler/zlib/releases/download/v$ZLIB_VERSION/zlib-$ZLIB_VERSION.tar.gz" \
  "$ZLIB_SHA256" "$work/zlib.tar.gz"
fetch "https://github.com/git/git/archive/refs/tags/v$GIT_VERSION.tar.gz" \
  "$GIT_SHA256" "$work/git.tar.gz"

if [[ $(uname -s) == Darwin ]]; then
  cc=cc ar=ar ranlib=ranlib
  jobs="$(sysctl -n hw.ncpu)"
else
  cc="zig cc" ar="zig ar" ranlib="zig ranlib"
  jobs="$(nproc)"
fi

zlib="$work/zlib-$ZLIB_VERSION"
{
  (cd "$zlib" && CC="$cc" ./configure --static && make -j"$jobs" libz.a AR="$ar" ARFLAGS=rc RANLIB="$ranlib")
  # testが使うのはgit本体だけである。HTTP、翻訳、Perl、Python、Tcl/Tkに頼る部分は作らない。
  make -C "$work/git-$GIT_VERSION" -j"$jobs" prefix="$prefix" \
    CC="$cc" AR="$ar" CFLAGS="-O2 -I$zlib" LDFLAGS="-L$zlib" \
    NO_CURL=1 NO_OPENSSL=1 NO_EXPAT=1 NO_GETTEXT=1 NO_TCLTK=1 NO_PERL=1 NO_PYTHON=1 \
    install
} >"$work/build.log" 2>&1 || {
  tail -n 40 "$work/build.log" >&2
  exit 1
}

actual="$("$prefix/bin/git" --version)"
if [[ $actual != "git version $GIT_VERSION" ]]; then
  echo "built $actual, expected git version $GIT_VERSION" >&2
  exit 1
fi
touch "$complete"
echo "$prefix/bin"
