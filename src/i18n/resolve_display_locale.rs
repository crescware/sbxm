use super::{Locale, shell_locale};

/// CLI、config、shellの候補から表示localeを決める。
///
/// 候補の取得は呼び出し側が行い、ここでは表示localeの優先順位だけを決める。
///
/// 1. argvから先読みした有効な`--lang`
/// 2. read-onlyかつbest-effortで読み込めた有効なglobal configの`language`
/// 3. shell locale
/// 4. `en`
pub(crate) fn resolve_display_locale(
    command_line: Option<Locale>,
    configured: Option<Locale>,
) -> Locale {
    command_line
        .or(configured)
        .or_else(shell_locale)
        .unwrap_or(Locale::SOURCE)
}
