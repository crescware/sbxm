use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::git;
use crate::metadata::CreationMode;

use crate::support::sandbox;

pub fn create_worktree(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    path: &str,
    branch: &str,
    mode: CreationMode,
) -> Result<()> {
    let reference = git::origin_ref(branch);
    let arguments: Vec<&str> = match mode {
        CreationMode::Attached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--track",
            "-b",
            branch,
            path,
            &reference,
        ],
        CreationMode::Detached => vec![
            "git",
            "--git-dir",
            git_dir,
            "worktree",
            "add",
            "--detach",
            path,
            &reference,
        ],
    };
    sandbox::exec(host, sandbox, &arguments)?.require_success()?;
    Ok(())
}
