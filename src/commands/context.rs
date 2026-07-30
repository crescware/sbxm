//! commandの実行に共通する文脈。
//!
//! 表示localeはconfigのvalidationを経て確定する。configを読む前に失敗した場合は、
//! argvとshellだけで決めたlocaleで報告する。
//!
//! 文脈は描画そのものを持たない。利用者向けの出力は`Ui`が受け持ち、commandは
//! `&mut Ui`を明示的な引数として受け取る。borrowを単純に保つため、文脈へ埋め込まない。

use crate::cli::Interactivity;
use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};
use crate::error::{Diagnostic, Error, ErrorId, ExitCode, Result};
use crate::i18n::{Locale, shell_locale};
use crate::msg;
use crate::paths;
use crate::ui::{Remediation, Ui};

/// command固有でない実行の入力。
pub struct Context<'a> {
    pub location: &'a ConfigLocation,
    /// argvが選んだ表示言語。configの`language`より優先する。
    pub lang: Option<Locale>,
    pub interactivity: Interactivity,
}

impl Context<'_> {
    /// 案件を対象とするcommandが必要とするconfigと、宣言された表示言語。
    pub fn require_config(&self) -> Result<(Box<GlobalConfig>, Locale)> {
        match config::load(self.location)? {
            ConfigState::Valid { config, .. } => {
                let locale = self.lang.unwrap_or(config.language);
                Ok((config, locale))
            }
            ConfigState::Missing => Err(self.missing_config()),
        }
    }

    /// configがなくても続けるcommandの表示言語。
    ///
    /// `status --global`はconfig不在そのものを診断結果として報告するため、読めない
    /// configを実行前の失敗として扱わない。
    pub fn tolerant_locale(&self) -> Result<Locale> {
        match config::load(self.location)? {
            ConfigState::Valid { config, .. } => Ok(self.lang.unwrap_or(config.language)),
            ConfigState::Missing => Ok(self.lang.or_else(shell_locale).unwrap_or(Locale::En)),
        }
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
            .remediation(Remediation::text(msg!("remediation-run-init")).try_run("sbxm init")),
        )
    }
}

/// errorを表示し、そのexit codeを返す。
pub fn report(ui: &mut Ui, error: &Error) -> ExitCode {
    ui.error(error);
    error.exit_code()
}
