#!/usr/bin/env bash
# scripts/release/release.shのend-to-end test。
#
# 実際のGitHubやnetworkには一切接続しない。fake command (git以外) と、scratchな
# source/tap repositoryを組み立て、release.shをsubprocessとして実際に実行して確認する。
#
# - git自体はfakeせず、実repositoryをlocalに用意する。tapのorigin識別だけは、
#   `url.<path>.insteadOf`でtransportを書き換え、`git config --get remote.origin.url`が
#   返す値 (release.shが読む値) はhttps://github.com/crescware/homebrew-tap.gitのまま
#   保たれるようにする。
# - gh、cargo、codesign、file、sw_vers、rustc、uname、miseはfake commandに置き換える。
#   env -iでsubprocessの環境を作り直し、host側の環境変数を一切継がない。
# - 各test caseは`( set -e; test_fn )`という形で個別のsubshellとして実行する。1つが
#   落ちても他のtestは続ける。
#
# formula candidateの値照合・非対象行のbyte比較といった、正しいcodeからは外部入力だけ
# では再現できない防御 (release.sh内のverify_tap_formula_candidateのcandidate値照合、
# byte比較部分) は、専用のfailure caseを持たない。これらはcount検査 (url/sha256が
# ちょうど1行) を通ったcandidateがawkの行単位置換で必ず期待どおりになるという構造から
# 保証されており、正常系testが毎回そのcode pathを通ることで間接的に確認している。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RELEASE_SCRIPT="${REPO_ROOT}/scripts/release/release.sh"
ARCHIVE_NAME="sbxm-aarch64-apple-darwin.tar.gz"

REAL_GIT="$(command -v git)"
export REAL_GIT

RUN_TMP="$(mktemp -d "${TMPDIR:-/tmp}/sbxm-release-test.XXXXXX")"
trap 'rm -rf "$RUN_TMP"' EXIT

FAKE_BIN_DIR="${RUN_TMP}/fake-bin"

TESTS_RUN=0
TESTS_FAILED=0
FAILED_NAMES=""

# ---------------------------------------------------------------------------
# fake command
# ---------------------------------------------------------------------------

write_fake_bin() {
  mkdir -p "$FAKE_BIN_DIR"

  cat >"${FAKE_BIN_DIR}/git" <<'SCRIPT'
#!/usr/bin/env bash
printf 'git %s\n' "$*" >>"$FAKE_CALL_LOG"

if [ "${FAKE_TAP_FETCH_FAIL:-0}" = "1" ] && [ "${1:-}" = "-C" ] \
  && [ "${2:-}" = "${EXPECT_TAP_CACHE_DIR:-__none__}" ] && [ "${3:-}" = "fetch" ]; then
  exit 1
fi
if [ "${FAKE_TAP_COMMIT_FAIL:-0}" = "1" ] && [ "${1:-}" = "-C" ] \
  && [ "${2:-}" = "${EXPECT_TAP_CACHE_DIR:-__none__}" ] && [ "${3:-}" = "commit" ]; then
  exit 1
fi
if [ "${FAKE_TAP_PUSH_FAIL:-0}" = "1" ] && [ "${1:-}" = "-C" ] && [ "${3:-}" = "push" ]; then
  for a in "$@"; do
    case "$a" in
      *refs/heads/chore/bump-sbxm-to-*) exit 1 ;;
    esac
  done
fi

exec "$REAL_GIT" "$@"
SCRIPT

  cat >"${FAKE_BIN_DIR}/gh" <<'SCRIPT'
#!/usr/bin/env bash
printf 'gh %s\n' "$*" >>"$FAKE_CALL_LOG"

subcommand="${1:-}"
shift || true

