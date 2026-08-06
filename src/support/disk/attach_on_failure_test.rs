use crate::compatibility::SandboxState;
use crate::diagnostics::{Diagnostic, Error, ErrorId, ExternalFailure};
use crate::msg;
use crate::testing::sandbox::InnerCommandSandbox;

use super::*;

const NAME: &str = "sbxm-example-org-example-repo-abc123";

/// 実際にsandbox内変更が失敗したときと同じ形のerror。ENOSPCの生stderrを持つ。
fn sample_error() -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandFailed,
            msg!(
                "error-external-command-failed",
                exit_status = "exit status: 28"
            ),
        )
        .external(ExternalFailure {
            program: "sbx".to_string(),
            safe_args: vec![
                "exec".to_string(),
                NAME.to_string(),
                "--".to_string(),
                "cp".to_string(),
            ],
            working_dir: None,
            exit_status: "exit status: 28".to_string(),
            stderr: b"No space left on device".to_vec(),
            stderr_lossy: false,
        })
        .remediation(msg!("remediation-worktree-tracked-changes")),
    )
}

#[test]
fn an_observed_disk_usage_is_added_as_a_fact_without_changing_anything_else() {
    let host = InnerCommandSandbox::new().answering(
        "df -Pk /",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n",
    );
    let original = sample_error();

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, original.clone());

    let (before, after) = (&original.diagnostics()[0], &decorated.diagnostics()[0]);
    assert_eq!(after.id, before.id);
    assert_eq!(after.description, before.description);
    assert_eq!(after.external, before.external);
    assert_eq!(after.remediation, before.remediation);
    assert_eq!(before.facts.len(), 0);
    assert_eq!(after.facts.len(), 3, "{:?}", after.facts);
}

#[test]
fn a_disk_observation_that_did_not_succeed_leaves_the_error_completely_untouched() {
    // `df`を1件も応答させないため、`ParseFailed`になる。
    let host = InnerCommandSandbox::new();
    let original = sample_error();

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, original.clone());

    assert_eq!(decorated, original);
}

#[test]
fn a_stopped_sandbox_is_never_started_to_observe_a_failure() {
    let host = InnerCommandSandbox::new();
    let original = sample_error();

    let decorated = attach_on_failure(&host, NAME, SandboxState::Stopped, original.clone());

    assert_eq!(decorated, original);
    assert!(
        host.calls().is_empty(),
        "a failure is never a reason to start a stopped sandbox: {:?}",
        host.calls()
    );
}

#[test]
fn cancellation_is_never_decorated() {
    let host = InnerCommandSandbox::new().answering(
        "df -Pk /",
        "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n",
    );

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, Error::Canceled);

    assert_eq!(decorated, Error::Canceled);
    assert!(host.calls().is_empty());
}
