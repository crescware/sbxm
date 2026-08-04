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
# buildの前に`mise run check`を通す。通らないtreeからはリリースを作らない。
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

# tapを自動更新してよいsource repositoryを1つに固定する。forkからの実行はtap更新を
# skipし、Release作成までは従来どおり行う。
OFFICIAL_SOURCE_REPO="crescware/sbxm"
# 更新対象のtapはこのrepositoryに固定する。任意のtap・formulaへ一般化しない。
TAP_REPO="crescware/homebrew-tap"
TAP_FORMULA_RELATIVE_PATH="Formula/sbxm.rb"
# tap開発者の通常のcheckoutとは別物とする、release script専用のcache。毎回消える
# WORK_DIRやdist/には置かず、実行間でcloneを使い回す。
TAP_CACHE_DIR="${XDG_CACHE_HOME:-$HOME/Library/Caches}/sbxm/release/homebrew-tap"

# build結果を左右しうるenv var。再現性のないbinaryを出荷しないよう、リリースは常に
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

# detect_source_repoが埋める。tap更新を有効にしてよいかどうかの判定に使う。
SOURCE_REPO=""
TAP_UPDATE_ENABLED=0
TAP_SKIP_REASON="the source repository could not be identified; the tap update could not be planned"

# publish_tap_pr以降が埋める。tag・versionから決定的に決まる値。
TAP_BRANCH=""
TAP_BASE_COMMIT=""
TAP_COMMIT_TITLE=""
RELEASE_ASSET_URL=""
TAP_PR_URL=""

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

# リリースは既定branchに入っているcommitからだけ切る。featureブランチのcommitへtagを
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

# 更新してよいのはcrescware/sbxmでの実行だけとする。forkのasset URLを公式tapへ
# 誤って入れないため、現在のsource repositoryをgh経由で明示的に取得し、remote URLの
# 文字列からは推測しない。届かないことと、フォークであることは別の状態として扱う。
# 前者はblockerとして報告し、後者はTAP_SKIP_REASONで理由を示してtap更新をskipする。
detect_source_repo() {
  log "identifying the source repository"
  if ! SOURCE_REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)"; then
    record_blocker "could not identify the source repository via gh repo view; the tap update could not be planned"
    return 0
  fi

  log "source repository: ${SOURCE_REPO}"
  if [ "$SOURCE_REPO" = "$OFFICIAL_SOURCE_REPO" ]; then
    TAP_UPDATE_ENABLED=1
  else
    TAP_SKIP_REASON="source repository is ${SOURCE_REPO}, not ${OFFICIAL_SOURCE_REPO}; skipping the tap update (a fork's Release is still published on its own)"
    log "$TAP_SKIP_REASON"
  fi
  return 0
}

# リリースが完成品かどうかは、指定した者だけが知っている。versionから推測しない。
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

# リリースするtreeがfmt・lint・test・coverageを通ることを、リリースする側が自分で確かめる。
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

# archiveのSHA-256はここで一度だけ計算する。release notesのchecksum行とformulaの
# sha256には、再計算せずこの値をそのまま渡す。正本を1箇所に保つことで、両者が別々に
# hashを取り直して食い違う余地をなくす。
compute_archive_sha256() {
  local archive="$1"
  local archive_dir archive_base
  archive_dir="$(dirname "$archive")"
  archive_base="$(basename "$archive")"

  log "computing the archive's SHA-256"
  local checksum_line
  checksum_line="$(cd "$archive_dir" && shasum -a 256 "$archive_base")"

  local digest
  digest="$(printf '%s\n' "$checksum_line" | awk '{print $1}')"
  printf '%s\n' "$digest" | grep -Eq '^[0-9a-f]{64}$' \
    || fail "shasum -a 256 did not produce a 64-character lowercase hex digest for ${archive}: ${checksum_line}"

  printf '%s' "$digest"
}

