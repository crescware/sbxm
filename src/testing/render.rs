//! documentを文字列として確かめる。
//!
//! 描画条件を明示して渡すため、testは端末の有無にも環境変数にも左右されない。

use crate::testing::outcome::{Checked, Required};

use crate::i18n::{Catalog, Locale};
use crate::ui::document::Document;
use crate::ui::policy::StreamPolicy;
use crate::ui::renderer::Renderer;

/// 色を出さない描画。既定の比較対象とする。
pub fn plain(document: &Document, locale: Locale) -> Checked<String> {
    render(document, locale, StreamPolicy::plain())
}

/// 指定した条件での描画。
pub fn render(document: &Document, locale: Locale, policy: StreamPolicy) -> Checked<String> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut renderer = Renderer::new(&mut buffer, policy);
        renderer.write(&Catalog::new(locale), document);
    }
    String::from_utf8(buffer)
        .required_because("the renderer writes UTF-8 except for external output")
}
