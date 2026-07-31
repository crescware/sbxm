//! 案件が登録された状態のtest環境。

use crate::config::{ConfigLocation, GitIdentity, GlobalConfig};
use crate::i18n::Locale;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning};
use crate::paths::{AbsoluteBasePath, ProjectPaths};
use crate::project::{ProjectId, SandboxName};
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;
use crate::testing::value::DIGEST;
use std::path::PathBuf;

/// testが書く案件IDは常に妥当とする。
pub fn project_id(value: &str) -> ProjectId {
    ProjectId::parse(value).expect("valid project id")
}

/// `<owner>/<repository>`からSSH clone URLのidentityを作る。
pub fn ssh_repository(value: &str) -> RepositoryIdentity {
    RepositoryIdentity::parse_clone_url(&format!("git@github.com:{value}.git"))
        .expect("valid clone URL")
}

/// `<owner>/<repository>`からHTTPS clone URLのidentityを作る。
pub fn https_repository(value: &str) -> RepositoryIdentity {
    RepositoryIdentity::parse_clone_url(&format!("https://github.com/{value}.git"))
        .expect("valid clone URL")
}

/// 登録済みの1案件。
#[derive(Debug, Clone)]
pub struct Registered {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub sandbox: SandboxName,
}

/// global state directory、base path、workspace rootを持つtest環境。
pub struct Fixture {
    pub _dir: tempfile::TempDir,
    pub location: ConfigLocation,
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
        location: ConfigLocation::from_home(dir.path().to_path_buf()),
        _dir: dir,
        config,
        workspace_root,
    }
}

impl Fixture {
    /// 案件を、registry entryとmetadataの両方が揃った状態にする。
    pub fn register(&self, project: &str) -> Registered {
        let repository = ssh_repository(project);
        let canonical = repository.canonical_id().clone();
        let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
        std::fs::create_dir_all(paths.sbxm_dir()).expect("create .sbxm");
        let metadata = ProjectMetadata {
            repository: repository.clone(),
            provisioning: Provisioning {
                mode: CreationMode::Attached,
                start_ref: Some("main".into()),
                requested_worktrees: 1,
                dockerfile_sha256: DIGEST.into(),
            },
            rebuild: None,
        };
        metadata::create(&paths, &metadata).expect("write the metadata");
        self.record(paths.root(), repository);
        let sandbox = SandboxName::derive(&canonical);
        Registered {
            paths,
            metadata,
            sandbox,
        }
    }

    /// registryへentryだけを記録する。metadataやproject rootは作らない。
    pub fn record(&self, root: &std::path::Path, repository: RepositoryIdentity) {
        let mut guard = RegistryGuard::acquire(&self.location).expect("acquire the registry lock");
        guard
            .insert(RegistryEntry::new(root, repository).expect("a valid entry"))
            .expect("record the registration");
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
