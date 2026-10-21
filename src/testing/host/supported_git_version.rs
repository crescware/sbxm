use crate::support::git_cli::MINIMUM_HOST_GIT;

/// `git --version`へ、このbuildが求める最低versionのgitが返す出力。
///
/// 最低versionを上げても、fakeの答えはここだけが追従する。
pub fn supported_git_version() -> String {
    format!("git version {MINIMUM_HOST_GIT}\n")
}
