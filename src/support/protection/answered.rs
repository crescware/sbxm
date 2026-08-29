use crate::boundary::host::CommandOutcome;
use crate::diagnostics::Result;

use crate::support::sandbox;

/// Sandbox内の検査commandが答えた終了status。
///
/// `sbx exec`がcommandを起動できなかった場合を、内側のcommandが返した結果として
/// 読まない。判定できない場合は、削除して良いことを示す値へ丸めずerrorとする。
pub(super) fn answered(outcome: &CommandOutcome, subject: &str) -> Result<i32> {
    sandbox::inner_exit_code(outcome).ok_or_else(|| sandbox::unobservable(outcome, subject))
}
