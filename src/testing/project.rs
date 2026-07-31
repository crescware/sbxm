//! 案件が登録された状態のtest環境。

use crate::testing::outcome::{Checked, Required};

use crate::config::{ConfigLocation, GlobalConfig};
use crate::i18n::Locale;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning};
use crate::paths::{ProjectParent, ProjectPaths};
use crate::project::{ProjectId, SandboxName};
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;
use crate::testing::value::DIGEST;
use std::path::PathBuf;

/// testが書く案件IDは常に妥当とする。
pub fn project_id(value: &str) -> Checked<ProjectId> {
    ProjectId::parse(value).required_because("valid project id")
}

/// `<owner>/<repository>`からSSH clone `URLのidentityを作る`。
pub fn ssh_repository(value: &str) -> Checked<RepositoryIdentity> {
    RepositoryIdentity::parse_clone_url(&format!("git@github.com:{value}.git"))
        .required_because("valid clone URL")
}

/// `<owner>/<repository>`からHTTPS clone `URLのidentityを作る`。
pub fn https_repository(value: &str) -> Checked<RepositoryIdentity> {
    RepositoryIdentity::parse_clone_url(&format!("https://github.com/{value}.git"))
        .required_because("valid clone URL")
}

/// 登録済みの1案件。
#[derive(Debug, Clone)]
pub struct Registered {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub sandbox: SandboxName,
}

/// global state directory、親directory、workspace rootを持つtest環境。
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub location: ConfigLocation,
    /// 新規案件を置く親directory。実行時のcwdの代わりとして使う。
    pub parent: ProjectParent,
    pub config: GlobalConfig,
    pub workspace_root: PathBuf,
}

pub fn fixture() -> Checked<Fixture> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let base = dir.path().join("Projects");
    std::fs::create_dir_all(&base).required_because("create the base path")?;
    let workspace_root = dir.path().join("workspaces");
    // 実環境と同じく、workspace rootは自分だけが辿れるdirectoryとして作る。
    crate::paths::ensure_private_dir(
        &workspace_root,
        crate::paths::PRIVATE_DIR_MODE,
        crate::paths::PathScope::ProjectPath,
    )
    .required_because("the workspace root belongs to the current user only")?;
    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: Vec::new(),
    };
    Ok(Fixture {
        location: ConfigLocation::from_home(dir.path().to_path_buf()),
        parent: ProjectParent::at(&base).required_because("valid parent directory")?,
        dir,
        config,
        workspace_root,
    })
}

impl Fixture {
    /// 案件を、registry entryとmetadataの両方が揃った状態にする。
    pub fn register(&self, project: &str) -> Checked<Registered> {
        let repository = ssh_repository(project)?;
        let canonical = repository.canonical_id().clone();
        let paths = ProjectPaths::derive(&self.parent, &canonical);
        std::fs::create_dir_all(paths.sbxm_dir()).required_because("create .sbxm")?;
        let metadata = ProjectMetadata {
            repository: repository.clone(),
            provisioning: Provisioning {
                mode: CreationMode::Attached,
                start_ref: Some("main".into()),
                requested_worktrees: 1,
                dockerfile_sha256: DIGEST.into(),
            },
            git_identity: crate::testing::metadata::git_identity(),
            rebuild: None,
        };
        metadata::create(&paths, &metadata).required_because("write the metadata")?;
        self.record(paths.root(), repository)?;
        let sandbox = SandboxName::derive(&canonical);
        Ok(Registered {
            paths,
            metadata,
            sandbox,
        })
    }

    /// registryへentryだけを記録する。metadataやproject rootは作らない。
    pub fn record(&self, root: &std::path::Path, repository: RepositoryIdentity) -> Checked {
        let mut guard =
            RegistryGuard::acquire(&self.location).required_because("acquire the registry lock")?;
        guard
            .insert(RegistryEntry::new(root, repository).required_because("a valid entry")?)
            .required_because("record the registration")?;
        Ok(())
    }

    /// 案件に対応するSandboxの一覧行。
    pub fn entry(&self, project: &Registered, state: &str) -> Checked<String> {
        let workspace = self.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).required_because("create the workspace")?;
        Ok(format!(
            r#"{{"name":"{}","state":"{state}","workspace":"{}"}}"#,
            project.sandbox,
            workspace.display()
        ))
    }
}
