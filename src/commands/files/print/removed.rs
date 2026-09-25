use std::path::Path;

use crate::config::FileDeclaration;
use crate::design::Document;
use crate::msg;
use crate::paths;

/// 外した宣言。Sandboxに置いたfileは残ることも述べる。
pub fn removed(path: &Path, removed: &FileDeclaration) -> Document {
    Document::new().summary(msg!(
        "files-removed",
        destination = paths::display(removed.destination.as_path()),
        path = paths::display(path)
    ))
}
