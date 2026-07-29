//! 実在しないことを見て分かる固定値。

/// digestの16進部分。`DIGEST`と`IMAGE_ID`はここから作る。
macro_rules! digest {
    () => {
        "1111111111111111111111111111111111111111111111111111111111111111"
    };
}

/// Dockerfileのdigestとして使う固定値。
pub const DIGEST: &str = digest!();

/// image IDとして使う固定値。
pub const IMAGE_ID: &str = concat!("sha256:", digest!());

/// worktreeのHEADとして使う固定のcommit。
pub const COMMIT: &str = "9f5b1c5a2b6d4e8f0a1b2c3d4e5f60718293a4b5";

/// 記録済みworktreeが起点から離れている状態。作業すればこうなる。
pub const MOVED: &str = "1a2b3c4d5e6f708192a3b4c5d6e7f80912a3b4c5";
