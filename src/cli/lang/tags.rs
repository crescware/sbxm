use crate::i18n::Locale;

/// 組み込みlocaleのtag。`--lang`が受け付ける値と一致する。
pub(super) fn tags() -> Vec<&'static str> {
    Locale::ALL
        .iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
}
