use crate::boundary::host::TimeoutClass;
use crate::boundary::host::protocol::SandboxState;
use crate::design::{Fact, Inline};
use crate::diagnostics::{Diagnostic, Error, ErrorId, ExternalFailure};
use crate::msg;
use crate::testing::host::FakeSbx;
use crate::testing::outcome::Checked;
use crate::testing::sandbox::InnerCommandSandbox;

use super::*;

const NAME: &str = "sbxm-example-org-example-repo-abc123";
const REALISTIC_DF: &str = "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n";

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
    let host = InnerCommandSandbox::new().answering("df -Pk /", REALISTIC_DF);
    let original = sample_error();

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, original.clone());

    let (before, after) = (&original.diagnostics()[0], &decorated.diagnostics()[0]);
    assert_eq!(after.id, before.id);
    assert_eq!(after.description, before.description);
    assert_eq!(after.external, before.external);
    assert_eq!(after.remediation, before.remediation);
    assert_eq!(before.facts.len(), 0);
    assert_eq!(
        after.facts,
        vec![
            Fact::new(msg!("diagnostic-disk-mount-label"), Inline::path("/")),
            Fact::new(msg!("diagnostic-disk-free-label"), Inline::text("4.7 GiB")),
            Fact::new(
                msg!("diagnostic-disk-usable-label"),
                Inline::text("18.5 GiB")
            ),
            Fact::new(msg!("diagnostic-disk-capacity-label"), Inline::text("75%")),
        ],
        "{:?}",
        after.facts
    );
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
    let host = InnerCommandSandbox::new().answering("df -Pk /", REALISTIC_DF);

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, Error::Canceled);

    assert_eq!(decorated, Error::Canceled);
    assert!(host.calls().is_empty());
}

#[test]
fn a_diagnostic_with_no_external_failure_is_never_decorated() {
    // hostのfile検査のように、Sandboxへ1つもcommandを出していない失敗。
    let host = InnerCommandSandbox::new().answering("df -Pk /", REALISTIC_DF);
    let original = Error::single(
        Diagnostic::new(
            ErrorId::DeclaredFileUnusable,
            msg!("error-declared-file-unusable"),
        )
        .fact(Fact::source("/tmp/declared.yaml"))
        .fact(Fact::reason(msg!("cause-not-absolute"))),
    );

    let decorated = attach_on_failure(&host, NAME, SandboxState::Running, original.clone());

    assert_eq!(decorated, original);
    assert!(
        host.calls().is_empty(),
        "a failure with no external command must not trigger a disk check: {:?}",
        host.calls()
    );
}

#[test]
fn the_failure_path_observation_uses_the_probe_timeout_not_the_lifecycle_timeout() -> Checked {
    let needle = format!("exec {NAME} -- df -Pk /");
    let host = FakeSbx::listing("[]").answering(&needle, 0, REALISTIC_DF);
    let original = sample_error();

    attach_on_failure(&host, NAME, SandboxState::Running, original);

    let spec = host.spec(&needle)?;
    assert_eq!(
        spec.timeout,
        TimeoutClass::Probe,
        "a failure diagnosis must not wait for the 600s lifecycle timeout"
    );
    Ok(())
}
