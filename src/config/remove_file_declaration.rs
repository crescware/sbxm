use std::path::{Path, PathBuf};

use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths::{self};

use super::{
    ConfigLocation, ConfigState, FileDeclaration, SandboxHomeRelativePath, parse, read_existing,
    remove_file_entry, write_config,
};

/// 配置先が`destination`である宣言を、configの`files`から外す。
///
/// 外すのは宣言だけであり、Sandboxへ既に置いたfileには触れない。外したあとのconfigを
/// 読み直し、その宣言だけが消えた有効なconfigにならなければ、利用者のfileを壊さず拒否する。
/// 外した宣言を返す。
pub fn remove_file_declaration(
    location: &ConfigLocation,
    destination: &SandboxHomeRelativePath,
) -> Result<(PathBuf, FileDeclaration)> {
    let path = location.config_file();
    let Some(text) = read_existing(&path)? else {
        return Err(not_declared(destination));
    };
    let config = parse(&text, &path)?.settings();
    let Some(index) = config
        .files
        .iter()
        .position(|declared| declared.destination.names_same_place(destination))
    else {
        return Err(not_declared(destination));
    };
    let mut expected = config.files.clone();
    let removed = expected.remove(index);

    if let Some(updated) = remove_file_entry(&text, index)
        && let Ok(ConfigState::Valid { config, .. }) = parse(&updated, &path)
        && config.files == expected
    {
        write_config(&path, &updated)?;
        return Ok((path, removed));
    }
    Err(not_removable(&path, &removed))
}

fn not_declared(destination: &SandboxHomeRelativePath) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::FileNotDeclared,
            msg!(
                "error-file-not-declared",
                destination = paths::display(destination.as_path())
            ),
        )
        .remediation(
            Remediation::text(msg!("remediation-file-not-declared")).try_run("sbxm files ls"),
        ),
    )
}

fn not_removable(path: &Path, removed: &FileDeclaration) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ConfigNotRewritable,
            msg!(
                "error-config-not-rewritable",
                path = paths::display(path),
                field = "files"
            ),
        )
        .remediation(msg!(
            "remediation-file-declaration-not-removable",
            path = paths::display(path),
            destination = paths::display(removed.destination.as_path())
        )),
    )
}
