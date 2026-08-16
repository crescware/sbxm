//! Sandboxと、その中で見える状態の診断。
use std::os::unix::fs::PermissionsExt;

use crate::commands::status::project::Value;
use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Required};

use super::{super::diagnose, super::fake::*};
use crate::support::secret::placeholder_probe;
use crate::testing::host::{FakeSbx, no_secrets, registered_secret};
use crate::testing::project::{Fixture, project_id};

#[test]
fn a_stopped_sandbox_is_not_started_to_look_inside_it() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = without_image(
        FakeSbx::listing(&format!(
            r#"{{"sandboxes":[{}]}}"#,
            fixture.entry(&project, "stopped")?
        )),
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
fn the_workspace_a_stopped_sandbox_declares_is_confirmed_on_the_host() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // `entry`はrecordと同時に、そのrecordが指すdirectoryをhostへ作る。
    let host = without_image(
        FakeSbx::listing(&format!(
            r#"{{"sandboxes":[{}]}}"#,
            fixture.entry(&project, "stopped")?
        )),
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
    assert_eq!(value_of(&status, "status-item-workspace")?, Value::Ready);
    Ok(())
}

#[test]
fn a_workspace_that_is_gone_is_not_reported_as_ready() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "stopped")?
    );
    // runtimeのrecordは残ったまま、hostのdirectoryだけが消える。停止中のworkspaceは
    // 誰も触らないため、`/tmp`の掃除でこの形になる。
    std::fs::remove_dir_all(fixture.workspace_root.join(project.sandbox.as_str()))
        .required_because("remove the workspace")?;
    let host = without_image(FakeSbx::listing(&listing), &project);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(
        value_of(&status, "status-item-sandbox")?,
        Value::Stopped,
        "the runtime still has the record, and that is what this item reports"
    );
    assert_eq!(
        value_of(&status, "status-item-workspace")?,
        Value::Missing,
        "a directory that is not on the host is never available as expected"
    );
    Ok(())
}

#[test]
fn a_workspace_that_cannot_be_observed_is_not_read_as_present_or_absent() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "stopped")?
    );
    let host = without_image(FakeSbx::listing(&listing), &project);
    // 親を辿れない間は、workspaceが在るかどうかそのものを観測できない。
    std::fs::set_permissions(
        &fixture.workspace_root,
        std::fs::Permissions::from_mode(0o000),
    )
    .required_because("close the workspace root")?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    );

    std::fs::set_permissions(
        &fixture.workspace_root,
        std::fs::Permissions::from_mode(crate::paths::PRIVATE_DIR_MODE),
    )
    .required_because("reopen the workspace root")?;
    let status = status.required_because("diagnose")?;

    assert_eq!(
        value_of(&status, "status-item-workspace")?,
        Value::NotObserved,
        "not being able to look is not the same as looking and finding nothing"
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::ProjectPathUnreadable),
        "the reason the workspace could not be observed is named: {:?}",
        status.diagnostics
    );
    assert!(!status.is_healthy(), "an unobservable item is not a pass");
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
    assert_eq!(
        value_of(&status, "status-item-sandbox")?,
        Value::NotObserved
    );
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(
            value_of(&status, item)?,
            Value::NotObserved,
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
fn colliding_sandbox_names_are_not_reported_as_a_mismatch() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let entry = fixture.entry(&project, "running")?;
    let host = without_image(
        FakeSbx::listing(&format!(r#"{{"sandboxes":[{entry},{entry}]}}"#)),
        &project,
    );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(
        value_of(&status, "status-item-sandbox")?,
        Value::NotObserved
    );
    assert_eq!(
        value_of(&status, "status-item-workspace")?,
        Value::NotObserved
    );
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(value_of(&status, item)?, Value::NotObserved, "{item}");
    }
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxNameCollision),
        "the colliding sandbox names are diagnosed: {:?}",
        status.diagnostics
    );
    assert!(!host.ran("exec"), "nothing runs in an ambiguous sandbox");
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
        FakeSbx::listing(&format!(
            r#"{{"sandboxes":[{}]}}"#,
            fixture.entry(&project, "stopped")?
        )),
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
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
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
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
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
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
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
    assert_eq!(
        value_of(&status, "status-item-ssh-agent")?,
        Value::NotObserved
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable)
    );
    assert!(!status.is_healthy(), "an unprovable check is not a pass");
    Ok(())
}

#[test]
fn a_token_that_was_never_registered_is_missing_rather_than_unusable() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
    let host = no_secrets(FakeSbx::listing(&listing), project.sandbox.as_str());

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-secret")?, Value::Missing);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GithubSecretMissing),
        "what is missing and how to register it are named: {:?}",
        status.diagnostics
    );

    // 登録済みでも、そのSandboxが受け取っていなければ使える状態ではない。届いていない
    // ことは、登録が無いことと同じ答えにしない。
    let host = registered_secret(FakeSbx::listing(&listing), project.sandbox.as_str()).answering(
        &format!("exec {} -- sh -c {}", project.sandbox, placeholder_probe()),
        0,
        "",
    );
    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(value_of(&status, "status-item-secret")?, Value::NotObserved);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxSecretNotApplied),
        "a registered token that never reached the sandbox is its own failure: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_sandbox_that_works_somewhere_else_is_not_taken_for_this_projects() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let elsewhere = fixture.dir.path().join("another-workspace");
    std::fs::create_dir_all(&elsewhere).required()?;
    // 同名でも、別のworkspaceで動くSandboxをこの案件のものとして読まない。
    let listing = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        project.sandbox,
        elsewhere.display()
    );
    let host = without_image(FakeSbx::listing(&listing), &project);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::Mismatch);
    assert_eq!(value_of(&status, "status-item-workspace")?, Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxUnusable),
        "the sandbox that belongs elsewhere is named: {:?}",
        status.diagnostics
    );
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
        "the listing was read; only this project's sandbox is unusable: {:?}",
        status.diagnostics
    );
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(
            value_of(&status, item)?,
            Value::NotObserved,
            "{item} was not observed, which is not the same as absent"
        );
    }
    assert!(
        !host.ran("exec"),
        "nothing runs inside a sandbox that is not this project's: {:?}",
        host.calls()
    );
    Ok(())
}
