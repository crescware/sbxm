use std::fs;
use std::path::{Path, PathBuf};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;

use crate::paths::inspect::{display, lexically_standardize};

use super::working_directory_unusable;

/// 新規project rootを置く親directory。
///
/// commandを実行したcurrent directoryそのものであり、sbxmが選ぶ場所ではない。実在する
/// directoryであることだけを条件とし、存在しないpathを作らない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectParent(PathBuf);

impl ProjectParent {
    /// processのcurrent directoryから決める。
    pub fn current() -> Result<ProjectParent> {
        // current directoryを読めなかった時点で、示せるpathは1つもない。
        let declared = std::env::current_dir().map_err(working_directory_unusable)?;
        ProjectParent::at(&declared)
    }

    /// 宣言されたdirectoryを検証する。
    pub fn at(declared: &Path) -> Result<ProjectParent> {
        let unusable = |cause: Fact| {
            Err(Error::single(
                Diagnostic::new(
                    ErrorId::WorkingDirectoryUnusable,
                    msg!("error-working-directory-unusable"),
                )
                .fact(Fact::path(&display(declared)))
                .fact(cause),
            ))
        };
        let observed = |reason: Msg| unusable(Fact::reason(reason));
        if !declared.is_absolute() {
            return observed(msg!("cause-not-absolute"));
        }
        let standardized = lexically_standardize(declared);
        match fs::symlink_metadata(&standardized) {
            Ok(metadata) if metadata.is_dir() => Ok(ProjectParent(standardized)),
            Ok(_) => observed(msg!("cause-not-a-directory")),
            Err(error) => unusable(Fact::cause(&error.to_string())),
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}
