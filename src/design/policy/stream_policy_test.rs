use super::*;

impl StreamPolicy {
    /// `ANSIを一切出さないstream`。
    pub fn plain() -> StreamPolicy {
        StreamPolicy {
            color: false,
            characters: CharacterSet::Unicode,
            width: None,
        }
    }

    /// 色を出すstream。
    pub fn colored() -> StreamPolicy {
        StreamPolicy {
            color: true,
            ..StreamPolicy::plain()
        }
    }

    /// ASCII glyphだけを使うstream。
    pub fn ascii() -> StreamPolicy {
        StreamPolicy {
            characters: CharacterSet::Ascii,
            ..StreamPolicy::plain()
        }
    }
}
