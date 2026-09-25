use crate::diagnostics::{Diagnostic, ErrorId};

/// hostのrepositoryへ保存すれば解ける診断か。
///
/// originに無いcommitのうち、保存が運ぶrefのものだけが保存で解ける。その診断は、
/// 対処として保存のcommandを示す。stashやnotesのcommitは保存しても辿れるように
/// ならず、保存を勧めない。
pub fn saving_resolves(diagnostic: &Diagnostic) -> bool {
    diagnostic.id == ErrorId::OriginCommitUnreachable
        && diagnostic.remediation.as_ref().is_some_and(|remediation| {
            remediation
                .commands
                .iter()
                .any(|command| command.as_str().starts_with("sbxm fetch "))
        })
}
