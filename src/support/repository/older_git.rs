use std::path::Path;

use crate::boundary::host::protocol::CliVersion;
use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use super::host_git;

/// hostのgitに求める最小version。hostで使ういちばん新しい機能は、2.36からの
/// `git worktree list -z`である。
const MINIMUM_HOST_GIT: CliVersion = CliVersion {
    major: 2,
    minor: 36,
    patch: 0,
};

/// hostのgitが`MINIMUM_HOST_GIT`より古ければ、そのことを示すerror。
///
/// gitが失敗したあとに、原因を示すためだけに読む。成功する経路ではversionを読まない。
/// versionを読めないか、古くなければ`None`を返す。呼び出し側は元の失敗を示す。
pub fn older_git(host: &dyn HostEnvironment, directory: &Path) -> Option<Error> {
    let outcome = host_git(host, directory, &["--version"], None, TimeoutClass::Probe).ok()?;
    let observed = CliVersion::extract_from_output(&outcome.stdout_text())?;
    (observed < MINIMUM_HOST_GIT).then(|| {
        Error::single(
            Diagnostic::new(
                ErrorId::HostGitTooOld,
                msg!(
                    "error-host-git-too-old",
                    observed = observed,
                    minimum = MINIMUM_HOST_GIT
                ),
            )
            .remediation(msg!("remediation-host-git-too-old")),
        )
    })
}

#[cfg(test)]
#[path = "older_git_test.rs"]
mod older_git_test;
