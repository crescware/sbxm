use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::msg;
use crate::paths;

/// archiveを受け付けられないという診断の骨格。原因だけが呼び出し側で決まる。
pub(super) fn reported(path: &Path) -> Diagnostic {
    Diagnostic::new(ErrorId::ArchiveUnusable, msg!("error-archive-unusable"))
        .fact(Fact::path(&paths::display(path)))
}
