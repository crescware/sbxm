//! testが状態の変化を待つ間隔。

use crate::support::inventory::Poll;
use crate::testing::scripted_clock::ScriptedClock;

/// 本番と同じ間隔と上限を、待つたびに進む時計で数える。
///
/// 実時間を待たないため、期限までに読み直す回数は機械の速さによらない。期限に届く待ちは、
/// 常に2秒の`sleep`30回である。
pub fn poll(clock: &ScriptedClock) -> Poll<'_> {
    Poll::standard(clock)
}
