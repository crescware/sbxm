use crate::command::{CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment};
use crate::compatibility::SandboxState;
use crate::diagnostics::{ErrorId, Result};
use crate::paths::{self, PRIVATE_DIR_MODE};
use crate::project::SandboxName;
use crate::support::template::LoadedTemplate;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

use crate::design::SilentProgress;
use crate::project::ProjectId;
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt;

struct FakeSbx {
    listings: RefCell<Vec<String>>,
    calls: RefCell<Vec<CommandSpec>>,
}

impl FakeSbx {
    fn listing(outputs: &[&str]) -> FakeSbx {
        FakeSbx {
            listings: RefCell::new(
                outputs
                    .iter()
                    .rev()
                    .map(|value| (*value).to_string())
                    .collect(),
            ),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let stdout = if spec.args.first().is_some_and(|arg| arg == "ls") {
            self.listings.borrow_mut().pop().unwrap_or_default()
        } else {
            String::new()
        };
        Ok(crate::testing::command::outcome(spec, 0, &stdout))
    }
}

/// SSH Agent検査の2つのcommandに、決め打ちの結果を返すhost。
struct FakeProbe {
    socket: (i32, &'static str),
    keys: (i32, &'static str),
}

impl HostEnvironment for FakeProbe {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let (code, stdout) = if spec.args.iter().any(|arg| arg == "printenv") {
            self.socket
        } else {
            self.keys
        };
        Ok(crate::testing::command::outcome(spec, code, stdout))
    }
}

#[test]
fn a_sandbox_that_cannot_reach_the_host_agent_passes_the_isolation_check() -> Checked {
    // `printenv`は未設定を`1`で、`ssh-add`はagent不在を`2`で示す。
    let host = FakeProbe {
        socket: (1, ""),
        keys: (SSH_ADD_NO_AGENT, ""),
    };
    assert!(
        ssh_agent_is_exposed(&host, "sandbox")
            .required_because("the check answered")?
            .is_empty()
    );
    require_credentials_isolated(&host, "sandbox").required_because("nothing is exposed")?;
    Ok(())
}

#[test]
fn either_a_socket_or_a_reachable_agent_counts_as_exposed() -> Checked {
    let by_socket = FakeProbe {
        socket: (0, "/tmp/agent.sock\n"),
        keys: (SSH_ADD_NO_AGENT, ""),
    };
    let by_agent = FakeProbe {
        socket: (1, ""),
        // 鍵が1件もない`1`でも、agentへ接続できている。
        keys: (1, ""),
    };

    for host in [by_socket, by_agent] {
        let error = require_credentials_isolated(&host, "sandbox")
            .refused_because("the host agent is reachable from inside")?;
        assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));
    }
    Ok(())
}

#[test]
fn the_key_material_an_agent_lists_never_reaches_a_diagnostic() -> Checked {
    let host = FakeProbe {
        socket: (1, ""),
        keys: (
            0,
            "ssh-rsa AAAAB3Nza-a-key-that-must-not-be-shown user@host\n",
        ),
    };

    assert_eq!(
        ssh_agent_is_exposed(&host, "sandbox").required_because("the check answered")?,
        ["ssh-add reached an agent"],
        "what is observed is that an agent answered, not what it holds"
    );

    let error = require_credentials_isolated(&host, "sandbox")
        .refused_because("an agent that lists keys is reachable from inside")?;
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("AAAAB3Nza") && !rendered.contains("must-not-be-shown"),
        "the diagnostic names the sandbox only: {rendered}"
    );
    Ok(())
}

#[test]
fn a_probe_that_could_not_run_is_not_read_as_isolation() -> Checked {
    let host = FakeProbe {
        socket: (126, ""),
        keys: (SSH_ADD_NO_AGENT, ""),
    };
    let error = require_credentials_isolated(&host, "sandbox")
        .refused_because("a check that did not answer never means isolated")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
    Ok(())
}

/// 中立Workspaceのrootを、実行時と同じ条件で用意する。
fn workspace_root() -> Checked<tempfile::TempDir> {
    let root = tempfile::tempdir().required_because("temporary workspace root")?;
    fs::set_permissions(root.path(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("the root belongs to the current user only")?;
    Ok(root)
}

fn sandbox() -> Checked<SandboxName> {
    Ok(SandboxName::derive(
        &ProjectId::parse("example-org/example-repo")
            .required()?
            .canonical(),
    ))
}

fn template() -> LoadedTemplate {
    LoadedTemplate {
        name: "sbxm-example-template:111111111111".to_string(),
        loaded: true,
    }
}

fn listing(workspace: &Path, state: &str) -> Checked<String> {
    Ok(format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"{state}","workspaces":["{}"]}}]}}"#,
        sandbox()?,
        workspace.display()
    ))
}

