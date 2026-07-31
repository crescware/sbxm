use crate::config::GlobalConfig;

use crate::support::StatusValue;

use crate::commands::status::global::{GlobalStatus, push};

/// 新規登録の既定として保存されているGit identity。
///
/// hostの設定は見ない。既定を選ぶのは利用者であり、未保存であることは、対話的な
/// `add`がまだ一度も訊いていないことを意味する。errorではないため案内も出さない。
///
/// configそのものが読めない場合は`check_config`が既に報告しているため、ここでは
/// 同じ事実を重ねて報告しない。
pub fn check_git_identity(config: Option<&GlobalConfig>, status: &mut GlobalStatus) {
    let value = match config {
        Some(config) if config.git_identity.is_some() => StatusValue::Ready,
        Some(_) => StatusValue::Missing,
        None => StatusValue::Error,
    };
    push(status, "status-item-git-identity", value);
}
