use crate::command::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::{
    AGENT_HOME, destination_path, digest_in_sandbox, read_source, require_no_symlink_in_sandbox,
};

/// 既存Sandboxへのdeclared file配置が安全に再利用できるかをread-onlyで確認する。
pub(crate) fn preflight(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
) -> Result<()> {
    for declaration in declarations {
        let source = declaration.source.as_path();
        let digest = read_source(source)?;
        let destination = destination_path(declaration.destination.as_path())?;
        let full = format!("{AGENT_HOME}/{destination}");
        require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;
        if let Some(observed) = digest_in_sandbox(host, sandbox, &full)?
            && observed != digest
        {
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
    Ok(())
}
