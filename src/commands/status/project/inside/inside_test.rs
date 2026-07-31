//! Sandboxと、その中で見える状態の診断。
use crate::commands::status::project::Value;
use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Required};

use super::{super::diagnose, super::fake::*};
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, project_id};

#[test]
fn a_stopped_sandbox_is_not_started_to_look_inside_it() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped")?)),
        &project,
    );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::Stopped);
    assert_eq!(
        value_of(&status, "status-item-ssh-agent")?,
        Value::NotObservedStopped
    );
    assert_eq!(
        value_of(&status, "status-item-worktrees")?,
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
    Ok(())
}

#[test]
fn a_sandbox_state_that_cannot_be_read_is_not_reported_as_a_missing_sandbox() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 一覧を読めない状態でも、取得できた項目は表示する。
    let host = without_image(FakeSbx::listing("not json"), &project);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-metadata")?, Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::Mismatch);
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(
            value_of(&status, item)?,
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
    Ok(())
}

#[test]
fn an_unrelated_project_does_not_decide_this_one() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 別案件のmetadataが壊れていても、この案件の状態は読める。
    let broken = fixture.parent.as_path().join("broken/broken.project/.sbxm");
    std::fs::create_dir_all(&broken).required()?;
    std::fs::write(broken.join("project.yaml"), "version: 2\n").required()?;

    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped")?)),
        &project,
    );
    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::Stopped);
    Ok(())
}

#[test]
fn an_ssh_agent_inside_the_sandbox_is_a_security_failure() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!("[{}]", fixture.entry(&project, "running")?);
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            0,
            "/tmp/ssh-agent.sock\n",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 2, "");

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-ssh-agent")?, Value::Exposed);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SshAgentExposed)
    );
    assert!(!status.is_healthy());
    Ok(())
}

#[test]
fn an_agent_that_answers_without_keys_is_still_reachable() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!("[{}]", fixture.entry(&project, "running")?);
    // socketは未設定でも、agentへ接続できる時点で露出している。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            1,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 1, "");

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-ssh-agent")?, Value::Exposed);
    Ok(())
}

#[test]
fn a_check_that_could_not_run_is_not_read_as_not_exposed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!("[{}]", fixture.entry(&project, "running")?);
    // command不在は、露出していないことの証明にならない。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            127,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 127, "");

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-ssh-agent")?, Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable)
    );
    assert!(!status.is_healthy(), "an unprovable check is not a pass");
    Ok(())
}
