//! Sandboxと、その中で見える状態の診断。
use super::super::diagnose;
use super::super::fake::*;
use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{fixture, project_id};

#[test]
fn a_stopped_sandbox_is_not_started_to_look_inside_it() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
        &project,
    );

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
    assert_eq!(
        value_of(&status, "status-item-ssh-agent"),
        Value::NotObservedStopped
    );
    assert_eq!(
        value_of(&status, "status-item-worktrees"),
        Value::NotObservedStopped
    );
    assert!(
        !host.ran("exec"),
        "nothing runs inside a stopped sandbox: {:?}",
        host.calls()
    );
    assert!(
        status.is_healthy(),
        "not observing on purpose is not a failure"
    );
}

#[test]
fn a_sandbox_state_that_cannot_be_read_is_not_reported_as_a_missing_sandbox() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 一覧を読めない状態でも、取得できた項目は表示する。
    let host = without_image(FakeSbx::listing("not json"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-metadata"), Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Mismatch);
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(
            value_of(&status, item),
            Value::Mismatch,
            "{item} is not observed, which is not the same as absent"
        );
    }
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
        "the global command that can diagnose this is named: {:?}",
        status.diagnostics
    );
    assert!(!host.ran("exec"), "nothing runs inside an unknown sandbox");
}

#[test]
fn an_unrelated_project_does_not_decide_this_one() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 別案件のmetadataが壊れていても、この案件の状態は読める。
    let broken = fixture
        .config
        .base_path
        .as_path()
        .join("broken/broken.project/.sbxm");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("project.toml"), "version = 2\n").unwrap();

    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
        &project,
    );
    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
}

#[test]
fn an_ssh_agent_inside_the_sandbox_is_a_security_failure() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            0,
            "/tmp/ssh-agent.sock\n",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 2, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SshAgentExposed)
    );
    assert!(!status.is_healthy());
}

#[test]
fn an_agent_that_answers_without_keys_is_still_reachable() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    // socketは未設定でも、agentへ接続できる時点で露出している。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            1,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 1, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
}

#[test]
fn a_check_that_could_not_run_is_not_read_as_not_exposed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    // command不在は、露出していないことの証明にならない。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            127,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 127, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable)
    );
    assert!(!status.is_healthy(), "an unprovable check is not a pass");
}
