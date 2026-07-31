use crate::testing::outcome::{Checked, Required};

/// testの実行中だけ存在するdirectory。dropで消える。
pub fn temp_dir() -> Checked<tempfile::TempDir> {
    tempfile::tempdir().required_because("temporary directory")
}
