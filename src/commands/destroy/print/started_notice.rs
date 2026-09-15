use crate::design::Warning;
use crate::msg;

/// 計画を作るために停止中のSandboxを起動したこと。結果ではなく注意である。
pub fn started_notice() -> Warning {
    Warning::text(msg!("destroy-started-notice"))
}
