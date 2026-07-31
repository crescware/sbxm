//! 案件のhost path。

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Diagnostic, Error, ErrorId, Result, fail};
use crate::msg;
use crate::project::CanonicalProjectId;

use super::inspect::{display, lexically_standardize};
use super::lock::{ExclusiveLock, acquire_exclusive_lock};
use super::scope::PathScope;
use super::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};

/// project rootのdirectory名に付ける接尾辞。
///
/// metadata探索が`<base-path>/*/*.project`だけを対象とするための目印でもある。
pub const PROJECT_DIR_SUFFIX: &str = ".project";

/// validation済みのbase path。
///
/// absoluteであり、symlink解決後も利用者が指定したrootの配下に収まる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteBasePath(PathBuf);

impl AbsoluteBasePath {
    /// 宣言されたbase pathを検証する。
    ///
    /// 存在しないpathも、作成可能であれば受け入れる。存在する部分のsymlink解決結果が
    /// 宣言pathの外を指す場合はsecurity errorとする。
    pub fn new(declared: &Path) -> Result<AbsoluteBasePath> {
        if !declared.is_absolute() {
            return fail(
                ErrorId::BasePathNotAbsolute,
                msg!("error-base-path-not-absolute", path = display(declared)),
            );
        }
        let standardized = lexically_standardize(declared);

        // 存在する最も近い祖先まで遡り、そこからsymlinkを解決する。
        let mut existing = standardized.as_path();
        let mut trailing: Vec<&std::ffi::OsStr> = Vec::new();
        let resolved = loop {
            match fs::canonicalize(existing) {
                Ok(resolved) => break resolved,
                Err(_) => match (existing.parent(), existing.file_name()) {
                    (Some(parent), Some(name)) => {
                        trailing.push(name);
                        existing = parent;
                    }
                    _ => {
                        return fail(
                            ErrorId::BasePathNotDirectory,
                            msg!("error-base-path-not-directory", path = display(declared)),
                        );
                    }
                },
            }
        };

        let mut full_resolved = resolved;
        for name in trailing.iter().rev() {
            full_resolved.push(name);
        }
        if full_resolved != standardized {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::BasePathEscapesRoot,
                    msg!(
                        "security-base-path-escape-description",
                        path = display(&standardized),
                        resolved = display(&full_resolved)
                    ),
                )
                .remediation(msg!("security-base-path-escape-remediation")),
            ));
        }

        if let Ok(metadata) = fs::symlink_metadata(&standardized)
            && !metadata.is_dir()
        {
            return fail(
                ErrorId::BasePathNotDirectory,
                msg!(
                    "error-base-path-not-directory",
                    path = display(&standardized)
                ),
            );
        }

        Ok(AbsoluteBasePath(standardized))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for AbsoluteBasePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.display().to_string())
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
    pub fn derive(parent: &AbsoluteBasePath, id: &CanonicalProjectId) -> ProjectPaths {
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
