//! `apply`の出力。
//!
//! worktreeとfileはそれぞれ独立した結果であるため、適用した範囲だけをsummaryにする。

use crate::i18n::Locale;
use crate::msg;
use crate::ui::Document;

use super::super::prepare::print::{files, notes};
use super::super::present::Legend;
use super::run::ApplyOutput;

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
        .concat(notes(&output.notes))
        .concat(files(&output.files, &mut legend))
        .legend(Legend::heading(), legend.entries())
}
