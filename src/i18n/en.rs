use super::{Locale, LocaleDefinition};

pub(super) const EN: LocaleDefinition = LocaleDefinition {
    locale: Locale::En,
    tag: "en",
    ftl: include_str!("../../locales/en.ftl"),
};
