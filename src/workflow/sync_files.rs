//! `sbxm sync-files`。
//!
//! 現在のglobal configが宣言するfileだけを、running Sandboxへ再配置する。
//! projectの登録、構築の継続、worktree構成の変更、image・Template操作は行わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::{ProjectId, SandboxName};

use super::files::{self, PlacedFile};
use super::image::image_name;
use super::{daemon, sandbox};

/// `sync-files`の結果。
#[derive(Debug, Clone)]
pub struct SyncOutput {
    pub project: String,
    pub sandbox: String,
    pub files: Vec<PlacedFile>,
}

/// 宣言fileを再配置する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<SyncOutput> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let Some(metadata) = metadata::load(&paths)? else {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectNotManaged,
                msg!("error-project-not-managed", project = project),
            )
            .remediation(msg!(
                "remediation-project-not-managed",
                command = format!("sbxm add {project}")
            )),
        ));
    };
    require_no_rebuild(&metadata)?;

    let name = SandboxName::derive(&canonical);
    let entry = daemon::list(host)?
        .into_iter()
        .find(|entry| entry.name == name.as_str())
        .ok_or_else(|| {
            Error::single(
                Diagnostic::new(
                    ErrorId::SandboxNotCreated,
                    msg!(
                        "error-sandbox-not-created",
                        project = metadata.display_id(),
                        sandbox = name
                    ),
                )
                .remediation(msg!(
                    "remediation-sandbox-not-created",
                    command = format!("sbxm add {}", metadata.display_id())
                )),
            )
        })?;

    // Templateはmetadataが持つ適用済み世代から導出する。
    let template = image_name(&name, &metadata.provisioning.dockerfile_sha256);
    sandbox::verify_identity(&entry, &name, &template, workspace_root)?;

    if entry.state != SandboxState::Running {
        // 停止中のSandboxを暗黙に起動しない。
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::SandboxNotRunning,
                msg!(
                    "error-sandbox-not-running",
                    sandbox = entry.name,
                    observed = state_of(entry.state)
                ),
            )
            .remediation(msg!(
                "remediation-sandbox-not-running",
                command = format!("sbxm open {}", metadata.display_id())
            )),
        ));
    }

    let files = files::place_all(host, &entry.name, &config.files, files::Conflict::Overwrite)?;
    Ok(SyncOutput {
        project: metadata.display_id(),
        sandbox: entry.name,
        files,
    })
}

/// 世代の切替中は、fileを配置せず`rebuild`の再実行を案内する。
fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

/// 翻訳しない状態値。
fn state_of(state: SandboxState) -> &'static str {
    match state {
        SandboxState::Running => "running",
        SandboxState::Stopped => "stopped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, CommandSpec};
    use crate::config::{FileDeclaration, GitIdentity, HostFileSource, SandboxHomeRelativePath};
    use crate::hash::sha256_hex;
    use crate::i18n::Locale;
    use crate::metadata::{CreationMode, ManagedWorktree, Provisioning, RebuildIntent};
    use crate::paths::AbsoluteBasePath;
    use crate::project::CanonicalProjectId;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;

    const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    struct FakeSbx {
        listing: String,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: output.to_string(),
                calls: RefCell::new(Vec::new()),
            }
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
            let (code, stdout) = if spec.args.first().is_some_and(|arg| arg == "ls") {
                (0, self.listing.clone())
            } else if spec.args.iter().any(|arg| arg == "test") {
                // Sandbox内のdestinationは存在しないものとして扱う。
                (1, String::new())
            } else {
                (0, String::new())
            };
            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
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
            managed_worktrees: vec![ManagedWorktree {
                path: "example-repo.tree-0".into(),
                created_from: "refs/remotes/origin/main".into(),
            }],
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

    #[test]
    fn a_running_project_gets_the_declared_files_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("declared.toml");
        std::fs::write(&source, b"declared = true\n").unwrap();
        let _ = sha256_hex(b"declared = true\n");

        let (_home, config, workspace_root) = setup(vec![declaration(&source)]);
        write_metadata(&config, None);
        let host = FakeSbx::listing(&listing(&workspace_root, "running"));

        let output = run(&config, &project(), &host, &workspace_root).expect("sync");
        assert_eq!(output.project, "Example-Org/Example-Repo");
        assert_eq!(output.files.len(), 1);
        assert!(host.ran("cp --follow-link"));
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

        run(&config, &project(), &host, &workspace_root).expect("sync");

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

        let error = run(&config, &project(), &host, &workspace_root)
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
        let error = run(&config, &project(), &host, &workspace_root)
            .expect_err("an unregistered project has nowhere to place files");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

        write_metadata(&config, None);
        let error = run(&config, &project(), &host, &workspace_root)
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

        let error = run(&config, &project(), &host, &workspace_root)
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

        let error = run(&config, &project(), &host, &workspace_root)
            .expect_err("a sandbox that cannot be identified is not written to");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    }
}
