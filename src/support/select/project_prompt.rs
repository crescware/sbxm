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
    /// 案件とworktree indexを1画面で選ぶ。maximumはmetadata未読時の楽観的な上限。
    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximum_index: u32,
    ) -> Result<(usize, u32)>;

    /// metadata計算の完了をpromptへ反映しながら案件とindexを選ぶ。
    ///
    /// 既存のfake promptは静的な`select_open`だけを実装すればよい。実端末のpromptは
    /// callbackを各描画前に呼び、バックグラウンド計算が返した案件ごとの最大値を使う。
    fn select_open_with_maximums(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximum_index: u32,
        maximums: &mut dyn FnMut(usize) -> Option<u32>,
    ) -> Result<(usize, u32)> {
        let _ = maximums;
        self.select_open(heading, candidates, maximum_index)
    }
}
