use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::{Error, ErrorId, Result, fail};
use crate::msg;

use crate::paths::inspect::{display, lexically_standardize};

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
