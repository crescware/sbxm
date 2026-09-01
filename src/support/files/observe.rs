use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::{
    AGENT_HOME, PlacedFile, Placement, destination_path, digest_in_sandbox, read_source,
    require_no_symlink_in_sandbox,
};

/// 宣言されたfileを変更せずに観測する。
pub fn observe(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
) -> Result<Vec<PlacedFile>> {
    let mut observed = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let source = declaration.source.as_path();
        let digest = read_source(source)?;
        let destination = destination_path(declaration.destination.as_path())?;
        let full = format!("{AGENT_HOME}/{destination}");
        require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;
        let placement = match digest_in_sandbox(host, sandbox, &full)? {
            None => Placement::Placed,
            Some(observed) if observed == digest => Placement::Unchanged,
            Some(_) => {
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
        };
        observed.push(PlacedFile {
            source: source.to_path_buf(),
            destination,
            placement,
        });
    }
    Ok(observed)
}