case "$subcommand" in
  auth)
    [ "${FAKE_GH_UNAUTHENTICATED:-0}" = "1" ] && exit 1
    exit 0
    ;;
  repo)
    action="${1:-}"; shift || true
    case "$action" in
      view)
        [ "${FAKE_GH_UNREACHABLE:-0}" = "1" ] && exit 1
        json_field=""
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --json) json_field="$2"; shift 2 ;;
            -q) shift 2 ;;
            *) shift ;;
          esac
        done
        case "$json_field" in
          nameWithOwner) printf '%s\n' "${FAKE_SOURCE_REPO_NAME:?FAKE_SOURCE_REPO_NAME not set}" ;;
          name) printf '{"name":"fake"}\n' ;;
          *) exit 1 ;;
        esac
        ;;
      clone)
        target_repo="$1"; dest="$2"
        if [ "$target_repo" = "crescware/homebrew-tap" ]; then
          mkdir -p "$dest"
          git init -q -b main "$dest"
          git -C "$dest" remote add origin "https://github.com/${target_repo}.git"
          git -C "$dest" fetch -q origin
          git -C "$dest" checkout -q -B main origin/main
        else
          exit 1
        fi
        ;;
      *) exit 1 ;;
    esac
    ;;
  release)
    action="${1:-}"; shift || true
    case "$action" in
      view)
        [ "${FAKE_GH_UNREACHABLE:-0}" = "1" ] && exit 1
        tag="${1:-}"; shift || true
        json_field=""
        while [ "$#" -gt 0 ]; do
          case "$1" in
            --json) json_field="$2"; shift 2 ;;
            --jq) shift 2 ;;
            *) shift ;;
          esac
        done
        store="${FAKE_GH_STATE}/releases/${tag}.assets.tsv"
        [ -f "$store" ] || exit 1
        [ "$json_field" = "assets" ] && cat "$store"
        exit 0
        ;;
      create)
        [ "${FAKE_GH_RELEASE_CREATE_FAIL:-0}" = "1" ] && exit 1
        tag="$1"; archive="$2"; shift 2 || true
        mkdir -p "${FAKE_GH_STATE}/releases"
        base="$(basename "$archive")"
        digest="sha256:$(shasum -a 256 "$archive" | awk '{print $1}')"
        if [ "${FAKE_GH_ASSET_MISMATCH:-0}" = "1" ]; then
          digest="sha256:deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        fi
        state="uploaded"
        [ "${FAKE_GH_ASSET_WRONG_STATE:-0}" = "1" ] && state="pending"
        name="$base"
        [ "${FAKE_GH_ASSET_WRONG_NAME:-0}" = "1" ] && name="unexpected-name.tar.gz"
        printf '%s\t%s\t%s\t%s\n' "$name" "$state" \
          "https://github.com/${FAKE_SOURCE_REPO_NAME}/releases/download/${tag}/${base}" \
          "$digest" >"${FAKE_GH_STATE}/releases/${tag}.assets.tsv"
        printf 'https://github.com/%s/releases/tag/%s\n' "$FAKE_SOURCE_REPO_NAME" "$tag"
        ;;
      *) exit 1 ;;
    esac
    ;;
  pr)
    action="${1:-}"; shift || true
    case "$action" in
      create)
        [ "${FAKE_GH_PR_CREATE_FAIL:-0}" = "1" ] && exit 1
        mkdir -p "$FAKE_GH_STATE"
        printf '%s\n' "$*" >>"${FAKE_GH_STATE}/pr-create-calls.log"
        n=1
        [ -f "${FAKE_GH_STATE}/pr-counter" ] && n=$(( $(cat "${FAKE_GH_STATE}/pr-counter") + 1 ))
        printf '%s\n' "$n" >"${FAKE_GH_STATE}/pr-counter"
        printf 'https://github.com/crescware/homebrew-tap/pull/%s\n' "$n"
        ;;
      *) exit 1 ;;
    esac
    ;;
  *)
    exit 1
    ;;
esac
SCRIPT

  cat >"${FAKE_BIN_DIR}/cargo" <<'SCRIPT'
#!/usr/bin/env bash
printf 'cargo %s\n' "$*" >>"$FAKE_CALL_LOG"
case "${1:-}" in
  build)
    mkdir -p target/release
    version="$(awk -F'"' '
      /^\[/ { in_package = ($0 == "[package]") }
      in_package && /^version[[:space:]]*=/ { print $2; exit }
    ' Cargo.toml)"
    cat >target/release/sbxm <<SBXM
#!/usr/bin/env bash
if [ "\$1" = "--version" ]; then
  printf 'sbxm %s\n' "$version"
fi
SBXM
    chmod +x target/release/sbxm
    ;;
  -V)
    printf 'cargo 1.89.0 (fake)\n'
    ;;
esac
SCRIPT

  cat >"${FAKE_BIN_DIR}/codesign" <<'SCRIPT'
#!/usr/bin/env bash
printf 'codesign %s\n' "$*" >>"$FAKE_CALL_LOG"
[ "${1:-}" = "-dv" ] && printf 'Signature=adhoc\n'
exit 0
SCRIPT

  cat >"${FAKE_BIN_DIR}/file" <<'SCRIPT'
#!/usr/bin/env bash
printf 'file %s\n' "$*" >>"$FAKE_CALL_LOG"
printf '%s: Mach-O 64-bit executable arm64\n' "$1"
SCRIPT

  cat >"${FAKE_BIN_DIR}/sw_vers" <<'SCRIPT'
#!/usr/bin/env bash
printf 'ProductName:\t\tmacOS\nProductVersion:\t\t14.0\nBuildVersion:\t\tFAKE0000\n'
SCRIPT

  cat >"${FAKE_BIN_DIR}/rustc" <<'SCRIPT'
#!/usr/bin/env bash
[ "${1:-}" = "-vV" ] && printf 'rustc 1.89.0 (fake)\nhost: aarch64-apple-darwin\n'
SCRIPT

  cat >"${FAKE_BIN_DIR}/uname" <<'SCRIPT'
#!/usr/bin/env bash
if [ "${1:-}" = "-m" ]; then
  printf 'arm64\n'
else
  printf 'FakeOS\n'
fi
SCRIPT

  cat >"${FAKE_BIN_DIR}/mise" <<'SCRIPT'
