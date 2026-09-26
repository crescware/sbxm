/// Sandboxの中で、各worktreeの`HEAD`を一時ref（`refs/sbxm/save/`）へ置く手順。引数は
/// `$1`がbare repositoryのgit directory。
///
/// worktreeの`HEAD`はbranchに載っていないcommitを指しうる。hostがfetchで読めるよう、
/// 一時refとして置く。前の保存が途中で止まって残した一時refは、先に消す。名前がref
/// として使えないworktreeや、別の場所にある同じ名前のworktreeは通し番号で呼ぶ。
///
/// 保存するrefが1つも無ければ`empty`、あれば`ready`とだけ書く。何も書かない答えを、
/// 保存するものが無いとは読まない。gitの失敗は、pipeや`[ ]`の中で読み落とさず、その場で
/// 止める。読み落とすと、worktreeのHEADを欠いた保存や、保存するものが無いという答えになる。
pub(crate) const PLACE_SAVE_REFS: &str = r#"set -eu
git_dir=$1
save=refs/sbxm/save
git() { command git --git-dir "$git_dir" "$@"; }
stale=$(git for-each-ref --format='delete %(refname)' "$save/")
[ -z "$stale" ] || printf '%s\n' "$stale" | git update-ref --stdin
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
listed=$(git for-each-ref --count=1 --format='%(refname)' refs/heads/ refs/tags/ "$save/")
if [ -z "$listed" ]; then
  echo empty
else
  echo ready
fi"#;
