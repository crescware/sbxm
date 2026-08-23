use crate::i18n::Locale;

/// 完全parseより前に先読みしたlocale override。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LocaleOverride {
    Absent,
    Valid(Locale),
    Invalid(String),
}
