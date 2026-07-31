use crate::command::HostEnvironment;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::git;
use crate::msg;

use crate::support::sandbox;

/// `git ls-remote --symref origin HEAD`が示すdefault branch。
pub fn remote_default_branch(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
) -> Result<String> {
    let output = sandbox::read(
        host,
        sandbox,
        &[
            "git",
            "--git-dir",
            git_dir,
            "ls-remote",
            "--symref",
            "origin",
            "HEAD",
        ],
    )?;

    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix("ref:") else {
            continue;
        };
        let Some(reference) = rest.split_whitespace().next() else {
            continue;
        };
        if let Some(branch) = reference.strip_prefix("refs/heads/")
            && git::validate_branch_name(branch).is_ok()
        {
            return Ok(branch.to_string());
        }
    }

    Err(Error::new(
        ErrorId::ExternalOutputUnparseable,
        msg!(
            "error-external-output-unparseable",
            program = "git ls-remote --symref",
            detail = "no branch was reported for HEAD"
        ),
    ))
}
