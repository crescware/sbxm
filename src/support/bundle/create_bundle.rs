/// Sandboxの中で、bare repositoryのbranch、tag、各worktreeのHEADを1つのbundleにして
/// stdoutへ書く手順。引数は`$1`がbare repositoryのgit directory。
///
/// worktreeのHEADはbranchに載っていないcommitを指しうる。`refs/sbxm/save/`へ一時refとして
/// 置いてからbundleへ含め、成否にかかわらず消す。名前がrefとして使えないworktreeは
/// 通し番号で呼ぶ。refが1つも無ければ何も書かずに終わる。
pub(super) const CREATE_BUNDLE: &str = r#"set -eu
git_dir=$1
save=refs/sbxm/save
git() { command git --git-dir "$git_dir" "$@"; }
clear() { git for-each-ref --format='delete %(refname)' "$save/" | git update-ref --stdin; }
clear
trap clear EXIT
n=0
git worktree list --porcelain | while IFS= read -r line; do
  case "$line" in
    "worktree "*) path=${line#worktree } ;;
    "HEAD "*)
      n=$((n + 1))
      ref="$save/${path##*/}"
      git check-ref-format "$ref" || ref="$save/worktree-$n"
      git update-ref "$ref" "${line#HEAD }"
      ;;
  esac
done
if [ -z "$(git for-each-ref --count=1 refs/heads/ refs/tags/ "$save/")" ]; then
  exit 0
fi
git bundle create --quiet - --branches --tags --glob="$save/*""#;
