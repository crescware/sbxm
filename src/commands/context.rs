use std::path::Path;

use crate::cli::Interactivity;
use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};
use crate::diagnostics::Result;
use crate::i18n::{Locale, shell_locale};

/// command固有でない実行の入力。
pub struct Context<'a> {
    pub location: &'a ConfigLocation,
    /// Sandboxのworkspaceを置くhost側のroot。
    ///
    /// 実行環境を名指すため、`location`と同じく`main`が選んだものだけを使う。commandが
    /// 自分で正本の定数へ手を伸ばすと、その経路は実hostのpathでしか動かなくなる。
    pub workspace_root: &'a Path,
    /// argvが選んだ表示言語。configの`language`より優先する。
    pub lang: Option<Locale>,
    pub interactivity: Interactivity,
}

impl Context<'_> {
    /// 利用者設定と、この実行の表示言語。
    ///
    /// configの不在は正常であり、default設定として扱う。表示言語は`--lang`、保存済みの
    /// `language`、system locale、正本localeの順で決める。
    pub fn settings(&self) -> Result<(GlobalConfig, Locale)> {
        let config = config::load(self.location)?.settings();
        let locale = self.locale_of(config.language);
        Ok((config, locale))
    }

    /// configがなくても続けるcommandの表示言語。
    ///
    /// `status --global`はconfigの状態そのものを診断結果として報告するため、読めない
    /// configを実行前の失敗として扱わない。
    pub fn tolerant_locale(&self) -> Result<Locale> {
        let declared = match config::load(self.location)? {
            ConfigState::Valid { config, .. } => config.language,
            ConfigState::Missing => None,
        };
        Ok(self.locale_of(declared))
    }

    fn locale_of(&self, declared: Option<Locale>) -> Locale {
        self.lang
            .or(declared)
            .or_else(shell_locale)
            .unwrap_or(Locale::SOURCE)
    }
}
