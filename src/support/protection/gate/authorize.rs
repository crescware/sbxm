use crate::diagnostics::{Error, Result};

use super::super::ProtectionAssessment;

/// 層Aの通過を確認した場合だけ通常経路を先へ進める。
///
/// blockerが1件でもあれば、既知の全件を安定順序で1つの`Error::Diagnostics`へ変換して
/// 拒否する。1件目で打ち切って、別のblockerを再実行のたびに小出しにはしない。
///
/// `assessment`をconsumeするのは意図的である。判定済みの評価を使い回して再度
/// 通過することを型で防ぎ、remove直前には必ず新しい`gate::assess`を求める。
#[allow(clippy::needless_pass_by_value)]
pub fn authorize(assessment: ProtectionAssessment) -> Result<()> {
    if assessment.blockers().is_empty() {
        return Ok(());
    }
    Err(Error::Diagnostics(
        assessment
            .blockers()
            .iter()
            .map(|blocker| blocker.diagnostic(assessment.project()))
            .collect(),
    ))
}
