/// 保存元は変更せず、branch・tagまたはworktree HEADがあるかだけを読む。
/// 名称は既存の観測用adapterとの互換のため維持する。
pub(crate) const PLACE_SAVE_REFS: &str = r#"set -eu
export GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1
git_dir=$1
git() { command git --git-dir "$git_dir" "$@"; }
worktrees=$(git worktree list --porcelain)
listed=$(git for-each-ref --count=1 --format='%(refname)' refs/heads/ refs/tags/)
while IFS= read -r line; do
  case "$line" in
    "HEAD "*) listed=ready ;;
  esac
done <<EOF
$worktrees
EOF
if [ -z "$listed" ]; then
  echo empty
else
  echo ready
fi"#;
