use super::*;

impl ErrorId {
    /// 全variant。宣言から組み立てるため、追加し忘れが起きない。
    pub const ALL: &'static [ErrorId] = declared_error_ids!();
}
