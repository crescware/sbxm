use std::path::Path;

use super::GIT_SUFFIX;

/// hostにあるrepositoryを`git clone`したときに、gitが作るdirectoryの名前。
///
/// gitと同じく、末尾の`/.git`を外し、残った名前から`.git`を外す。`/path/to/.git`は
/// `to`、bare repositoryの`/srv/app.git`は`app`になる。名前が残らなければ`None`。
pub fn clone_directory_name(path: &str) -> Option<&str> {
    let path = Path::new(path);
    let path = if path.file_name().and_then(|name| name.to_str()) == Some(GIT_SUFFIX) {
        path.parent()?
    } else {
        path
    };
    let name = path.file_name()?.to_str()?;
    let name = name.strip_suffix(GIT_SUFFIX).unwrap_or(name);
    (!name.is_empty()).then_some(name)
}
