use crate::design::Document;
use crate::i18n::Locale;
use crate::msg;

use crate::commands::apply::ApplyOutput;
use crate::commands::prepare::print::files;
use crate::commands::present::Legend;

/// `apply`が並べるもの。
pub fn document(output: &ApplyOutput, locale: Locale) -> Document {
    let mut legend = Legend::new(locale);
    let mut document = Document::new();

    if let Some(count) = output.worktrees {
        document = document.summary(msg!(
            "apply-worktrees-done",
            count = count,
            project = output.project,
            sandbox = output.sandbox
        ));
    }
    // worktreeだけを適用した実行では、fileの結果を0件として報告しない。
    if !output.files.is_empty() || output.worktrees.is_none() {
        document = document.summary(msg!(
            "apply-files-done",
            count = output.files.len(),
            project = output.project,
            sandbox = output.sandbox
        ));
    }

    document
        .concat(files(&output.files, &mut legend))
        .legend(Legend::heading(), legend.entries())
}
