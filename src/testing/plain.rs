use crate::testing::outcome::Checked;

use crate::design::document::Document;
use crate::design::policy::StreamPolicy;
use crate::i18n::Locale;

use super::render::render;

/// 色を出さない描画。既定の比較対象とする。
pub fn plain(document: &Document, locale: Locale) -> Checked<String> {
    render(document, locale, StreamPolicy::plain())
}
