use super::exists_in_path_value;

/// PATH上にcommandが存在するかを、実行せずに調べる。
pub fn exists_on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    exists_in_path_value(program, &path)
}
