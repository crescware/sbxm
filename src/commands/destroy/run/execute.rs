use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self, PathScope};
use crate::project::SandboxName;

use crate::design::{Fact, ProgressSink, Warning};
use crate::support::daemon;
use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::secret;

use super::{DestroyOutcome, Prepared};

/// Sandboxと管理情報を削除する。
///
/// metadataの削除を管理解除のcommit pointとし、最後にlock fileを削除する。
pub fn execute(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<DestroyOutcome> {
    if prepared.state == ProjectState::NotCreated {
        // 削除commandを実行しない場合だけ、一覧で不在を1回確かめる。
        require_absent(host, &prepared.name)?;
    } else {
        // 削除は、一覧から消えたことを確かめるまで完了しない。
        inventory::remove(host, &prepared.name, poll, progress)?;
    }

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

/// Sandboxが存在しないことを1回確認する。
fn require_absent(host: &dyn HostEnvironment, name: &SandboxName) -> Result<()> {
    if inventory::single(&daemon::list(host)?, name.as_str())?.is_some() {
        return Err(inventory::still_present(name));
    }
    Ok(())
}

/// 管理情報の削除に失敗した。残ったpathを示す。
fn cleanup_failed(path: &Path, error: &std::io::Error) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::CleanupFailed,
            msg!(
                "error-cleanup-failed",
                path = paths::display(path),
                detail = error
            ),
        )
        .remediation(msg!(
            "remediation-cleanup-failed",
            path = paths::display(path)
        )),
    )
}
