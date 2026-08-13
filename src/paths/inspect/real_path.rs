use std::fs::{self};
use std::path::{Path, PathBuf};

use super::lexically_standardize;

/// symlinkを解決できない場合は宣言されたpathのまま比較する。
///
/// 答えるのは「どのpath文字列を比べるか」だけである。canonicalizeが失敗する理由には
/// 不在も含まれるため、この関数が返した値どうしが一致したことを、pathが在ることの
/// 根拠にしてはならない。実在を問う場所は実在を観測する関数を使う。
pub fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| lexically_standardize(path))
}
