use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn the_login_parser_reads_whichever_field_states_the_answer() -> Checked {
    // 対象versionはfield名を変えてきた。読める綴りをすべて受け、真偽をそのまま返す。
    for key in ["logged_in", "loggedIn", "authenticated", "signed_in"] {
        assert!(parse_login_status(&format!("{{\"{key}\": true}}")).required()?);
        assert!(!parse_login_status(&format!("{{\"{key}\": false}}")).required()?);
    }

    // 先に現れる綴りを正本とする。後ろのfieldで上書きしない。
    assert!(
        parse_login_status(r#"{"authenticated": false, "logged_in": true}"#).required()?,
        "the first spelling that appears decides the answer"
    );

    // 周囲の空白は出力の体裁であり、documentの一部ではない。
    assert!(parse_login_status("\n  {\"logged_in\": true}\n").required()?);
    Ok(())
}

#[test]
fn a_login_is_never_inferred_from_an_output_that_does_not_state_it() -> Checked {
    for output in [
        "",
        "Logged in as user@example.com\n",
        "[]",
        r#""logged_in""#,
        r#"{"user": "user@example.com"}"#,
        r#"{"logged_in": "yes"}"#,
        r#"{"logged_in": null}"#,
    ] {
        let error =
            parse_login_status(output).refused_because("a signed-in host is not guessed")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalOutputUnparseable),
            "{output:?} produced the wrong error"
        );
    }
    Ok(())
}