record_provenance() {
  local archive="$1" archive_sha256="$2"
  local archive_base
  archive_base="$(basename "$archive")"
  # dry runで中身を確かめられるよう、WORK_DIRではなくdist/へ残す。
  local notes_file="${DIST_DIR}/${NOTES_NAME}"

  local commit_sha rustc_version cargo_version_str sw_vers_output checksum
  commit_sha="$(git rev-parse HEAD)"
  rustc_version="$(rustc -vV)"
  cargo_version_str="$(cargo -V)"
  sw_vers_output="$(sw_vers)"
  # shasum -a 256の出力形式 (「hash  filename」) に合わせる。dist/を含めず、archive名
  # だけをdigest行へ記録する。shasum -cでの検証時に同じ階層へ置くだけで済むようにする。
  checksum="${archive_sha256}  ${archive_base}"

  log "recording the Git commit SHA: ${commit_sha}"
  log "recording rustc -vV"
  printf '%s\n' "$rustc_version" >&2
  log "recording cargo -V"
  printf '%s\n' "$cargo_version_str" >&2
  log "recording sw_vers"
  printf '%s\n' "$sw_vers_output" >&2
  log "recording the archive SHA-256"
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
  # annotated tagにする。リリースを指すtagには打ち手と日時を残す。
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

# ここから先はRelease公開後にだけ呼ぶ。create_github_releaseまたはこの検証が失敗した
# 場合、tapのcache、branch、PRのいずれも作らない。
#
# GitHubが返すasset情報を読み、組み立てたURLの推測ではなく、いま作成したReleaseが
# 実際に公開したassetをformulaが参照するようにする。
verify_release_asset() {
  local tag="$1" archive_base="$2" archive_sha256="$3"
  log "verifying the published Release asset: ${archive_base}"

  local assets_tsv
  assets_tsv="$(gh release view "$tag" --json assets \
    --jq '.assets[] | [.name, .state, .url, .digest] | @tsv')" \
    || fail "could not read Release ${tag}'s assets to verify the published asset"

  local result
  result="$(printf '%s\n' "$assets_tsv" | awk -F'\t' -v name="$archive_base" -v expect="sha256:${archive_sha256}" '
    BEGIN { count = 0 }
    $1 == name { count++; state = $2; url = $3; digest = $4 }
    END {
      if (count != 1) { print "count\t" count; exit }
      if (state != "uploaded") { print "state\t" state; exit }
      if (digest != expect) { print "digest\t" digest; exit }
      print "ok\t" url
    }
  ')"

  local status value
  status="${result%%$'\t'*}"
  value="${result#*$'\t'}"

  case "$status" in
    ok)
      RELEASE_ASSET_URL="$value"
      log "verified the Release asset: state=uploaded digest=sha256:${archive_sha256}"
      ;;
    count)
      fail "Release ${tag} has ${value} asset(s) named ${archive_base}, expected exactly 1"
      ;;
    state)
      fail "Release ${tag}'s asset ${archive_base} has state '${value}', expected 'uploaded'"
      ;;
    digest)
      fail "Release ${tag}'s asset ${archive_base} digest is '${value}', expected 'sha256:${archive_sha256}'"
      ;;
    *)
      fail "could not verify Release ${tag}'s asset ${archive_base}"
      ;;
  esac
}

# git remote get-urlはinsteadOf書き換え後のURLを返すため使わない。設定されたそのままの
# 値を読み、tapの取り違えを防ぐ。
tap_origin_url() {
  git -C "$TAP_CACHE_DIR" config --get remote.origin.url 2>/dev/null
}

tap_origin_matches() {
  local url="$1"
  local normalized
  normalized="$(printf '%s' "$url" \
    | sed -E 's#^git@github\.com:#https://github.com/#' \
    | sed -E 's#\.git$##' \
    | sed -E 's#^https://github\.com/##')"
  [ "$normalized" = "$TAP_REPO" ]
}

clone_tap_checkout() {
  log "cloning ${TAP_REPO} into ${TAP_CACHE_DIR}"
  mkdir -p "$(dirname "$TAP_CACHE_DIR")"
  gh repo clone "$TAP_REPO" "$TAP_CACHE_DIR" \
    || fail "could not clone ${TAP_REPO} into ${TAP_CACHE_DIR}"
}

