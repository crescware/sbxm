//! `registry.yaml`の書き出し。
//!
//! 追記ではなく、memory上の完全なdocumentを毎回描き直す。

use crate::paths;

use super::{
    Index, REGISTRY_VERSION,
    document::{RawEntry, RawRegistry},
};

/// registryをYAMLへ描画する。
pub fn render(registry: &Index) -> crate::diagnostics::Result<String> {
    let raw = RawRegistry {
        version: Some(i64::from(REGISTRY_VERSION)),
        projects: registry
            .entries()
            .iter()
            .map(|entry| RawEntry {
                canonical_id: Some(entry.canonical_id().as_str().to_string()),
                project_root: Some(paths::display(entry.project_root())),
                provider: Some(entry.repository().provider().as_str().to_string()),
                clone_transport: Some(entry.repository().transport().as_str().to_string()),
                clone_url: Some(entry.repository().clone_url().to_string()),
            })
            .collect(),
    };
    crate::config::serialized(&raw, "registry.yaml")
}
