#!/usr/bin/env bash
# Apple Silicon macOS向けにsbxmをbuildし、tagを打ってGitHub Releaseを作成する。
#
# 使い方:
#   scripts/release/release.sh [--dry-run] <tag>
#
# 例:
#   scripts/release/release.sh --dry-run v0.0.1
#   scripts/release/release.sh v0.0.1
#
# 手順と背景はscripts/release/README.mdに置く。
#
# tagは事前に用意しない。このscriptがHEADへ打ち、originへpushする。ただしそれを行う
# のは、検査・build・署名・package・provenanceの記録がすべて通った後とする。buildが
# 落ちたときにoriginへtagだけが残る状態を作らない。
#
# --dry-runは、書き込みだけを行わない。tagを打たず、pushせず、Releaseも作らない。
# publishの前提条件 (clean tree、tagの衝突、gh認証、既存Release) は即座に落とさず、
# 警告として記録して最後にまとめて報告する。build結果の正しさに関わる検査は、dry run
# でも本番と同じく即座に落とす。
#
# 実行にはmacOS (Apple Silicon)、Xcode Command Line Tools、gh CLI (認証済み) を要する。

set -euo pipefail

BIN_NAME="sbxm"
TARGET_TRIPLE="aarch64-apple-darwin"
ARCHIVE_NAME="${BIN_NAME}-${TARGET_TRIPLE}.tar.gz"
# repository直下ではなくdist/へ置く。working treeをcleanなまま保ち、次回実行の
# check_clean_worktreeが前回の生成物を誤検知しないようにする (dist/はgitignore対象)。
DIST_DIR="dist"
NOTES_NAME="release-notes.md"
REMOTE="origin"

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
DRY_RUN=0
BLOCKERS=""
BLOCKER_COUNT=0
CREATED_LOCAL_TAG=0

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
  printf 'usage: %s [--dry-run] <tag>\n' "$(basename "$0")" >&2
}

# publishの前提条件が満たされていないときに呼ぶ。本番では即座に落とす。dry runでは
# 記録して続け、残りの工程も最後まで見せる。
record_blocker() {
  local message="$1"
  if [ "$DRY_RUN" -eq 0 ]; then
    fail "$message"
  fi
  printf 'warning: %s\n' "$message" >&2
  BLOCKERS="${BLOCKERS}  - ${message}"$'\n'
  BLOCKER_COUNT=$((BLOCKER_COUNT + 1))
  return 0
}

check_clean_worktree() {
  log "checking that the working tree is clean"
  if [ -n "$(git status --porcelain)" ]; then
    record_blocker "working tree is not clean; commit or stash before releasing"
  fi
  return 0
}

# tagが指すcommitをremoteから読む。annotated tagは`refs/tags/x`がtag objectを、
# `refs/tags/x^{}`がcommitを指すため、peeled行があればそちらを採る。
remote_tag_commit() {
  local tag="$1" output
  output="$(git ls-remote --tags "$REMOTE" "refs/tags/${tag}" "refs/tags/${tag}^{}")"
  [ -n "$output" ] || return 1
  printf '%s\n' "$output" | awk '
    $2 ~ /\^\{\}$/ { peeled = $1; next }
    { plain = $1 }
    END { print (peeled != "" ? peeled : plain) }
  '
}

# tagをHEADへ打てる状態か確かめる。既にHEADを指しているなら、それを使い回す。
check_tag_available() {
  local tag="$1"
  local head_commit
  head_commit="$(git rev-parse HEAD)"

  log "checking that tag ${tag} can be placed at HEAD"
  if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    local local_commit
    local_commit="$(git rev-list -n 1 "${tag}")"
    if [ "$local_commit" != "$head_commit" ]; then
      record_blocker "tag ${tag} already exists locally at ${local_commit}, not HEAD (${head_commit}); delete it with: git tag -d ${tag}"
    else
      log "tag ${tag} already exists at HEAD; it will be reused"
    fi
  fi

  log "checking tag ${tag} on ${REMOTE}"
  local remote_commit
  if remote_commit="$(remote_tag_commit "$tag")"; then
    if [ "$remote_commit" != "$head_commit" ]; then
      record_blocker "tag ${tag} on ${REMOTE} points at ${remote_commit}, not HEAD (${head_commit}); releasing would need a different tag"
    else
      log "tag ${tag} is already on ${REMOTE} at HEAD; it will be reused"
    fi
  fi
  return 0
}

