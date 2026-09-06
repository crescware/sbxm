//! 案件専用Templateのimage。
//!
//! Dockerfileの世代ごとにimageを持ち、labelで案件と世代を宣言する。buildは、
//! project fileもsecretも入らない空の一時directoryをbuild contextとして使う。

mod build;
mod build_context_prefix;
mod built_image;
mod cleanup_stale_archives;
#[cfg(test)]
#[path = "cleanup_stale_archives_test.rs"]
mod cleanup_stale_archives_test;
mod empty_private_context;
mod ensure;
mod ensure_archive;
mod ephemeral_context;
mod expected_labels;
#[cfg(test)]
mod fake;
mod generation_is_built;
mod image_name;
mod inspect;
mod label_canonical_id;
mod label_dockerfile_sha256;
mod label_metadata_version;
mod labels_match;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_test;
mod transient_archive;
#[cfg(test)]
#[path = "transient_archive_test.rs"]
mod transient_archive_test;

use build::build;
use build_context_prefix::BUILD_CONTEXT_PREFIX;
pub use built_image::BuiltImage;
pub use cleanup_stale_archives::cleanup_stale_archives;
use empty_private_context::empty_private_context;
pub use ensure::ensure;
pub use ensure_archive::ensure_archive;
use ephemeral_context::ephemeral_context;
pub use expected_labels::expected_labels;
pub use generation_is_built::generation_is_built;
pub use image_name::image_name;
pub use inspect::inspect;
pub use label_canonical_id::LABEL_CANONICAL_ID;
pub use label_dockerfile_sha256::LABEL_DOCKERFILE_SHA256;
pub use label_metadata_version::LABEL_METADATA_VERSION;
pub use labels_match::labels_match;
pub use transient_archive::TransientArchive;
