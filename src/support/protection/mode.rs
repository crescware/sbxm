/// HEADの持ち方。翻訳しない安定したenum。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Attached,
    Detached,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Attached => "attached",
            Mode::Detached => "detached",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            Mode::Attached => "legend-attached",
            Mode::Detached => "legend-detached",
        }
    }
}
