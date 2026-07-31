//! `apply`のtestが動かすSandboxのfake。

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::config::{
    ConfigLocation, FileDeclaration, GitIdentity, GlobalConfig, HostFileSource,
    SandboxHomeRelativePath,
};
use crate::error::Result;
use crate::i18n::Locale;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning, RebuildIntent};
use crate::paths::{AbsoluteBasePath, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{CanonicalProjectId, ProjectId, SandboxLayout, SandboxName};
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::support::image::image_name;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::{COMMIT, DIGEST};

use super::run::Scope;

pub struct FakeSbx {
    pub listing: String,
    pub inner: InnerCommandSandbox,
    /// 外部commandを呼ぶ時点でproject lockを取れてしまうか。
    pub lock_path: Option<PathBuf>,
    pub lock_was_free: RefCell<Option<bool>>,
}

impl FakeSbx {
    pub fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: output.to_string(),
            inner: InnerCommandSandbox::new(),
            lock_path: None,
            lock_was_free: RefCell::new(None),
        }
    }

    pub fn answering(mut self, command: &str, stdout: &str) -> FakeSbx {
        self.inner = self.inner.answering(command, stdout);
        self
    }

    pub fn failing(mut self, command: &str) -> FakeSbx {
        self.inner = self.inner.failing(command);
        self
    }

    pub fn holding(mut self, paths: &[&str]) -> FakeSbx {
        self.inner = self.inner.holding(paths);
        self
    }

    /// 共有repositoryとworktreeが揃ったSandboxとして答える。
    pub fn holding_repository(self) -> FakeSbx {
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
        host.holding(&[&git_dir])
    }

    /// workflowの実行中にlockが保持されているかを観測する。
    pub fn watching_lock(mut self, path: PathBuf) -> FakeSbx {
        self.lock_path = Some(path);
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.inner.calls()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.inner.ran(needle)
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
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
        let outcome = self.inner.run(spec)?;
        // Sandbox一覧だけはこちらが答える。残りはinner commandへの応答に任せる。
        match spec.args.first() {
            Some(arg) if arg == "ls" => {
                Ok(crate::testing::command::outcome(spec, 0, &self.listing))
            }
            _ => Ok(outcome),
        }
    }
}

pub fn project() -> ProjectId {
    ProjectId::parse("Example-Org/Example-Repo").unwrap()
}

pub fn canonical() -> CanonicalProjectId {
    project().canonical()
}

pub fn setup(
    files: Vec<FileDeclaration>,
) -> (tempfile::TempDir, ConfigLocation, GlobalConfig, PathBuf) {
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
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    (dir, location, config, workspace_root)
}

pub fn write_metadata(
    location: &ConfigLocation,
    config: &GlobalConfig,
    rebuild: Option<RebuildIntent>,
) -> ProjectPaths {
    let paths = ProjectPaths::derive(&config.base_path, &canonical());
    std::fs::create_dir_all(paths.sbxm_dir()).unwrap();
    let repository = crate::testing::project::ssh_repository("Example-Org/Example-Repo");
    let mut guard = RegistryGuard::acquire(location).unwrap();
    guard
        .insert(RegistryEntry::new(paths.root(), repository.clone()).unwrap())
        .unwrap();
    drop(guard);
    let metadata = ProjectMetadata {
        repository,
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

pub fn listing(workspace_root: &Path, state: &str) -> String {
    let name = SandboxName::derive(&canonical());
    format!(
        r#"[{{"name":"{name}","state":"{state}","workspace":"{}","template":"{}","active_sessions":0}}]"#,
        workspace_root.join(name.as_str()).display(),
        image_name(&name, DIGEST)
    )
}

pub fn declaration(source: &Path) -> FileDeclaration {
    FileDeclaration {
        source: HostFileSource::new(&crate::paths::display(source)).unwrap(),
        destination: SandboxHomeRelativePath::new(".config/example/settings.yaml").unwrap(),
    }
}

/// worktreeだけを適用するscope。
pub const WORKTREES_ONLY: Scope = Scope {
    files: false,
    worktrees: Some(3),
};
