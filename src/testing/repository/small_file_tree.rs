use std::fmt::Write;

use crate::testing::outcome::{Checked, Required};

/// 同じ1byte blobを多数の異なるpathでcheckoutするtreeの一覧。
pub fn small_file_tree(count: usize) -> Checked<String> {
    let mut tree = String::new();
    for index in 0..count {
        writeln!(tree, "100644 blob abc 1\tfile-{index}").required()?;
    }
    Ok(tree)
}