# 既存cacheを、削除・reset・stashのいずれもせずに検査する。1つでも満たさなければ
# 停止する。owner不明の変更を上書きしない。
verify_existing_tap_checkout() {
  log "verifying the existing tap checkout: ${TAP_CACHE_DIR}"

  [ -d "$TAP_CACHE_DIR" ] \
    || fail "${TAP_CACHE_DIR} exists but is not a directory; refusing to touch it"

  local toplevel
  toplevel="$(git -C "$TAP_CACHE_DIR" rev-parse --show-toplevel 2>/dev/null)" \
    || fail "${TAP_CACHE_DIR} exists but is not a Git working tree; refusing to touch it"
  [ "$(cd "$toplevel" && pwd -P)" = "$(cd "$TAP_CACHE_DIR" && pwd -P)" ] \
    || fail "${TAP_CACHE_DIR} is not a Git repository root (root: ${toplevel}); refusing to touch it"

  local origin_url
  origin_url="$(tap_origin_url)" \
    || fail "${TAP_CACHE_DIR} has no 'origin' remote configured; refusing to touch it"
  tap_origin_matches "$origin_url" \
    || fail "${TAP_CACHE_DIR}'s origin (${origin_url}) is not ${TAP_REPO}; refusing to touch it"

  [ -z "$(git -C "$TAP_CACHE_DIR" status --porcelain)" ] \
    || fail "${TAP_CACHE_DIR} has uncommitted changes; refusing to touch it. Inspect it, then re-run."
}

fetch_tap_main() {
  log "fetching ${TAP_REPO}'s main branch"
  git -C "$TAP_CACHE_DIR" fetch --prune origin main \
    || fail "could not fetch origin's main branch in ${TAP_CACHE_DIR}"
}

ensure_tap_checkout() {
  log "preparing the tap checkout: ${TAP_CACHE_DIR}"
  if [ -e "$TAP_CACHE_DIR" ] || [ -L "$TAP_CACHE_DIR" ]; then
    verify_existing_tap_checkout
    fetch_tap_main
  else
    clone_tap_checkout
  fi

  TAP_BASE_COMMIT="$(git -C "$TAP_CACHE_DIR" rev-parse origin/main)" \
    || fail "could not resolve ${TAP_REPO}'s origin/main in ${TAP_CACHE_DIR}"
}

resolve_tap_branch_name() {
  local tag="$1"
  TAP_BRANCH="chore/bump-sbxm-to-${tag}"
}

resolve_tap_commit_title() {
  local version="$1"
  TAP_COMMIT_TITLE="Bump sbxm to ${version}"
}

# local/remoteのどちらかにだけ衝突があっても、どちらかを正しいものと推測して再利用
# しない。branchの削除やforce pushは行わず、commit SHAとcache pathを示して止まる。
check_tap_branch_available() {
  log "checking that ${TAP_BRANCH} does not already exist in the tap"

  if git -C "$TAP_CACHE_DIR" rev-parse -q --verify "refs/heads/${TAP_BRANCH}" >/dev/null; then
    local local_commit
    local_commit="$(git -C "$TAP_CACHE_DIR" rev-parse "refs/heads/${TAP_BRANCH}")"
    fail "tap branch ${TAP_BRANCH} already exists locally at ${local_commit} (cache: ${TAP_CACHE_DIR}); resolve it manually, this script will not reuse, delete, or force-push it"
  fi

  local remote_commit
  remote_commit="$(git -C "$TAP_CACHE_DIR" ls-remote origin "refs/heads/${TAP_BRANCH}" | awk '{print $1}')" \
    || fail "could not check whether ${TAP_BRANCH} exists on ${TAP_REPO}"
  [ -z "$remote_commit" ] \
    || fail "tap branch ${TAP_BRANCH} already exists on ${TAP_REPO} at ${remote_commit} (cache: ${TAP_CACHE_DIR}); resolve it manually, this script will not reuse, delete, or force-push it"
}

