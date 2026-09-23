use std::path::{Component, Path};

use crate::config::{
    self, ConfigLocation, FileDeclaration, GlobalConfig, HostFileSource, SandboxHomeRelativePath,
};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;
use crate::support::files;

use super::{Added, looks_like_credential};

/// 宣言fileを1件、global configへ足す。
///
/// `source`は絶対pathで受け取る。配置できるfileであることを、宣言を足す前に確かめる。
/// 配置先を省略すると、home directoryからの相対pathをそのまま使う。同じ配置先に別の
/// sourceが宣言されていれば、どちらを置くかを推測せず拒否する。
pub fn add(
    location: &ConfigLocation,
    current: &GlobalConfig,
    source: &Path,
    destination: Option<&str>,
) -> Result<Added> {
    let source_text = paths::display(source);
    let source = HostFileSource::new(&source_text).map_err(|reason| {
        Error::single(
            Diagnostic::new(
                ErrorId::FileDeclarationInvalidSource,
                msg!("error-file-declaration-invalid-source"),
            )
            .fact(Fact::source(&source_text))
            .fact(Fact::reason(reason)),
        )
    })?;
    // 配置と同じ検査を通す。symbolic link、通常fileでないもの、上限を超える大きさは、
    // 宣言した時点で分かる。
    files::read_source_bytes(source.as_path())?;
    let given = match destination {
        Some(given) => given.to_string(),
        None => home_relative(location, source.as_path())?,
    };
    // 検証してから`./`などを除く。`.`のように、除くと何も残らない値もここで拒否する。
    let destination = SandboxHomeRelativePath::new(&given)
        .and_then(|valid| SandboxHomeRelativePath::new(&normalized(valid.as_path())))
        .map_err(|reason| {
            Error::single(
                Diagnostic::new(
                    ErrorId::FileDeclarationInvalidDestination,
                    msg!("error-file-declaration-invalid-destination"),
                )
                .fact(Fact::destination(&given))
                .fact(Fact::reason(reason)),
            )
        })?;
    let declaration = FileDeclaration {
        source,
        destination,
    };
    let credential_like = [
        declaration.source.as_path(),
        declaration.destination.as_path(),
    ]
    .iter()
    .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
    .any(looks_like_credential);

    if let Some(existing) = current.files.iter().find(|existing| {
        existing
            .destination
            .names_same_place(&declaration.destination)
    }) {
        if existing.source == declaration.source {
            return Ok(Added {
                declaration,
                path: location.config_file(),
                already: true,
                credential_like,
            });
        }
        let destination = paths::display(existing.destination.as_path());
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::FileAlreadyDeclared,
                msg!(
                    "error-file-already-declared",
                    destination = destination.clone(),
                    source = paths::display(existing.source.as_path())
                ),
            )
            .remediation(
                Remediation::text(msg!("remediation-file-already-declared"))
                    .try_run(format!("sbxm files rm {destination}")),
            ),
        ));
    }

    let path = config::save_file_declaration(location, &declaration)?;
    Ok(Added {
        declaration,
        path,
        already: false,
        credential_like,
    })
}

/// home directoryからの相対path。homeの外にあるfileは、配置先を決められない。
fn home_relative(location: &ConfigLocation, source: &Path) -> Result<String> {
    // sourceの親はすでに実体のpathへ解決してある。homeも同じ形にしてから比べる。
    let home = match std::fs::canonicalize(location.home()) {
        Ok(home) => home,
        // 実体を解決できないhomeも、そのままの綴りで比べる。
        Err(_) => location.home().to_path_buf(),
    };
    match source.strip_prefix(&home) {
        Ok(relative) if !relative.as_os_str().is_empty() => Ok(paths::display(relative)),
        _ => {
            let source = paths::display(source);
            Err(Error::single(
                Diagnostic::new(
                    ErrorId::FileDestinationRequired,
                    msg!("error-file-destination-required", source = source.clone()),
                )
                .remediation(
                    Remediation::text(msg!("remediation-file-destination-required"))
                        .try_run(format!("sbxm files add {source} --dest <destination>")),
                ),
            ))
        }
    }
}

/// `./`や重なった`/`を除いた綴り。同じ配置先を同じ文字列で記録する。
fn normalized(destination: &Path) -> String {
    destination
        .components()
        .filter(|component| *component != Component::CurDir)
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
