use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;

/// hostのrepositoryへ保存すれば解ける診断か。
///
/// originに無いcommitのうち、保存が運ぶrefのものだけが保存で解ける。その診断は、
/// 保存を勧める説明を対処方法に持つ。stashやnotesのcommitは保存しても辿れるように
/// ならず、保存を勧めない。
pub fn saving_resolves(diagnostic: &Diagnostic) -> bool {
    diagnostic.id == ErrorId::OriginCommitUnreachable
        && diagnostic.remediation.as_ref().is_some_and(|remediation| {
            remediation
                .explanation
                .contains(&msg!("remediation-origin-commit-save"))
        })
}
