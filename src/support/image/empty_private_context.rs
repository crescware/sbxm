use std::fs;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PathScope};

use super::BUILD_CONTEXT_PREFIX;

/// 作られた一時directoryを、buildへ渡せるbuild contextとして受け取る。
///
/// 作成の失敗も、空で私有であるという事後条件の不成立も、buildを始める前に同じ場所で
/// 決める。作成そのものはhostのtemporary directoryに依るため、受け取る側と分けて置く。
pub(super) fn empty_private_context(
    created: std::io::Result<tempfile::TempDir>,
) -> Result<tempfile::TempDir> {
    let context = created.map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteFailed,
                msg!("error-atomic-write-failed"),
            )
            .fact(Fact::path(BUILD_CONTEXT_PREFIX))
            .fact(Fact::cause(&error.to_string())),
        )
    })?;

    let path = context.path().to_path_buf();
    if paths::is_symlink(&path) {
        return Err(PathScope::ProjectPath.symlink_error(&path));
    }
    let resolved = fs::canonicalize(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
    let entries = fs::read_dir(&resolved)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&resolved, &error.to_string()))?
        .count();
    if entries != 0 {
        return Err(Error::new(
            ErrorId::BuildContextNotEmpty,
            msg!(
                "error-build-context-not-empty",
                path = paths::display(&resolved),
                observed = entries
            ),
        ));
    }
    Ok(context)
}
