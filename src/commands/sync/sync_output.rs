use std::path::PathBuf;

use crate::support::host_sync::{ReflectResult, Reflected};

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

    /// gitが断ったrefが、hostの側かSandboxのoriginの側にあるか。
    ///
    /// hostの側は`left_as_it_was`と同じである。Sandboxのoriginの側は、同じ名前で別の先を
    /// 指すtagのように、Sandboxが自分の側を残したものである。Sandboxの中の`git fetch`も、
    /// 既存のtagを上書きできなければ失敗で終わる。
    pub fn refused_any(&self) -> bool {
        self.left_as_it_was()
            || self
                .sent
                .iter()
                .any(|change| matches!(change, SentChange::Refused { .. }))
    }
}
