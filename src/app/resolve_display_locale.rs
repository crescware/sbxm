use crate::cli::PeekedLang;
use crate::config::{self, ConfigLocation, ConfigState};
use crate::i18n::{Locale, shell_locale};

/// helpとusageのlocaleを決める。
///
/// 1. argvから先読みした有効な`--lang`
/// 2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
/// 3. shell locale
/// 4. `en`
pub(super) fn resolve_display_locale(peeked: &PeekedLang, location: &ConfigLocation) -> Locale {
    if let PeekedLang::Valid(locale) = peeked {
        return *locale;
    }
    // configが不在、構文不正、未知version、permission不正、symlink、read失敗のいずれでも
    // help表示自体は妨げず、shell localeへfallbackする。
    if let Ok(ConfigState::Valid { config, .. }) = config::load(location)
        && let Some(locale) = config.language
    {
        return locale;
    }
    shell_locale().unwrap_or(Locale::SOURCE)
}
