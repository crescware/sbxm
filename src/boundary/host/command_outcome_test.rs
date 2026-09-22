use crate::diagnostics::ErrorId;
use crate::testing::command::outcome_with_stderr;
use crate::testing::outcome::{Checked, Refused, Required};

use super::CommandSpec;

#[test]
fn sbx_authentication_failures_keep_the_command_and_offer_login() -> Checked {
    let stderr = "ERROR: user is not authenticated to Docker: secret not found\n\
                  no valid user session found, please sign in to Docker to proceed\n\
                  \nSign in with: sbx login\n";
    for args in [
        vec!["ls", "--json"],
        vec!["stop", "example"],
        vec!["rm", "--force", "example"],
        vec!["template", "ls", "--json"],
    ] {
        let spec = CommandSpec::probe("sbx", &args);
        let outcome = outcome_with_stderr(&spec, 1, "", stderr);
        let failure = outcome.failure();
        let error = outcome.require_success().refused_because("login expired")?;
        assert_eq!(error.first_id(), Some(ErrorId::SbxLoginMissing));
        let diagnostic = &error.diagnostics()[0];
        assert_eq!(diagnostic.external.as_ref(), Some(&failure));
        let remediation = diagnostic.remediation.as_ref().required()?;
        assert_eq!(remediation.commands[0].as_str(), "sbx login");
    }
    Ok(())
}

#[test]
fn unrelated_command_failures_and_successful_stderr_are_not_login_errors() -> Checked {
    for (program, stderr) in [
        ("sbx", "401 Unauthorized"),
        ("sbx", "secret not found"),
        ("sbx", "the daemon is not running"),
        ("git", "user is not authenticated to Docker"),
    ] {
        let spec = CommandSpec::probe(program, &[]);
        let error = outcome_with_stderr(&spec, 1, "", stderr)
            .require_success()
            .refused_because("an unrelated failure keeps its category")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    }
    let spec = CommandSpec::probe("sbx", &["ls", "--json"]);
    outcome_with_stderr(&spec, 0, "[]", "user is not authenticated to Docker")
        .require_success()
        .required_because("a successful command is not reclassified from its stderr")?;
    Ok(())
}
