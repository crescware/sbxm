use crate::diagnostics::{DocumentVersion, ErrorId};

use super::REGISTRY_VERSION;

/// registryのversionの読み方。
pub(super) const DOCUMENT_VERSION: DocumentVersion = DocumentVersion {
    supported: REGISTRY_VERSION,
    unknown: ErrorId::RegistryUnknownVersion,
    unknown_message: "error-registry-unknown-version",
};
