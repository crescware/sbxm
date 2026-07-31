use super::is_executable;

/// 与えられたPATHの値だけを見て、commandが存在するかを調べる。
///
/// process全体のPATHを書き換えずにこの探索をtestできるよう、環境変数の読み取りと分ける。
pub(super) fn exists_in_path_value(program: &str, path: &std::ffi::OsStr) -> bool {
    std::env::split_paths(path).any(|dir| {
        let candidate = dir.join(program);
        is_executable(&candidate)
    })
}