#!/usr/bin/env bash
printf 'mise %s\n' "$*" >>"$FAKE_CALL_LOG"
exit 0
SCRIPT

  chmod +x "${FAKE_BIN_DIR}"/*
}

# ---------------------------------------------------------------------------
# assertion helper
# ---------------------------------------------------------------------------

assert_eq() {
  local expected="$1" actual="$2" label="$3"
  if [ "$expected" != "$actual" ]; then
    printf 'assertion failed: %s\n  expected: %s\n  actual:   %s\n' "$label" "$expected" "$actual" >&2
    exit 1
  fi
}

assert_contains() {
  local haystack="$1" needle="$2" label="$3"
  case "$haystack" in
    *"$needle"*) ;;
    *)
      printf 'assertion failed: %s\n  expected to contain: %s\n  actual:\n%s\n' "$label" "$needle" "$haystack" >&2
      exit 1
      ;;
  esac
}

assert_not_contains() {
  local haystack="$1" needle="$2" label="$3"
  case "$haystack" in
    *"$needle"*)
      printf 'assertion failed: %s\n  expected NOT to contain: %s\n  actual:\n%s\n' "$label" "$needle" "$haystack" >&2
      exit 1
      ;;
    *) ;;
  esac
}

assert_status() {
  local expected="$1" label="$2"
  assert_eq "$expected" "$RUN_STATUS" "$label (exit status)"
}

# ---------------------------------------------------------------------------
# scratch environment
# ---------------------------------------------------------------------------

default_formula() {
  local url="$1" sha256="$2"
  cat <<EOF
class Sbxm < Formula
  desc "CLI for managing Docker Sandboxes: per-project setup and daily operations"
  homepage "https://github.com/crescware/sbxm"
  url "$url"
  sha256 "$sha256"
  license "MIT"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  def install
    bin.install "sbxm"
  end

  test do
    assert_match "sbxm #{version}", shell_output("#{bin}/sbxm --version")
  end
end
EOF
}

# 64文字の小文字16進数として妥当だが、実archiveとは一致しないplaceholder。
INIT_SHA256="$(printf '0%.0s' $(seq 1 64))"

# version、および既存formulaの内容 (省略時は上のplaceholderを使う) を指定して、
# scratchなsource/tap repository一式を作る。呼び出し元のtest関数のsubshellに
# global変数として結果を残す。
setup_scratch() {
  local version="$1"
  local formula_content="${2:-}"

  SCRATCH="$(mktemp -d "${RUN_TMP}/case.XXXXXX")"
  SOURCE_ORIGIN_DIR="${SCRATCH}/source-origin.git"
  TAP_ORIGIN_DIR="${SCRATCH}/tap-origin.git"
  SOURCE_DIR="${SCRATCH}/source"
  XDG_CACHE_HOME_DIR="${SCRATCH}/cache"
  SCRATCH_HOME="${SCRATCH}/home"
  SCRATCH_TMP="${SCRATCH}/tmp"
  GIT_CONFIG_GLOBAL="${SCRATCH}/gitconfig"
  FAKE_GH_STATE="${SCRATCH}/gh-state"
  FAKE_CALL_LOG="${SCRATCH}/call.log"
  FAKE_SOURCE_REPO_NAME="${FAKE_SOURCE_REPO_NAME:-crescware/sbxm}"
  TAP_CACHE_DIR="${XDG_CACHE_HOME_DIR}/sbxm/release/homebrew-tap"
  EXPECT_TAP_CACHE_DIR="$TAP_CACHE_DIR"
  TAG="v${version}"

  export GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM=1 FAKE_CALL_LOG

  mkdir -p "$SCRATCH_HOME" "$SCRATCH_TMP" "$XDG_CACHE_HOME_DIR" "$FAKE_GH_STATE"
  : >"$FAKE_CALL_LOG"

  cat >"$GIT_CONFIG_GLOBAL" <<EOF
[user]
	name = release-test
	email = release-test@example.invalid
[init]
	defaultBranch = main
[advice]
	detachedHead = false
[url "${TAP_ORIGIN_DIR}"]
	insteadOf = https://github.com/crescware/homebrew-tap.git
EOF

  git init --bare -q -b main "$SOURCE_ORIGIN_DIR"
  git init --bare -q -b main "$TAP_ORIGIN_DIR"

  # 直後にcloneするbare repoはこの時点でまだ空なので、gitのempty repository警告を
  # 抑える (このtest harnessの動作には関係ない)。
  git clone -q "$SOURCE_ORIGIN_DIR" "$SOURCE_DIR" 2>/dev/null
  cat >"${SOURCE_DIR}/Cargo.toml" <<EOF
[package]
name = "sbxm"
version = "${version}"
edition = "2024"
EOF
  (
    cd "$SOURCE_DIR"
    git add Cargo.toml
    git commit -q -m "Set version ${version}"
    git push -q origin main
  )

  local seed_tap="${SCRATCH}/tap-seed"
  git clone -q "$TAP_ORIGIN_DIR" "$seed_tap" 2>/dev/null
  mkdir -p "${seed_tap}/Formula"
  if [ -n "$formula_content" ]; then
    printf '%s\n' "$formula_content" >"${seed_tap}/Formula/sbxm.rb"
  else
    default_formula \
      "https://github.com/${FAKE_SOURCE_REPO_NAME}/releases/download/v0.0.1/${ARCHIVE_NAME}" \
      "$INIT_SHA256" \
      >"${seed_tap}/Formula/sbxm.rb"
  fi
  (
    cd "$seed_tap"
    git add -A
    git commit -q -m "seed formula"
    git push -q origin main
  )
}

run_release() {
  RUN_STATUS=0
  (
    cd "$SOURCE_DIR"
    env -i \
      PATH="${FAKE_BIN_DIR}:/usr/bin:/bin:/usr/local/bin" \
      HOME="$SCRATCH_HOME" \
      TMPDIR="$SCRATCH_TMP" \
      XDG_CACHE_HOME="$XDG_CACHE_HOME_DIR" \
      GIT_CONFIG_GLOBAL="$GIT_CONFIG_GLOBAL" \
      GIT_CONFIG_NOSYSTEM=1 \
      REAL_GIT="$REAL_GIT" \
      FAKE_GH_STATE="$FAKE_GH_STATE" \
      FAKE_CALL_LOG="$FAKE_CALL_LOG" \
      FAKE_SOURCE_REPO_NAME="$FAKE_SOURCE_REPO_NAME" \
      EXPECT_TAP_CACHE_DIR="$EXPECT_TAP_CACHE_DIR" \
      FAKE_GH_UNAUTHENTICATED="${FAKE_GH_UNAUTHENTICATED:-0}" \
      FAKE_GH_UNREACHABLE="${FAKE_GH_UNREACHABLE:-0}" \
      FAKE_GH_RELEASE_CREATE_FAIL="${FAKE_GH_RELEASE_CREATE_FAIL:-0}" \
      FAKE_GH_ASSET_MISMATCH="${FAKE_GH_ASSET_MISMATCH:-0}" \
      FAKE_GH_ASSET_WRONG_NAME="${FAKE_GH_ASSET_WRONG_NAME:-0}" \
      FAKE_GH_ASSET_WRONG_STATE="${FAKE_GH_ASSET_WRONG_STATE:-0}" \
      FAKE_GH_PR_CREATE_FAIL="${FAKE_GH_PR_CREATE_FAIL:-0}" \
      FAKE_TAP_FETCH_FAIL="${FAKE_TAP_FETCH_FAIL:-0}" \
      FAKE_TAP_COMMIT_FAIL="${FAKE_TAP_COMMIT_FAIL:-0}" \
      FAKE_TAP_PUSH_FAIL="${FAKE_TAP_PUSH_FAIL:-0}" \
      "$RELEASE_SCRIPT" "$@" \
      >"${SCRATCH}/stdout.log" 2>"${SCRATCH}/stderr.log"
  ) || RUN_STATUS=$?
  RUN_STDERR="$(cat "${SCRATCH}/stderr.log")"
}

# release.sh自身のclone手順 (`gh repo clone`) と同じ形でcacheを用意する。origin urlは
# https://github.com/crescware/homebrew-tap.gitのまま保ち、実際のtransportだけを
# GIT_CONFIG_GLOBALのinsteadOfでtap originへ書き換える。tap_origin_matchesが読む値を
# release.shが実際に作るcacheと揃えるため、単純なgit cloneではなくこの手順を使う。
seed_tap_cache_like_release() {
  local dest="$1"
  mkdir -p "$dest"
  git init -q -b main "$dest"
  git -C "$dest" remote add origin "https://github.com/crescware/homebrew-tap.git"
  git -C "$dest" fetch -q origin
  git -C "$dest" checkout -q -B main origin/main
}

tap_branch_exists_on_origin() {
  [ -n "$(git -C "$TAP_ORIGIN_DIR" branch --list "$1")" ]
}

tap_origin_main_commit() {
  git -C "$TAP_ORIGIN_DIR" rev-parse main
}

# ---------------------------------------------------------------------------
# 正常系
# ---------------------------------------------------------------------------

test_fresh_clone_creates_pr() {
  setup_scratch "0.0.2"
  local main_before
  main_before="$(tap_origin_main_commit)"

  run_release --stable "$TAG"
  assert_status 0 "fresh clone: release.sh"

  local branch="chore/bump-sbxm-to-${TAG}"
  tap_branch_exists_on_origin "$branch" \
    || { printf 'expected tap branch %s on origin\n' "$branch" >&2; exit 1; }
  assert_eq "$main_before" "$(tap_origin_main_commit)" "fresh clone: tap main is unchanged"

  # branchにはformulaだけを変更したcommitが1つ増えている。
  local commit_count
  commit_count="$(git -C "$TAP_ORIGIN_DIR" rev-list --count "${main_before}..${branch}")"
  assert_eq "1" "$commit_count" "fresh clone: exactly one commit on the tap branch"
  local changed_files
  changed_files="$(git -C "$TAP_ORIGIN_DIR" diff --name-only "$main_before" "$branch")"
  assert_eq "Formula/sbxm.rb" "$changed_files" "fresh clone: only the formula changed"

  # notesのchecksumとformulaのsha256、fake ghが記録したdigestがすべて一致する。
  local archive_sha256
  archive_sha256="$(awk '{print $1}' "${SCRATCH}/source/dist/release-notes.md" | grep -E '^[0-9a-f]{64}$')"
  local formula_sha256
  formula_sha256="$(git -C "$TAP_ORIGIN_DIR" show "${branch}:Formula/sbxm.rb" | sed -n 's/^  sha256 "\(.*\)"$/\1/p')"
  assert_eq "$archive_sha256" "$formula_sha256" "fresh clone: notes checksum matches formula sha256"

  local formula_url
  formula_url="$(git -C "$TAP_ORIGIN_DIR" show "${branch}:Formula/sbxm.rb" | sed -n 's/^  url "\(.*\)"$/\1/p')"
  assert_eq "https://github.com/crescware/sbxm/releases/download/${TAG}/${ARCHIVE_NAME}" "$formula_url" \
    "fresh clone: formula url matches the published Release asset"

  # gh pr createがすべての値を明示的に渡している。
  local pr_call
  pr_call="$(cat "${FAKE_GH_STATE}/pr-create-calls.log")"
  assert_contains "$pr_call" "--repo crescware/homebrew-tap" "fresh clone: gh pr create --repo"
  assert_contains "$pr_call" "--base main" "fresh clone: gh pr create --base"
  assert_contains "$pr_call" "--head ${branch}" "fresh clone: gh pr create --head"
  assert_contains "$pr_call" "--title Bump sbxm to 0.0.2" "fresh clone: gh pr create --title"
  assert_contains "$pr_call" "--body-file" "fresh clone: gh pr create --body-file"

  # 実行順: Release作成 -> asset検証 -> tap checkout(clone) -> push -> PR作成。
  local log
  log="$(cat "$FAKE_CALL_LOG")"
  local i_create i_assets i_clone i_push i_pr
  i_create="$(grep -n '^gh release create' "$FAKE_CALL_LOG" | head -1 | cut -d: -f1)"
  i_assets="$(grep -n -- '--json assets' "$FAKE_CALL_LOG" | head -1 | cut -d: -f1)"
  i_clone="$(grep -n '^gh repo clone' "$FAKE_CALL_LOG" | head -1 | cut -d: -f1)"
  i_push="$(grep -n -- "push origin HEAD:refs/heads/${branch}" "$FAKE_CALL_LOG" | head -1 | cut -d: -f1)"
  i_pr="$(grep -n '^gh pr create' "$FAKE_CALL_LOG" | head -1 | cut -d: -f1)"
  [ "$i_create" -lt "$i_assets" ] || { printf 'expected release create before asset verification\n%s\n' "$log" >&2; exit 1; }
  [ "$i_assets" -lt "$i_clone" ] || { printf 'expected asset verification before tap clone\n%s\n' "$log" >&2; exit 1; }
  [ "$i_clone" -lt "$i_push" ] || { printf 'expected tap clone before tap push\n%s\n' "$log" >&2; exit 1; }
  [ "$i_push" -lt "$i_pr" ] || { printf 'expected tap push before PR creation\n%s\n' "$log" >&2; exit 1; }

  # mise run checkは1回だけ呼ばれ、release-test自身への再帰は起きない。
  local mise_calls
  mise_calls="$(grep -c '^mise run check' "$FAKE_CALL_LOG" || true)"
  assert_eq "1" "$mise_calls" "fresh clone: mise run check called exactly once"
}

test_existing_cache_fetches_and_uses_updated_main() {
  setup_scratch "0.0.3"

  # 前回のreleaseで残ったcacheを模す: 別branchをcheckoutした状態でcloneしておく。
  seed_tap_cache_like_release "$TAP_CACHE_DIR"
  (
    cd "$TAP_CACHE_DIR"
    git checkout -q -b chore/bump-sbxm-to-v0.0.2
  )

  # 既存cacheをcloneした後にtap originのmainを進める。fetchしなければ
  # 古いorigin/mainのままになる。
  local seed_tap="${SCRATCH}/tap-seed-2"
  git clone -q "$TAP_ORIGIN_DIR" "$seed_tap"
  (
    cd "$seed_tap"
    printf '# advanced\n' >>Formula/sbxm.rb
    git add -A
    git commit -q -m "advance main"
    git push -q origin main
  )
  local advanced_main
  advanced_main="$(tap_origin_main_commit)"

  run_release --stable "$TAG"
  assert_status 0 "existing cache: release.sh"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh repo clone" "existing cache: gh repo clone is not called"
  assert_contains "$log" "fetch --prune origin main" "existing cache: origin main is fetched"

  local branch="chore/bump-sbxm-to-${TAG}"
  local base_commit
  base_commit="$(git -C "$TAP_ORIGIN_DIR" merge-base main "$branch")"
  assert_eq "$advanced_main" "$base_commit" \
    "existing cache: new branch is based on the freshly fetched origin/main, not the stale local main"
}

# ---------------------------------------------------------------------------
# Dry run
# ---------------------------------------------------------------------------

test_dry_run_official_repo_reports_plan_without_touching_tap() {
  setup_scratch "0.0.4"

  local cache_before
  cache_before="$( [ -e "$TAP_CACHE_DIR" ] && echo present || echo absent )"
  local main_before
  main_before="$(tap_origin_main_commit)"
  local refs_before
  refs_before="$(git -C "$TAP_ORIGIN_DIR" for-each-ref --format='%(refname) %(objectname)')"

  run_release --dry-run --stable "$TAG"
  assert_status 0 "dry run: release.sh"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh release create" "dry run: gh release create is not called"
  assert_not_contains "$log" "gh repo clone" "dry run: gh repo clone is not called"
  assert_not_contains "$log" "fetch --prune origin main" "dry run: tap main is not fetched"
  assert_not_contains "$log" "gh pr create" "dry run: gh pr create is not called"

  local cache_after
  cache_after="$( [ -e "$TAP_CACHE_DIR" ] && echo present || echo absent )"
  assert_eq "$cache_before" "$cache_after" "dry run: tap cache existence is unchanged"
  assert_eq "$main_before" "$(tap_origin_main_commit)" "dry run: tap main is unchanged"
  assert_eq "$refs_before" "$(git -C "$TAP_ORIGIN_DIR" for-each-ref --format='%(refname) %(objectname)')" \
    "dry run: tap origin refs are unchanged"

  assert_contains "$RUN_STDERR" "tap update plan (crescware/homebrew-tap):" "dry run: report has a tap plan section"
  assert_contains "$RUN_STDERR" "https://github.com/crescware/sbxm/releases/download/${TAG}/${ARCHIVE_NAME}" \
    "dry run: report shows the planned asset url"
  assert_contains "$RUN_STDERR" "chore/bump-sbxm-to-${TAG}" "dry run: report shows the planned branch name"
  assert_contains "$RUN_STDERR" "Formula/sbxm.rb" "dry run: report shows the formula path"
  assert_contains "$RUN_STDERR" "Bump sbxm to 0.0.4" "dry run: report shows the commit message"
  assert_contains "$RUN_STDERR" "gh pr create --repo crescware/homebrew-tap --base main --head chore/bump-sbxm-to-${TAG}" \
    "dry run: report shows the exact gh pr create command"

  local archive_sha256
  archive_sha256="$(awk '{print $1}' "${SCRATCH}/source/dist/release-notes.md" | grep -E '^[0-9a-f]{64}$')"
  assert_contains "$RUN_STDERR" "$archive_sha256" "dry run: report shows the real archive sha256"
}

test_dry_run_fork_skips_tap_with_reason() {
  FAKE_SOURCE_REPO_NAME="someone-else/sbxm"
  setup_scratch "0.0.5"

  run_release --dry-run --stable "$TAG"
  assert_status 0 "fork dry run: release.sh"

  assert_contains "$RUN_STDERR" "tap update: skipped" "fork dry run: tap update is reported as skipped"
  assert_contains "$RUN_STDERR" "someone-else/sbxm" "fork dry run: skip reason names the actual source repository"
  assert_not_contains "$RUN_STDERR" "gh pr create --repo crescware/homebrew-tap" \
    "fork dry run: no tap pr create command is shown"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh repo clone" "fork dry run: gh repo clone is not called"
}

# ---------------------------------------------------------------------------
# Release境界
# ---------------------------------------------------------------------------

test_release_create_failure_skips_tap_entirely() {
  setup_scratch "0.0.6"
  FAKE_GH_RELEASE_CREATE_FAIL=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'expected a non-zero exit status\n' >&2; exit 1; }

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "--json assets" "release create failure: asset verification never runs"
  assert_not_contains "$log" "gh repo clone" "release create failure: tap is never cloned"
  assert_not_contains "$log" "gh pr create" "release create failure: no PR is created"

  # tagはpush済みのまま残る (release.shの既存挙動)。
  git -C "$SOURCE_ORIGIN_DIR" rev-parse -q --verify "refs/tags/${TAG}" >/dev/null \
    || { printf 'expected the tag to remain on the source origin\n' >&2; exit 1; }
}

test_asset_mismatch_stops_before_tap_checkout() {
  setup_scratch "0.0.7"
  FAKE_GH_ASSET_MISMATCH=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'expected a non-zero exit status\n' >&2; exit 1; }
  assert_contains "$RUN_STDERR" "digest" "asset mismatch: error mentions the digest"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh repo clone" "asset mismatch: tap is never cloned"
  assert_not_contains "$log" "gh pr create" "asset mismatch: no PR is created"

  # ReleaseとtagはRelease境界の失敗として残る。
  git -C "$SOURCE_ORIGIN_DIR" rev-parse -q --verify "refs/tags/${TAG}" >/dev/null \
    || { printf 'expected the tag to remain on the source origin\n' >&2; exit 1; }
}

test_asset_name_mismatch_stops_before_tap_checkout() {
  setup_scratch "0.0.21"
  FAKE_GH_ASSET_WRONG_NAME=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'expected a non-zero exit status\n' >&2; exit 1; }
  assert_contains "$RUN_STDERR" "asset(s) named ${ARCHIVE_NAME}" "asset name mismatch: error names the expected asset"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh repo clone" "asset name mismatch: tap is never cloned"
  assert_not_contains "$log" "gh pr create" "asset name mismatch: no PR is created"
}

test_asset_wrong_state_stops_before_tap_checkout() {
  setup_scratch "0.0.22"
  FAKE_GH_ASSET_WRONG_STATE=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'expected a non-zero exit status\n' >&2; exit 1; }
  assert_contains "$RUN_STDERR" "has state 'pending', expected 'uploaded'" "wrong state: error names the actual state"

  local log
  log="$(cat "$FAKE_CALL_LOG")"
  assert_not_contains "$log" "gh repo clone" "wrong state: tap is never cloned"
  assert_not_contains "$log" "gh pr create" "wrong state: no PR is created"
}

# ---------------------------------------------------------------------------
# Formula guard
# ---------------------------------------------------------------------------

assert_tap_untouched_after_failure() {
  local label="$1"
  [ "$RUN_STATUS" -ne 0 ] || { printf '%s: expected a non-zero exit status\n' "$label" >&2; exit 1; }
  local branches
  branches="$(git -C "$TAP_ORIGIN_DIR" branch --list 'chore/bump-sbxm-to-*')"
  assert_eq "" "$branches" "${label}: no tap branch was pushed"
  [ ! -f "${FAKE_GH_STATE}/pr-create-calls.log" ] \
    || { printf '%s: expected no gh pr create call\n' "$label" >&2; exit 1; }
}

test_formula_missing_url_line_rejected() {
  local broken
  broken="$(cat <<EOF
class Sbxm < Formula
  sha256 "${INIT_SHA256}"
end
EOF
)"
  setup_scratch "0.0.8" "$broken"
  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "missing url"
  assert_contains "$RUN_STDERR" "'url' line" "missing url: error names the missing url line"
}

test_formula_duplicate_sha256_line_rejected() {
  local broken
  broken="$(cat <<EOF
class Sbxm < Formula
  url "https://github.com/crescware/sbxm/releases/download/v0.0.1/${ARCHIVE_NAME}"
  sha256 "${INIT_SHA256}"
  sha256 "${INIT_SHA256}"
end
EOF
)"
  setup_scratch "0.0.9" "$broken"
  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "duplicate sha256"
  assert_contains "$RUN_STDERR" "'sha256' line" "duplicate sha256: error names the duplicated sha256 line"
}

test_formula_malformed_existing_sha256_rejected() {
  local broken
  broken="$(cat <<EOF
class Sbxm < Formula
  url "https://github.com/crescware/sbxm/releases/download/v0.0.1/${ARCHIVE_NAME}"
  sha256 "not-a-real-checksum"
end
EOF
)"
  setup_scratch "0.0.10" "$broken"
  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "malformed sha256"
  assert_contains "$RUN_STDERR" "64 lowercase hex" "malformed sha256: error names the format requirement"
}

test_formula_wrong_asset_basename_rejected() {
  local broken
  broken="$(cat <<EOF
class Sbxm < Formula
  url "https://github.com/crescware/sbxm/releases/download/v0.0.1/some-other-tool.tar.gz"
  sha256 "${INIT_SHA256}"
end
EOF
)"
  setup_scratch "0.0.11" "$broken"
  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "wrong asset basename"
  assert_contains "$RUN_STDERR" "does not reference" "wrong asset basename: error explains the mismatch"
}

# ---------------------------------------------------------------------------
# Git・GitHubの失敗
# ---------------------------------------------------------------------------

test_cache_non_git_directory_untouched() {
  setup_scratch "0.0.12"
  mkdir -p "$TAP_CACHE_DIR"
  printf 'not a git repository\n' >"${TAP_CACHE_DIR}/marker.txt"

  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "non-git cache"
  assert_eq "not a git repository" "$(cat "${TAP_CACHE_DIR}/marker.txt")" "non-git cache: existing file is untouched"
}

test_cache_wrong_origin_untouched() {
  setup_scratch "0.0.13"
  local other_bare="${SCRATCH}/unrelated-origin.git"
  git init --bare -q -b main "$other_bare"
  git clone -q "$other_bare" "$TAP_CACHE_DIR" 2>/dev/null

  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "wrong origin cache"
  assert_contains "$RUN_STDERR" "is not crescware/homebrew-tap" "wrong origin cache: error names the mismatch"
}

test_cache_dirty_untouched() {
  setup_scratch "0.0.14"
  seed_tap_cache_like_release "$TAP_CACHE_DIR"
  printf 'dirty\n' >>"${TAP_CACHE_DIR}/Formula/sbxm.rb"

  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "dirty cache"
  assert_contains "$(git -C "$TAP_CACHE_DIR" status --porcelain)" "Formula/sbxm.rb" "dirty cache: dirty state is preserved"
}

test_local_branch_collision_untouched() {
  setup_scratch "0.0.15"
  seed_tap_cache_like_release "$TAP_CACHE_DIR"
  (
    cd "$TAP_CACHE_DIR"
    git checkout -q -b "chore/bump-sbxm-to-${TAG}"
    git commit -q --allow-empty -m "pre-existing local branch"
  )

  run_release --stable "$TAG"
  assert_tap_untouched_after_failure_allow_local_branch "local branch collision"
  assert_contains "$RUN_STDERR" "already exists locally" "local branch collision: error explains the collision"
}

# 衝突testでは、衝突しているbranch自身が既にorigin上には存在しないため、
# assert_tap_untouched_after_failureをそのまま使うと矛盾する。localのみを見る。
assert_tap_untouched_after_failure_allow_local_branch() {
  local label="$1"
  [ "$RUN_STATUS" -ne 0 ] || { printf '%s: expected a non-zero exit status\n' "$label" >&2; exit 1; }
  [ ! -f "${FAKE_GH_STATE}/pr-create-calls.log" ] \
    || { printf '%s: expected no gh pr create call\n' "$label" >&2; exit 1; }
}

test_remote_branch_collision_untouched() {
  setup_scratch "0.0.16"
  local seed="${SCRATCH}/tap-seed-collision"
  git clone -q "$TAP_ORIGIN_DIR" "$seed"
  (
    cd "$seed"
    git checkout -q -b "chore/bump-sbxm-to-${TAG}"
    git commit -q --allow-empty -m "pre-existing remote branch"
    git push -q origin "chore/bump-sbxm-to-${TAG}"
  )

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'remote branch collision: expected a non-zero exit status\n' >&2; exit 1; }
  [ ! -f "${FAKE_GH_STATE}/pr-create-calls.log" ] \
    || { printf 'remote branch collision: expected no gh pr create call\n' >&2; exit 1; }
  assert_contains "$RUN_STDERR" "already exists on crescware/homebrew-tap" \
    "remote branch collision: error explains the collision"
}

test_tap_fetch_failure_recovery_message() {
  setup_scratch "0.0.17"
  seed_tap_cache_like_release "$TAP_CACHE_DIR"
  FAKE_TAP_FETCH_FAIL=1

  run_release --stable "$TAG"
  assert_tap_untouched_after_failure "fetch failure"
  assert_contains "$RUN_STDERR" "could not fetch origin's main branch" "fetch failure: error names the failed step"
}

test_tap_commit_failure_recovery_message() {
  setup_scratch "0.0.18"
  FAKE_TAP_COMMIT_FAIL=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'commit failure: expected a non-zero exit status\n' >&2; exit 1; }
  assert_contains "$RUN_STDERR" "could not commit Formula/sbxm.rb" "commit failure: error names the failed step"
  assert_contains "$RUN_STDERR" "$TAP_CACHE_DIR" "commit failure: error names the cache path"
  [ ! -f "${FAKE_GH_STATE}/pr-create-calls.log" ] \
    || { printf 'commit failure: expected no gh pr create call\n' >&2; exit 1; }
}

test_tap_push_failure_recovery_message() {
  setup_scratch "0.0.19"
  FAKE_TAP_PUSH_FAIL=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'push failure: expected a non-zero exit status\n' >&2; exit 1; }
  local branch="chore/bump-sbxm-to-${TAG}"
  assert_contains "$RUN_STDERR" "could not push ${branch}" "push failure: error names the failed step"
  assert_contains "$RUN_STDERR" "git -C ${TAP_CACHE_DIR} push origin HEAD:refs/heads/${branch}" \
    "push failure: error shows the exact retry command"
  [ ! -f "${FAKE_GH_STATE}/pr-create-calls.log" ] \
    || { printf 'push failure: expected no gh pr create call\n' >&2; exit 1; }
  # remote branchは (このscriptの意図どおり) 通常pushできていないので存在しない。
  local branches
  branches="$(git -C "$TAP_ORIGIN_DIR" branch --list "$branch")"
  assert_eq "" "$branches" "push failure: no branch reached the tap origin"
}

test_tap_pr_create_failure_leaves_branch_and_shows_retry_command() {
  setup_scratch "0.0.20"
  FAKE_GH_PR_CREATE_FAIL=1

  run_release --stable "$TAG"
  [ "$RUN_STATUS" -ne 0 ] || { printf 'pr create failure: expected a non-zero exit status\n' >&2; exit 1; }

  local branch="chore/bump-sbxm-to-${TAG}"
  tap_branch_exists_on_origin "$branch" \
    || { printf 'pr create failure: expected the branch to remain on the tap origin\n' >&2; exit 1; }

  assert_contains "$RUN_STDERR" "could not create the tap PR" "pr create failure: error names the failed step"
  assert_contains "$RUN_STDERR" "--repo crescware/homebrew-tap" "pr create failure: retry command has --repo"
  assert_contains "$RUN_STDERR" "--base main" "pr create failure: retry command has --base"
  assert_contains "$RUN_STDERR" "--head ${branch}" "pr create failure: retry command has --head"
  assert_contains "$RUN_STDERR" "--body-file" "pr create failure: retry command has --body-file"
}

# ---------------------------------------------------------------------------
# runner
# ---------------------------------------------------------------------------

run_case() {
  local name="$1"
  TESTS_RUN=$((TESTS_RUN + 1))
  printf -- '--- %s\n' "$name"
  # 各test関数は毎回mainからforkされる新しいsubshellで動く。FAKE_GH_*/FAKE_TAP_*の
  # toggleはtest関数の中でだけ設定されるため、他のtestへ漏れない。
  if ( set -e; "$name" ); then
    printf 'ok\n'
  else
    TESTS_FAILED=$((TESTS_FAILED + 1))
    FAILED_NAMES="${FAILED_NAMES}  - ${name}"$'\n'
    printf 'FAIL\n'
  fi
}

