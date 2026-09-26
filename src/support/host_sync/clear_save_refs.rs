/// Sandboxの中で、`PLACE_SAVE_REFS`が置いた一時refを消す手順。引数は`$1`がbare
/// repositoryのgit directory。
pub(super) const CLEAR_SAVE_REFS: &str = r#"set -eu
stale=$(command git --git-dir "$1" for-each-ref --format='delete %(refname)' refs/sbxm/save/)
[ -z "$stale" ] || printf '%s\n' "$stale" | command git --git-dir "$1" update-ref --stdin"#;
