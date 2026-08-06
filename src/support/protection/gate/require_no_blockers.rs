use crate::diagnostics::{Error, Result};

use super::super::ProtectionAssessment;

/// 拒否理由（`ProtectionBlocker`）が1件も無いことを確かめる。
///
/// 空でなければ、既知の全件を安定順序で1つの`Error::Diagnostics`へ変換して拒否する。
/// 削除計画を表示する前、明示確認を求める前に呼ぶ。
pub fn require_no_blockers(assessment: &ProtectionAssessment) -> Result<()> {
    if assessment.blockers().is_empty() {
        return Ok(());
    }
    Err(Error::Diagnostics(
        assessment
            .blockers()
            .iter()
            .map(super::super::ProtectionBlocker::diagnostic)
            .collect(),
    ))
}
