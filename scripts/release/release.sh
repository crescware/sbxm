#!/usr/bin/env bash
# Apple Silicon macOS向けにsbxmをbuildし、tagを打ってGitHub Releaseを作成する。
#
# 使い方:
#   scripts/release/release.sh [--dry-run] (--prerelease | --stable) <tag>
#
# 例:
#   scripts/release/release.sh --dry-run --prerelease v0.0.1
#   scripts/release/release.sh --prerelease v0.0.1
#
# prereleaseかどうかはversionから推測せず、必ず指定させる。指定が無ければ何も書かずに
# 拒否する。
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
# buildの前に`mise run check`を通す。通らないtreeからはreleaseを作らない。
#
# 実行にはmacOS (Apple Silicon)、Xcode Command Line Tools、mise、gh CLI (認証済み) を
# 要する。

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
# auto: versionから決める。yes/no: --prerelease/--stableで明示された。
PRERELEASE_MODE="auto"
PRERELEASE=0

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
  {
    printf 'usage: %s [--dry-run] (--prerelease | --stable) <tag>\n' "$(basename "$0")"
    printf '\n'
    printf '  --dry-run     run everything but the writes (no tag, no push, no Release)\n'
    printf '  --prerelease  mark the Release as a prerelease\n'
    printf '  --stable      mark the Release as a full release\n'
    printf '\n'
    printf 'One of --prerelease or --stable is required. The version is not read\n'
    printf 'as an answer: 0.0.1 hints that a release is unfinished, but a hint is\n'
    printf 'not a statement, and this decides how the release is presented.\n'
  } >&2
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

# releaseは既定branchに入っているcommitからだけ切る。featureブランチのcommitへtagを
# 打つと、そのbranchをsquash mergeやrebaseした後、tagはどのbranchからも辿れないcommit
# を指したまま残る。「このversionのsourceはどれか」に答えられなくなる。
check_head_is_on_default_branch() {
  log "checking that HEAD is on ${REMOTE}'s default branch"

  local symref
  if ! symref="$(git ls-remote --symref "$REMOTE" HEAD 2>/dev/null)"; then
    record_blocker "cannot reach ${REMOTE}; whether HEAD is on the default branch could not be observed"
    return 0
  fi

  local default_branch
  default_branch="$(printf '%s\n' "$symref" | awk '
    $1 == "ref:" { sub("refs/heads/", "", $2); print $2; exit }
  ')"
  [ -n "$default_branch" ] || default_branch="the default branch"

  # 既定branchの先端をlocalへ取り寄せる。FETCH_HEADへ書くだけで、remoteへは書かない。
  if ! git fetch --quiet "$REMOTE" HEAD 2>/dev/null; then
    record_blocker "could not fetch ${REMOTE}'s default branch; whether HEAD is on it could not be observed"
    return 0
  fi

  if ! git merge-base --is-ancestor HEAD FETCH_HEAD; then
    record_blocker "HEAD is not on ${default_branch}; merge it there first, or the tag will point at a commit no branch reaches"
  fi
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
# 「tagが無い」と「remoteへ到達できない」を区別する。同じ扱いにすると、networkが
# 切れているだけの状態を「衝突なし」と読み、pushできないtreeを通してしまう。
#   0 — tagがある。commitをstdoutへ出す
#   1 — remoteは見えたが、tagは無い
#   2 — remoteへ到達できない
remote_tag_commit() {
  local tag="$1" output
  if ! output="$(git ls-remote --tags "$REMOTE" "refs/tags/${tag}" "refs/tags/${tag}^{}" 2>/dev/null)"; then
    return 2
  fi
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
  local remote_commit=""
  local status=0
  remote_commit="$(remote_tag_commit "$tag")" || status=$?
  case "$status" in
    0)
      if [ "$remote_commit" != "$head_commit" ]; then
        record_blocker "tag ${tag} on ${REMOTE} points at ${remote_commit}, not HEAD (${head_commit}); releasing would need a different tag"
      else
        log "tag ${tag} is already on ${REMOTE} at HEAD; it will be reused"
      fi
      ;;
    1)
      # remoteにtagが無い。これから打つのだから、これが通常の状態とする。
      ;;
    *)
      record_blocker "cannot reach ${REMOTE}; the tag could not be checked and could not be pushed"
      ;;
  esac
  return 0
}

