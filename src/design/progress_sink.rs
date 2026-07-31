use crate::diagnostics::Msg;

/// 長い工程の開始を知らせる先。
///
/// 深いworkflowへUI全体を渡さず、報告できることだけを型で示す。globalな可変状態を
/// 持たないため、並行実行しても出力が混ざらず、testは「何を報告したか」だけを見られる。
pub trait ProgressSink {
    /// これから始める工程を1行で示す。
    fn step(&mut self, message: Msg);
}
