use super::*;

impl Error {
    /// 複数の診断をまとめた失敗。
    pub fn many(diagnostics: Vec<Diagnostic>) -> Self {
        Error::Diagnostics(diagnostics)
    }

    /// 最初の診断のerror ID。testの検証に使う。
    pub fn first_id(&self) -> Option<ErrorId> {
        self.diagnostics().first().map(|d| d.id)
    }
}
