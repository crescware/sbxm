use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use super::{register_command, require_github};

/// 現在このSandboxが使う登録を同じscopeとplaceholderのまま更新するcommand。
///
/// tokenそのものは一覧の`SECRET`列から取得せず、commandにも値の入力位置だけを置く。
/// 登録が無い場合や複数あって選べない場合は、通常の前提条件検査と同じ診断を返す。
pub fn replace_github_command(host: &dyn HostEnvironment, sandbox: &str) -> Result<String> {
    let registration = require_github(host, sandbox)?;
    Ok(register_command(
        registration.scope(),
        Some(registration.placeholder()),
    ))
}
