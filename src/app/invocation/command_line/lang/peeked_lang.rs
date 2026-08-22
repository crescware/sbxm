use crate::i18n::Locale;

/// argvから先読みした`--lang`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeekedLang {
    Absent,
    Valid(Locale),
    Invalid(String),
}
