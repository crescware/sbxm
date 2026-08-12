use crate::testing::outcome::{Checked, Required};

use crate::config::{ConfigLocation, GlobalConfig};
use crate::i18n::Locale;
use crate::metadata::{self, CreationMode, ProjectMetadata, Provisioning};
use crate::paths::{ProjectParent, ProjectPaths};
use crate::project::SandboxName;
use crate::registry::{RegistryEntry, RegistryGuard};
use crate::repository::RepositoryIdentity;
use crate::testing::value::DIGEST;
use std::path::PathBuf;

use super::{Registered, ssh_repository};

/// global state directory、親directory、workspace rootを持つtest環境。
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub location: ConfigLocation,
    /// 新規案件を置く親directory。実行時のcwdの代わりとして使う。
    pub parent: ProjectParent,
    pub config: GlobalConfig,
    pub workspace_root: PathBuf,
}

impl Fixture {
    pub fn new() -> Checked<Fixture> {
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
    ///
    /// sbx v0.37.0が実際に返す形（`status`、`workspaces`は配列）に合わせる。呼び出し側は
    /// これを`{"sandboxes": [...]}`で包んで`sbx ls --json`の応答とする。
    pub fn entry(&self, project: &Registered, state: &str) -> Checked<String> {
        let workspace = self.workspace_root.join(project.sandbox.as_str());
        std::fs::create_dir_all(&workspace).required_because("create the workspace")?;
        Ok(format!(
            r#"{{"name":"{}","status":"{state}","workspaces":["{}"]}}"#,
            project.sandbox,
            workspace.display()
        ))
    }
}
