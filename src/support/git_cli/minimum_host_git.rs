use crate::boundary::host::protocol::CliVersion;

/// hostのgitに求める最小version。hostで使ういちばん新しい機能は、2.36からの
/// `git worktree list -z`である。
pub const MINIMUM_HOST_GIT: CliVersion = CliVersion {
    major: 2,
    minor: 36,
    patch: 0,
};
