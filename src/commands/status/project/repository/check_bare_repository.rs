use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::sandbox;

use crate::commands::status::project::{ProjectStatus, Value};

pub fn check_bare_repository(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    layout: &SandboxLayout,
    status: &mut ProjectStatus,
    progress: &mut dyn ProgressSink,
) {
    let git_dir = layout.bare_git_dir();
    let outcome = crate::support::repository::inspect_bare(host, name.as_str(), &git_dir, progress);
    let value = match outcome {
        Ok(outcome) => match sandbox::inner_exit_code(&outcome) {
            Some(0) if outcome.stdout_text().trim() == "true" => Value::Ready,
            Some(0) => {
                status.diagnostics.push(
                    Diagnostic::new(
                        ErrorId::SandboxRepositoryUnusable,
                        msg!("error-sandbox-repository-unusable"),
                    )
                    .fact(Fact::path(&git_dir))
                    .fact(Fact::reason(msg!("cause-not-bare-repository"))),
                );
                Value::Mismatch
            }
            // `git`がrepositoryとして扱えない場合の終了statusだけを不在とする。
            Some(sandbox::GIT_FATAL) => Value::Missing,
            _ => {
                status.diagnostics.extend(
                    sandbox::unobservable(&outcome, &git_dir)
                        .diagnostics()
                        .iter()
                        .cloned(),
                );
                Value::NotObserved
            }
        },
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::NotObserved
        }
    };
    status.push("status-item-bare-repository", value);
}
