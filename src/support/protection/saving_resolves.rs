use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::support::bundle;

/// hostのrepositoryへ保存すれば解ける診断か。
///
/// originに無いcommitのうち、保存が運ぶrefのものだけが保存で解ける。stashやnotesの
/// commitは、保存しても辿れるようにならない。案内の文言ではなく、診断が示すrefで決める。
pub fn saving_resolves(diagnostic: &Diagnostic) -> bool {
    diagnostic.id == ErrorId::OriginCommitUnreachable
        && diagnostic
            .facts
            .iter()
            .filter_map(Fact::as_reference)
            .any(bundle::carries)
}
