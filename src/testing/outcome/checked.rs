use super::Unmet;

/// testが成立を前提とした値を取り出せなかったときに返る型。
pub type Checked<T = ()> = std::result::Result<T, Unmet>;
