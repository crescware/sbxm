use crate::i18n::Locale;

pub(super) fn supported_language_list() -> String {
    Locale::ALL
        .iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
