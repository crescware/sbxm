use crate::diagnostics::Msg;

use super::{ExternalOutput, Warning};

/// 長い工程の開始を知らせる先。
///
/// 深いworkflowへUI全体を渡さず、報告できることだけを型で示す。globalな可変状態を
/// 持たないため、並行実行しても出力が混ざらず、testは「何を報告したか」だけを見られる。
///
/// 長い工程は外部toolを起動する工程でもある。工程を知らせる先と、その外部toolの出力が
/// 出る先を別々に渡すと、両者の間隔を誰が決めるのかが呼び出しごとに変わる。
pub trait ProgressSink: ExternalOutput {
    /// これから始める工程を1行で示す。
    fn step(&mut self, message: Msg);

    /// 最終結果を待たずに、注意を今すぐ伝える。
    ///
    /// `Error::Canceled`のように最終結果へ事実を残せない経路が使う。
    fn warn(&mut self, warning: Warning);

    /// `--verbose`の実行だけが読む注記を1行で残す。進捗には出さない。
    ///
    /// 省略した工程や、結果に影響しなかった失敗のように、利用者へ知らせるほどではないが
    /// 調べるときには要る事実が使う。
    fn trace(&mut self, message: Msg);
}
