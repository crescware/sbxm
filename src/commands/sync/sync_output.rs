use std::path::PathBuf;

use crate::support::bundle::{ReflectResult, Reflected};

use super::SentChange;

/// `sync`の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutput {
    pub project: String,
    /// 同期したhostのrepository。
    pub repository: PathBuf,
    /// Sandboxのcommitを保存したhostの名前空間。`refs/sbx/<sandbox>/`の`<sandbox>`。
    pub namespace: String,
    /// hostのbranchとtagへの反映のうち、変わったか断られたもの。Sandboxが保存する
    /// refを1つも持たなければ`None`。
    pub reflected: Option<Vec<Reflected>>,
    /// Sandboxのorigin側で変わったref。
    pub sent: Vec<SentChange>,
}

impl SyncOutput {
    /// gitが断り、hostのbranchやtagがそのまま残ったものがあるか。
    ///
    /// Sandboxのbranchが遅れているだけのものは数えない。hostは既にそのcommitを持つ。
    pub fn left_as_it_was(&self) -> bool {
        self.reflected.iter().flatten().any(|entry| {
            !matches!(
                entry.result,
                ReflectResult::Created | ReflectResult::Updated | ReflectResult::Behind
            )
        })
    }
}
