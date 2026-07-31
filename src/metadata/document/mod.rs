//! `project.yaml`の生表現。読み取りと書き出しが同じ形を共有する。
//!
//! ここでは値の妥当性を判定しない。structへ写した後の検査は`parse`が持つ。

mod raw_git_identity;
mod raw_metadata;
mod raw_provisioning;
mod raw_rebuild;
mod raw_repository;
mod raw_start_ref;

pub(super) use raw_git_identity::RawGitIdentity;
pub(super) use raw_metadata::RawMetadata;
pub(super) use raw_provisioning::RawProvisioning;
pub(super) use raw_rebuild::RawRebuild;
pub(super) use raw_repository::RawRepository;
pub(super) use raw_start_ref::RawStartRef;
