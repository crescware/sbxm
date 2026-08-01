//! `registry.yaml`の読み取り。
//!
//! 解釈できない値から登録内容を推測せず、validation規則に合わない文書は拒否する。
//! entry 1件の不正でdocument全体のmutationを止める。

use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::paths;
use crate::repository::RepositoryIdentity;

use super::{DOCUMENT_VERSION, Index, RegistryEntry, document::RawRegistry};

/// registryのtextを検証する。
pub fn parse(text: &str, path: &Path) -> Result<Index> {
    // 空のdocumentはnullとして読める。keyを1つも持たないmappingと同じ扱いにし、
    // 欠落したfieldをsyntax errorではなく名前で報告する。
    let raw = yaml_serde::from_str::<Option<RawRegistry>>(text)
        .map_err(|error: yaml_serde::Error| {
            Error::single(
                Diagnostic::new(
                    ErrorId::RegistryInvalidSyntax,
                    msg!("error-registry-invalid-syntax"),
                )
                .fact(Fact::path(&paths::display(path)))
                .fact(Fact::cause(&error.to_string())),
            )
        })?
        .unwrap_or_default();

    let missing = |field: String| {
        Error::new(
            ErrorId::RegistryMissingField,
            msg!(
                "error-registry-missing-field",
                path = paths::display(path),
                field = field
            ),
        )
    };
    let invalid = |field: String, reason: Msg| {
        Error::single(
            Diagnostic::new(
                ErrorId::RegistryInvalidValue,
                msg!("error-registry-invalid-value"),
            )
            .fact(Fact::field(&field))
            .fact(Fact::reason(reason)),
        )
    };

    DOCUMENT_VERSION.require(raw.version, &paths::display(path), || {
        missing("version".to_string())
    })?;

    let mut entries = Vec::with_capacity(raw.projects.len());
    for (index, entry) in raw.projects.into_iter().enumerate() {
        let field = |name: &str| format!("projects.{index}.{name}");
        let canonical_id = entry
            .canonical_id
            .ok_or_else(|| missing(field("canonical_id")))?;
        let project_root = entry
            .project_root
            .ok_or_else(|| missing(field("project_root")))?;
        let provider = entry.provider.ok_or_else(|| missing(field("provider")))?;
        let clone_transport = entry
            .clone_transport
            .ok_or_else(|| missing(field("clone_transport")))?;
        let clone_url = entry.clone_url.ok_or_else(|| missing(field("clone_url")))?;

        // clone URLを正本として読み直し、ほかのfieldがその解釈と一致することを確かめる。
        let repository = RepositoryIdentity::from_index_parts(
            &provider,
            &canonical_id,
            &clone_transport,
            &clone_url,
        )
        .map_err(|detail| invalid(field("clone_url"), detail))?;

        entries.push(RegistryEntry::new(Path::new(&project_root), repository)?);
    }

    let mut registry = Index { entries };
    registry.sort();
    Ok(registry)
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;