create_tap_branch() {
  log "creating tap branch ${TAP_BRANCH} from origin/main (${TAP_BASE_COMMIT})"
  git -C "$TAP_CACHE_DIR" branch "$TAP_BRANCH" "$TAP_BASE_COMMIT" \
    || fail "could not create tap branch ${TAP_BRANCH} at ${TAP_BASE_COMMIT}"
  git -C "$TAP_CACHE_DIR" checkout --quiet "$TAP_BRANCH" \
    || fail "could not check out tap branch ${TAP_BRANCH}"
}

# 編集前にFormula/sbxm.rbが契約を満たすことを確認する。urlとsha256がそれぞれちょうど
# 1行あり、既存sha256が64文字の小文字16進数で、既存urlのasset basenameがARCHIVE_NAME
# と一致し、checkoutがformula以外も含めてcleanであることを要求する。
check_tap_formula_contract() {
  local formula="${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"
  log "checking ${TAP_FORMULA_RELATIVE_PATH}'s existing url/sha256 lines"

  [ -f "$formula" ] || fail "${formula} does not exist"

  local url_count sha_count
  url_count="$(grep -c '^  url "' "$formula" || true)"
  sha_count="$(grep -c '^  sha256 "' "$formula" || true)"
  [ "$url_count" -eq 1 ] \
    || fail "${formula} has ${url_count} 'url' line(s) with a 2-space indent, expected exactly 1"
  [ "$sha_count" -eq 1 ] \
    || fail "${formula} has ${sha_count} 'sha256' line(s) with a 2-space indent, expected exactly 1"

  local existing_url existing_sha256
  existing_url="$(sed -n 's/^  url "\(.*\)"$/\1/p' "$formula")"
  existing_sha256="$(sed -n 's/^  sha256 "\(.*\)"$/\1/p' "$formula")"

  printf '%s\n' "$existing_sha256" | grep -Eq '^[0-9a-f]{64}$' \
    || fail "${formula}'s existing sha256 (${existing_sha256}) is not 64 lowercase hex characters"

  local existing_basename
  existing_basename="$(basename "$existing_url")"
  [ "$existing_basename" = "$ARCHIVE_NAME" ] \
    || fail "${formula}'s existing url (${existing_url}) does not reference ${ARCHIVE_NAME}; refusing to treat this as sbxm's formula"

  [ -z "$(git -C "$TAP_CACHE_DIR" status --porcelain)" ] \
    || fail "${TAP_CACHE_DIR} is not clean before editing ${TAP_FORMULA_RELATIVE_PATH}"
}

# 元fileを直接in-place編集せず、WORK_DIRへcandidateを作る。awkだけで、一致した2行を
# 期待する完全な行へ置き換え、その他の行はそのまま出力する。
build_tap_formula_candidate() {
  local new_url="$1" new_sha256="$2"
  local formula="${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"
  local candidate="${WORK_DIR}/sbxm.rb.candidate"

  awk -v new_url="$new_url" -v new_sha256="$new_sha256" '
    /^  url "/    { print "  url \"" new_url "\""; next }
    /^  sha256 "/ { print "  sha256 \"" new_sha256 "\""; next }
    { print }
  ' "$formula" >"$candidate"

  printf '%s' "$candidate"
}

# candidateの2行だけを書き換え線として置き換えたと確かめる。urlとsha256を空へ置換した
# masked版同士をbyte単位で比較することで、対象2行を除いた元fileとcandidateが一致する
# ことを確認する。
mask_tap_formula_target_lines() {
  awk '
    /^  url "/    { print "  url \"\""; next }
    /^  sha256 "/ { print "  sha256 \"\""; next }
    { print }
  ' "$1"
}

