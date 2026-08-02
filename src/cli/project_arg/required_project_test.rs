use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::project_id;

use super::required_project;
use crate::cli::project_arg::PROJECT_VALUE_NAME;

/// 位置引数を必須にしないparserでmatchesを作る。
///
/// 本番のparserは同じ引数を必須として宣言するため、値の無いmatchesはparserの宣言と
/// 読み出しが食い違ったときにだけ届く。その食い違いをpanicではなく診断として扱う。
fn matches(arguments: &[&str]) -> Checked<ArgMatches> {
    ClapCommand::new("sbxm")
        .no_binary_name(true)
        .arg(Arg::new("project").value_name(PROJECT_VALUE_NAME))
        .try_get_matches_from(arguments)
        .required_because("an omitted optional positional still parses")
}

#[test]
fn a_captured_value_becomes_the_project_it_names() -> Checked {
    assert_eq!(
        required_project(&matches(&["owner/repository"])?)?,
        project_id("owner/repository")?
    );
    Ok(())
}

#[test]
fn a_value_that_is_not_a_project_identifier_is_refused_by_the_project_rules() -> Checked {
    for value in ["owner", "owner//repository", "owner/repository/extra"] {
        let error = required_project(&matches(&[value])?)
            .refused_because(&format!("{value} is not a project ID"))?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::InvalidProjectId),
            "{value} produced the wrong error"
        );
    }
    Ok(())
}

/// 引数が届かなかった場合は、helpが見せるvalue nameでその引数を名指す。
#[test]
fn an_absent_value_is_reported_as_the_missing_argument_help_names() -> Checked {
    let error = required_project(&matches(&[])?)
        .refused_because("a command that requires a project cannot continue without one")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::MissingRequiredArgument);
    assert_eq!(diagnostic.description.id, "error-missing-required-argument");
    assert_eq!(
        diagnostic.description.args,
        vec![("argument", format!("<{PROJECT_VALUE_NAME}>"))]
    );
    Ok(())
}
