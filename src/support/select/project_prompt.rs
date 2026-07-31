use crate::diagnostics::{Msg, Result};

/// 対話選択。testでは差し替える。
///
/// 見出しをcommandから受け取るのは、promptの描き方をcommandごとに変えないためである。
/// 実装は`design`のpromptだけが持ち、commandは何を訊くかだけを決める。
pub trait ProjectPrompt {
    /// 1件を選ぶ。
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize>;
    /// 1件以上を選ぶ。未選択の確定は受け付けない。
    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>>;
}
