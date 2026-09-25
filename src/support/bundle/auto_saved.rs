use crate::design::Warning;
use crate::diagnostics::Msg;

/// Sandboxを止める、作り直す、消す前などに、hostへ自動で保存した結果。
#[derive(Debug, Clone)]
pub enum AutoSaved {
    /// GitHubの案件か、新しく保存するものが無かった。
    Nothing,
    /// hostへ保存した。表示する一文を持つ。
    Saved(Msg),
    /// 保存できなかった。保存していない作業がSandboxにだけ残ることを伝える。
    Failed(Warning),
}
