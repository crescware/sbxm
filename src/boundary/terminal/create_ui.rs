use crate::design::{RenderingPolicy, Ui};
use crate::i18n::Locale;

/// processのstdioをdesignの描画portへ接続する。
pub(crate) fn create_ui(locale: Locale, policy: RenderingPolicy) -> Ui<'static> {
    Ui::new(locale, policy, std::io::stdout(), std::io::stderr())
}
