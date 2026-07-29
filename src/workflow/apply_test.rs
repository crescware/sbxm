/// 既存testは宣言fileの配置を確かめる。
const FILES_ONLY: Scope = Scope {
    files: true,
    worktrees: None,
};

use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use crate::config::{FileDeclaration, GitIdentity, HostFileSource, SandboxHomeRelativePath};
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata::{CreationMode, Provisioning, RebuildIntent};
use crate::paths::AbsoluteBasePath;
use crate::project::CanonicalProjectId;
use crate::testing::value::{COMMIT, DIGEST};
use crate::workflow::image::image_name;
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

struct FakeSbx {
    listing: String,
    calls: RefCell<Vec<Vec<String>>>,
    /// Sandbox内commandへの応答。keyは`--`より後ろを空白で連結したもの。
    answers: std::collections::HashMap<String, (i32, String)>,
    /// Sandbox内に存在するpath。
    present: RefCell<Vec<String>>,
    /// 外部commandを呼ぶ時点でproject lockを取れてしまうか。
    lock_path: Option<PathBuf>,
    lock_was_free: RefCell<Option<bool>>,
}

impl FakeSbx {
    fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: output.to_string(),
            calls: RefCell::new(Vec::new()),
            answers: std::collections::HashMap::new(),
            present: RefCell::new(Vec::new()),
            lock_path: None,
            lock_was_free: RefCell::new(None),
        }
    }

    fn answering(mut self, command: &str, stdout: &str) -> FakeSbx {
        self.answers
            .insert(command.to_string(), (0, stdout.to_string()));
        self
    }

    fn failing(mut self, command: &str) -> FakeSbx {
        self.answers.insert(command.to_string(), (1, String::new()));
        self
    }

    /// 共有repositoryとworktreeが揃ったSandboxとして答える。
    fn holding_repository(self) -> FakeSbx {
        let layout = SandboxLayout::new(&canonical());
        let git_dir = layout.bare_git_dir();
        let host = self
            .answering(
                &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
                "true\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
                "https://github.com/Example-Org/Example-Repo.git\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
                "+refs/heads/*:refs/remotes/origin/*\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/main"),
                &format!("{COMMIT}\n"),
            );
        let mut host = host;
        for index in 0..32 {
            let path = layout.worktree(index);
            host = host.answering(
                &format!("git -C {path} rev-parse HEAD"),
                &format!("{COMMIT}\n"),
            );
            host = host.answering(
                &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
                &format!("{}\n", layout.bare_git_dir()),
            );
            // 案件はattached modeであり、branchを持てるのは最初の1本だけである。
            host = match index {
                0 => host.answering(
                    &format!("git -C {path} symbolic-ref -q HEAD"),
                    "refs/heads/main\n",
                ),
                _ => host.failing(&format!("git -C {path} symbolic-ref -q HEAD")),
            };
        }
        host.present.borrow_mut().push(git_dir);
        host
    }

    /// workflowの実行中にlockが保持されているかを観測する。
    fn watching_lock(mut self, path: PathBuf) -> FakeSbx {
        self.lock_path = Some(path);
        self
    }

    fn ran(&self, needle: &str) -> bool {
        self.calls
            .borrow()
            .iter()
            .any(|args| args.join(" ").contains(needle))
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        if let Some(path) = &self.lock_path
            && self.lock_was_free.borrow().is_none()
        {
            let taken = crate::paths::acquire_exclusive_lock(
                path,
                std::time::Duration::from_millis(50),
                PRIVATE_FILE_MODE,
                PathScope::ProjectPath,
            );
            *self.lock_was_free.borrow_mut() = Some(taken.is_ok());
        }
        let (code, stdout) = if spec.args.first().is_some_and(|arg| arg == "ls") {
            (0, self.listing.clone())
        } else {
            let inner = crate::testing::command::inner_args(spec);
            if inner.first().is_some_and(|arg| *arg == "test") {
                let target = inner.last().copied().unwrap_or_default();
                (
                    i32::from(!self.present.borrow().iter().any(|known| known == target)),
                    String::new(),
                )
            } else if inner.join(" ").contains("worktree add") {
                let path = inner
                    .iter()
                    .find(|arg| arg.contains(".tree-"))
                    .copied()
                    .unwrap_or_default();
                self.present.borrow_mut().push(path.to_string());
                (0, String::new())
            } else {
                match self.answers.get(&inner.join(" ")) {
                    Some((code, stdout)) => (*code, stdout.clone()),
                    None => (0, String::new()),
                }
            }
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

fn project() -> ProjectId {
    ProjectId::parse("Example-Org/Example-Repo").unwrap()
}

fn canonical() -> CanonicalProjectId {
    project().canonical()
}

fn setup(files: Vec<FileDeclaration>) -> (tempfile::TempDir, GlobalConfig, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("Projects");
    std::fs::create_dir_all(&base).unwrap();
    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(&base).unwrap(),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files,
    };
    let workspace_root = dir.path().join("workspaces");
    std::fs::create_dir_all(workspace_root.join(SandboxName::derive(&canonical()).as_str()))
        .unwrap();
    (dir, config, workspace_root)
}

fn write_metadata(config: &GlobalConfig, rebuild: Option<RebuildIntent>) -> ProjectPaths {
    let paths = ProjectPaths::derive(&config.base_path, &canonical());
    std::fs::create_dir_all(paths.sbxm_dir()).unwrap();
    let metadata = ProjectMetadata {
        owner: "Example-Org".into(),
        repository: "Example-Repo".into(),
        canonical_id: canonical(),
        provisioning: Provisioning {
            mode: CreationMode::Attached,
            start_ref: Some("main".into()),
            requested_worktrees: 1,
            dockerfile_sha256: DIGEST.into(),
        },
        rebuild,
    };
    metadata::create(&paths, &metadata).unwrap();
    paths
}

fn listing(workspace_root: &Path, state: &str) -> String {
    let name = SandboxName::derive(&canonical());
    format!(
        r#"[{{"name":"{name}","state":"{state}","workspace":"{}","template":"{}","active_sessions":0}}]"#,
        workspace_root.join(name.as_str()).display(),
        image_name(&name, DIGEST)
    )
}

fn declaration(source: &Path) -> FileDeclaration {
    FileDeclaration {
        source: HostFileSource::new(&crate::paths::display(source)).unwrap(),
        destination: SandboxHomeRelativePath::new(".config/example/config.toml").unwrap(),
    }
}

/// worktreeだけを適用するscope。
const WORKTREES_ONLY: Scope = Scope {
    files: false,
    worktrees: Some(3),
};

#[test]
fn asking_for_worktrees_leaves_the_declared_files_alone() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "running")).holding_repository();

    let output = run(&config, &project(), WORKTREES_ONLY, &host, &workspace_root)
        .expect("worktrees are applied on their own");

    assert_eq!(output.worktrees, Some(3));
    assert!(output.files.is_empty());
    // 宣言fileの配置は既存のfileを上書きする。名指していない対象へは触れない。
    assert!(
        !host.ran("cp --follow-link"),
        "the declared files were not asked for"
    );

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(stored.provisioning.requested_worktrees, 3);
}

