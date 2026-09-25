use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxLayout;
use crate::support::sandbox;

/// Sandboxのbare repositoryが、branchを1本でも持つか。
///
/// 読めなければ、持たないとは扱わずに失敗する。
pub fn has_local_branches(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    layout: &SandboxLayout,
) -> Result<bool> {
    let git_dir = layout.bare_git_dir();
    let listed = sandbox::exec(
        host,
        sandbox_name,
        &[
            "git",
            "--git-dir",
            &git_dir,
            "for-each-ref",
            "--count=1",
            "--format=%(refname)",
            "refs/heads/",
        ],
    )?
    .require_success()?;
    Ok(!listed.stdout_text().trim().is_empty())
}
