//! 統合testが前提とした値の取り出し。
//!
//! `unwrap`と`expect`はどちらもpanicで判定を終える。testの中では成立の確認そのものが
//! 目的であるため、成立しなかったことは失敗として返し、`?`で呼び出し元へ渡す。
//!
//! 失敗した場所は`#[track_caller]`が記録するため、行を探すのに追加の情報を要さない。
//!
//! 3本のtest binaryが同じ実体を取り込むため、ここには全binaryが使うものだけを置く。
//! 1本でしか使わない補助を足すと、ほかの2本で未使用として現れる。

use std::fmt::Debug;
use std::panic::Location;

/// testが途中で成立しなかったこと。
///
/// 失敗した場所と、成立しなかった理由だけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmet {
    pub at: String,
    pub reason: String,
}

impl Unmet {
    #[track_caller]
    pub fn new(reason: impl Into<String>) -> Unmet {
        Unmet {
            at: Location::caller().to_string(),
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for Unmet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.at, self.reason)
    }
}

/// testが成立を前提とした値を取り出せなかったときに返る型。
pub type Checked<T = ()> = std::result::Result<T, Unmet>;

/// 成立を前提とした値の取り出し。
pub trait Required<T> {
    /// 何を前提としたかを添えて取り出す。
    fn required_because(self, reason: &str) -> Checked<T>;
}

impl<T> Required<T> for Option<T> {
    #[track_caller]
    fn required_because(self, reason: &str) -> Checked<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(Unmet::new(reason)),
        }
    }
}

impl<T, E: Debug> Required<T> for std::result::Result<T, E> {
    #[track_caller]
    fn required_because(self, reason: &str) -> Checked<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Unmet::new(format!("{reason}: {error:?}"))),
        }
    }
}
