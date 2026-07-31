use std::path::{Path, PathBuf};

use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::msg;
use crate::paths::{self};

use super::{
    ConfigLocation, ConfigState, GlobalConfig, ensure_config_dir, parse, read_existing, render,
    replace_line, write_config,
};

/// 表示言語だけをconfigへ保存する。
///
/// 既存configがあれば`language`の行だけを足すか差し替え、利用者が手で書いたコメント、
/// 空行、key順、`files`をそのまま残す。既知fieldだけで全文書を描き直さない。
///
/// 行単位の編集はYAMLの書き方すべてを扱えない。書く前に編集結果を読み直し、意図した
/// 設定にならないなら、利用者のfileを壊さず拒否する。
pub fn save_language(location: &ConfigLocation, locale: Locale) -> Result<PathBuf> {
    ensure_config_dir(location)?;
    let path = location.config_file();
    let line = format!("language: {}", locale.as_str());

    let updated = match read_existing(&path)? {
        Some(text) => {
            let updated = replace_line(&text, "language:", &line);
            require_declares_language(&updated, &path, locale)?;
            updated
        }
        None => render(&GlobalConfig {
            language: Some(locale),
            git_identity: None,
            files: Vec::new(),
        })?,
    };

    write_config(&path, &updated)?;
    Ok(path)
}

/// 編集結果が、意図した言語だけを足した有効なconfigになっているか。
fn require_declares_language(updated: &str, path: &Path, locale: Locale) -> Result<()> {
    if let Ok(ConfigState::Valid { config, .. }) = parse(updated, path)
        && config.language == Some(locale)
    {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::ConfigNotRewritable,
            msg!(
                "error-config-not-rewritable",
                path = paths::display(path),
                field = "language"
            ),
        )
        .remediation(msg!(
            "remediation-config-not-rewritable",
            path = paths::display(path),
            declaration = line_of(locale)
        )),
    ))
}

/// 利用者へ書き足してもらう1行。
fn line_of(locale: Locale) -> String {
    format!("language: {}", locale.as_str())
}
