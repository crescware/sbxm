use std::cell::Cell;
use std::fs;

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::design::SilentProgress;
use crate::diagnostics::Result;
use crate::testing::command::outcome_with_stderr;
use crate::testing::host::AnsweredHost;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::sandbox::LocalSandbox;

use super::*;

struct FullDuringInit(Cell<bool>);

impl AnsweredHost for FullDuringInit {
    fn has_command(&self, _: &str) -> bool {
        true
    }
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if spec.args.iter().any(|arg| arg == "init") && !self.0.replace(true) {
            let path = spec.args.last().cloned().unwrap_or_default();
            fs::create_dir_all(format!("{path}/refs/heads")).map_err(|error| {
                crate::paths::atomic_write_failed(std::path::Path::new(&path), &error.to_string())
            })?;
            return Ok(outcome_with_stderr(
                spec,
                128,
                "",
                "No space left on device",
            ));
        }
        LocalSandbox.answer(spec)
    }
}

#[test]
fn a_failed_init_keeps_a_marker_and_the_next_attempt_completes_without_deleting_content() -> Checked
{
    let directory = tempfile::tempdir().required()?;
    let target = directory.path().join("repo.git");
    let git_dir = target.to_str().required()?;
    let host = FullDuringInit(Cell::new(false));
    initialize(&host, "sandbox", git_dir).refused_because("the initial writes ran out of space")?;
    assert!(target.join("sbxm-initializing").is_dir());
    fs::write(target.join("preserved"), b"user content").required()?;
    let origin = SandboxOrigin::Github(crate::project::ProjectId::parse("owner/repo").required()?);
    assert!(
        complete_initialization(&host, "sandbox", &origin, git_dir, &mut SilentProgress)
            .required()?
    );
    super::super::verify_bare_config(
        &Live::new(&host, "sandbox"),
        &origin,
        git_dir,
        &mut SilentProgress,
    )
    .required()?;
    assert_eq!(
        fs::read(target.join("preserved")).required()?,
        b"user content"
    );
    Ok(())
}
