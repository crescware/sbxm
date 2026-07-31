//! `project.yaml`の書き出し。

use super::document::{RawGitIdentity, RawMetadata, RawProvisioning, RawRebuild, RawRepository};
use super::{METADATA_VERSION, ProjectMetadata};

/// metadataをYAMLへ描画する。
pub fn render(metadata: &ProjectMetadata) -> String {
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
            start_ref: Some(provisioning.start_ref.clone()),
            requested_worktrees: Some(i64::from(provisioning.requested_worktrees)),
            dockerfile_sha256: Some(provisioning.dockerfile_sha256.clone()),
        }),
        git_identity: Some(RawGitIdentity {
            user_name: Some(metadata.git_identity.user_name.clone()),
            user_email: Some(metadata.git_identity.user_email.clone()),
        }),
        rebuild: metadata.rebuild.as_ref().map(|rebuild| RawRebuild {
            target_dockerfile_sha256: Some(rebuild.target_dockerfile_sha256.clone()),
            previous_dockerfile_sha256: Some(rebuild.previous_dockerfile_sha256.clone()),
        }),
    };
    // RawMetadataは文字列と整数だけで構成され、YAMLで表現できない値を持たない。
    yaml_serde::to_string(&raw).expect("project metadata is representable as YAML")
}
