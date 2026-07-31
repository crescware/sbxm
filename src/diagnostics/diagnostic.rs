use super::{ErrorId, ExternalFailure, Msg};

/// 1件の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: ErrorId,
    pub description: Msg,
    /// 説明と、実行するcommandを分けて持つ対処方法。
    ///
    /// 説明文へcommandを埋め込むと、command行を独立させるという表示の不変条件を
    /// rendererが守れず、翻訳者がcommandの綴りを預かることにもなる。
    pub remediation: Option<crate::design::Remediation>,
    pub external: Option<ExternalFailure>,
}

impl Diagnostic {
    pub fn new(id: ErrorId, description: Msg) -> Self {
        Diagnostic {
            id,
            description,
            remediation: None,
            external: None,
        }
    }

    pub fn remediation(mut self, remediation: impl Into<crate::design::Remediation>) -> Self {
        self.remediation = Some(remediation.into());
        self
    }

    pub fn external(mut self, external: ExternalFailure) -> Self {
        self.external = Some(external);
        self
    }
}
