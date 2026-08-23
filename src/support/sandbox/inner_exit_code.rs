use crate::boundary::host::CommandOutcome;

/// exec自体の失敗を示す終了status。POSIX shellとcontainer runtimeの慣例に従う。
const EXEC_FAILURE: std::ops::RangeInclusive<i32> = 125..=127;

/// Sandbox内で動いたcommand自身の終了status。
///
/// `sbx exec`が内側のcommandを起動できなかった場合、およびsignalで終わった場合は
/// `None`とする。実行できなかったことを、内側のcommandが返した結果として読まない。
pub fn inner_exit_code(outcome: &CommandOutcome) -> Option<i32> {
    match outcome.status.code() {
        Some(code) if !EXEC_FAILURE.contains(&code) => Some(code),
        _ => None,
    }
}