verify_tap_formula_candidate() {
  local candidate="$1" expected_url="$2" expected_sha256="$3"
  local formula="${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"

  local url_count sha_count
  url_count="$(grep -c '^  url "' "$candidate" || true)"
  sha_count="$(grep -c '^  sha256 "' "$candidate" || true)"
  [ "$url_count" -eq 1 ] || fail "candidate formula has ${url_count} 'url' line(s), expected exactly 1"
  [ "$sha_count" -eq 1 ] || fail "candidate formula has ${sha_count} 'sha256' line(s), expected exactly 1"

  local candidate_url candidate_sha256
  candidate_url="$(sed -n 's/^  url "\(.*\)"$/\1/p' "$candidate")"
  candidate_sha256="$(sed -n 's/^  sha256 "\(.*\)"$/\1/p' "$candidate")"
  [ "$candidate_url" = "$expected_url" ] \
    || fail "candidate formula's url (${candidate_url}) does not match the verified Release asset url (${expected_url})"
  [ "$candidate_sha256" = "$expected_sha256" ] \
    || fail "candidate formula's sha256 (${candidate_sha256}) does not match the archive sha256 (${expected_sha256})"

  diff -q <(mask_tap_formula_target_lines "$formula") <(mask_tap_formula_target_lines "$candidate") >/dev/null \
    || fail "candidate formula changes lines other than url/sha256 in ${formula}"
}

# 反映後に、working treeの変更範囲がformulaだけで、差分がちょうど2行削除・2行追加で
# あることを検査する。1つでも満たさなければcommitへ進まない。
verify_tap_worktree_diff_scope() {
  log "checking the unstaged diff is limited to ${TAP_FORMULA_RELATIVE_PATH}"

  local status
  status="$(git -C "$TAP_CACHE_DIR" status --porcelain)"
  [ "$status" = " M ${TAP_FORMULA_RELATIVE_PATH}" ] \
    || fail "${TAP_CACHE_DIR}'s working tree changed beyond ${TAP_FORMULA_RELATIVE_PATH}: $(printf '%s' "$status" | tr '\n' ' ')"

  local numstat added removed
  numstat="$(git -C "$TAP_CACHE_DIR" diff --numstat -- "$TAP_FORMULA_RELATIVE_PATH")"
  added="$(printf '%s' "$numstat" | awk '{print $1}')"
  removed="$(printf '%s' "$numstat" | awk '{print $2}')"
  [ "$added" = "2" ] && [ "$removed" = "2" ] \
    || fail "the unstaged diff for ${TAP_FORMULA_RELATIVE_PATH} in ${TAP_CACHE_DIR} is +${added}/-${removed}, expected +2/-2"

  git -C "$TAP_CACHE_DIR" diff --check -- "$TAP_FORMULA_RELATIVE_PATH" \
    || fail "git diff --check reported whitespace errors in ${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"
}

# stagingした後も同じ条件を確認し、unstagedの変更が残っていないことも確認する。
verify_tap_staged_diff_scope() {
  log "checking the staged diff is limited to ${TAP_FORMULA_RELATIVE_PATH}"

  local status
  status="$(git -C "$TAP_CACHE_DIR" status --porcelain)"
  [ "$status" = "M  ${TAP_FORMULA_RELATIVE_PATH}" ] \
    || fail "${TAP_CACHE_DIR}'s staged changes are not limited to ${TAP_FORMULA_RELATIVE_PATH} (or unstaged changes remain): $(printf '%s' "$status" | tr '\n' ' ')"

  local numstat added removed
  numstat="$(git -C "$TAP_CACHE_DIR" diff --cached --numstat -- "$TAP_FORMULA_RELATIVE_PATH")"
  added="$(printf '%s' "$numstat" | awk '{print $1}')"
  removed="$(printf '%s' "$numstat" | awk '{print $2}')"
  [ "$added" = "2" ] && [ "$removed" = "2" ] \
    || fail "the staged diff for ${TAP_FORMULA_RELATIVE_PATH} in ${TAP_CACHE_DIR} is +${added}/-${removed}, expected +2/-2"

  git -C "$TAP_CACHE_DIR" diff --cached --check -- "$TAP_FORMULA_RELATIVE_PATH" \
    || fail "git diff --cached --check reported whitespace errors in ${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"
}

