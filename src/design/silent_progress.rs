use crate::diagnostics::Msg;

use super::ProgressSink;

/// 何も表示しないsink。出力を持たない経路が使う。
#[cfg(test)]
pub struct SilentProgress;

#[cfg(test)]
impl ProgressSink for SilentProgress {
    fn step(&mut self, message: Msg) {
        let _ = message;
    }
}