check_release_absent() {
  local tag="$1"

  log "checking that gh is authenticated"
  if ! gh auth status >/dev/null 2>&1; then
    record_blocker "gh is not authenticated; run: gh auth login"
    return 0
  fi

  # `gh release view`は、Releaseが無いときも、APIへ届かないときも失敗する。observe
  # できないことをReleaseが無いことと同一視しないため、先に届くことを確かめる。
  # 届かないなら、既存Releaseの有無はこの実行では判断しない。
  log "checking that GitHub is reachable"
  if ! gh repo view --json name >/dev/null 2>&1; then
    record_blocker "cannot reach GitHub; whether Release ${tag} exists could not be observed"
    return 0
  fi

  log "checking that GitHub Release ${tag} does not already exist"
  if gh release view "${tag}" >/dev/null 2>&1; then
    record_blocker "GitHub Release ${tag} already exists"
  fi
  return 0
}

# releaseが完成品かどうかは、指定した者だけが知っている。versionから推測しない。
# `0.0.1`という並びは初期開発を示唆するが、示唆は宣言ではない。
resolve_prerelease() {
  case "$PRERELEASE_MODE" in
    yes)
      PRERELEASE=1
      log "publishing as a prerelease (--prerelease)"
      ;;
    no)
      PRERELEASE=0
      log "publishing as a stable release (--stable)"
      ;;
    *)
      usage
      fail "pass --prerelease or --stable; the version is not read as an answer"
      ;;
  esac
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

# dist/には今回の実行の成果物だけを置く。前回の生成物が残っていると、失敗した実行の
# 後にdist/を覗いた人が、古いversionのarchiveとnotesを今回のものと読む。
reset_dist() {
  [ -e "$DIST_DIR" ] || return 0
  log "clearing ${DIST_DIR}/"
  rm -rf "$DIST_DIR"
}

# 出荷するtreeがfmt・lint・test・coverageを通ることを、releaseする側が自分で確かめる。
# 他所の検査結果には委ねない。それが通ったかどうかはここから観測できず、通ったはずだと
# いう推測にしかならない。何を出荷してよいかの判断を推測の上に置かない。
# 出力は流したままにする。落ちたときに何が落ちたかを見るためにここに居る。
run_repository_checks() {
  log "running mise run check (fmt, lint, test, coverage)"
  command -v mise >/dev/null 2>&1 || fail "mise is not installed"
  mise run check \
    || fail "mise run check did not pass; this tree is not in a releasable state"
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
  # --clobberは付けない。既存assetを誤って上書きしない。
  # 空arrayはbash 3.2のset -uで展開できないが、この配列は必ず要素を持つ。
  local args=("$tag" "$archive" --title "$tag" --notes-file "$notes_file" --verify-tag)
  [ "$PRERELEASE" -eq 1 ] && args+=(--prerelease)
  if gh release create "${args[@]}"; then
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
      if [ "$PRERELEASE" -eq 1 ]; then
        printf '    --verify-tag \\\n'
        printf '    --prerelease\n'
      else
        printf '    --verify-tag\n'
      fi
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
        --prerelease)
          [ "$PRERELEASE_MODE" = "no" ] && fail "--prerelease and --stable contradict each other"
          PRERELEASE_MODE="yes"
          continue
          ;;
        --stable)
          [ "$PRERELEASE_MODE" = "yes" ] && fail "--prerelease and --stable contradict each other"
          PRERELEASE_MODE="no"
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

  # 副作用より前に決める。指定が無ければ、dist/へ触れる前に拒否する。
  resolve_prerelease

  cd "$(git rev-parse --show-toplevel)"

  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sbxm-release.XXXXXX")"
  trap 'rm -rf "$WORK_DIR"' EXIT

  [ "$DRY_RUN" -eq 1 ] && log "dry run: nothing will be tagged, pushed, or published"

  # 検査より前に消す。どこで落ちても、dist/には今回の実行が作ったものしか無い状態に
  # する。途中で落ちた実行の後に前回の生成物が残っていると、それを今回のものと読む。
  reset_dist

  local version="${tag#v}"
  [ "$version" != "$tag" ] || fail "tag must start with v (e.g. v0.0.1): ${tag}"

  check_clean_worktree
  check_head_is_on_default_branch
  check_tag_available "$tag"
  check_release_absent "$tag"
  check_cargo_version_matches "$version"
  check_no_build_affecting_env_vars
  check_host_is_arm64
  check_rustc_host_is_apple_silicon

  # 安いものを先に落としてから、時間のかかる検査へ入る。
  run_repository_checks

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