check_release_absent() {
  local tag="$1"

  log "checking that gh is authenticated"
  # 認証切れのままだとgh release viewが失敗し、それが「Releaseは無い」と区別
  # できない。既存Releaseの検査はここで打ち切る。
  if ! gh auth status >/dev/null 2>&1; then
    record_blocker "gh is not authenticated; run: gh auth login"
    return 0
  fi

  log "checking that GitHub Release ${tag} does not already exist"
  if gh release view "${tag}" >/dev/null 2>&1; then
    record_blocker "GitHub Release ${tag} already exists"
  fi
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
  # macOSが同梱するbash 3.2は、set -uのもとで空arrayの展開を未定義変数として
  # 扱う。arrayを溜めず、空文字列を初期値にできる文字列で数える。
  local var set_vars=""
  for var in "${BUILD_AFFECTING_ENV_VARS[@]}"; do
    [ -n "${!var:-}" ] && set_vars="${set_vars}${set_vars:+ }${var}"
  done
  [ -z "$set_vars" ] \
    || fail "build-affecting env vars are set: ${set_vars}"
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
  # dry runで中身を確かめられるよう、WORK_DIRではなくdist/へ残す。
  local notes_file="${DIST_DIR}/${NOTES_NAME}"

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

create_tag() {
  local tag="$1"
  if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    log "tag ${tag} already exists at HEAD"
    return 0
  fi
  log "tagging HEAD as ${tag}"
  # annotated tagにする。releaseを指すtagには打ち手と日時を残す。
  git tag -a "$tag" -m "$tag"
  CREATED_LOCAL_TAG=1
}

push_tag() {
  local tag="$1"
  log "pushing tag ${tag} to ${REMOTE}"
  if git push "$REMOTE" "refs/tags/${tag}"; then
    return 0
  fi
  # pushが通らなければ、このscriptが作った分だけを畳んで元へ戻す。
  if [ "$CREATED_LOCAL_TAG" -eq 1 ]; then
    git tag -d "$tag" >/dev/null
    log "removed the local tag ${tag} again"
  fi
  fail "could not push tag ${tag} to ${REMOTE}"
}

create_github_release() {
  local tag="$1" archive="$2" notes_file="$3"
  log "creating GitHub Release ${tag}"
  # --verify-tag: remoteに同名tagが無ければ失敗させる。直前にpushしているので
  # 通るはずだが、取り違えの最後の歯止めとして残す。
  # --prerelease、--clobberはどちらも付けない。正式版のみを対象にし、既存asset
  # を誤って上書きしない。
  if gh release create "$tag" "$archive" \
    --title "$tag" \
    --notes-file "$notes_file" \
    --verify-tag; then
    return 0
  fi
  # tagはpush済みで、Releaseだけが無い状態になる。remoteのtagを黙って消しに
  # いかず、戻し方を示して止まる。
  {
    printf 'error: creating GitHub Release %s failed; the tag is already on %s\n' "$tag" "$REMOTE"
    printf 'undo the tag with:\n'
    printf '  git push %s :refs/tags/%s\n' "$REMOTE" "$tag"
    printf '  git tag -d %s\n' "$tag"
  } >&2
  exit 1
}

publish() {
  local tag="$1" archive="$2" notes_file="$3"

  if [ "$DRY_RUN" -eq 1 ]; then
    log "dry run: not tagging, not pushing, not creating a Release"
    {
      printf 'would run:\n'
      printf '  git tag -a %q -m %q\n' "$tag" "$tag"
      printf '  git push %q refs/tags/%q\n' "$REMOTE" "$tag"
      printf '  gh release create %q %q \\\n' "$tag" "$archive"
      printf '    --title %q \\\n' "$tag"
      printf '    --notes-file %q \\\n' "$notes_file"
      printf '    --verify-tag\n'
    } >&2
    return 0
  fi

  create_tag "$tag"
  push_tag "$tag"
  create_github_release "$tag" "$archive" "$notes_file"
}

report_dry_run() {
  local archive="$1" notes_file="$2"
  log "dry run finished; nothing was written"
  {
    printf 'inspect the artifacts it would have uploaded:\n'
    printf '  asset:         %s\n' "$archive"
    printf '  release notes: %s\n' "$notes_file"
  } >&2

  if [ "$BLOCKER_COUNT" -eq 0 ]; then
    log "no blockers; the real release would proceed"
    return 0
  fi

  # buildは通っているので、落とすのはpublishの前提条件だけであることを明示する。
  local noun="things"
  [ "$BLOCKER_COUNT" -eq 1 ] && noun="thing"
  {
    printf '\n%d %s would block the real release:\n' "$BLOCKER_COUNT" "$noun"
    printf '%s' "$BLOCKERS"
    printf 'the build itself succeeded; only these preconditions are unmet.\n'
  } >&2
  exit 1
}

main() {
  # optionはtagの前後どちらでも受ける。`release.sh v0.0.1 --dry-run`と書いた人が
  # 本番releaseを作ってしまう余地を残さない。
  # bash 3.2ではset -uのもとで空arrayを展開できないため、arrayを使わず数える。
  local tag="" tag_count=0
  local end_of_options=0
  local arg
  for arg in "$@"; do
    if [ "$end_of_options" -eq 0 ]; then
      case "$arg" in
        --dry-run)
          DRY_RUN=1
          continue
          ;;
        -h | --help)
          usage
          exit 0
          ;;
        --)
          end_of_options=1
          continue
          ;;
        -*)
          usage
          fail "unknown option: ${arg}"
          ;;
      esac
    fi
    tag="$arg"
    tag_count=$((tag_count + 1))
  done

  if [ "$tag_count" -ne 1 ]; then
    usage
    fail "expected exactly one tag argument (e.g. v0.0.1)"
  fi

  cd "$(git rev-parse --show-toplevel)"

  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sbxm-release.XXXXXX")"
  trap 'rm -rf "$WORK_DIR"' EXIT

  [ "$DRY_RUN" -eq 1 ] && log "dry run: nothing will be tagged, pushed, or published"

  local version="${tag#v}"
  [ "$version" != "$tag" ] || fail "tag must start with v (e.g. v0.0.1): ${tag}"

  check_clean_worktree
  check_tag_available "$tag"
  check_release_absent "$tag"
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

  publish "$tag" "$archive_path" "$notes_file"

  if [ "$DRY_RUN" -eq 1 ]; then
    report_dry_run "$archive_path" "$notes_file"
  else
    log "created release ${tag}"
  fi
}

main "$@"
