use std::path::Path;

use crate::boundary::host::Executable;

use super::{APPLE_GIT_SHIM, NotReusable};

/// 確認結果を再利用してよい実行ファイルか。
///
/// PATH上の名前が別の実行ファイルを呼ぶshimやwrapperは、自身の識別情報が同じでも呼び先が
/// 変わる。誤判定はどれも「本物なのに再利用しない」側に倒れ、その場合は毎回確認が走る。
pub(super) fn reusable(program: &str, executable: &Executable) -> Result<(), NotReusable> {
    let path = executable.identity.path.as_path();
    if path.file_name().and_then(|name| name.to_str()) != Some(program) {
        return Err(NotReusable::NameMismatch);
    }
    if executable.script {
        return Err(NotReusable::Script);
    }
    if path == Path::new(APPLE_GIT_SHIM) {
        return Err(NotReusable::AppleShim);
    }
    Ok(())
}
