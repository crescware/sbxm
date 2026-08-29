use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::{
    AGENT_HOME, Conflict, PlacedFile, Placement, copy_into_sandbox, destination_path,
    digest_in_sandbox, read_source, require_no_symlink_in_sandbox,
};

pub(super) fn place(
    host: &dyn HostEnvironment,
    sandbox: &str,
    index: usize,
    declaration: &FileDeclaration,
    conflict: Conflict,
) -> Result<PlacedFile> {
    let source = declaration.source.as_path();
    let digest = read_source(source)?;
    let destination = destination_path(declaration.destination.as_path())?;
    let full = format!("{AGENT_HOME}/{destination}");
    // 宣言されたpath自体が`agent` home配下でも、Sandbox内のsymlinkが外を指し得る。
    require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;

    if let Some(observed) = digest_in_sandbox(host, sandbox, &full)? {
        if observed == digest {
            return Ok(PlacedFile {
                source: source.to_path_buf(),
                destination,
                placement: Placement::Unchanged,
            });
        }
        if conflict == Conflict::Refuse {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::DeclaredFileConflict,
                    msg!(
                        "error-declared-file-conflict",
                        source = paths::display(source),
                        destination = full
                    ),
                )
                .remediation(msg!("remediation-declared-file-conflict")),
            ));
        }
    }

    copy_into_sandbox(host, sandbox, index, source, &full)?;
    Ok(PlacedFile {
        source: source.to_path_buf(),
        destination,
        placement: Placement::Placed,
    })
}
