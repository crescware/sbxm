use crate::design::Document;
use crate::msg;
use crate::paths;

use crate::commands::files::Pulled;

/// hostの宣言fileから、Sandbox側の内容への差分。`diff`は表示できる形へ直したものを渡す。
pub fn pull_diff(pulled: &Pulled, diff: &str) -> Document {
    Document::new()
        .summary(msg!(
            "files-pull-diff",
            destination = paths::display(pulled.declaration.destination.as_path()),
            project = pulled.project.clone(),
            source = paths::display(pulled.declaration.source.as_path())
        ))
        .verbatim(diff)
}
