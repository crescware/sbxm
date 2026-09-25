use crate::design::Document;
use crate::msg;
use crate::paths;

use crate::commands::files::Added;

/// 足した宣言、または既にあった宣言。
pub fn added(added: &Added) -> Document {
    let source = paths::display(added.declaration.source.as_path());
    let destination = paths::display(added.declaration.destination.as_path());
    if added.already {
        return Document::new().summary(msg!(
            "files-already-declared",
            source = source,
            destination = destination
        ));
    }
    Document::new().summary(msg!(
        "files-added",
        source = source,
        destination = destination,
        path = paths::display(&added.path)
    ))
}
