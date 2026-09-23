use crate::diagnostics::Diagnostic;

use super::ProjectOutcome;

/// `apply --files --all`の結果。
#[derive(Debug, Clone)]
pub struct AllReport {
    /// 登録済みの全案件。canonical ID昇順。
    pub outcomes: Vec<ProjectOutcome>,
    /// 適用できなかった案件の診断。1件でもあればexit code `1`とする。
    pub failures: Vec<Diagnostic>,
}
