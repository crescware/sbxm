use super::{ErrorId, ExternalFailure, Msg};

/// 1件の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub id: ErrorId,
    pub description: Msg,
    /// 説明文から追い出した事実。
    ///
    /// commandやpath、外部が示した原因のように翻訳しない値は、地の文へ埋め込まず
    /// 項目名付きの行として並べる。どこが変数かを読み手が探さずに済み、翻訳messageも
    /// 値の連結から解放される。
    pub facts: Vec<crate::design::Fact>,
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
            facts: Vec::new(),
            remediation: None,
            external: None,
        }
    }

    /// 事実を1件足す。rendererが説明文の直後へ並べる。
    pub fn fact(mut self, fact: crate::design::Fact) -> Self {
        self.facts.push(fact);
        self
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
