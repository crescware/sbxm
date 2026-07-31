use super::{EN, JA, LocaleDefinition};

/// 組み込みlocaleの定義。
///
/// 言語を増やすときは、[`Locale`]のvariantとこの表の行、そして`locales/<tag>.ftl`だけを
/// 足す。ほかのmoduleとtestは、この表からの導出だけを見る。
pub(super) const DEFINITIONS: [LocaleDefinition; 2] = [EN, JA];
