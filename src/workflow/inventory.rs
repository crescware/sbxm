//! 管理案件とSandboxの突き合わせ。
//!
//! runtime状態を複製せず、command実行のたびに1回のSandbox一覧取得から組み立てる。
//! 名前は完全一致で突き合わせ、substringや表示textの検索は使わない。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::compatibility::{SandboxEntry, SandboxState};
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, ProjectMetadata};
use crate::msg;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use super::image::image_name;
use super::{daemon, sandbox};

/// 利用者へ見せるSandboxの状態。
///
/// `registered`は内部の管理状態名であり、対応Sandboxがまだない同じ状態を
/// 利用者向けには`not-created`と表示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectState {
    NotCreated,
    Running,
    Stopped,
}

impl ProjectState {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "not-created",
            ProjectState::Running => "running",
            ProjectState::Stopped => "stopped",
        }
    }

    /// 凡例に使うFTL message ID。
    pub fn legend_id(self) -> &'static str {
        match self {
            ProjectState::NotCreated => "legend-not-created",
            ProjectState::Running => "legend-running",
            ProjectState::Stopped => "legend-stopped",
        }
    }
}

impl std::fmt::Display for ProjectState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 1件の管理案件と、その現在の状態。
#[derive(Debug, Clone)]
pub struct ManagedProject {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub sandbox: SandboxName,
    pub state: ProjectState,
    /// 対応するSandboxが元にしたTemplate。runtimeが示さない場合は`None`。
    pub template: Option<String>,
}

impl ManagedProject {
    /// 表示に使う`<owner>/<repository>`。
    pub fn display_id(&self) -> String {
        self.metadata.display_id()
    }

    /// Sandboxが元にしたTemplate。世代の判定に使う。
    pub fn entry_template(&self) -> Option<String> {
        self.template.clone()
    }
}

/// 管理案件と、管理外のSandbox。
#[derive(Debug, Clone)]
pub struct Inventory {
    /// canonical ID byte昇順。
    pub projects: Vec<ManagedProject>,
    /// Sandbox name byte昇順。
    pub unmanaged: Vec<SandboxEntry>,
}

impl Inventory {
    /// canonical IDが一致する案件。
    pub fn find(&self, canonical: &crate::project::CanonicalProjectId) -> Option<&ManagedProject> {
        self.projects
            .iter()
            .find(|project| project.metadata.canonical_id == *canonical)
    }
}

/// 現在のinventoryを1回の一覧取得から組み立てる。
///
/// metadataが1件でも不正、Sandbox名が重複、対応が矛盾する場合は、部分的に正しそうな
/// 結果を返さずerrorとする。
pub fn take(
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<Inventory> {
    let discovered = metadata::discover(&config.base_path)?;
    let entries = daemon::list(host)?;
    require_unique_names(&entries)?;

    let mut projects = Vec::with_capacity(discovered.len());
    let mut claimed: Vec<String> = Vec::new();
    for project in discovered {
        let name = project.metadata.sandbox_name();
        let entry = entries.iter().find(|entry| entry.name == name.as_str());
        let state = match entry {
            Some(entry) => {
                let template = image_name(&name, &project.metadata.provisioning.dockerfile_sha256);
                // 対応が矛盾する案件は、正常な状態として返さない。
                sandbox::verify_identity(entry, &name, &template, workspace_root)?;
                claimed.push(entry.name.clone());
                match entry.state {
                    SandboxState::Running => ProjectState::Running,
                    SandboxState::Stopped => ProjectState::Stopped,
                }
            }
            None => ProjectState::NotCreated,
        };
        projects.push(ManagedProject {
            paths: project.paths,
            metadata: project.metadata,
            sandbox: name,
            state,
            template: entry.and_then(|entry| entry.template.clone()),
        });
    }

    let mut unmanaged: Vec<SandboxEntry> = entries
        .into_iter()
        .filter(|entry| !claimed.contains(&entry.name))
        .collect();
    unmanaged.sort_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));

    Ok(Inventory {
        projects,
        unmanaged,
    })
}

