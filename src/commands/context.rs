//! commandの実行に共通する文脈。
//!
//! 表示localeはconfigのvalidationを経て確定する。configを読む前に失敗した場合は、
//! argvとshellだけで決めたlocaleで報告する。

use crate::cli::Interactivity;
use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};
use crate::error::{Diagnostic, Error, ErrorId, ExitCode, Result};
use crate::i18n::{Catalog, Locale, shell_locale};
use crate::msg;
use crate::paths;
use crate::support::display;

/// command固有でない実行の入力。
pub struct Context<'a> {
    pub location: &'a ConfigLocation,
    /// argvが選んだ表示言語。configの`language`より優先する。
    pub lang: Option<Locale>,
    /// configを読む前に決めた表示locale。
    pub display_locale: Locale,
    pub interactivity: Interactivity,
}

impl Context<'_> {
    /// 案件を対象とするcommandが必要とするconfigとcatalog。
    pub fn require_config(&self) -> Result<(Box<GlobalConfig>, Catalog)> {
        match config::load(self.location)? {
            ConfigState::Valid { config, .. } => {
                let catalog = Catalog::new(self.lang.unwrap_or(config.language));
                Ok((config, catalog))
            }
            ConfigState::Missing => Err(self.missing_config()),
        }
    }

    /// configがなくても続けるcommandのcatalog。
    ///
    /// `status --global`はconfig不在そのものを診断結果として報告するため、読めない
    /// configを実行前の失敗として扱わない。
    pub fn tolerant_catalog(&self) -> Result<Catalog> {
        match config::load(self.location)? {
            ConfigState::Valid { config, .. } => {
                Ok(Catalog::new(self.lang.unwrap_or(config.language)))
            }
            ConfigState::Missing => Ok(Catalog::new(
                self.lang.or_else(shell_locale).unwrap_or(Locale::En),
            )),
        }
    }

    /// configを読む前に使う、argvとshellだけで決めたlocaleのcatalog。
    pub fn fallback_catalog(&self) -> Catalog {
        Catalog::new(self.display_locale)
    }

    fn missing_config(&self) -> Error {
        Error::single(
            Diagnostic::new(
                ErrorId::ConfigMissing,
                msg!(
                    "error-config-missing",
                    path = paths::display(&self.location.config_file())
                ),
            )
            .remediation(msg!("remediation-run-init")),
        )
    }
}

/// errorを表示し、そのexit codeを返す。
pub fn report(catalog: &Catalog, error: &Error) -> ExitCode {
    display::report(catalog, error);
    error.exit_code()
}
