use std::path::{Path, PathBuf};

use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::GitIdentity;
use crate::msg;
use crate::paths::{self};

use super::{
    ConfigLocation, ConfigState, GlobalConfig, declaration, ensure_config_dir, parse,
    read_existing, render, replace_line, write_config,
};

/// 選ばれたGit identityだけをconfigへ保存する。
///
/// `save_language`と同じ契約で、既存configの原文を保ったまま2行だけを足すか差し替える。
/// 名義は2つで1つの意図であるため、片方だけが書かれた状態を残さない。
pub fn save_git_identity(location: &ConfigLocation, git: &GitIdentity) -> Result<PathBuf> {
    ensure_config_dir(location)?;
    let path = location.config_file();

    let updated = match read_existing(&path)? {
        Some(text) => {
            // 新しく足す行は`version`の直後へ入る。あとの呼び出しが前の行を押し下げる
            // ため、読み手が期待する名前・mail addressの順に並ぶよう逆から書く。
            let updated = replace_line(&text, "git_user_email:", &email_line(git)?);
            let updated = replace_line(&updated, "git_user_name:", &name_line(git)?);
            require_declares_git_identity(&updated, &path, git)?;
            updated
        }
        None => render(&GlobalConfig {
            language: None,
            git_identity: Some(git.clone()),
            files: Vec::new(),
        })?,
    };

    write_config(&path, &updated)?;
    Ok(path)
}

/// 編集結果が、意図した名義だけを足した有効なconfigになっているか。
fn require_declares_git_identity(updated: &str, path: &Path, git: &GitIdentity) -> Result<()> {
    if let Ok(ConfigState::Valid { config, .. }) = parse(updated, path)
        && config.git_identity.as_ref() == Some(git)
    {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::ConfigNotRewritable,
            msg!(
                "error-config-not-rewritable",
                path = paths::display(path),
                field = "git_user_name, git_user_email"
            ),
        )
        .remediation(msg!(
            "remediation-config-not-rewritable",
            path = paths::display(path),
            declaration = format!("{}\n{}", name_line(git)?, email_line(git)?)
        )),
    ))
}

fn name_line(git: &GitIdentity) -> Result<String> {
    declaration("git_user_name", &git.user_name)
}

fn email_line(git: &GitIdentity) -> Result<String> {
    declaration("git_user_email", &git.user_email)
}
