use std::time::Duration;

use super::Moment;

/// 時間の流れ。
///
/// 判断のcodeは実時間を読まず、これを通して経過を測り、待つ。実物はOS層の`SystemClock`で
/// あり、testは待つたびに待った分だけ進む台本の時計を渡す。どちらも同じ判断のcodeを通る。
pub trait Clock {
    /// 今の読み。減ることはない。
    fn now(&self) -> Moment;

    /// `duration`だけ待つ。
    fn sleep(&self, duration: Duration);
}
