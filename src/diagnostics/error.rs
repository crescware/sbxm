use super::{Diagnostic, ErrorId, ExitCode, Msg};

/// 1件以上の診断、または対話キャンセル。
///
/// 複数種類のerrorがあってもexit codeは`1`とし、個々のerror IDと診断をすべて表示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Diagnostics(Vec<Diagnostic>),
    /// Ctrl-CまたはEscによる対話キャンセル。何も変更していないことを表す。
    Canceled,
}

impl Error {
    pub fn new(id: ErrorId, description: Msg) -> Self {
        Error::Diagnostics(vec![Diagnostic::new(id, description)])
    }

    pub fn single(diagnostic: Diagnostic) -> Self {
        Error::Diagnostics(vec![diagnostic])
    }

    #[cfg(test)]
    pub fn many(diagnostics: Vec<Diagnostic>) -> Self {
        Error::Diagnostics(diagnostics)
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Error::Diagnostics(_) => ExitCode::Failure,
            Error::Canceled => ExitCode::Canceled,
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Error::Diagnostics(items) => items,
            Error::Canceled => &[],
        }
    }

    /// 指定したerror IDを含むか。呼び出し側の分岐に使う。
    pub fn contains_id(&self, id: ErrorId) -> bool {
        self.diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.id == id)
    }

    /// 最初の診断のerror ID。testの検証に使う。
    #[cfg(test)]
    pub fn first_id(&self) -> Option<ErrorId> {
        self.diagnostics().first().map(|d| d.id)
    }
}
