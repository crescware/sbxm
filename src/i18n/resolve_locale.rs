use super::Locale;

pub(crate) fn resolve_locale(
    command_line: Option<Locale>,
    configured: Option<Locale>,
    shell: Option<Locale>,
) -> Locale {
    command_line
        .or(configured)
        .or(shell)
        .unwrap_or(Locale::SOURCE)
}

#[cfg(test)]
#[path = "resolve_locale_test.rs"]
mod resolve_locale_test;
