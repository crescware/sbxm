/// commitがremoteへ渡っている根拠。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remote {
    Pushed,
    Reachable,
}

impl Remote {
    pub fn as_str(self) -> &'static str {
        match self {
            Remote::Pushed => "pushed",
            Remote::Reachable => "reachable",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Remote::Pushed => "legend-pushed",
            Remote::Reachable => "legend-reachable",
        }
    }
}
