use crate::config::GlobalConfig;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::InitialProvisioningIntent;
use crate::msg;
use crate::paths;
use crate::support::files;

/// 初回構築中に固定したglobal file入力が変わっていないことを確認する。
pub(crate) fn validate_intent(
    intent: &InitialProvisioningIntent,
    config: &GlobalConfig,
    project: &str,
) -> Result<()> {
    if intent.files.len() != config.files.len() {
        return Err(changed(project, "files"));
    }
    for (index, (snapshot, declaration)) in intent.files.iter().zip(&config.files).enumerate() {
        if snapshot.source != paths::display(declaration.source.as_path())
            || snapshot.destination != paths::display(declaration.destination.as_path())
        {
            return Err(changed(project, &index.to_string()));
        }
        let observed = files::read_source(declaration.source.as_path())?;
        if observed != snapshot.sha256 {
            return Err(changed(project, &index.to_string()));
        }
    }
    Ok(())
}

fn changed(project: &str, entry: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningInputChanged,
            msg!(
                "error-initial-provisioning-input-changed",
                project = project
            ),
        )
        .fact(Fact::entry(entry))
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-input-changed"
        )))
        .remediation(msg!("remediation-initial-provisioning-input-changed")),
    )
}

#[cfg(test)]
#[path = "validate_intent_test.rs"]
mod validate_intent_test;
