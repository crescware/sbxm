use crate::i18n::Locale;

use super::ConfigLocation;

/// 起動時に一度だけ行ったconfigの観測。
///
/// config全体やload errorをcacheせず、locale候補と後続処理が使うlocationだけを運ぶ。
#[derive(Debug)]
pub(crate) struct ConfigObservation {
    location: ConfigLocation,
    language: Option<Locale>,
}

impl ConfigObservation {
    pub(crate) fn new(location: ConfigLocation, language: Option<Locale>) -> Self {
        Self { location, language }
    }

    pub(crate) fn location(&self) -> &ConfigLocation {
        &self.location
    }

    pub(crate) fn language(&self) -> Option<Locale> {
        self.language
    }
}
