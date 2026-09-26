use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::support::repository::host_git;

/// hostの`repository`に、Sandboxへ送るbranchかtagがあることを確かめる。
///
/// branchもtagも無いrepositoryからは、Sandboxのoriginへ送るものが無い。理由を名指しして断る。
/// 無くなったrepositoryも、ここで名指しして断る。
pub fn require_something_to_send(host: &dyn HostEnvironment, repository: &Path) -> Result<()> {
    let listed = host_git(
        host,
        repository,
        &[
            "for-each-ref",
            "--count=1",
            "--format=%(refname)",
            "refs/heads/",
            "refs/tags/",
        ],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?;
    if listed.stdout_text().trim().is_empty() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::HostRepositoryEmpty,
                msg!(
                    "error-host-repository-empty",
                    repository = paths::display(repository)
                ),
            )
            .remediation(msg!("remediation-host-repository-empty")),
        ));
    }
    Ok(())
}
