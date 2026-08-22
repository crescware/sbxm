use std::path::Path;

use crate::config::{self, ConfigLocation, GlobalConfig};
use crate::diagnostics::Result;
use crate::i18n::Locale;

/// command固有でない実行の入力。
pub struct Context<'a> {
    pub location: &'a ConfigLocation,
    /// Sandboxのworkspaceを置くhost側のroot。
    ///
    /// 実行環境を名指すため、`location`と同じく`main`が選んだものだけを使う。commandが
    /// 自分で正本の定数へ手を伸ばすと、その経路は実hostのpathでしか動かなくなる。
    pub workspace_root: &'a Path,
    /// この実行で使う表示言語。
    pub locale: Locale,
    pub can_prompt: bool,
}

impl Context<'_> {
    /// 利用者設定と、この実行の表示言語。
    ///
    /// configの不在は正常であり、default設定として扱う。
    pub fn settings(&self) -> Result<(GlobalConfig, Locale)> {
        let config = config::load(self.location)?.settings();
        Ok((config, self.locale))
    }

    /// configがなくても続けるcommandの表示言語。
    ///
    /// `status --global`はconfigの状態そのものを診断結果として報告するため、読めない
    /// configを実行前の失敗として扱わない。
    pub fn tolerant_locale(&self) -> Result<Locale> {
        let _ = config::load(self.location)?;
        Ok(self.locale)
    }
}
