use super::CharacterSet;

/// 1 streamの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPolicy {
    pub color: bool,
    pub characters: CharacterSet,
    /// 端末の桁数。折り返しではなく、prompt labelの省略にだけ使う。
    pub width: Option<usize>,
}

impl StreamPolicy {
    /// `ANSIを一切出さないstream`。
    #[cfg(test)]
    pub fn plain() -> StreamPolicy {
        StreamPolicy {
            color: false,
            characters: CharacterSet::Unicode,
            width: None,
        }
    }

    /// 色を出すstream。
    #[cfg(test)]
    pub fn colored() -> StreamPolicy {
        StreamPolicy {
            color: true,
            ..StreamPolicy::plain()
        }
    }

    /// ASCII glyphだけを使うstream。
    #[cfg(test)]
    pub fn ascii() -> StreamPolicy {
        StreamPolicy {
            characters: CharacterSet::Ascii,
            ..StreamPolicy::plain()
        }
    }
}
