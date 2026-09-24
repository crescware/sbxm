use crate::design::Warning;
use crate::diagnostics::Error;
use crate::msg;

/// 登録済みの案件を読めず、足した宣言をすぐに配置するかを訊けなかったこと。
///
/// 宣言そのものは保存済みであり、失敗ではない。読めなかった理由を添える。
pub fn not_offered(error: &Error) -> Warning {
    let mut warning = Warning::text(msg!("warning-files-apply-not-offered"));
    for diagnostic in error.diagnostics() {
        warning = warning.explain(diagnostic.description.clone());
    }
    warning
}
