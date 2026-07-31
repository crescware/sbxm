use super::{Locale, LocaleDefinition};

pub(super) const JA: LocaleDefinition = LocaleDefinition {
    locale: Locale::Ja,
    tag: "ja",
    ftl: include_str!("../../locales/ja.ftl"),
};
