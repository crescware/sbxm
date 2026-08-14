use crate::design::{ExternalOutput, ProgressSink, Warning};
use crate::diagnostics::Msg;

/// 何も表示しないsink。出力を持たない経路が使う。
pub struct SilentProgress;

impl ProgressSink for SilentProgress {
    fn step(&mut self, message: Msg) {
        let _ = message;
    }

    fn warn(&mut self, warning: Warning) {
        let _ = warning;
    }
}

impl ExternalOutput for SilentProgress {
    fn relay(&mut self, bytes: &[u8]) {
        let _ = bytes;
    }

    fn hand_over(&mut self) {}

    fn finished(&mut self) {}
}
