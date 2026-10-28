use std::path::PathBuf;

use crate::boundary::host::{Executable, ExecutableIdentity};

/// Homebrewが置く形の、確認結果を再利用してよい実行ファイル。
///
/// `/opt/homebrew/bin/<program>`のsymlinkを解決した、Cellar内の通常fileである。
pub fn reusable_executable(program: &str) -> Executable {
    Executable {
        identity: ExecutableIdentity {
            path: PathBuf::from(format!(
                "/opt/homebrew/Cellar/{program}/1.0.0/bin/{program}"
            )),
            device: 16_777_234,
            inode: 1_234_567,
            size: 4_096,
            mtime_secs: 1_761_600_000,
            mtime_nanos: 123_456_789,
            ctime_secs: 1_761_600_000,
            ctime_nanos: 123_456_789,
        },
        script: false,
    }
}
