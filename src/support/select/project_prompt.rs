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
    /// 案件とworktree indexを1画面で選ぶ。
    ///
    /// `maximum_index`はmetadata未読時の楽観的な上限であり、案件を問わない。実端末の
    /// promptは`maximums`を各描画前に呼び、バックグラウンド計算が返した案件ごとの
    /// 最大値へ切り替える。上限の入口をここ1つに保ち、静的な上限だけを渡す経路を残さない。
    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximum_index: u32,
        maximums: &mut dyn FnMut(usize) -> Option<u32>,
    ) -> Result<(usize, u32)>;
}
