use crate::diagnostics::Msg;

/// 削除対象・保持対象の1件。
#[derive(Debug, Clone)]
pub enum Target {
    /// hostのpath。翻訳しない。
    Path(String),
    /// pathで示せない対象。選択した言語で説明する。
    Described(Msg),
}