#[test]
fn a_number_below_what_the_project_has_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let paths = write_metadata(&config, None);
    let mut metadata = metadata::load(&paths).unwrap().expect("present");
    metadata.provisioning.requested_worktrees = 3;
    metadata::update(&paths, &metadata).unwrap();
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let scope = Scope {
        files: false,
        worktrees: Some(2),
    };
    let error = run(&config, &project(), scope, &host, &workspace_root)
        .expect_err("removing a worktree deletes what is checked out in it");
    assert_eq!(error.first_id(), Some(ErrorId::WorktreesNotReducible));

    let stored = metadata::load(&paths).unwrap().expect("present");
    assert_eq!(
        stored.provisioning.requested_worktrees, 3,
        "a refused run leaves the target where it was"
    );
}

#[test]
fn a_running_project_gets_the_declared_files_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();
    let _ = sha256_hex(b"declared = true\n");

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let output = run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");
    assert_eq!(output.project, "Example-Org/Example-Repo");
    assert_eq!(output.files.len(), 1);
    assert!(host.ran("cp --follow-link"));
}

#[test]
fn the_project_lock_is_held_while_the_files_are_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();

    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let host =
        FakeSbx::listing(&listing(&workspace_root, "running")).watching_lock(paths.lock_file());

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");

    assert_eq!(
        *host.lock_was_free.borrow(),
        Some(false),
        "another run must not reach the same sandbox while files are being replaced"
    );
    assert_eq!(
        std::fs::metadata(paths.lock_file())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        PRIVATE_FILE_MODE
    );

    // lockはworkflow終了後に解放され、lock file自体は残る。
    crate::paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect("the lock is released when the workflow ends");
}

