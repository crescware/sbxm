use std::path::Path;

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::git;
use crate::msg;
use crate::paths::{self};
use crate::repository::RepositoryIdentity;

use crate::metadata::document::{
    RawGitIdentity, RawMetadata, RawProvisioning, RawRebuild, RawRepository, RawStartRef,
};
use crate::metadata::{
    CreationMode, DOCUMENT_VERSION, GitIdentity, MAX_WORKTREES, MIN_WORKTREES, ProjectMetadata,
    Provisioning, RebuildIntent, validate_git_identity_value,
};

use super::{missing, require_sha256};

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

    DOCUMENT_VERSION.require(raw.version, &paths::display(path), || {
        missing(path, "version")
    })?;

    let repository = parse_repository(raw.repository, path)?;
    let provisioning = parse_provisioning(raw.provisioning, path)?;
    let git_identity = parse_git_identity(raw.git_identity, path)?;
    let rebuild = parse_rebuild(raw.rebuild, path)?;

    Ok(ProjectMetadata {
        repository,
        provisioning,
        git_identity,
        rebuild,
    })
}

/// fieldの値が受け付けられないことを報告する。
fn invalid(path: &Path, field: &'static str, detail: &str) -> Error {
    Error::single(reported(path, field).fact(Fact::cause(detail)))
}

/// 受け付けられない理由をmessageで受け取る形。
fn invalid_because(path: &Path, field: &'static str, reason: Msg) -> Error {
    Error::single(reported(path, field).fact(Fact::reason(reason)))
}

fn reported(path: &Path, field: &'static str) -> Diagnostic {
    Diagnostic::new(
        ErrorId::MetadataInvalidValue,
        msg!(
            "error-metadata-invalid-value",
            path = paths::display(path),
            field = field
        ),
    )
}

/// clone URLを正本として読み直し、ほかのfieldがその解釈と一致することを確かめる。
fn parse_repository(raw: Option<RawRepository>, path: &Path) -> Result<RepositoryIdentity> {
    let declared = raw.ok_or_else(|| missing(path, "repository"))?;
    let provider = declared
        .provider
        .ok_or_else(|| missing(path, "repository.provider"))?;
    let owner = declared
        .owner
        .ok_or_else(|| missing(path, "repository.owner"))?;
    let name = declared
        .name
        .ok_or_else(|| missing(path, "repository.name"))?;
    let canonical_value = declared
        .canonical_id
        .ok_or_else(|| missing(path, "repository.canonical_id"))?;
    let transport = declared
        .clone_transport
        .ok_or_else(|| missing(path, "repository.clone_transport"))?;
    let clone_url = declared
        .clone_url
        .ok_or_else(|| missing(path, "repository.clone_url"))?;
    RepositoryIdentity::from_parts(
        &provider,
        &owner,
        &name,
        &canonical_value,
        &transport,
        &clone_url,
    )
    .map_err(|reason| invalid_because(path, "repository", reason))
}

/// 構築の指定を読む。
fn parse_provisioning(raw: Option<RawProvisioning>, path: &Path) -> Result<Provisioning> {
    let provisioning = raw.ok_or_else(|| missing(path, "provisioning"))?;
    let mode_value = provisioning
        .mode
        .ok_or_else(|| missing(path, "provisioning.mode"))?;
    let mode = CreationMode::parse(&mode_value).ok_or_else(|| {
        invalid(
            path,
            "provisioning.mode",
            &format!(
                "{mode_value} is neither {} nor {}",
                CreationMode::Attached,
                CreationMode::Detached
            ),
        )
    })?;

    // keyの欠落は記録そのものの欠落、`null`は起点branchが未確定であることを指す。
    let start_ref = match provisioning.start_ref {
        RawStartRef::Missing => return Err(missing(path, "provisioning.start_ref")),
        RawStartRef::Unset => {
            if mode == CreationMode::Detached {
                return Err(invalid(
                    path,
                    "provisioning.start_ref",
                    &format!("{mode} mode requires an explicit start branch"),
                ));
            }
            None
        }
        RawStartRef::Named(value) => {
            git::validate_branch_name(&value).map_err(|_| {
                invalid(
                    path,
                    "provisioning.start_ref",
                    &format!("{value} is not a branch name"),
                )
            })?;
            Some(value)
        }
    };

    let requested = provisioning
        .requested_worktrees
        .ok_or_else(|| missing(path, "provisioning.requested_worktrees"))?;
    let requested_worktrees = u32::try_from(requested)
        .ok()
        .filter(|value| (MIN_WORKTREES..=MAX_WORKTREES).contains(value))
        .ok_or_else(|| {
            invalid(
                path,
                "provisioning.requested_worktrees",
                &format!("{requested} is outside {MIN_WORKTREES}-{MAX_WORKTREES}"),
            )
        })?;

    let dockerfile_sha256 = provisioning
        .dockerfile_sha256
        .ok_or_else(|| missing(path, "provisioning.dockerfile_sha256"))?;
    require_sha256(&dockerfile_sha256)
        .map_err(|detail| invalid(path, "provisioning.dockerfile_sha256", &detail))?;

    Ok(Provisioning {
        mode,
        start_ref,
        requested_worktrees,
        dockerfile_sha256,
    })
}

/// 登録時に固定した名義を読む。
fn parse_git_identity(raw: Option<RawGitIdentity>, path: &Path) -> Result<GitIdentity> {
    let declared = raw.ok_or_else(|| missing(path, "git_identity"))?;
    let user_name = declared
        .user_name
        .ok_or_else(|| missing(path, "git_identity.user_name"))?;
    let user_email = declared
        .user_email
        .ok_or_else(|| missing(path, "git_identity.user_email"))?;
    validate_git_identity_value(&user_name)
        .map_err(|reason| invalid_because(path, "git_identity.user_name", reason))?;
    validate_git_identity_value(&user_email)
        .map_err(|reason| invalid_because(path, "git_identity.user_email", reason))?;
    Ok(GitIdentity {
        user_name,
        user_email,
    })
}

/// 途中で止まった世代交代の記録を読む。
fn parse_rebuild(raw: Option<RawRebuild>, path: &Path) -> Result<Option<RebuildIntent>> {
    let Some(rebuild) = raw else {
        return Ok(None);
    };
    let target = rebuild
        .target_dockerfile_sha256
        .ok_or_else(|| missing(path, "rebuild.target_dockerfile_sha256"))?;
    require_sha256(&target)
        .map_err(|detail| invalid(path, "rebuild.target_dockerfile_sha256", &detail))?;
    let previous = rebuild
        .previous_dockerfile_sha256
        .ok_or_else(|| missing(path, "rebuild.previous_dockerfile_sha256"))?;
    require_sha256(&previous)
        .map_err(|detail| invalid(path, "rebuild.previous_dockerfile_sha256", &detail))?;
    Ok(Some(RebuildIntent {
        target_dockerfile_sha256: target,
        previous_dockerfile_sha256: previous,
    }))
}

#[cfg(test)]
#[path = "parse_test.rs"]
mod parse_test;
