use std::path::{Path, PathBuf};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

/// 利用者が指定した宣言fileのsourceを、絶対pathへ解決する。
///
/// 親directoryは実体のpathへ解決し、`..`や途中のsymbolic linkを残さない。末尾のfile
/// 名は解決しない。sourceそのものがsymbolic linkであることは、配置の検査が拒否する。
pub fn absolute_source(given: &Path) -> Result<PathBuf> {
    let unusable = |cause: String| {
        Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(given)))
            .fact(Fact::cause(&cause)),
        )
    };
    let joined = if given.is_absolute() {
        given.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| unusable(error.to_string()))?
            .join(given)
    };
    let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(given)))
            .fact(Fact::reason(msg!("cause-not-a-regular-file"))),
        ));
    };
    let parent = std::fs::canonicalize(parent).map_err(|error| unusable(error.to_string()))?;
    Ok(parent.join(name))
}
