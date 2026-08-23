use clap::Arg;

/// 完全parseより前に先読みするglobal option。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparseOption {
    Color,
    Lang,
}

impl PreparseOption {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Lang => "lang",
        }
    }

    pub(super) fn arg(self) -> Arg {
        let name = self.name();
        Arg::new(name).long(name).global(true)
    }
}
