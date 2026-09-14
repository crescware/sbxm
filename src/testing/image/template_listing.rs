use crate::testing::outcome::Checked;

use super::template_listing_with_id;

/// runtimeのimage storeが示す一覧。registry prefixを補って表示する。
pub fn template_listing(image: &str) -> Checked<String> {
    template_listing_with_id(image, "a3d0f4449170")
}
