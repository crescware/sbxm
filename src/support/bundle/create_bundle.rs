/// Sandboxの中で、bare repositoryのbranch、tag、各worktreeのHEADを1つのbundleにして
/// stdoutへ書く手順。引数は`$1`がbare repositoryのgit directory。
///
/// worktreeのHEADはbranchに載っていないcommitを指しうる。`refs/sbxm/save/`へ一時refとして
/// 置いてからbundleへ含め、成否にかかわらず消す。signalで止められても消す。名前がref
/// として使えないworktreeや、別の場所にある同じ名前のworktreeは通し番号で呼ぶ。
/// refが1つも無ければ何も書かずに終わる。
///
/// gitの失敗は、pipeや`[ ]`の中で読み落とさず、その場で止める。読み落とすと、
/// worktreeのHEADを欠いたbundleや、保存するものが無いという答えになる。
pub(super) const CREATE_BUNDLE: &str = r#"set -eu
git_dir=$1
save=refs/sbxm/save
git() { command git --git-dir "$git_dir" "$@"; }
clear() {
  stale=$(git for-each-ref --format='delete %(refname)' "$save/")
  [ -z "$stale" ] || printf '%s\n' "$stale" | git update-ref --stdin
}
clear
trap clear EXIT
trap 'exit 143' HUP INT TERM
worktrees=$(git worktree list --porcelain)
n=0
while IFS= read -r line; do
  case "$line" in
    "worktree "*) path=${line#worktree } ;;
    "HEAD "*)
      n=$((n + 1))
      head=${line#HEAD }
      ref="$save/${path##*/}"
      git check-ref-format "$ref" || ref="$save/worktree-$n"
      git update-ref "$ref" "$head" "" 2>/dev/null ||
        git update-ref "$save/worktree-$n" "$head" ""
      ;;
  esac
done <<EOF
$worktrees
EOF
refs=$(git for-each-ref --count=1 refs/heads/ refs/tags/ "$save/")
if [ -z "$refs" ]; then
  exit 0
fi
git bundle create --quiet - --branches --tags --glob="$save/*""#;
