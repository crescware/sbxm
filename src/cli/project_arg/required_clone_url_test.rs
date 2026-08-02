use clap::{Arg, ArgMatches, Command as ClapCommand};

use crate::diagnostics::ErrorId;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::project::https_repository;

use super::required_clone_url;
use crate::cli::project_arg::CLONE_URL_VALUE_NAME;

/// 位置引数を必須にしないparserでmatchesを作る。
///
/// 本番のparserは同じ引数を必須として宣言するため、値の無いmatchesはparserの宣言と
/// 読み出しが食い違ったときにだけ届く。その食い違いをpanicではなく診断として扱う。
fn matches(arguments: &[&str]) -> Checked<ArgMatches> {
    ClapCommand::new("sbxm")
        .no_binary_name(true)
        .arg(Arg::new("repository").value_name(CLONE_URL_VALUE_NAME))
        .try_get_matches_from(arguments)
        .required_because("an omitted optional positional still parses")
}

#[test]
fn a_captured_clone_url_becomes_the_repository_it_names() -> Checked {
    assert_eq!(
        required_clone_url(&matches(&["https://github.com/owner/repository.git"])?)?,
        https_repository("owner/repository")?
    );
    Ok(())
}

#[test]
fn a_value_that_is_not_a_clone_url_is_refused_by_the_clone_url_rules() -> Checked {
    let error = required_clone_url(&matches(&["owner/repository"])?)
        .refused_because("a project ID is not a clone URL")?;
    assert_eq!(error.first_id(), Some(ErrorId::InvalidCloneUrl));
    Ok(())
}

/// 引数が届かなかった場合は、helpが見せるvalue nameでその引数を名指す。
#[test]
fn an_absent_value_is_reported_as_the_missing_argument_help_names() -> Checked {
    let error = required_clone_url(&matches(&[])?)
        .refused_because("registration cannot continue without a clone URL")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::MissingRequiredArgument);
    assert_eq!(diagnostic.description.id, "error-missing-required-argument");
    assert_eq!(
        diagnostic.description.args,
        vec![("argument", format!("<{CLONE_URL_VALUE_NAME}>"))]
    );
    Ok(())
}
