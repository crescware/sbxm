use super::*;

use crate::project::ProjectId;
use std::cell::RefCell;
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
                    .map(|value| value.to_string())
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
fn a_sandbox_that_cannot_reach_the_host_agent_passes_the_isolation_check() {
    // `printenv`は未設定を`1`で、`ssh-add`はagent不在を`2`で示す。
    let host = FakeProbe {
        socket: (1, ""),
        keys: (SSH_ADD_NO_AGENT, ""),
    };
    assert!(
        ssh_agent_is_exposed(&host, "sandbox")
            .expect("the check answered")
            .is_empty()
    );
    require_credentials_isolated(&host, "sandbox").expect("nothing is exposed");
}

#[test]
fn either_a_socket_or_a_reachable_agent_counts_as_exposed() {
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
            .expect_err("the host agent is reachable from inside");
        assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));
    }
}

#[test]
fn a_probe_that_could_not_run_is_not_read_as_isolation() {
    let host = FakeProbe {
        socket: (126, ""),
        keys: (SSH_ADD_NO_AGENT, ""),
    };
    let error = require_credentials_isolated(&host, "sandbox")
        .expect_err("a check that did not answer never means isolated");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
}

/// 中立Workspaceのrootを、実行時と同じ条件で用意する。
fn workspace_root() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("temporary workspace root");
    fs::set_permissions(root.path(), fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .expect("the root belongs to the current user only");
    root
}

fn sandbox() -> SandboxName {
    SandboxName::derive(
        &ProjectId::parse("example-org/example-repo")
            .unwrap()
            .canonical(),
    )
}

fn template() -> LoadedTemplate {
    LoadedTemplate {
        name: "sbxm-example-template:111111111111".to_string(),
        loaded: true,
    }
}

fn listing(workspace: &Path, state: &str) -> String {
    format!(
        r#"[{{"name":"{}","state":"{state}","workspace":"{}"}}]"#,
        sandbox(),
        workspace.display()
    )
}

#[test]
fn a_missing_sandbox_is_created_from_the_template_in_a_neutral_workspace() {
    let root = workspace_root();
    let workspace = workspace_path(root.path(), &sandbox());
    let host = FakeSbx::listing(&["[]", &listing(&workspace, "running")]);

    let ready = ensure(&host, &sandbox(), &template(), root.path()).expect("create");
    assert!(ready.created);
    assert_eq!(ready.state, SandboxState::Running);
    assert_eq!(ready.workspace, workspace);
    assert_eq!(
        fs::metadata(&workspace).unwrap().permissions().mode() & 0o777,
        PRIVATE_DIR_MODE
    );

    assert_eq!(
        host.calls()[1],
        vec![
            "create".to_string(),
            "--name".to_string(),
            sandbox().as_str().to_string(),
            "--template".to_string(),
            template().name,
            AGENT_KIT.to_string(),
            paths::display(&workspace),
        ]
    );
}

#[test]
fn the_workspace_path_carries_no_project_or_home_path() {
    let workspace = workspace_path(Path::new(WORKSPACE_ROOT), &sandbox());
    assert_eq!(
        workspace,
        Path::new("/tmp/docker-sandboxes").join(sandbox().as_str())
    );
    assert!(!paths::display(&workspace).contains("/Users/"));
}

#[test]
fn a_sandbox_that_matches_the_expected_state_is_reused_whoever_made_it() {
    let root = workspace_root();
    let workspace = workspace_path(root.path(), &sandbox());
    let host = FakeSbx::listing(&[&listing(&workspace, "stopped")]);

    let ready = ensure(&host, &sandbox(), &template(), root.path()).expect("reuse");
    assert!(!ready.created);
    assert_eq!(ready.state, SandboxState::Stopped);
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "create")),
        "an existing sandbox is never created over"
    );
}

#[test]
fn a_sandbox_with_another_workspace_stops_the_run() {
    let root = workspace_root();
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let host = FakeSbx::listing(&[&listing(&elsewhere, "running")]);
    let error = ensure(&host, &sandbox(), &template(), root.path())
        .expect_err("a sandbox that works elsewhere is not this project's");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
}

#[test]
fn a_runtime_that_hides_the_workspace_is_not_guessed_at() {
    let root = workspace_root();
    // workspaceが分からない一覧からは、この案件のSandboxだと言えない。
    let listing = format!(
        r#"[{{"name":"{}","state":"running","template":"{}"}}]"#,
        sandbox(),
        template().name
    );
    let host = FakeSbx::listing(&[&listing]);
    let error = ensure(&host, &sandbox(), &template(), root.path())
        .expect_err("an unverifiable sandbox is not reused");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
}

#[test]
fn the_listing_of_the_target_version_identifies_the_sandbox() {
    let root = workspace_root();
    // 対象versionの一覧はTemplateを持たない。名前とworkspaceの実pathで対応を判定する。
    let listing = format!(
        r#"[{{"name":"{}","status":"running","workspaces":["{}"]}}]"#,
        sandbox(),
        workspace_path(root.path(), &sandbox()).display()
    );
    let host = FakeSbx::listing(&[&listing]);

    let ready = ensure(&host, &sandbox(), &template(), root.path())
        .expect("the sandbox of this project is used");
    assert!(!ready.created, "an existing sandbox is not made again");
    assert!(
        !host
            .calls()
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "create")),
        "nothing is created over it: {:?}",
        host.calls()
    );
}

#[test]
fn a_workspace_that_is_a_symlink_is_refused_before_anything_is_created() {
    let root = workspace_root();
    let real = root.path().join("real");
    fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, workspace_path(root.path(), &sandbox())).unwrap();

    let host = FakeSbx::listing(&["[]"]);
    let error = ensure(&host, &sandbox(), &template(), root.path())
        .expect_err("a symlinked workspace is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(host.calls().is_empty(), "nothing is asked of the runtime");
}
