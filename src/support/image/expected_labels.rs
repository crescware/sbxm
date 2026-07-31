use crate::metadata::METADATA_VERSION;
use crate::project::CanonicalProjectId;

use super::{LABEL_CANONICAL_ID, LABEL_DOCKERFILE_SHA256, LABEL_METADATA_VERSION};

/// 案件と世代が一致することを宣言するlabelの組。
pub fn expected_labels(
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Vec<(String, String)> {
    vec![
        (LABEL_CANONICAL_ID.to_string(), canonical.to_string()),
        (
            LABEL_DOCKERFILE_SHA256.to_string(),
            dockerfile_sha256.to_string(),
        ),
        (
            LABEL_METADATA_VERSION.to_string(),
            METADATA_VERSION.to_string(),
        ),
    ]
}
