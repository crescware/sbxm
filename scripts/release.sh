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
  log "working treeがcleanか確認する"
  [ -z "$(git status --porcelain)" ] \
    || fail "working treeがcleanではない。commitまたはstashしてから実行すること"
}

check_tag_exists() {
  local tag="$1"
  log "tag ${tag} の存在を確認する"
  git rev-parse -q --verify "refs/tags/${tag}" >/dev/null \
    || fail "tag ${tag} が存在しない"
}

check_tag_matches_head() {
  local tag="$1"
  log "tag ${tag} がHEADと一致するか確認する"
  local tag_commit head_commit
  tag_commit="$(git rev-list -n 1 "${tag}")"
  head_commit="$(git rev-parse HEAD)"
  [ "$tag_commit" = "$head_commit" ] \
    || fail "tag ${tag} (${tag_commit}) がHEAD (${head_commit}) と一致しない"
}

check_no_existing_release() {
  local tag="$1"
  log "GitHub Release ${tag} が存在しないか確認する"
  gh release view "${tag}" >/dev/null 2>&1 \
    && fail "GitHub Release ${tag} は既に存在する"
  return 0
}

check_cargo_version_matches() {
  local expected="$1"
  log "Cargo.tomlのversionとtagが一致するか確認する"
  local actual
  actual="$(awk -F'"' '
    /^\[/ { in_package = ($0 == "[package]") }
    in_package && /^version[[:space:]]*=/ { print $2; exit }
  ' Cargo.toml)"
  [ -n "$actual" ] || fail "Cargo.tomlから[package] versionを読めなかった"
  [ "$actual" = "$expected" ] \
    || fail "Cargo.tomlのversion (${actual}) がtagのversion (${expected}) と一致しない"
}

check_no_build_affecting_env_vars() {
  log "build結果へ影響するenv varが設定されていないか確認する"
  local var set_vars=()
  for var in "${BUILD_AFFECTING_ENV_VARS[@]}"; do
    [ -n "${!var:-}" ] && set_vars+=("$var")
  done
  [ "${#set_vars[@]}" -eq 0 ] \
    || fail "build結果へ影響するenv varが設定されている: ${set_vars[*]}"
}

check_host_is_arm64() {
  log "hostのarchitectureを確認する"
  local machine
  machine="$(uname -m)"
  [ "$machine" = "arm64" ] || fail "hostがarm64ではない (uname -m: ${machine})"
}

check_rustc_host_is_apple_silicon() {
  log "rustcのhost tripleを確認する"
  local host
  host="$(rustc -vV | sed -n 's/^host: //p')"
  [ "$host" = "$TARGET_TRIPLE" ] \
    || fail "rustcのhostが${TARGET_TRIPLE}ではない (host: ${host})"
}

build_release_binary() {
  log "cargo build --release --locked を実行する"
  cargo build --release --locked
}

sign_binary() {
  local bin_path="$1"
  log "ad-hoc署名を付ける"
  codesign --force --sign - "$bin_path"
}

verify_signature() {
  local bin_path="$1"
  log "署名をcodesign --verifyで検証する"
  codesign --verify --strict --verbose=2 "$bin_path"
}

verify_adhoc_signature() {
  local bin_path="$1"
  log "codesign -dvでad-hoc署名であることを確認する"
  local info
  info="$(codesign -dv "$bin_path" 2>&1)"
  printf '%s\n' "$info" >&2
  printf '%s\n' "$info" | grep -q '^Signature=adhoc$' \
    || fail "ad-hoc署名ではない"
}

verify_binary_arch() {
  local bin_path="$1"
  log "fileでarm64のMach-Oか確認する"
  local info
  info="$(file "$bin_path")"
  printf '%s\n' "$info" >&2
  printf '%s\n' "$info" | grep -q 'Mach-O.*arm64' \
    || fail "arm64のMach-Oではない: ${info}"
}

verify_binary_version() {
  local bin_path="$1" version="$2"
  log "sbxm --versionの出力を確認する"
  local actual expected
  actual="$("$bin_path" --version)"
  expected="${BIN_NAME} ${version}"
  [ "$actual" = "$expected" ] \
    || fail "sbxm --versionの出力 (${actual}) がrelease version (${expected}) と一致しない"
}

package_archive() {
  local bin_path="$1"
  local archive_path="${DIST_DIR}/${ARCHIVE_NAME}"
  log "release assetを作成する: ${archive_path}"
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
  log "archive直下に${BIN_NAME}だけが含まれているか確認する"
  local listing
  listing="$(tar -tzf "$archive")"
  [ "$listing" = "$BIN_NAME" ] \
    || fail "archiveの内容が${BIN_NAME}だけではない: $(printf '%s' "$listing" | tr '\n' ' ')"
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

  log "Git commit SHAを記録する: ${commit_sha}"
  log "rustc -vV を記録する"
  printf '%s\n' "$rustc_version" >&2
  log "cargo -V を記録する"
  printf '%s\n' "$cargo_version_str" >&2
  log "sw_vers を記録する"
  printf '%s\n' "$sw_vers_output" >&2
  log "shasum -a 256 を出力する"
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
  log "GitHub Release ${tag} を作成する"
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
    fail "tagを1つ指定すること (例: v0.0.1)"
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
  [ "$version" != "$tag" ] || fail "tagはvから始まること (例: v0.0.1): ${tag}"

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

  log "release ${tag} を作成した"
}

main "$@"
