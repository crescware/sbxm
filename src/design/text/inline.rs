use crate::design::style::VisualState;

/// 翻訳しない短い値。装飾の判断を型で持つ。
///
/// pathやIDをtableの行ごとにboldにするとノイズになるため、既定は端末の既定色である。
/// 照合の基準になる値だけを[`Inline::Important`]で示す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    /// 端末の既定色のまま出す値。
    Text(String),
    /// 照合の基準になる短い値。project ID、sandbox名、error IDなど。
    Important(String),
    /// host上のpath。行ごとの強調はしない。
    Path(String),
    /// 状態値。文脈が決めたsemantic stateで着色する。
    State { text: String, state: VisualState },
}

impl Inline {
    pub fn text(value: impl Into<String>) -> Inline {
        Inline::Text(value.into())
    }

    pub fn important(value: impl Into<String>) -> Inline {
        Inline::Important(value.into())
    }

    pub fn path(value: impl Into<String>) -> Inline {
        Inline::Path(value.into())
    }

    pub fn state(value: impl Into<String>, state: VisualState) -> Inline {
        Inline::State {
            text: value.into(),
            state,
        }
    }

    /// 装飾を除いた元の文字列。列幅はこの値から数える。
    pub fn as_str(&self) -> &str {
        match self {
            Inline::Text(value) | Inline::Important(value) | Inline::Path(value) => value,
            Inline::State { text, .. } => text,
        }
    }
}
