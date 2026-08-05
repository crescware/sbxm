use crate::diagnostics::{Error, Result};

use super::super::{ProtectionAssessment, ProtectionPermit};

/// 層Aの通過を確認した場合だけ許可証を発行する。
///
/// blockerが1件でもあれば、既知の全件を安定順序で1つの`Error::Diagnostics`へ変換し、
/// 許可証を発行せずに拒否する。1件目で打ち切って、別のblockerを再実行のたびに
/// 小出しにはしない。
///
/// `assessment`をconsumeするのは意図的である。判定済みの評価を使い回して再度
/// 許可を得ることを型で防ぎ、remove直前には必ず新しい`gate::assess`を求める。
#[allow(clippy::needless_pass_by_value)]
pub fn authorize(assessment: ProtectionAssessment) -> Result<ProtectionPermit> {
    if assessment.blockers().is_empty() {
        return Ok(ProtectionPermit::issue());
    }
    Err(Error::Diagnostics(
        assessment
            .blockers()
            .iter()
            .map(super::super::ProtectionBlocker::diagnostic)
            .collect(),
    ))
}
