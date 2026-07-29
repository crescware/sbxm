//! filesystemを使うtestが共有するfixture。

/// testの実行中だけ存在するdirectory。dropで消える。
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary directory")
}
