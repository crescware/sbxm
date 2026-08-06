use std::path::{Path, PathBuf};

use crate::design::{Fact, Warning};
use crate::diagnostics::{Error, Result};
use crate::msg;
use crate::paths;

/// `sbx template load`の間だけ存在する、短命なTemplate archive。
///
/// 世代別の永続成果物ではなく、loadの成功・失敗を問わずbest-effortで削除する対象で
/// あることを型で示す。dropだけに頼ると、panicやprocess終了で削除できないまま残る
/// ため、呼び出し側は[`TransientArchive::cleanup_after`]でloadの結果と一緒に片付ける。
#[derive(Debug)]
pub struct TransientArchive {
    path: PathBuf,
}

impl TransientArchive {
    pub(super) fn new(path: PathBuf) -> TransientArchive {
        TransientArchive { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// このarchiveをbest-effortで削除する。
    fn cleanup(&self) -> std::io::Result<()> {
        std::fs::remove_file(&self.path)
    }

    /// `outcome`（`template::ensure`の結果）を返しながら、archiveを片付ける。
    ///
    /// 削除自体が失敗しても`outcome`の主要な結果は置き換えない。成功していた場合は
    /// warningとして残し、失敗していた場合は元のerrorへ事実として追記する。
    pub fn cleanup_after<T>(self, outcome: Result<T>, warnings: &mut Vec<Warning>) -> Result<T> {
        let Err(cleanup_error) = self.cleanup() else {
            return outcome;
        };
        match outcome {
            Ok(value) => {
                warnings.push(cleanup_failed_warning(&self.path, &cleanup_error));
                Ok(value)
            }
            Err(error) => Err(decorate_with_cleanup_failure(
                error,
                &self.path,
                &cleanup_error,
            )),
        }
    }
}

impl Drop for TransientArchive {
    fn drop(&mut self) {
        // 明示的な`cleanup_after`が既に消していれば、ここでの再挑戦はNotFoundとして
        // 静かに終わる。panicや早期returnでcleanup_afterに届かなかった場合の最後の
        // 網であり、それでも取り切れなかったものは次回のstale cleanupが拾う。
        let _ = self.cleanup();
    }
}

fn cleanup_failed_warning(path: &Path, cleanup_error: &std::io::Error) -> Warning {
    Warning::text(msg!("warning-archive-cleanup-failed"))
        .fact(Fact::path(&paths::display(path)))
        .fact(Fact::cause(&cleanup_error.to_string()))
}

fn decorate_with_cleanup_failure(
    error: Error,
    path: &Path,
    cleanup_error: &std::io::Error,
) -> Error {
    let Error::Diagnostics(diagnostics) = &error else {
        return error;
    };
    Error::Diagnostics(
        diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| {
                diagnostic
                    .fact(Fact::path(&paths::display(path)))
                    .fact(Fact::cause(&cleanup_error.to_string()))
            })
            .collect(),
    )
}