update_tap_formula() {
  local expected_url="$1" expected_sha256="$2"

  check_tap_formula_contract

  local candidate
  candidate="$(build_tap_formula_candidate "$expected_url" "$expected_sha256")"
  verify_tap_formula_candidate "$candidate" "$expected_url" "$expected_sha256"

  cp "$candidate" "${TAP_CACHE_DIR}/${TAP_FORMULA_RELATIVE_PATH}"
  verify_tap_worktree_diff_scope

  git -C "$TAP_CACHE_DIR" add "$TAP_FORMULA_RELATIVE_PATH" \
    || fail "could not stage ${TAP_FORMULA_RELATIVE_PATH} in ${TAP_CACHE_DIR}"
  verify_tap_staged_diff_scope
}

commit_tap_formula() {
  log "committing ${TAP_FORMULA_RELATIVE_PATH} on ${TAP_BRANCH}"
  git -C "$TAP_CACHE_DIR" commit --quiet -m "$TAP_COMMIT_TITLE" \
    || fail "could not commit ${TAP_FORMULA_RELATIVE_PATH} in ${TAP_CACHE_DIR}"
}

push_tap_branch() {
  log "pushing ${TAP_BRANCH} to ${TAP_REPO}"
  git -C "$TAP_CACHE_DIR" push origin "HEAD:refs/heads/${TAP_BRANCH}" \
    || fail "could not push ${TAP_BRANCH} to ${TAP_REPO}; retry with: git -C ${TAP_CACHE_DIR} push origin HEAD:refs/heads/${TAP_BRANCH}"
}

write_tap_pr_body() {
  local tag="$1" source_release_url="$2" asset_url="$3" archive_sha256="$4"
  local body_file="${WORK_DIR}/tap-pr-body.md"
  {
    printf 'Automatically generated by `scripts/release/release.sh` in [%s](https://github.com/%s) after publishing %s.\n\n' \
      "$SOURCE_REPO" "$SOURCE_REPO" "$tag"
    printf -- '- Release: %s\n' "$source_release_url"
    printf -- '- Asset: %s\n' "$asset_url"
    printf -- '- SHA-256: `%s`\n' "$archive_sha256"
  } >"$body_file"
  printf '%s' "$body_file"
}

create_tap_pr() {
  local body_file="$1"
  log "creating the tap PR: ${TAP_REPO} ${TAP_BRANCH} -> main"

  local pr_url
  pr_url="$(gh pr create \
    --repo "$TAP_REPO" \
    --base main \
    --head "$TAP_BRANCH" \
    --title "$TAP_COMMIT_TITLE" \
    --body-file "$body_file")" \
    || fail "could not create the tap PR; retry with: gh pr create --repo ${TAP_REPO} --base main --head ${TAP_BRANCH} --title $(printf '%q' "$TAP_COMMIT_TITLE") --body-file ${body_file}"

  TAP_PR_URL="$pr_url"
}

# Release公開後にだけ呼ぶ。公式repositoryでの実行でなければ、理由を示してtapへ触れずに
# 戻る。asset検証、tap checkout、branchの衝突検査、formulaのtransactionalな更新、
# commit、push、PR作成をこの順に行う。途中で失敗すれば、そこで残る状態と回復方法は
# 各functionのfail messageが示す。
publish_tap_pr() {
  local tag="$1" version="$2" archive_sha256="$3"

  if [ "$TAP_UPDATE_ENABLED" -ne 1 ]; then
    log "skipping the tap update: ${TAP_SKIP_REASON}"
    return 0
  fi

  verify_release_asset "$tag" "$ARCHIVE_NAME" "$archive_sha256"

  resolve_tap_branch_name "$tag"
  resolve_tap_commit_title "$version"

  ensure_tap_checkout
  check_tap_branch_available

  create_tap_branch
  update_tap_formula "$RELEASE_ASSET_URL" "$archive_sha256"
  commit_tap_formula
  push_tap_branch

  local source_release_url body_file
  source_release_url="https://github.com/${SOURCE_REPO}/releases/tag/${tag}"
  body_file="$(write_tap_pr_body "$tag" "$source_release_url" "$RELEASE_ASSET_URL" "$archive_sha256")"
  create_tap_pr "$body_file"
}

