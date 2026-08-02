#!/usr/bin/env bash
# Apple Silicon macOS向けにsbxmをbuildし、GitHub Releaseを作成する。
#
# 使い方:
#   scripts/release.sh <tag>
#
# 例:
#   scripts/release.sh v0.0.1
#
# 実行にはmacOS (Apple Silicon)、Xcode Command Line Tools、gh CLI (認証済み) を要する。

set -euo pipefail

BIN_NAME="sbxm"
TARGET_TRIPLE="aarch64-apple-darwin"
ARCHIVE_NAME="${BIN_NAME}-${TARGET_TRIPLE}.tar.gz"
# repository直下ではなくdist/へ置く。working treeをcleanなまま保ち、次回実行の
# check_clean_worktreeが前回の生成物を誤検知しないようにする (dist/はgitignore対象)。
DIST_DIR="dist"

# build結果を左右しうるenv var。再現性のないbinaryを出荷しないよう、releaseは常に
# 素のtoolchain設定で行う。
BUILD_AFFECTING_ENV_VARS=(
  RUSTFLAGS
  CARGO_ENCODED_RUSTFLAGS
  CARGO_BUILD_RUSTFLAGS
  CARGO_BUILD_TARGET
  CARGO_TARGET_DIR
  RUSTC
  RUSTC_WRAPPER
  RUSTC_BOOTSTRAP
  CC
  CFLAGS
  LDFLAGS
  SDKROOT
  MACOSX_DEPLOYMENT_TARGET
  SOURCE_DATE_EPOCH
)

WORK_DIR=""

log() {
  # stderrへ書く。package_archiveとrecord_provenanceは`$(...)`で戻り値を
  # 受け取るため、進捗表示をstdoutへ書くと戻り値へ混ざる。
  printf '==> %s\n' "$1" >&2
}

fail() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

usage() {
  printf 'usage: %s <tag>\n' "$(basename "$0")" >&2
}

check_clean_worktree() {
  log "checking that the working tree is clean"
  [ -z "$(git status --porcelain)" ] \
    || fail "working tree is not clean; commit or stash before running this"
}

check_tag_exists() {
  local tag="$1"
  log "checking that tag ${tag} exists"
  git rev-parse -q --verify "refs/tags/${tag}" >/dev/null \
    || fail "tag ${tag} does not exist"
}

check_tag_matches_head() {
  local tag="$1"
  log "checking that tag ${tag} matches HEAD"
  local tag_commit head_commit
  tag_commit="$(git rev-list -n 1 "${tag}")"
  head_commit="$(git rev-parse HEAD)"
  [ "$tag_commit" = "$head_commit" ] \
    || fail "tag ${tag} (${tag_commit}) does not match HEAD (${head_commit})"
}

check_no_existing_release() {
  local tag="$1"
  log "checking that GitHub Release ${tag} does not already exist"
  gh release view "${tag}" >/dev/null 2>&1 \
    && fail "GitHub Release ${tag} already exists"
  return 0
}

