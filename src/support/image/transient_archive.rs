use std::path::{Path, PathBuf};

use crate::design::{Fact, ProgressSink, Warning};
use crate::diagnostics::{Error, Result};
use crate::msg;
use crate::paths::{self, FileIdentity, PathScope};

/// `sbx template load`の間だけ存在する、短命なTemplate archive。
///
/// 世代別の永続成果物ではなく、loadの成功・失敗を問わずbest-effortで削除する対象で
/// あることを型で示す。dropだけに頼ると、panicやprocess終了で削除できないまま残る
/// ため、呼び出し側は[`TransientArchive::cleanup_after`]でloadの結果と一緒に片付ける。
#[derive(Debug)]
pub struct TransientArchive {
    path: PathBuf,
    /// 作成した時点の実体。削除の直前に確かめ、入れ替わっていれば削除しない。
    identity: FileIdentity,
}

impl TransientArchive {
    pub(super) fn new(path: PathBuf) -> Result<TransientArchive> {
        let identity = FileIdentity::of_path_without_following(&path)
            .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))?;
        Ok(TransientArchive { path, identity })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// このarchiveをbest-effortで削除する。
    ///
    /// 作成した時点の実体と入れ替わっていれば削除しない。この実行が作った成果物では
    /// ない何かを、名前が一致するという理由だけで削除しない。
    fn cleanup(&self) -> std::io::Result<()> {
        match FileIdentity::of_path_without_following(&self.path) {
            Ok(observed) if observed == self.identity => std::fs::remove_file(&self.path),
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// `outcome`（`template::ensure`の結果）を返しながら、archiveを片付ける。
    ///
    /// 削除自体が失敗しても`outcome`の主要な結果は置き換えない。成功していた場合は
    /// warningとして残し、失敗していた場合は元のerrorへ事実として追記する。
    /// `Error::Canceled`は診断を持たないため事実を追記できず、`warnings`もこの経路の
    /// 早期returnでは呼び出し側に届かない。archiveが残った事実を握り潰さないよう、
    /// その場で`progress`へ伝える。
    pub fn cleanup_after<T>(
        self,
        outcome: Result<T>,
        warnings: &mut Vec<Warning>,
        progress: &mut dyn ProgressSink,
    ) -> Result<T> {
        let Err(cleanup_error) = self.cleanup() else {
            return outcome;
        };
        match outcome {
            Ok(value) => {
                warnings.push(cleanup_failed_warning(&self.path, &cleanup_error));
                Ok(value)
            }
            Err(Error::Canceled) => {
                progress.warn(cleanup_failed_warning(&self.path, &cleanup_error));
                Err(Error::Canceled)
            }
            Err(Error::Diagnostics(diagnostics)) => Err(Error::Diagnostics(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        diagnostic
                            .fact(Fact::path(&paths::display(&self.path)))
                            .fact(Fact::cause(&cleanup_error.to_string()))
                    })
                    .collect(),
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
