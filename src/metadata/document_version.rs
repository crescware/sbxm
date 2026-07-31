use crate::diagnostics::{DocumentVersion, ErrorId};

use super::METADATA_VERSION;

/// metadataのversionの読み方。
pub(super) const DOCUMENT_VERSION: DocumentVersion = DocumentVersion {
    supported: METADATA_VERSION,
    unknown: ErrorId::MetadataUnknownVersion,
    unknown_message: "error-metadata-unknown-version",
};
