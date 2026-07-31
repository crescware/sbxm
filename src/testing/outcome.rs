//! testが前提とした値の取り出し。
//!
//! `unwrap`と`expect`はどちらもpanicで判定を終える。testの中では成立の確認そのものが
//! 目的であるため、成立しなかったことは失敗として返し、`?`で呼び出し元へ渡す。
//!
//! 失敗した場所は`#[track_caller]`が記録するため、行を探すのに追加の情報を要さない。

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

/// crateのerrorをtestの失敗としてそのまま受ける。
///
/// `?`で伝播した製品codeのerrorは、testが前提とした成立が崩れたことを意味する。
impl From<crate::error::Error> for Unmet {
    #[track_caller]
    fn from(error: crate::error::Error) -> Unmet {
        Unmet::new(format!("{error:?}"))
    }
}

impl From<std::io::Error> for Unmet {
    #[track_caller]
    fn from(error: std::io::Error) -> Unmet {
        Unmet::new(error.to_string())
    }
}

/// testが成立を前提とした値を取り出せなかったときに返る型。
pub type Checked<T = ()> = std::result::Result<T, Unmet>;

/// 成立を前提とした値の取り出し。
pub trait Required<T> {
    /// 値が在ることを前提に取り出す。
    fn required(self) -> Checked<T>;

    /// 何を前提としたかを添えて取り出す。
    fn required_because(self, reason: &str) -> Checked<T>;
}

impl<T> Required<T> for Option<T> {
    #[track_caller]
    fn required(self) -> Checked<T> {
        match self {
            Some(value) => Ok(value),
            None => Err(Unmet::new("a value was required, but none was present")),
        }
    }

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
    fn required(self) -> Checked<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Unmet::new(format!("{error:?}"))),
        }
    }

    #[track_caller]
    fn required_because(self, reason: &str) -> Checked<T> {
        match self {
            Ok(value) => Ok(value),
            Err(error) => Err(Unmet::new(format!("{reason}: {error:?}"))),
        }
    }
}

/// 拒否されることを前提とした結果の取り出し。
pub trait Refused<T, E> {
    /// 拒否されることを前提にerrorを取り出す。
    fn refused(self) -> Checked<E>;

    /// 何を拒否させたかを添えてerrorを取り出す。
    fn refused_because(self, reason: &str) -> Checked<E>;
}

impl<T: Debug, E> Refused<T, E> for std::result::Result<T, E> {
    #[track_caller]
    fn refused(self) -> Checked<E> {
        match self {
            Err(error) => Ok(error),
            Ok(value) => Err(Unmet::new(format!(
                "a refusal was required, but {value:?} was produced"
            ))),
        }
    }

    #[track_caller]
    fn refused_because(self, reason: &str) -> Checked<E> {
        match self {
            Err(error) => Ok(error),
            Ok(value) => Err(Unmet::new(format!("{reason}, but {value:?} was produced"))),
        }
    }
}