check_cargo_version_matches() {
  local expected="$1"
  log "checking that Cargo.toml's version matches the tag"
  local actual
  actual="$(awk -F'"' '
    /^\[/ { in_package = ($0 == "[package]") }
    in_package && /^version[[:space:]]*=/ { print $2; exit }
  ' Cargo.toml)"
  [ -n "$actual" ] || fail "could not read [package] version from Cargo.toml"
  [ "$actual" = "$expected" ] \
    || fail "Cargo.toml's version (${actual}) does not match the tag's version (${expected})"
}

check_no_build_affecting_env_vars() {
  log "checking that no build-affecting env vars are set"
  local var set_vars=()
  for var in "${BUILD_AFFECTING_ENV_VARS[@]}"; do
    [ -n "${!var:-}" ] && set_vars+=("$var")
  done
  [ "${#set_vars[@]}" -eq 0 ] \
    || fail "build-affecting env vars are set: ${set_vars[*]}"
}

check_host_is_arm64() {
  log "checking the host architecture"
  local machine
  machine="$(uname -m)"
  [ "$machine" = "arm64" ] || fail "host is not arm64 (uname -m: ${machine})"
}

check_rustc_host_is_apple_silicon() {
  log "checking rustc's host triple"
  local host
  host="$(rustc -vV | sed -n 's/^host: //p')"
  [ "$host" = "$TARGET_TRIPLE" ] \
    || fail "rustc's host is not ${TARGET_TRIPLE} (host: ${host})"
}

build_release_binary() {
  log "running cargo build --release --locked"
  cargo build --release --locked
}

sign_binary() {
  local bin_path="$1"
  log "applying an ad-hoc signature"
  codesign --force --sign - "$bin_path"
}

verify_signature() {
  local bin_path="$1"
  log "verifying the signature with codesign --verify"
  codesign --verify --strict --verbose=2 "$bin_path"
}

verify_adhoc_signature() {
  local bin_path="$1"
  log "confirming an ad-hoc signature with codesign -dv"
  local info
  info="$(codesign -dv "$bin_path" 2>&1)"
  printf '%s\n' "$info" >&2
  printf '%s\n' "$info" | grep -q '^Signature=adhoc$' \
    || fail "signature is not ad-hoc"
}

verify_binary_arch() {
  local bin_path="$1"
  log "confirming an arm64 Mach-O binary with file"
  local info
  info="$(file "$bin_path")"
  printf '%s\n' "$info" >&2
  printf '%s\n' "$info" | grep -q 'Mach-O.*arm64' \
    || fail "not an arm64 Mach-O binary: ${info}"
}

verify_binary_version() {
  local bin_path="$1" version="$2"
  log "checking the sbxm --version output"
  local actual expected
  actual="$("$bin_path" --version)"
  expected="${BIN_NAME} ${version}"
  [ "$actual" = "$expected" ] \
    || fail "sbxm --version output (${actual}) does not match the release version (${expected})"
}

package_archive() {
  local bin_path="$1"
  local archive_path="${DIST_DIR}/${ARCHIVE_NAME}"
  log "creating the release asset: ${archive_path}"
  local stage_dir="${WORK_DIR}/stage"
  mkdir -p "$stage_dir" "$DIST_DIR"
  cp "$bin_path" "${stage_dir}/${BIN_NAME}"
  chmod 755 "${stage_dir}/${BIN_NAME}"
  # COPYFILE_DISABLE=1: macOSがAppleDouble file (._*) やresource forkをarchiveへ
  # 混ぜないようにする。
  COPYFILE_DISABLE=1 tar -czf "$archive_path" -C "$stage_dir" "$BIN_NAME"
  printf '%s' "$archive_path"
}

verify_archive_contents() {
  local archive="$1"
  log "checking that the archive contains only ${BIN_NAME} at its root"
  local listing
  listing="$(tar -tzf "$archive")"
  [ "$listing" = "$BIN_NAME" ] \
    || fail "archive contains more than just ${BIN_NAME}: $(printf '%s' "$listing" | tr '\n' ' ')"
}

record_provenance() {
  local archive="$1"
  local archive_dir archive_base
  archive_dir="$(dirname "$archive")"
  archive_base="$(basename "$archive")"
  local notes_file="${WORK_DIR}/release-notes.md"

  local commit_sha rustc_version cargo_version_str sw_vers_output checksum
  commit_sha="$(git rev-parse HEAD)"
  rustc_version="$(rustc -vV)"
  cargo_version_str="$(cargo -V)"
  sw_vers_output="$(sw_vers)"
  # dist/を含めず、archive名だけをdigest行へ記録する。shasum -cでの検証時に
  # 同じ階層へ置くだけで済むようにする。
  checksum="$(cd "$archive_dir" && shasum -a 256 "$archive_base")"

  log "recording the Git commit SHA: ${commit_sha}"
  log "recording rustc -vV"
  printf '%s\n' "$rustc_version" >&2
  log "recording cargo -V"
  printf '%s\n' "$cargo_version_str" >&2
  log "recording sw_vers"
  printf '%s\n' "$sw_vers_output" >&2
  log "outputting shasum -a 256"
  printf '%s\n' "$checksum" >&2

  {
    printf '## %s\n\n' "$archive_base"
    printf '```\n%s\n```\n\n' "$checksum"
    printf '## Build provenance\n\n'
    printf -- '- Git commit: `%s`\n\n' "$commit_sha"
    printf '### rustc -vV\n\n```\n%s\n```\n\n' "$rustc_version"
    printf '### cargo -V\n\n```\n%s\n```\n\n' "$cargo_version_str"
    printf '### sw_vers\n\n```\n%s\n```\n' "$sw_vers_output"
  } >"$notes_file"

  printf '%s' "$notes_file"
}

create_github_release() {
  local tag="$1" archive="$2" notes_file="$3"
  log "creating GitHub Release ${tag}"
  # --verify-tag: remoteに同名tagが無ければ失敗させる。tagのpushはこのscriptの
  # 責務外とし、ここではpush済みのtagだけを対象にする。
  # --prerelease、--clobberはどちらも付けない。正式版のみを対象にし、既存asset
  # を誤って上書きしない。
  gh release create "$tag" "$archive" \
    --title "$tag" \
    --notes-file "$notes_file" \
    --verify-tag
}

main() {
  if [ "$#" -ne 1 ]; then
    usage
    fail "expected exactly one tag argument (e.g. v0.0.1)"
  fi
  local tag="$1"

  cd "$(git rev-parse --show-toplevel)"

  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sbxm-release.XXXXXX")"
  trap 'rm -rf "$WORK_DIR"' EXIT

  check_clean_worktree
  check_tag_exists "$tag"
  check_tag_matches_head "$tag"
  check_no_existing_release "$tag"

  local version="${tag#v}"
  [ "$version" != "$tag" ] || fail "tag must start with v (e.g. v0.0.1): ${tag}"

  check_cargo_version_matches "$version"
  check_no_build_affecting_env_vars
  check_host_is_arm64
  check_rustc_host_is_apple_silicon

  build_release_binary

  local bin_path="target/release/${BIN_NAME}"
  sign_binary "$bin_path"
  verify_signature "$bin_path"
  verify_adhoc_signature "$bin_path"
  verify_binary_arch "$bin_path"
  verify_binary_version "$bin_path" "$version"

  local archive_path
  archive_path="$(package_archive "$bin_path")"
  verify_archive_contents "$archive_path"

  local notes_file
  notes_file="$(record_provenance "$archive_path")"

  create_github_release "$tag" "$archive_path" "$notes_file"

  log "created release ${tag}"
}

main "$@"
