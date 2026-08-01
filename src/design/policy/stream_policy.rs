use super::CharacterSet;

/// 1 streamの描画条件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamPolicy {
    pub color: bool,
    pub characters: CharacterSet,
    /// 端末の桁数。折り返しではなく、prompt labelの省略にだけ使う。
    pub width: Option<usize>,
}

#[cfg(test)]
#[path = "stream_policy_test.rs"]
mod stream_policy_test;
