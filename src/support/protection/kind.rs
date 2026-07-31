/// metadataとの対応。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Managed,
    Unmanaged,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Managed => "managed",
            Kind::Unmanaged => "unmanaged",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Kind::Managed => "legend-managed",
            Kind::Unmanaged => "legend-unmanaged",
        }
    }
}
