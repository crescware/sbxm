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
impl From<crate::diagnostics::Error> for Unmet {
    #[track_caller]
    fn from(error: crate::diagnostics::Error) -> Unmet {
        Unmet::new(format!("{error:?}"))
    }
}

impl From<std::io::Error> for Unmet {
    #[track_caller]
    fn from(error: std::io::Error) -> Unmet {
        Unmet::new(error.to_string())
    }
}
