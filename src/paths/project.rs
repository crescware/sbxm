//! 案件のhost path。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, ErrorId, Result, fail};
use crate::msg;
use crate::project::CanonicalProjectId;

use super::inspect::{display, lexically_standardize};
use super::lock::{ExclusiveLock, acquire_exclusive_lock};
use super::scope::PathScope;
use super::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};

/// project rootのdirectory名に付ける接尾辞。
const PROJECT_DIR_SUFFIX: &str = ".project";

/// 新規project rootを置く親directory。
///
/// commandを実行したcurrent directoryそのものであり、sbxmが選ぶ場所ではない。実在する
/// directoryであることだけを条件とし、存在しないpathを作らない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectParent(PathBuf);

impl ProjectParent {
    /// processのcurrent directoryから決める。
    pub fn current() -> Result<ProjectParent> {
        let declared = std::env::current_dir().map_err(|error| {
            Error::new(
                ErrorId::WorkingDirectoryUnusable,
                msg!(
                    "error-working-directory-unusable",
                    path = "-",
                    detail = error
                ),
            )
        })?;
        ProjectParent::at(&declared)
    }

    /// 宣言されたdirectoryを検証する。
    pub fn at(declared: &Path) -> Result<ProjectParent> {
        let unusable = |detail: &str| {
            fail(
                ErrorId::WorkingDirectoryUnusable,
                msg!(
                    "error-working-directory-unusable",
                    path = display(declared),
                    detail = detail
                ),
            )
        };
        if !declared.is_absolute() {
            return unusable("the directory is not an absolute path");
        }
        let standardized = lexically_standardize(declared);
        match fs::symlink_metadata(&standardized) {
            Ok(metadata) if metadata.is_dir() => Ok(ProjectParent(standardized)),
            Ok(_) => unusable("the path is not a directory"),
            Err(error) => unusable(&error.to_string()),
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

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

#[cfg(test)]
#[path = "project_test.rs"]
mod project_test;
