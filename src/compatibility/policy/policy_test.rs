use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::Fact;
use crate::diagnostics::ErrorId;

/// 拒否が`Cause:`として示した原文。
///
/// 同じerror IDで拒む道が何本もあるため、どれを通ったかはこの行でしか区別できない。
fn refusal_cause(error: &crate::diagnostics::Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the refusal states what could not be read")
}

#[test]
fn the_network_policy_parser_reads_the_active_entry_only() -> Checked {
    let balanced = r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#;
    assert_eq!(parse_network_policy(balanced).required()?, "Balanced");

    let other = r#"[{"name":"Balanced","active":false},{"name":"Open","active":true}]"#;
    assert_ne!(
        parse_network_policy(other).required()?,
        EXPECTED_NETWORK_POLICY
    );
    Ok(())
}

#[test]
fn the_entry_in_force_is_recognised_under_every_word_the_listing_uses() -> Checked {
    // 有効な1件を指す語は`active`だけではない。印の名前が違う一覧を「有効な行がない」と
    // 読むと、設定済みのpolicyを未設定として報告する。
    for output in [
        r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#,
        r#"[{"name":"Balanced","current":true},{"name":"Open"}]"#,
        r#"[{"name":"Balanced","selected":true},{"name":"Open"}]"#,
    ] {
        assert_eq!(
            parse_network_policy(output).required_because(output)?,
            EXPECTED_NETWORK_POLICY
        );
    }
    Ok(())
}

#[test]
fn a_lone_object_names_the_policy_under_every_key_the_output_uses() -> Checked {
    // 一覧ではなく現在値だけを返す形もある。項目名が違うだけの同じ答えを拒まない。
    for output in [
        r#"{"policy":"Balanced"}"#,
        r#"{"current":"Balanced"}"#,
        r#"{"name":"Balanced"}"#,
    ] {
        assert_eq!(
            parse_network_policy(output).required_because(output)?,
            EXPECTED_NETWORK_POLICY
        );
    }
    Ok(())
}

#[test]
fn a_policy_output_that_cannot_be_read_states_which_reading_failed() -> Checked {
    for (output, cause) in [
        ("{}", "no policy field is present"),
        (
            r#"[{"name":"Balanced","active":false}]"#,
            "no entry is marked as the active policy",
        ),
        (
            r#"[{"name":"Balanced","active":true},{"name":"Open","active":true}]"#,
            "more than one entry is marked as the active policy",
        ),
        // 一覧の要素が物体でなければ、印も名前も読む場所がない。
        (r#"["Balanced"]"#, "an entry is not an object"),
        // 配列でも物体でもない答えは、一覧としても現在値としても読めない。
        ("true", "the document is neither an array nor an object"),
    ] {
        let error =
            parse_network_policy(output).refused_because("an ambiguous policy is not guessed")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}

#[test]
fn an_output_that_is_not_json_is_refused_with_what_the_json_reader_said() -> Checked {
    // 読めなかった位置はJSON側にしか分からない。sbxmが言い換えると、原文のどこで
    // 途切れたかが読み手から消える。
    let broken = r#"[{"name":"Balanced","active":true}"#;
    let reported = serde_json::from_str::<serde_json::Value>(broken)
        .refused_because("the sample is not JSON")?
        .to_string();

    let error = parse_network_policy(broken).refused_because("a truncated document")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, reported);
    Ok(())
}
