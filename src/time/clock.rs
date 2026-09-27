use std::time::{Duration, SystemTime};

use super::Moment;

/// 時間の流れ。
///
/// 判断のcodeは実時間を読まず、これを通して経過を測り、待ち、時刻を刻む。実物はOS層の
/// `SystemClock`であり、testは待つたびに待った分だけ進む台本の時計を渡す。どちらも同じ
/// 判断のcodeを通る。
pub trait Clock {
    /// 今の読み。減ることはない。
    fn now(&self) -> Moment;

    /// `duration`だけ待つ。
    fn sleep(&self, duration: Duration);

    /// 壁時計の今。合わせ直せば戻りうるため、期限の判定には使わず、名前に刻む時刻にだけ
    /// 使う。
    fn wall(&self) -> SystemTime;
}
