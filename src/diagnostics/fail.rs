use super::{Error, ErrorId, Msg, Result};

/// 診断を1件だけ持つ`Err`を作る短縮形。
pub fn fail<T>(id: ErrorId, description: Msg) -> Result<T> {
    Err(Error::new(id, description))
}
