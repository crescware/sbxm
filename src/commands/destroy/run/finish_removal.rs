use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PathScope};

use crate::design::{Fact, Warning};
use crate::support::secret;

use super::{DestroyOutcome, Prepared};

/// Sandbox本体の削除が終わったあとの、管理情報の後始末。
///
/// metadataの削除を管理解除のcommit pointとし、最後にlock fileを削除する。
pub(super) fn finish_removal(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
) -> Result<DestroyOutcome> {
    // tokenの登録はSandboxを消しても残る。Sandboxが消えたあとに解くのは、消し損ねた
    // Sandboxがplaceholderを持ったまま動き続ける状態を作らないためである。commit pointの
    // 前に行うため、失敗したときは案件が管理下に残り、同じcommandでやり直せる。
    secret::forget_github(host, prepared.name.as_str())?;

    // 削除もほかのmutationと同じ規則で行う。symlinkの先を消さない。
    let cache = prepared.paths.cache_dir();
    if paths::is_symlink(&cache) {
        return Err(PathScope::ProjectPath.symlink_error(&cache));
    }
    if cache.exists() {
        std::fs::remove_dir_all(&cache).map_err(|error| cleanup_failed(&cache, &error))?;
    }

    let metadata_file = prepared.paths.metadata_file();
    if paths::is_symlink(&metadata_file) {
        return Err(PathScope::ProjectPath.symlink_error(&metadata_file));
    }
    if metadata_file.exists() {
        // ここが管理解除のcommit pointである。
        std::fs::remove_file(&metadata_file)
            .map_err(|error| cleanup_failed(&metadata_file, &error))?;
    }

    // 管理解除後は、lock fileの残存だけを警告として扱う。
    let mut warnings = Vec::new();
    let lock_file = prepared.paths.lock_file();
    if lock_file.exists()
        && let Err(error) = std::fs::remove_file(&lock_file)
    {
        warnings.push(
            Warning::text(msg!("warning-lock-file-left-behind"))
                .fact(Fact::path(&paths::display(&lock_file)))
                .fact(Fact::cause(&error.to_string())),
        );
    }

    Ok(DestroyOutcome {
        project: prepared.plan.project.clone(),
        re_register: prepared.plan.re_register.clone(),
        warnings,
    })
}

/// 管理情報の削除に失敗した。残ったpathを示す。
fn cleanup_failed(path: &Path, error: &std::io::Error) -> Error {
    Error::single(
        Diagnostic::new(ErrorId::CleanupFailed, msg!("error-cleanup-failed"))
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::cause(&error.to_string()))
            .remediation(msg!(
                "remediation-cleanup-failed",
                path = paths::display(path)
            )),
    )
}
