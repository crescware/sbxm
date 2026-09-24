/// 送ったことでSandboxのorigin側のrefがどう変わったか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentChange {
    /// 新しく届いた。
    Created { reference: String },
    /// 別の先端へ動いた。
    Updated { reference: String },
    /// hostに無くなったため、Sandboxのoriginからも消えた。
    Removed { reference: String },
}

impl SentChange {
    pub fn reference(&self) -> &str {
        match self {
            SentChange::Created { reference }
            | SentChange::Updated { reference }
            | SentChange::Removed { reference } => reference,
        }
    }
}
