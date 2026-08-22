use super::{ConfigLocation, ConfigObservation, ConfigState, load};
use crate::diagnostics::Result;

/// 起動時localeの候補と、後続処理が使うconfig locationを観測する。
///
/// locationの発見だけはstartup failureとし、存在するconfigの不備はhelp、version、parse
/// diagnosticを妨げないよう保存localeなしへfallbackする。通常commandは後段で同じlocationを
/// 再loadし、必要なconfig errorを利用者へ返す。
pub(crate) fn observe() -> Result<ConfigObservation> {
    let location = ConfigLocation::discover()?;
    Ok(observe_at(location))
}

fn observe_at(location: ConfigLocation) -> ConfigObservation {
    let language = match load(&location) {
        Ok(ConfigState::Valid { config, .. }) => config.language,
        Ok(ConfigState::Missing) | Err(_) => None,
    };
    ConfigObservation::new(location, language)
}

#[cfg(test)]
#[path = "observe_test.rs"]
mod observe_test;
