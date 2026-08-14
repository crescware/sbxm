use std::path::{Path, PathBuf};

use crate::diagnostics::Result;
use crate::project::CanonicalProjectId;

use crate::paths::lock::{ExclusiveLock, acquire_exclusive_lock};
use crate::paths::scope::PathScope;
use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};

use super::ProjectParent;

/// project rootのdirectory名に付ける接尾辞。
const PROJECT_DIR_SUFFIX: &str = ".project";

/// 案件が使うhost path。
///
/// project rootは、親directoryの直下へ`<repository-lower>.project`として置く。owner名を
/// 含むdirectoryは作らないため、同じ親directoryでは同じrepository名が同じpathを要求する。
/// repositoryのlowercase化により、case-insensitive filesystem上の重複を防ぐ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPaths {
    root: PathBuf,
    repository: String,
}

impl ProjectPaths {
    pub fn derive(parent: &ProjectParent, id: &CanonicalProjectId) -> ProjectPaths {
        let root = parent
            .as_path()
            .join(format!("{}{PROJECT_DIR_SUFFIX}", id.repository()));
        ProjectPaths::at(&root, id)
    }

    /// 保存済みのproject rootから組み立てる。
    ///
    /// 登録済み案件の配置は保存済みrootだけを正本とし、実行時の配置規則で再計算しない。
    pub fn at(root: &Path, id: &CanonicalProjectId) -> ProjectPaths {
        ProjectPaths {
            root: root.to_path_buf(),
            repository: id.repository().to_string(),
        }
    }

    /// `<parent>/<repository-lower>.project`
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<project-root>/<repository-lower>`
    pub fn host_clone(&self) -> PathBuf {
        self.root.join(&self.repository)
    }

    /// `<project-root>/.sbxm`
    pub fn sbxm_dir(&self) -> PathBuf {
        self.root.join(".sbxm")
    }

    /// `<project-root>/.sbxm/project.yaml`
    pub fn metadata_file(&self) -> PathBuf {
        self.sbxm_dir().join("project.yaml")
    }

    /// `<project-root>/.sbxm/project.lock`
    pub fn lock_file(&self) -> PathBuf {
        self.sbxm_dir().join("project.lock")
    }

    /// 案件のlockを取る。timeout、mode、scopeは全workflowで共通とする。
    pub fn acquire_lock(&self) -> Result<ExclusiveLock> {
        acquire_exclusive_lock(
            &self.lock_file(),
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
    }

    /// `<project-root>/.sbxm/session.lock`
    ///
    /// project lockとは別inodeのfileである。fileの存在自体は`sbxm open`のsessionが
    /// 生きていることを意味せず、保持しているOS file lockの成否だけが根拠になる。
    /// project lockを保持している間だけ取得できるよう、`support::select::Locked`経由でのみ
    /// このpathへlockを取る。
    pub fn session_lease_file(&self) -> PathBuf {
        self.sbxm_dir().join("session.lock")
    }

    /// `<project-root>/.sbxm/Dockerfile`
    pub fn dockerfile(&self) -> PathBuf {
        self.sbxm_dir().join("Dockerfile")
    }

    /// `<project-root>/.sbxm/.cache`
    pub fn cache_dir(&self) -> PathBuf {
        self.sbxm_dir().join(".cache")
    }

    /// 世代別のTemplate archive。
    pub fn template_archive(&self, short_hash: &str) -> PathBuf {
        self.cache_dir().join(format!("template-{short_hash}.tar"))
    }

    /// 検証が終わるまで正式なarchiveへ触れないための一時path。
    pub fn template_archive_temp(&self, short_hash: &str) -> PathBuf {
        self.cache_dir()
            .join(format!("template-{short_hash}.tar.tmp"))
    }
}
