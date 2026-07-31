use crate::diagnostics::{DocumentVersion, ErrorId};

use super::CONFIG_VERSION;

/// configのversionの読み方。
pub(super) const DOCUMENT_VERSION: DocumentVersion = DocumentVersion {
    supported: CONFIG_VERSION,
    unknown: ErrorId::ConfigUnknownVersion,
    unknown_message: "error-config-unknown-version",
};
