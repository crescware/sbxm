use std::path::PathBuf;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::InitialProvisioningFile;
use crate::msg;

use super::{
    AGENT_HOME, PlacedFile, Placement, destination_path, digest_in_sandbox,
    require_no_symlink_in_sandbox,
};

/// 宣言fileを、生きているsourceを読まずbaselineのdigestと照合するだけで観測する。
///
/// `repair`が復旧対象にするのは、初回構築が固定したbaselineであり、現在のglobal
/// configではない。baselineは`source`・`destination`・digestだけを保存済みの記録
/// として持つため、この照合はhost上のsource pathへ一切触れない。sandboxが別内容を
/// 持っている場合は、上書きせず既存fileの衝突として拒否する。
pub fn observe_against_baseline(
    host: &dyn HostEnvironment,
    sandbox: &str,
    baseline: &[InitialProvisioningFile],
) -> Result<Vec<PlacedFile>> {
    let mut observed = Vec::with_capacity(baseline.len());
    for entry in baseline {
        let destination = destination_path(&PathBuf::from(&entry.destination))?;
        let full = format!("{AGENT_HOME}/{destination}");
        require_no_symlink_in_sandbox(host, sandbox, &PathBuf::from(&entry.source), &destination)?;
        let placement = match digest_in_sandbox(host, sandbox, &full)? {
            None => Placement::Placed,
            Some(digest) if digest == entry.sha256 => Placement::Unchanged,
            Some(_) => {
                return Err(Error::single(
                    Diagnostic::new(
                        ErrorId::DeclaredFileConflict,
                        msg!(
                            "error-declared-file-conflict",
                            source = entry.source.clone(),
                            destination = full
                        ),
                    )
                    .remediation(msg!("remediation-declared-file-conflict")),
                ));
            }
        };
        observed.push(PlacedFile {
            source: PathBuf::from(&entry.source),
            destination,
            placement,
        });
    }
    Ok(observed)
}
