use crate::design::Document;
use crate::hash::short_hex;
use crate::msg;

use crate::commands::rebuild::RebuildOutput;

/// `rebuild`が並べるもの。
///
/// 適用した世代は、metadataが持つhash全体ではなく、image名と同じ短縮表記で示す。
pub fn document(output: &RebuildOutput) -> Document {
    Document::new().summary(msg!(
        "rebuild-applied",
        project = output.project,
        sandbox = output.sandbox,
        generation = short_hex(&output.applied)
    ))
}

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
