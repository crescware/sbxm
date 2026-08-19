//! `project.yaml`の書き出し。

use super::document::{
    RawGitIdentity, RawInitialProvisioning, RawMetadata, RawProvisioning, RawRebuild,
    RawRepository, RawStartRef,
};
use super::{METADATA_VERSION, ProjectMetadata};

/// metadataをYAMLへ描画する。
pub fn render(metadata: &ProjectMetadata) -> crate::diagnostics::Result<String> {
    let provisioning = &metadata.provisioning;
    let repository = &metadata.repository;
    let raw = RawMetadata {
        version: Some(i64::from(METADATA_VERSION)),
        repository: Some(RawRepository {
            provider: Some(repository.provider().as_str().to_string()),
            owner: Some(repository.owner().to_string()),
            name: Some(repository.name().to_string()),
            canonical_id: Some(repository.canonical_id().as_str().to_string()),
            clone_transport: Some(repository.transport().as_str().to_string()),
            clone_url: Some(repository.clone_url().to_string()),
        }),
        provisioning: Some(RawProvisioning {
            mode: Some(provisioning.mode.as_str().to_string()),
            start_ref: provisioning
                .start_ref
                .clone()
                .map_or(RawStartRef::Unset, RawStartRef::Named),
            requested_worktrees: Some(i64::from(provisioning.requested_worktrees)),
            dockerfile_sha256: Some(provisioning.dockerfile_sha256.clone()),
        }),
        git_identity: Some(RawGitIdentity {
            user_name: Some(metadata.git_identity.user_name.clone()),
            user_email: Some(metadata.git_identity.user_email.clone()),
        }),
        initial_provisioning: metadata.initial_provisioning.as_ref().map(|intent| {
            RawInitialProvisioning {
                target_dockerfile_sha256: Some(intent.target_dockerfile_sha256.clone()),
            }
        }),
        rebuild: metadata.rebuild.as_ref().map(|rebuild| RawRebuild {
            target_dockerfile_sha256: Some(rebuild.target_dockerfile_sha256.clone()),
            previous_dockerfile_sha256: Some(rebuild.previous_dockerfile_sha256.clone()),
        }),
    };
    crate::config::serialized(&raw, "project.yaml")
}