#[test]
fn a_project_that_is_not_managed_gets_no_lock_file() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let host = FakeSbx::listing("[]");

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect_err("nothing to place");

    let paths = ProjectPaths::derive(&config.base_path, &canonical());
    assert!(
        !paths.lock_file().exists(),
        "an unmanaged project is not given a lock file"
    );
}

#[test]
fn nothing_else_in_the_project_is_touched() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("declared.toml");
    std::fs::write(&source, b"declared = true\n").unwrap();
    let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
    let paths = write_metadata(&config, None);
    let before = std::fs::read_to_string(paths.metadata_file()).unwrap();
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    run(&config, &project(), FILES_ONLY, &host, &workspace_root).expect("sync");

    for forbidden in [
        "build",
        "image save",
        "template load",
        "worktree add",
        "clone",
    ] {
        assert!(
            !host.ran(forbidden),
            "sync-files must not run {forbidden}: {:?}",
            host.calls.borrow()
        );
    }
    assert_eq!(
        std::fs::read_to_string(paths.metadata_file()).unwrap(),
        before,
        "the metadata is read-only for sync-files"
    );
}

#[test]
fn a_stopped_sandbox_is_not_started_and_the_user_is_sent_to_open() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(&config, None);
    let host = FakeSbx::listing(&listing(&workspace_root, "stopped"));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a stopped sandbox is not started implicitly");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    assert_eq!(
        error.diagnostics()[0]
            .remediation
            .as_ref()
            .map(|message| message.id),
        Some("remediation-sandbox-not-running")
    );
}

#[test]
fn a_project_that_is_not_managed_or_not_built_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    let host = FakeSbx::listing("[]");
    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("an unregistered project has nowhere to place files");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    write_metadata(&config, None);
    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a registered project without a sandbox has nowhere to place files");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
}

#[test]
fn a_rebuild_in_progress_places_nothing() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(
        &config,
        Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: DIGEST.into(),
        }),
    );
    let host = FakeSbx::listing(&listing(&workspace_root, "running"));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a half-switched sandbox is not the target of a placement");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(
        host.calls.borrow().is_empty(),
        "nothing is asked of the runtime"
    );
}

#[test]
fn a_sandbox_that_belongs_to_another_project_is_refused() {
    let (_home, config, workspace_root) = setup(Vec::new());
    write_metadata(&config, None);
    let name = SandboxName::derive(&canonical());
    let host = FakeSbx::listing(&format!(
        r#"[{{"name":"{name}","state":"running","workspace":"/tmp/elsewhere","template":"other:1"}}]"#
    ));

    let error = run(&config, &project(), FILES_ONLY, &host, &workspace_root)
        .expect_err("a sandbox that cannot be identified is not written to");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
}
