#!/bin/sh
# 対象Mac上でDocker Sandboxes CLIのread-only出力を採取する。
#
# 採取したfixtureは、それを使用するcommandのPRへparser testとともに追加する。
# 実行結果はexact versionのdirectoryへ保存され、home directoryのpathだけを
# `/Users/example`へ置換する。ほかのredactionは目視で行う。

set -eu

fixture_root=$(cd "$(dirname "$0")" && pwd)

if ! command -v sbx >/dev/null 2>&1; then
  echo "sbx was not found on PATH" >&2
  exit 1
fi

raw_version=$(sbx version 2>&1 || true)
version=$(printf '%s' "$raw_version" | sed -n 's/.*\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\).*/\1/p' | head -n 1)
if [ -z "$version" ]; then
  echo "an exact version could not be determined from: $raw_version" >&2
  exit 1
fi

target="$fixture_root/$version"
mkdir -p "$target"

redact() {
  sed "s#$HOME#/Users/example#g"
}

capture() {
  name=$1
  shift
  echo "collecting $name: $*" >&2
  if "$@" >"$target/$name.partial" 2>"$target/$name.stderr.partial"; then
    status=0
  else
    status=$?
  fi
  redact <"$target/$name.partial" >"$target/$name"
  rm -f "$target/$name.partial"
  if [ -s "$target/$name.stderr.partial" ]; then
    redact <"$target/$name.stderr.partial" >"$target/$name.stderr"
  fi
  rm -f "$target/$name.stderr.partial"
  echo "$status" >"$target/$name.exit-status"
}

capture version.txt sbx version
capture help.txt sbx --help
capture ls.json sbx ls --json
capture daemon-status.json sbx daemon status
capture policy-ls.json sbx policy ls

cat <<EOF >&2

Collected into $target

Remaining manual steps:
  1. Rename ls.json to ls-empty.json / ls-running.json / ls-stopped.json after
     collecting each sandbox state, and do the same for daemon-status.json.
  2. Collect policy-ls.json once with Balanced selected and once with another
     policy selected.
  3. Review every file for user names, tokens, public keys and real repository
     names before committing.
  4. Add $version to validated_cli_versions in compatibility.toml and tighten
     the parsers in src/compatibility.rs to the collected schema.
EOF