#[test]
fn a_missing_sandbox_is_created_from_the_template_in_a_neutral_workspace() -> Checked {
    let root = workspace_root()?;
    let workspace = workspace_path(root.path(), &sandbox()?);
    let host = FakeSbx::listing(&[r#"{"sandboxes":[]}"#, &listing(&workspace, "running")?]);

    let ready = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .required_because("create")?;
    assert!(ready.created);
    assert_eq!(ready.state, SandboxState::Running);
    assert_eq!(ready.workspace, workspace);
    assert_eq!(
        fs::metadata(&workspace).required()?.permissions().mode() & 0o777,
        PRIVATE_DIR_MODE
    );

    assert_eq!(
        host.calls()[1],
        vec![
            "create".to_string(),
            "--name".to_string(),
            sandbox()?.as_str().to_string(),
            "--template".to_string(),
            template().name,
            AGENT_KIT.to_string(),
            paths::display(&workspace),
        ]
    );
    assert_eq!(
        host.calls.borrow()[1].env,
        EnvPolicy::InheritWithoutSshAgent,
        "environment such as DOCKER_SANDBOXES_ROOT_SIZE must reach `sbx create` unfiltered"
    );
    Ok(())
}

#[test]
fn the_workspace_path_carries_no_project_or_home_path() -> Checked {
    let workspace = workspace_path(Path::new(WORKSPACE_ROOT), &sandbox()?);
    assert_eq!(
        workspace,
        Path::new("/tmp/docker-sandboxes").join(sandbox()?.as_str())
    );
    assert!(!paths::display(&workspace).contains("/Users/"));
    Ok(())
}

#[test]
fn a_sandbox_that_matches_the_expected_state_is_reused_whoever_made_it() -> Checked {
    let root = workspace_root()?;
    let workspace = workspace_path(root.path(), &sandbox()?);
    let host = FakeSbx::listing(&[&listing(&workspace, "stopped")?]);

    let ready = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .required_because("reuse")?;
    assert!(!ready.created);
    assert_eq!(ready.state, SandboxState::Stopped);
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "create")),
        "an existing sandbox is never created over"
    );
    Ok(())
}

#[test]
fn a_sandbox_with_another_workspace_stops_the_run() -> Checked {
    let root = workspace_root()?;
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).required()?;
    let host = FakeSbx::listing(&[&listing(&elsewhere, "running")?]);
    let error = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .refused_because("a sandbox that works elsewhere is not this project's")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    Ok(())
}

#[test]
fn a_runtime_that_hides_the_workspace_is_not_guessed_at() -> Checked {
    let root = workspace_root()?;
    // workspaceが分からない一覧からは、この案件のSandboxだと言えない。
    let listing = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running"}}]}}"#,
        sandbox()?
    );
    let host = FakeSbx::listing(&[&listing]);
    let error = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .refused_because("an unverifiable sandbox is not reused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    Ok(())
}

#[test]
fn the_listing_of_the_target_version_identifies_the_sandbox() -> Checked {
    let root = workspace_root()?;
    // 対象versionの一覧はTemplateを持たない。名前とworkspaceの実pathで対応を判定する。
    let listing = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        sandbox()?,
        workspace_path(root.path(), &sandbox()?).display()
    );
    let host = FakeSbx::listing(&[&listing]);

    let ready = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .required_because("the sandbox of this project is used")?;
    assert!(!ready.created, "an existing sandbox is not made again");
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "create")),
        "nothing is created over it: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_workspace_that_is_a_symlink_is_refused_before_anything_is_created() -> Checked {
    let root = workspace_root()?;
    let real = root.path().join("real");
    fs::create_dir_all(&real).required()?;
    std::os::unix::fs::symlink(&real, workspace_path(root.path(), &sandbox()?)).required()?;

    let host = FakeSbx::listing(&[r#"{"sandboxes":[]}"#]);
    let error = ensure(
        &host,
        &sandbox()?,
        &template(),
        root.path(),
        &mut SilentProgress,
    )
    .refused_because("a symlinked workspace is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(host.calls().is_empty(), "nothing is asked of the runtime");
    Ok(())
}