publish() {
  local tag="$1" version="$2" archive="$3" archive_sha256="$4" notes_file="$5"

  if [ "$DRY_RUN" -eq 1 ]; then
    log "dry run: not tagging, not pushing, not creating a Release, not touching the tap"
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

  publish_tap_pr "$tag" "$version" "$archive_sha256"
}

# tapについてはclone、fetch、branch作成、formula編集、commit、push、PR作成のいずれも
# 行わない。cache pathのstatを読むだけの、read-onlyな予定表示に限る。
report_dry_run_tap_plan() {
  local tag="$1" version="$2" archive_sha256="$3"

  if [ "$TAP_UPDATE_ENABLED" -ne 1 ]; then
    {
      printf '\n'
      printf 'tap update: skipped\n'
      printf '  %s\n' "$TAP_SKIP_REASON"
    } >&2
    return 0
  fi

  resolve_tap_branch_name "$tag"
  resolve_tap_commit_title "$version"

  local dry_run_asset_url source_release_url checkout_plan
  dry_run_asset_url="https://github.com/${SOURCE_REPO}/releases/download/${tag}/${ARCHIVE_NAME}"
  source_release_url="https://github.com/${SOURCE_REPO}/releases/tag/${tag}"

  checkout_plan="clone ${TAP_REPO} into"
  if [ -e "$TAP_CACHE_DIR" ] || [ -L "$TAP_CACHE_DIR" ]; then
    checkout_plan="fetch origin main in the existing checkout at"
  fi

  {
    printf '\n'
    printf 'tap update plan (%s):\n' "$TAP_REPO"
    printf '  source Release: %s\n' "$source_release_url"
    printf '  formula:        %s\n' "$TAP_FORMULA_RELATIVE_PATH"
    printf '  checkout:       %s %s\n' "$checkout_plan" "$TAP_CACHE_DIR"
    printf '  base branch:    main (origin/main, resolved at fetch/clone time)\n'
    printf '  new branch:     %s\n' "$TAP_BRANCH"
    printf '  asset url:      %s\n' "$dry_run_asset_url"
    printf '  sha256:         %s\n' "$archive_sha256"
    printf '  commit message: %s\n' "$TAP_COMMIT_TITLE"
    printf '  push refspec:   HEAD:refs/heads/%s\n' "$TAP_BRANCH"
    printf '  would run:\n'
    printf '    gh pr create --repo %s --base main --head %s \\\n' "$TAP_REPO" "$TAP_BRANCH"
    printf '      --title %q --body-file %q\n' "$TAP_COMMIT_TITLE" "${WORK_DIR}/tap-pr-body.md"
  } >&2
}

report_dry_run() {
  local tag="$1" version="$2" archive="$3" archive_sha256="$4" notes_file="$5"
  log "dry run finished; nothing was written"
  {
    printf 'inspect the artifacts it would have uploaded:\n'
    printf '  asset:         %s\n' "$archive"
    printf '  sha256:        %s\n' "$archive_sha256"
    printf '  release notes: %s\n' "$notes_file"
  } >&2

  report_dry_run_tap_plan "$tag" "$version" "$archive_sha256"

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
  # 本番のリリースを作ってしまう余地を残さない。
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
  detect_source_repo
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

  local archive_sha256
  archive_sha256="$(compute_archive_sha256 "$archive_path")"

  local notes_file
  notes_file="$(record_provenance "$archive_path" "$archive_sha256")"

  publish "$tag" "$version" "$archive_path" "$archive_sha256" "$notes_file"

  if [ "$DRY_RUN" -eq 1 ]; then
    report_dry_run "$tag" "$version" "$archive_path" "$archive_sha256" "$notes_file"
  else
    log "created release ${tag}"
    if [ "$TAP_UPDATE_ENABLED" -eq 1 ]; then
      log "created the tap PR: ${TAP_PR_URL}"
    else
      log "tap update skipped: ${TAP_SKIP_REASON}"
    fi
  fi
}

main "$@"
