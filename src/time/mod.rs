//! 時間。
//!
//! 判断のcodeは実時間を読まず、`Clock`を通して経過を測り、待ち、時刻を刻む。実物はOS層が持つ。

mod clock;
mod moment;

pub use clock::Clock;
pub use moment::Moment;
