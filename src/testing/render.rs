use crate::testing::outcome::{Checked, Required};

use crate::design::document::Document;
use crate::design::policy::StreamPolicy;
use crate::design::renderer::Renderer;
use crate::i18n::{Catalog, Locale};

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
