//! testが共有するfixture。
//!
//! moduleを跨いで使うfakeと環境の置き場。ここに無いfixtureは、それを使うtest fileの
//! 中だけで完結している。

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::config::{GitIdentity, GlobalConfig};
use crate::error::{Error, Result};
use crate::i18n::Locale;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning};
use crate::paths::{AbsoluteBasePath, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::workflow::add::AddRequest;
use crate::workflow::select::ProjectPrompt;
use std::cell::RefCell;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;

/// Dockerfileのdigestとして使う固定値。
pub const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// worktreeのHEADとして使う固定のcommit。
pub const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

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

/// 検査を通るworktreeを持つhost。
pub fn clean_host(fixture: &Fixture, project: &Registered) -> FakeSbx {
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    FakeSbx::listing(&format!("[{}]", fixture.entry(project, "running")))
        .answering(
            &format!(
                "exec {name} -- git --git-dir {} worktree list --porcelain -z",
                layout.bare_git_dir()
            ),
            0,
            &format!(
                "worktree {}\0bare\0\0worktree {managed}\0branch refs/heads/main\0\0",
                layout.bare_root()
            ),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
            0,
            "",
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-parse --git-dir"),
            0,
            &format!("{managed}/.git\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-parse HEAD"),
            0,
            "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5\n",
        )
        .answering(
            &format!("exec {name} -- git -C {managed} symbolic-ref --quiet --short HEAD"),
            0,
            "main\n",
        )
        .answering(
            &format!(
                "exec {name} -- git -C {managed} rev-parse --abbrev-ref --symbolic-full-name @{{upstream}}"
            ),
            0,
            "origin/main\n",
        )
        .answering(
            &format!("exec {name} -- git -C {managed} rev-list --count origin/main..HEAD"),
            0,
            "0\n",
        )
        // 進行中のGit操作を示すfileはない。
        .answering(&format!("exec {name} -- test -e {managed}/.git/MERGE_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/CHERRY_PICK_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/REVERT_HEAD"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/BISECT_LOG"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-merge"), 1, "")
        .answering(&format!("exec {name} -- test -e {managed}/.git/rebase-apply"), 1, "")
}

/// 選択結果を決め打ちするprompt。
pub struct ScriptedPrompt {
    pub one: usize,
    pub many: Vec<usize>,
    pub canceled: bool,
    pub asked: std::cell::RefCell<Vec<Vec<String>>>,
}

impl ScriptedPrompt {
    pub fn choosing(one: usize) -> ScriptedPrompt {
        ScriptedPrompt {
            one,
            many: Vec::new(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn choosing_many(many: &[usize]) -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: many.to_vec(),
            canceled: false,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn canceling() -> ScriptedPrompt {
        ScriptedPrompt {
            one: 0,
            many: Vec::new(),
            canceled: true,
            asked: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl ProjectPrompt for ScriptedPrompt {
    fn select_one(&mut self, candidates: &[String]) -> Result<usize> {
        self.asked.borrow_mut().push(candidates.to_vec());
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.one)
    }

    fn select_many(&mut self, candidates: &[String]) -> Result<Vec<usize>> {
        self.asked.borrow_mut().push(candidates.to_vec());
        if self.canceled {
            return Err(Error::Canceled);
        }
        Ok(self.many.clone())
    }
}

/// `add`が受け取る要求。
pub fn request(project: &str, worktrees: Option<u32>, detach: Option<&str>) -> AddRequest {
    AddRequest {
        project: ProjectId::parse(project).expect("valid project id"),
        worktrees,
        detach: detach.map(|value| value.to_string()),
    }
}
