use std::path::{Path, PathBuf};

use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self};

use super::{
    ConfigLocation, ConfigState, FileDeclaration, GlobalConfig, append_file_entry, declaration,
    ensure_config_dir, parse, read_existing, render, write_config,
};

/// 宣言fileを1件、configの`files`の末尾へ足す。
///
/// `save_language`と同じ契約で、既存configの原文を保ったまま宣言だけを足す。足したあとの
/// configを読み直し、既存の宣言のあとにこの宣言だけが加わった有効なconfigにならなければ、
/// 利用者のfileを壊さず拒否する。
pub fn save_file_declaration(
    location: &ConfigLocation,
    declared: &FileDeclaration,
) -> Result<PathBuf> {
    ensure_config_dir(location)?;
    let path = location.config_file();

    let updated = match read_existing(&path)? {
        Some(text) => appended(&text, &path, declared)?,
        None => render(&GlobalConfig {
            language: None,
            git_identity: None,
            files: vec![declared.clone()],
        })?,
    };

    write_config(&path, &updated)?;
    Ok(path)
}

/// 既存configの原文へ宣言を足す。既存の宣言のあとにこの宣言だけが加わらなければ拒否する。
fn appended(text: &str, path: &Path, declared: &FileDeclaration) -> Result<String> {
    // 読めないconfigへ足すと、読めなかった理由が見えなくなる。先にその理由で止める。
    let mut expected = parse(text, path)?.settings().files;
    expected.push(declared.clone());

    let source = paths::display(declared.source.as_path());
    let destination = paths::display(declared.destination.as_path());
    if let Some(updated) = append_file_entry(text, &source, &destination)
        && let Ok(ConfigState::Valid { config, .. }) = parse(&updated, path)
        && config.files == expected
    {
        return Ok(updated);
    }
    Err(not_rewritable(path, declared))
}

fn not_rewritable(path: &Path, declared: &FileDeclaration) -> Error {
    // 書けないと分かった以上、利用者が手で書き足せる宣言をそのまま渡す。
    let lines = || -> Result<String> {
        Ok(format!(
            "files:\n  - {}\n    {}",
            declaration("source", &paths::display(declared.source.as_path()))?,
            declaration(
                "destination",
                &paths::display(declared.destination.as_path())
            )?
        ))
    };
    let mut diagnostic = Diagnostic::new(
        ErrorId::ConfigNotRewritable,
        msg!(
            "error-config-not-rewritable",
            path = paths::display(path),
            field = "files"
        ),
    );
    if let Ok(lines) = lines() {
        diagnostic = diagnostic.remediation(msg!(
            "remediation-config-not-rewritable",
            path = paths::display(path),
            declaration = lines
        ));
    }
    Error::single(diagnostic)
}
