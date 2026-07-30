//! Sandboxと、その中で見える状態の診断。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::error::{Diagnostic, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::{SandboxLayout, SandboxName};

use crate::support::inventory::{self, ProjectState};
use crate::support::{daemon, sandbox, secret};

use super::repository::{check_bare_repository, check_worktrees};
use super::{ProjectStatus, Value};
use crate::ui::Remediation;

/// Sandboxとworkspaceの状態。
///
/// 対象案件だけを名前の完全一致で突き合わせる。ほかの案件の破損で、この案件の状態が
/// 読めなくなることはない。
pub(super) fn check_sandbox(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
    status: &mut ProjectStatus,
) -> Option<ProjectState> {
    let observed = match daemon::list(host) {
        Ok(entries) => inventory::state_of(&entries, metadata, workspace_root),
        Err(error) => {
            // 一覧そのものを読めないのはglobal環境の問題である。
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status.global_scope_failure(&error);
            return None;
        }
    };

    match observed {
        Ok(state) => {
            let (sandbox, workspace) = match state {
                ProjectState::Running => (Value::Running, Value::Ready),
                ProjectState::Stopped => (Value::Stopped, Value::Ready),
                ProjectState::NotCreated => (Value::NotCreated, Value::NotApplicable),
            };
            status.push("status-item-sandbox", sandbox);
            status.push("status-item-workspace", workspace);
            Some(state)
        }
        Err(error) => {
            status.push("status-item-sandbox", Value::Mismatch);
            status.push("status-item-workspace", Value::Mismatch);
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            None
        }
    }
}

/// Sandbox内部の検査。
///
/// Sandboxがない場合は`not-applicable`、停止中は状態を変えないため検査せず
/// `not-observed-stopped`とする。状態そのものを観測できなかった場合は、Sandboxが
/// 無いことにせず`mismatch`とする。
pub(super) fn check_inside(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    state: Option<ProjectState>,
    status: &mut ProjectStatus,
) {
    let inner = [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ];
    let uniform = match state {
        Some(ProjectState::NotCreated) => Some(Value::NotApplicable),
        // read-onlyの検査でもSandboxを起動し得るため実行しない。
        Some(ProjectState::Stopped) => Some(Value::NotObservedStopped),
        None => Some(Value::Mismatch),
        Some(ProjectState::Running) => None,
    };
    if let Some(value) = uniform {
        for item in inner {
            status.push(item, value);
        }
        return;
    }

    check_secret(host, name, status);
    let layout = SandboxLayout::new(&metadata.canonical_id);
    check_bare_repository(host, name, &layout, status);
    check_worktrees(host, name, &layout, metadata, status);
    check_ssh_agent(host, name, status);
}

pub(super) fn check_secret(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    status: &mut ProjectStatus,
) {
    // 登録されていることと、そのSandboxが受け取っていることは別である。片方だけを見て
    // 使える状態とは言えない。
    let value = match secret::require_github(host, name.as_str())
        .and_then(|()| secret::require_placeholder_present(host, name.as_str()))
    {
        Ok(()) => Value::Ready,
        Err(error) if error.contains_id(ErrorId::GithubSecretMissing) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Missing
        }
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-secret", value);
}

/// SSH Agentが露出していないこと。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査自体が成立しない
/// 場合を`not-exposed`へ丸めない。
pub(super) fn check_ssh_agent(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    status: &mut ProjectStatus,
) {
    let value = match sandbox::ssh_agent_is_exposed(host, name.as_str()) {
        Ok(observed) if !observed.is_empty() => {
            status.diagnostics.push(
                Diagnostic::new(
                    ErrorId::SshAgentExposed,
                    msg!(
                        "security-ssh-agent-exposed-description",
                        sandbox = name,
                        observed = observed.join(", ")
                    ),
                )
                .remediation(
                    Remediation::text(msg!("security-ssh-agent-exposed-remediation"))
                        .try_run(format!("sbx rm {name}")),
                ),
            );
            Value::Exposed
        }
        Ok(_) => Value::NotExposed,
        Err(error) => {
            status
                .diagnostics
                .extend(error.diagnostics().iter().cloned());
            Value::Mismatch
        }
    };
    status.push("status-item-ssh-agent", value);
}

#[cfg(test)]
#[path = "inside_test.rs"]
mod inside_test;
