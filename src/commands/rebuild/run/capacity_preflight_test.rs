use super::*;
use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::testing::host::{AnsweredHost, FakeSbx};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::project_id;
use crate::testing::prompt::ScriptedPrompt;

enum Fault {
    OldFull,
    Changed,
    Full,
    Cleanup,
    Create,
    PartialCreate,
    Foreign,
}
struct FailingProbe {
    inner: FakeSbx,
    fault: Fault,
}

impl AnsweredHost for FailingProbe {
    fn has_command(&self, _: &str) -> bool {
        true
    }
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let temporary = spec
            .args
            .iter()
            .any(|arg| arg.contains("sbxm-capacity-check-"));
        if !temporary
            && matches!(self.fault, Fault::OldFull)
            && spec.args.iter().any(|arg| arg == "df")
        {
            return Ok(crate::testing::command::outcome(
                spec,
                0,
                "Filesystem 1024-blocks Used Available Capacity Mounted on\noverlay 100 100 0 100% /\n",
            ));
        }
        if temporary && spec.args.iter().any(|arg| arg == "df") && matches!(self.fault, Fault::Full)
        {
            return Ok(crate::testing::command::outcome(
                spec,
                0,
                "Filesystem 1024-blocks Used Available Capacity Mounted on\noverlay 100 100 0 100% /\n",
            ));
        }
        if temporary
            && spec.args.first().is_some_and(|arg| arg == "create")
            && matches!(self.fault, Fault::PartialCreate)
        {
            self.inner.answer(spec)?;
            return Ok(crate::testing::command::outcome_with_stderr(
                spec,
                1,
                "",
                "No space left on device",
            ));
        }
        if temporary
            && ((spec.args.iter().any(|arg| arg == "DELETE")
                && matches!(self.fault, Fault::Cleanup))
                || (spec.args.first().is_some_and(|arg| arg == "create")
                    && matches!(self.fault, Fault::Create)))
        {
            return Ok(crate::testing::command::outcome_with_stderr(
                spec,
                1,
                "",
                "No space left on device",
            ));
        }
        if matches!(self.fault, Fault::Changed)
            && self.inner.ran("sbxm-capacity-check-")
            && spec.args.iter().any(|arg| arg == "status")
        {
            return Ok(crate::testing::command::outcome(
                spec,
                0,
                "? changed-after-probe\0",
            ));
        }
        let mut outcome = self.inner.answer(spec)?;
        if spec.args == ["ls", "--json"]
            && matches!(self.fault, Fault::Foreign)
            && outcome.stdout_text().contains("sbxm-capacity-check-")
        {
            let mut entries: serde_json::Value =
                serde_json::from_slice(&outcome.stdout).map_err(|_| Error::Canceled)?;
            entries[0]["workspaces"] = serde_json::json!(["/somebody-else"]);
            outcome.stdout = entries.to_string().into_bytes();
        }
        Ok(outcome)
    }
}

#[test]
fn failed_target_capacity_creation_or_cleanup_preserves_the_old_sandbox() -> Checked {
    for fault in [
        Fault::Full,
        Fault::Cleanup,
        Fault::Create,
        Fault::PartialCreate,
        Fault::Foreign,
        Fault::Changed,
    ] {
        let (fixture, project, inner) = switched_sandbox()?;
        let host = FailingProbe { inner, fault };
        let error = rebuild(
            crate::commands::rebuild::Target {
                location: &fixture.location,
                requested: Some(&project_id("example-org/example-repo")?),
                prompt: &mut ScriptedPrompt::choosing(0),
            },
            &fixture.config,
            &host,
            &fixture.workspace_root,
        )
        .refused_because("the old sandbox must survive any failed preflight")?;
        assert!(!host.inner.removal_requested(project.sandbox.as_str()));
        assert!(
            matches!(host.fault, Fault::Changed)
                || error.diagnostics().iter().any(|diagnostic| matches!(
                    diagnostic.id,
                    ErrorId::StorageExhausted
                        | ErrorId::RebuildCapacityUnconfirmed
                        | ErrorId::ExternalCommandFailed
                ))
        );
        assert!(
            crate::metadata::load(&project.paths)
                .required()?
                .required()?
                .rebuild
                .is_none()
        );
    }
    Ok(())
}

#[test]
fn a_full_old_root_does_not_prevent_rebuilding_into_a_roomy_new_root() -> Checked {
    let (fixture, project, inner) = switched_sandbox()?;
    let host = FailingProbe {
        inner,
        fault: Fault::OldFull,
    };
    rebuild(
        crate::commands::rebuild::Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .required()?;
    assert!(host.inner.removed(project.sandbox.as_str()));
    Ok(())
}
