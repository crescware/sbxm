use crate::testing::outcome::Checked;

use crate::metadata::METADATA_VERSION;
use crate::testing::value::DIGEST;

use crate::support::image::*;

use super::canonical;

pub fn declared_labels() -> Checked<Vec<(&'static str, String)>> {
    Ok(vec![
        (LABEL_CANONICAL_ID, canonical()?.to_string()),
        (LABEL_DOCKERFILE_SHA256, DIGEST.to_string()),
        (LABEL_METADATA_VERSION, METADATA_VERSION.to_string()),
    ])
}
