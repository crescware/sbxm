use std::ffi::OsStr;
use std::path::PathBuf;

use super::is_executable;

/// 与えられたPATHの値だけを見て、commandとして起動されるfileを探す。
///
/// 先頭のdirectoryから順に見て、最初に見つかった実行可能fileを返す。shellが起動する
/// fileと同じ選び方である。process全体のPATHを書き換えずにこの探索をtestできるよう、
/// 環境変数の読み取りと分ける。
pub(super) fn locate_in_path_value(program: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}
