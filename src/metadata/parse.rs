//! `project.yaml`の読み取り。
//!
//! 解釈できない値から目標構成を推測せず、validation規則に合わない文書は拒否する。

use std::path::Path;

use crate::error::{Error, ErrorId, Result};
use crate::git;
use crate::msg;
use crate::paths::{self};
use crate::project::{CanonicalProjectId, ProjectId};

use super::document::RawMetadata;
use super::{
    CreationMode, DOCUMENT, MAX_WORKTREES, MIN_WORKTREES, ProjectMetadata, Provisioning,
    RebuildIntent,
};

/// metadataのtextを検証する。
pub fn parse(text: &str, path: &Path) -> Result<ProjectMetadata> {
    // 空のdocumentはnullとして読める。keyを1つも持たないmappingと同じ扱いにし、
    // 欠落したfieldをsyntax errorではなく名前で報告する。
    let raw = yaml_serde::from_str::<Option<RawMetadata>>(text)
        .map_err(|error: yaml_serde::Error| {
            Error::new(
                ErrorId::MetadataInvalidSyntax,
                msg!(
                    "error-metadata-invalid-syntax",
                    path = paths::display(path),
                    detail = error
                ),
            )
        })?
        .unwrap_or_default();

    let missing = |field: &'static str| {
        Error::new(
            ErrorId::MetadataMissingField,
            msg!(
                "error-metadata-missing-field",
                path = paths::display(path),
                field = field
            ),
        )
    };
    let invalid = |field: &'static str, detail: String| {
        Error::new(
            ErrorId::MetadataInvalidValue,
            msg!(
                "error-metadata-invalid-value",
                path = paths::display(path),
                field = field,
                detail = detail
            ),
        )
    };

    DOCUMENT.require(raw.version, &paths::display(path), || missing("version"))?;

    let owner = raw.owner.ok_or_else(|| missing("owner"))?;
    let repository = raw.repository.ok_or_else(|| missing("repository"))?;
    let canonical_value = raw.canonical_id.ok_or_else(|| missing("canonical_id"))?;

    let display_id = ProjectId::parse(&format!("{owner}/{repository}"))
        .map_err(|_| invalid("owner", format!("{owner}/{repository} is not a project ID")))?;
    let canonical_id = CanonicalProjectId::parse(&canonical_value).map_err(|_| {
        invalid(
            "canonical_id",
            format!("{canonical_value} is not a canonical project ID"),
        )
    })?;
    if display_id.canonical() != canonical_id {
        return Err(invalid(
            "canonical_id",
            format!(
                "{canonical_id} does not match owner and repository, which fold to {}",
                display_id.canonical()
            ),
        ));
    }

    let provisioning = raw.provisioning.ok_or_else(|| missing("provisioning"))?;
    let mode_value = provisioning
        .mode
        .ok_or_else(|| missing("provisioning.mode"))?;
    let mode = CreationMode::parse(&mode_value).ok_or_else(|| {
        invalid(
            "provisioning.mode",
            format!(
                "{mode_value} is neither {} nor {}",
                CreationMode::Attached,
                CreationMode::Detached
            ),
        )
    })?;

    // keyの欠落は記録そのものの欠落、`null`は起点branchが未確定であることを指す。
    let start_ref = match provisioning
        .start_ref
        .ok_or_else(|| missing("provisioning.start_ref"))?
    {
        None => {
            if mode == CreationMode::Detached {
                return Err(invalid(
                    "provisioning.start_ref",
                    format!("{mode} mode requires an explicit start branch"),
                ));
            }
            None
        }
        Some(value) => {
            git::validate_branch_name(&value).map_err(|_| {
                invalid(
                    "provisioning.start_ref",
                    format!("{value} is not a branch name"),
                )
            })?;
            Some(value)
        }
    };

    let requested = provisioning
        .requested_worktrees
        .ok_or_else(|| missing("provisioning.requested_worktrees"))?;
    let requested_worktrees = u32::try_from(requested)
        .ok()
        .filter(|value| (MIN_WORKTREES..=MAX_WORKTREES).contains(value))
        .ok_or_else(|| {
            invalid(
                "provisioning.requested_worktrees",
                format!("{requested} is outside {MIN_WORKTREES}-{MAX_WORKTREES}"),
            )
        })?;

    let dockerfile_sha256 = provisioning
        .dockerfile_sha256
        .ok_or_else(|| missing("provisioning.dockerfile_sha256"))?;
    require_sha256(&dockerfile_sha256)
        .map_err(|detail| invalid("provisioning.dockerfile_sha256", detail))?;

    let rebuild = match raw.rebuild {
        Some(rebuild) => {
            let target = rebuild
                .target_dockerfile_sha256
                .ok_or_else(|| missing("rebuild.target_dockerfile_sha256"))?;
            require_sha256(&target)
                .map_err(|detail| invalid("rebuild.target_dockerfile_sha256", detail))?;
            let previous = rebuild
                .previous_dockerfile_sha256
                .ok_or_else(|| missing("rebuild.previous_dockerfile_sha256"))?;
            require_sha256(&previous)
                .map_err(|detail| invalid("rebuild.previous_dockerfile_sha256", detail))?;
            Some(RebuildIntent {
                target_dockerfile_sha256: target,
                previous_dockerfile_sha256: previous,
            })
        }
        None => None,
    };

    Ok(ProjectMetadata {
        owner,
        repository,
        canonical_id,
        provisioning: Provisioning {
            mode,
            start_ref,
            requested_worktrees,
            dockerfile_sha256,
        },
        rebuild,
    })
}

/// SHA-256のlowercase hexであること。
pub(super) fn require_sha256(value: &str) -> std::result::Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!("{value} is not a lowercase SHA-256 hex digest"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;