/// 同名のSandboxが複数ある一覧からは、対応を決められない。
fn require_unique_names(entries: &[SandboxEntry]) -> Result<()> {
    let mut names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    names.sort_unstable();
    let duplicates: Vec<&str> = names
        .windows(2)
        .filter(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
        .collect();
    if duplicates.is_empty() {
        return Ok(());
    }
    Err(Error::new(
        ErrorId::SandboxNameCollision,
        msg!(
            "error-sandbox-name-collision",
            sandbox = duplicates.join(", "),
            projects = "the sandbox listing"
        ),
    ))
}

/// 選択候補となる管理案件が0件であることを、対象選択を開始できないerrorとして返す。
pub fn no_managed_projects() -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::NoManagedProjects,
            msg!("error-no-managed-projects"),
        )
        .remediation(msg!(
            "remediation-no-managed-projects",
            command = "sbxm add <owner>/<repository>"
        )),
    )
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::command::{CommandOutcome, CommandSpec};
    use crate::config::{GitIdentity, GlobalConfig};
    use crate::i18n::Locale;
    use crate::metadata::{CreationMode, ManagedWorktree, Provisioning};
    use crate::paths::AbsoluteBasePath;
    use crate::project::ProjectId;
    use std::cell::RefCell;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;

    pub const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    /// Sandbox一覧を返し、実行された引数を記録するhost。
    pub struct FakeSbx {
        pub listing: RefCell<Vec<String>>,
        pub answers: std::collections::HashMap<String, (i32, String)>,
        pub calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        pub fn listing(output: &str) -> FakeSbx {
            FakeSbx {
                listing: RefCell::new(vec![output.to_string()]),
                answers: std::collections::HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// 呼び出しごとに異なる一覧を返す。最後の1件は繰り返し使う。
        pub fn listings(outputs: &[&str]) -> FakeSbx {
            FakeSbx {
                listing: RefCell::new(
                    outputs
                        .iter()
                        .rev()
                        .map(|value| value.to_string())
                        .collect(),
                ),
                answers: std::collections::HashMap::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        pub fn answering(mut self, command: &str, code: i32, stdout: &str) -> FakeSbx {
            self.answers
                .insert(command.to_string(), (code, stdout.to_string()));
            self
        }

        pub fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        pub fn ran(&self, needle: &str) -> bool {
            self.calls()
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
            let key = spec.args.join(" ");
            let (code, stdout) = if spec.args.first().is_some_and(|arg| arg == "ls") {
                let mut listings = self.listing.borrow_mut();
                let output = if listings.len() > 1 {
                    listings.pop().unwrap_or_default()
                } else {
                    listings.last().cloned().unwrap_or_default()
                };
                (0, output)
            } else {
                match self.answers.get(&key) {
                    Some((code, stdout)) => (*code, stdout.clone()),
                    None => (0, String::new()),
                }
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

    /// base pathとworkspace rootを持つtest環境。
    pub struct Fixture {
        pub _dir: tempfile::TempDir,
        pub config: GlobalConfig,
        pub workspace_root: PathBuf,
    }

    pub fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("temporary home");
        let base = dir.path().join("Projects");
        std::fs::create_dir_all(&base).expect("create the base path");
        let workspace_root = dir.path().join("workspaces");
        std::fs::create_dir_all(&workspace_root).expect("create the workspace root");
        let config = GlobalConfig {
            language: Locale::En,
            base_path: AbsoluteBasePath::new(&base).expect("valid base path"),
            git: GitIdentity {
                user_name: "Example User".into(),
                user_email: "user@example.com".into(),
            },
            files: Vec::new(),
        };
        Fixture {
            _dir: dir,
            config,
            workspace_root,
        }
    }

    impl Fixture {
        /// 案件を登録済みの状態にする。
        pub fn register(&self, project: &str) -> ManagedProject {
            let id = ProjectId::parse(project).expect("valid project id");
            let canonical = id.canonical();
            let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
            std::fs::create_dir_all(paths.sbxm_dir()).expect("create .sbxm");
            let metadata = ProjectMetadata {
                owner: id.owner().to_string(),
                repository: id.repository().to_string(),
                canonical_id: canonical.clone(),
                provisioning: Provisioning {
                    mode: CreationMode::Attached,
                    start_ref: Some("main".into()),
                    requested_worktrees: 1,
                    dockerfile_sha256: DIGEST.into(),
                },
                managed_worktrees: vec![ManagedWorktree {
                    path: format!("{}.tree-0", canonical.repository()),
                    created_from: "refs/remotes/origin/main".into(),
                }],
                rebuild: None,
            };
            metadata::create(&paths, &metadata).expect("write the metadata");
            let name = SandboxName::derive(&canonical);
            ManagedProject {
                paths,
                metadata,
                sandbox: name,
                state: ProjectState::NotCreated,
                template: None,
            }
        }

        /// 案件に対応するSandboxの一覧行。
        pub fn entry(&self, project: &ManagedProject, state: &str) -> String {
            let workspace = self.workspace_root.join(project.sandbox.as_str());
            std::fs::create_dir_all(&workspace).expect("create the workspace");
            format!(
                r#"{{"name":"{}","state":"{state}","workspace":"{}","template":"{}","active_sessions":0}}"#,
                project.sandbox,
                workspace.display(),
                image_name(
                    &project.sandbox,
                    &project.metadata.provisioning.dockerfile_sha256
                )
            )
        }
    }

    #[test]
    fn projects_and_sandboxes_are_paired_by_exact_name() {
        let fixture = fixture();
        let first = fixture.register("Example-Org/Example-Repo");
        let second = fixture.register("other/repo");
        let host = FakeSbx::listing(&format!(
            "[{},{}]",
            fixture.entry(&first, "running"),
            fixture.entry(&second, "stopped")
        ));

        let inventory = take(&fixture.config, &host, &fixture.workspace_root).expect("inventory");
        assert_eq!(
            inventory
                .projects
                .iter()
                .map(|project| (project.display_id(), project.state))
                .collect::<Vec<_>>(),
            vec![
                (
                    "Example-Org/Example-Repo".to_string(),
                    ProjectState::Running
                ),
                ("other/repo".to_string(), ProjectState::Stopped),
            ],
            "projects are listed in canonical order"
        );
        assert!(inventory.unmanaged.is_empty());
    }

    #[test]
    fn a_project_without_a_sandbox_is_not_created_rather_than_missing() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let host = FakeSbx::listing("[]");

        let inventory = take(&fixture.config, &host, &fixture.workspace_root).expect("inventory");
        assert_eq!(inventory.projects[0].state, ProjectState::NotCreated);
        assert_eq!(inventory.projects[0].state.as_str(), "not-created");
    }

    #[test]
    fn a_sandbox_that_belongs_to_no_project_is_listed_separately() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = FakeSbx::listing(&format!(
            r#"[{},{{"name":"sbxm-zeta","state":"running","workspace":"/tmp/elsewhere","template":"other:1"}},{{"name":"sbxm-alpha","state":"stopped","workspace":"/tmp/elsewhere","template":"other:1"}}]"#,
            fixture.entry(&project, "running")
        ));

        let inventory = take(&fixture.config, &host, &fixture.workspace_root).expect("inventory");
        assert_eq!(inventory.projects.len(), 1);
        assert_eq!(
            inventory
                .unmanaged
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["sbxm-alpha", "sbxm-zeta"],
            "unmanaged sandboxes are listed by name"
        );
    }

    #[test]
    fn an_inconsistent_pairing_is_refused_rather_than_reported_as_a_state() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let host = FakeSbx::listing(&format!(
            r#"[{{"name":"{}","state":"running","workspace":"/tmp/elsewhere","template":"{}"}}]"#,
            project.sandbox,
            image_name(
                &project.sandbox,
                &project.metadata.provisioning.dockerfile_sha256
            )
        ));

        let error = take(&fixture.config, &host, &fixture.workspace_root)
            .expect_err("a sandbox that works elsewhere is not this project's");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    }

    #[test]
    fn a_listing_that_cannot_be_paired_stops_before_anything_is_shown() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");

        // 同名のSandboxが2件ある一覧からは対応を決められない。
        let duplicated = format!(
            "[{},{}]",
            fixture.entry(&project, "running"),
            fixture.entry(&project, "stopped")
        );
        let error = take(
            &fixture.config,
            &FakeSbx::listing(&duplicated),
            &fixture.workspace_root,
        )
        .expect_err("duplicate names are refused");
        assert_eq!(error.first_id(), Some(ErrorId::SandboxNameCollision));

        // 未対応のraw stateも同じく一覧を成立させない。
        let unknown = format!(
            r#"[{{"name":"{}","state":"pausing","workspace":"/tmp/x","template":"x"}}]"#,
            project.sandbox
        );
        let error = take(
            &fixture.config,
            &FakeSbx::listing(&unknown),
            &fixture.workspace_root,
        )
        .expect_err("an unknown state is not rounded to a known one");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

        // metadataが1件でも壊れていれば一覧を作らない。
        let broken = fixture
            .config
            .base_path
            .as_path()
            .join("broken/broken.project/.sbxm");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("project.toml"), "version = 2\n").unwrap();
        let error = take(
            &fixture.config,
            &FakeSbx::listing("[]"),
            &fixture.workspace_root,
        )
        .expect_err("a broken project stops the listing");
        assert!(error.contains(ErrorId::MetadataUnknownVersion));
    }
}
