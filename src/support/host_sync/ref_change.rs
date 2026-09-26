/// 取り込みでhostのrefがどう変わったか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefChange {
    /// 新しく作った。
    Created { reference: String },
    /// 前の先端から早送りした。
    Updated { reference: String },
    /// 前の先端を退避してから、早送りでない先端へ置き換えた。
    Replaced { reference: String, archived: String },
    /// Sandboxに無くなったため、前の先端を退避してから消した。
    Deleted { reference: String, archived: String },
}

impl RefChange {
    pub fn reference(&self) -> &str {
        match self {
            RefChange::Created { reference }
            | RefChange::Updated { reference }
            | RefChange::Replaced { reference, .. }
            | RefChange::Deleted { reference, .. } => reference,
        }
    }
}
