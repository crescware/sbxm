use crate::boundary::host::HostEnvironment;
use crate::project::SandboxName;

use crate::support::sandbox;

use crate::commands::status::project::{ProjectStatus, Value};

/// 作業中の変更があるか。submoduleの変更も`git status`が示すとおりに扱う。
pub fn worktree_state(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    path: &str,
    status: &mut ProjectStatus,
) -> Value {
    match sandbox::exec(
        host,
        name.as_str(),
        &[
            "git",
            "-C",
            path,
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=all",
        ],
    ) {
        Ok(outcome) if outcome.success() => {
            if outcome
                .stdout_text()
                .trim_matches(['\0', '\n', ' '])
                .is_empty()
            {
                Value::Clean
            } else {
                Value::Dirty
            }
        }
        Ok(outcome) => {
            status.diagnostics.extend(
                sandbox::unobservable(&outcome, path)
                    .diagnostics()
                    .iter()
                    .cloned(),
            );
            Value::NotObserved
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::NotObserved
        }
    }
}
