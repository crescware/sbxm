use std::path::Path;

/// 表示用のpath文字列。非UTF-8 pathもlossyに表示する。
pub fn display(path: &Path) -> String {
    path.display().to_string()
}
