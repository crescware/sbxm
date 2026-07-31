use std::fs::{self};
use std::path::Path;

use crate::diagnostics::{Error, ErrorId};
use crate::msg;

use super::display;

/// 既存pathのfile type。診断で観測値として示す。翻訳しない技術表記。
fn file_type_of(metadata: &fs::Metadata) -> &'static str {
    let file_type = metadata.file_type();
    if file_type.is_dir() {
        "directory"
    } else if file_type.is_file() {
        "regular file"
    } else if file_type.is_symlink() {
        "symbolic link"
    } else {
        "special file"
    }
}

/// 期待するfile typeと異なるpathを、内容を変更せず拒否する。
pub fn unexpected_type(path: &Path, expected: &'static str, metadata: &fs::Metadata) -> Error {
    Error::new(
        ErrorId::ProjectPathUnexpectedType,
        msg!(
            "error-project-path-unexpected-type",
            path = display(path),
            expected = expected,
            observed = file_type_of(metadata)
        ),
    )
}
