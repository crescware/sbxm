use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::design::Remediation;
use crate::diagnostics::{Diagnostic, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::{
    AGENT_HOME, Conflict, Placement, PlannedFile, destination_path, digest_in_sandbox, read_source,
    require_no_symlink_in_sandbox,
};

/// 1件の宣言を、Sandboxを変えずに観測して扱いを決める。
///
/// 置けない理由のうち、既存のdestinationを上書きしないという判断だけは内側の`Err`で
/// 返す。呼び出し側はほかの宣言も観測し終えてから、判断をまとめて示せる。観測そのものが
/// 成り立たなかった場合は外側の`Err`で止まる。
pub(super) fn plan(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declaration: &FileDeclaration,
    conflict: Conflict,
) -> Result<std::result::Result<PlannedFile, Diagnostic>> {
    let source = declaration.source.as_path();
    let digest = read_source(source)?;
    let destination = destination_path(declaration.destination.as_path())?;
    let full = format!("{AGENT_HOME}/{destination}");
    // 宣言されたpath自体が`agent` home配下でも、Sandbox内のsymlinkが外を指し得る。
    require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;

    let planned = |placement| {
        Ok(Ok(PlannedFile {
            source: source.to_path_buf(),
            digest: digest.clone(),
            destination: destination.clone(),
            placement,
        }))
    };
    let Some(observed) = digest_in_sandbox(host, sandbox, &full)? else {
        return planned(Placement::Placed);
    };
    if observed == digest {
        return planned(Placement::Unchanged);
    }
    // `apply`はsnapshotから置くため、sourceはsbxmのprivateなcopyを指す。拒否の説明は
    // 利用者が失いうるSandbox側のfileだけで述べる。
    match conflict {
        Conflict::Overwrite => planned(Placement::Placed),
        Conflict::Refuse => Ok(Err(Diagnostic::new(
            ErrorId::DeclaredFileConflict,
            msg!(
                "error-declared-file-conflict",
                source = paths::display(source),
                destination = full
            ),
        )
        .remediation(msg!("remediation-declared-file-conflict")))),
        Conflict::Protect(baseline) => match last_placed(baseline, &destination) {
            // sbxmが最後に置いた内容のままであり、置き換えても失われるものは無い。
            Some(placed) if placed == observed => planned(Placement::Placed),
            Some(_) => Ok(Err(Diagnostic::new(
                ErrorId::DeclaredFileModified,
                msg!("error-declared-file-modified", destination = full),
            )
            .remediation(overwrite_remediation()))),
            None => Ok(Err(Diagnostic::new(
                ErrorId::DeclaredFileConflict,
                msg!("error-declared-file-unrecorded", destination = full),
            )
            .remediation(overwrite_remediation()))),
        },
    }
}

/// 上書きしてよいかは利用者だけが決められる。Sandbox側の内容を退避してから明示させる。
fn overwrite_remediation() -> Remediation {
    Remediation::text(msg!("remediation-declared-file-overwrite"))
}

/// baselineが記録した、sbxmがそのdestinationへ最後に置いた内容のdigest。
fn last_placed<'a>(
    baseline: &'a [crate::metadata::InitialProvisioningFile],
    destination: &str,
) -> Option<&'a str> {
    baseline
        .iter()
        .find(|entry| {
            destination_path(Path::new(&entry.destination))
                .is_ok_and(|recorded| recorded == destination)
        })
        .map(|entry| entry.sha256.as_str())
}
