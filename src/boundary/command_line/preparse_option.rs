/// 完全parseより前に先読みするglobal option。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreparseOption {
    Color,
    Lang,
}

impl PreparseOption {
    pub(crate) const fn option_name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Lang => "lang",
        }
    }
}
