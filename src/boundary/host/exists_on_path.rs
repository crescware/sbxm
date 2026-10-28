use super::locate_on_path;

/// PATH上にcommandが存在するかを、実行せずに調べる。
pub fn exists_on_path(program: &str) -> bool {
    locate_on_path(program).is_some()
}
