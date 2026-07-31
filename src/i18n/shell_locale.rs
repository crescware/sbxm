use super::Locale;

/// shell localeから表示言語を推測する。
///
/// `LC_ALL`、`LC_MESSAGES`、`LANG`の順に見る。
pub fn shell_locale() -> Option<Locale> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(value) = std::env::var_os(key)
            && let Some(locale) = Locale::from_language_tag(&value.to_string_lossy())
        {
            return Some(locale);
        }
    }
    None
}