main() {
  bash -n "$RELEASE_SCRIPT"
  write_fake_bin

  run_case test_fresh_clone_creates_pr
  run_case test_existing_cache_fetches_and_uses_updated_main

  run_case test_dry_run_official_repo_reports_plan_without_touching_tap
  run_case test_dry_run_fork_skips_tap_with_reason

  run_case test_release_create_failure_skips_tap_entirely
  run_case test_asset_mismatch_stops_before_tap_checkout
  run_case test_asset_name_mismatch_stops_before_tap_checkout
  run_case test_asset_wrong_state_stops_before_tap_checkout

  run_case test_formula_missing_url_line_rejected
  run_case test_formula_duplicate_sha256_line_rejected
  run_case test_formula_malformed_existing_sha256_rejected
  run_case test_formula_wrong_asset_basename_rejected

  run_case test_cache_non_git_directory_untouched
  run_case test_cache_wrong_origin_untouched
  run_case test_cache_dirty_untouched
  run_case test_local_branch_collision_untouched
  run_case test_remote_branch_collision_untouched
  run_case test_tap_fetch_failure_recovery_message
  run_case test_tap_commit_failure_recovery_message
  run_case test_tap_push_failure_recovery_message
  run_case test_tap_pr_create_failure_leaves_branch_and_shows_retry_command

  printf '\n%d/%d tests passed\n' "$((TESTS_RUN - TESTS_FAILED))" "$TESTS_RUN"
  if [ "$TESTS_FAILED" -gt 0 ]; then
    printf 'failed:\n%s' "$FAILED_NAMES"
    exit 1
  fi
}

main "$@"
