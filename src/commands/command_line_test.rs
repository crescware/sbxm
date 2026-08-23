use crate::boundary::command_line::Arguments;
use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::https_repository;

use super::command_line_values::CommandLineValues;

#[test]
fn a_captured_clone_url_becomes_the_repository_it_names() -> Checked {
    let mut arguments = Arguments::default();
    arguments.insert_value(
        "repository",
        "https://github.com/owner/repository.git".to_owned(),
    );
    assert_eq!(
        CommandLineValues::required_clone_url(&arguments)?,
        https_repository("owner/repository")?
    );
    Ok(())
}

#[test]
fn a_value_that_is_not_a_clone_url_is_refused_by_the_clone_url_rules() -> Checked {
    let mut arguments = Arguments::default();
    arguments.insert_value("repository", "owner/repository".to_owned());
    let error = CommandLineValues::required_clone_url(&arguments)
        .refused_because("a project ID is not a clone URL")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidCloneUrl));
    Ok(())
}

#[test]
fn an_absent_value_is_reported_as_the_missing_argument_help_names() -> Checked {
    let error = CommandLineValues::required_clone_url(&Arguments::default())
        .refused_because("registration cannot continue without a clone URL")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::MissingRequiredArgument);
    assert_eq!(diagnostic.description.id, "error-missing-required-argument");
    assert_eq!(
        diagnostic.description.args,
        vec![(
            "argument",
            format!("<{}>", CommandLineValues::CLONE_URL_VALUE_NAME)
        )]
    );
    Ok(())
}
