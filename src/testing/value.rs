//! 実在しないことを見て分かる固定値。

/// Dockerfileのdigestとして使う固定値。
pub const DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

/// image IDとして使う固定値。`DIGEST`に`sha256:`を付けたもの。
pub const IMAGE_ID: &str =
    "sha256:1111111111111111111111111111111111111111111111111111111111111111";

/// worktreeのHEADとして使う固定のcommit。
pub const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";
