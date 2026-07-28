use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use crate::config::{GitIdentity, GlobalConfig};
use crate::i18n::Locale;
use crate::metadata::{CreationMode, Provisioning};
use crate::paths::{AbsoluteBasePath, ProjectPaths};
use crate::project::{ProjectId, SandboxName};
use crate::workflow::image::image_name;
use std::cell::RefCell;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;

pub const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// Sandbox一覧を返し、実行された指定を記録するhost。
pub struct FakeSbx {
    pub listing: RefCell<Vec<String>>,
    pub answers: std::collections::HashMap<String, (i32, String)>,
    pub specs: RefCell<Vec<CommandSpec>>,
}

impl FakeSbx {
    pub fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: RefCell::new(vec![output.to_string()]),
            answers: std::collections::HashMap::new(),
            specs: RefCell::new(Vec::new()),
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
            specs: RefCell::new(Vec::new()),
        }
    }

    pub fn answering(mut self, command: &str, code: i32, stdout: &str) -> FakeSbx {
        self.answers
            .insert(command.to_string(), (code, stdout.to_string()));
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.specs
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.join(" ").contains(needle))
    }

    /// 引数が一致した最後の1件の指定。envとoutput policyの検証に使う。
    pub fn spec(&self, needle: &str) -> CommandSpec {
        self.specs
            .borrow()
            .iter()
            .rev()
            .find(|spec| spec.args.join(" ").contains(needle))
            .unwrap_or_else(|| panic!("no command matched {needle}"))
            .clone()
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.specs.borrow_mut().push(spec.clone());
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

/// 登録済みの1案件。
#[derive(Debug, Clone)]
pub struct Registered {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub sandbox: SandboxName,
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
    // 実環境と同じく、workspace rootは自分だけが辿れるdirectoryとして作る。
    crate::paths::ensure_private_dir(
        &workspace_root,
        crate::paths::PRIVATE_DIR_MODE,
        crate::paths::PathScope::ProjectPath,
    )
    .expect("the workspace root belongs to the current user only");
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
    pub fn register(&self, project: &str) -> Registered {
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
            rebuild: None,
        };
        metadata::create(&paths, &metadata).expect("write the metadata");
        let sandbox = SandboxName::derive(&canonical);
        Registered {
            paths,
            metadata,
            sandbox,
        }
    }

    /// 案件に対応するSandboxの一覧行。
    pub fn entry(&self, project: &Registered, state: &str) -> String {
        let workspace = self.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).expect("create the workspace");
        format!(
            r#"{{"name":"{}","state":"{state}","workspace":"{}"}}"#,
            project.sandbox,
            workspace.display()
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

#[test]
fn one_project_is_resolved_without_the_rest_of_the_listing_being_sound() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 別のSandboxが2件同名でも、この案件の対応は名前の完全一致で決まる。
    let listing = format!(
        r#"[{},{{"name":"sbxm-other","state":"running"}},{{"name":"sbxm-other","state":"stopped"}}]"#,
        fixture.entry(&project, "running")
    );
    let entries = crate::compatibility::parse_sandbox_list(&listing).expect("listing");

    assert_eq!(
        state_of(&entries, &project.metadata, &fixture.workspace_root).expect("state"),
        ProjectState::Running
    );
}
