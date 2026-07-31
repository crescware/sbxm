use super::Locale;

/// 1 localeの定義。
pub(super) struct LocaleDefinition {
    pub(super) locale: Locale,
    /// `--lang`とconfigで使う安定した表記。翻訳しない。
    pub(super) tag: &'static str,
    /// 組み込みFTL resource。
    pub(super) ftl: &'static str,
}
