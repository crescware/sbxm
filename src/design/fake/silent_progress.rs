use crate::design::ProgressSink;
use crate::diagnostics::Msg;

/// 何も表示しないsink。出力を持たない経路が使う。
pub struct SilentProgress;

impl ProgressSink for SilentProgress {
    fn step(&mut self, message: Msg) {
        let _ = message;
    }
}
