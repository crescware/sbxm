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

/// 対象versionが実際に出力する形。scope名とsecretは記録から伏せてある。
const OBSERVED: &str = r#"{
  "secrets": [
    {
      "scope": "sbxm-example",
      "type": "service",
      "name": "github",
      "secret": "(stored)"
    }
  ],
  "custom_secrets": [
    {
      "scope": "sbxm-example",
      "targets": [
        "github.com",
        "**.github.com",
        "**.githubusercontent.com",
        "ghcr.io"
      ],
      "env": "GH_TOKEN",
      "placeholder": "sbx-cs-J0uA6pOfxmdzMF1W",
      "secret": "github******...******6KGT"
    }
  ],
  "shadowed_services": [],
  "env_only_count": 0
}"#;

#[test]
fn the_secret_listing_of_the_target_version_is_read_as_it_is() -> Checked {
    let listing = parse_secret_listing(OBSERVED).required()?;
    assert_eq!(
        listing.services,
        vec![ServiceSecret {
            scope: "sbxm-example".to_string(),
            name: "github".to_string(),
        }]
    );
    assert_eq!(
        listing.customs,
        vec![CustomSecret {
            scope: "sbxm-example".to_string(),
            targets: vec![
                "github.com".to_string(),
                "**.github.com".to_string(),
                "**.githubusercontent.com".to_string(),
                "ghcr.io".to_string(),
            ],
            env: "GH_TOKEN".to_string(),
            placeholder: "sbx-cs-J0uA6pOfxmdzMF1W".to_string(),
        }],
        "the pattern is compared as written, so it has to survive the listing unexpanded"
    );
    Ok(())
}

#[test]
fn an_empty_listing_and_a_null_listing_hold_nothing() -> Checked {
    for output in [
        r#"{"secrets":[],"custom_secrets":[],"shadowed_services":[],"env_only_count":0}"#,
        r#"{"secrets":null,"custom_secrets":null}"#,
        "{}",
    ] {
        let listing = parse_secret_listing(output).required_because(output)?;
        assert!(
            listing.services.is_empty() && listing.customs.is_empty(),
            "{output}"
        );
    }
    Ok(())
}

#[test]
fn registrations_that_are_not_services_are_left_out_of_the_services() -> Checked {
    // registry credentialも同じ一覧に並ぶ。serviceの登録と取り違えない。
    let output = r#"{"secrets":[{"scope":"(global)","type":"registry","name":"ghcr.io","secret":"(stored)"}],"custom_secrets":[]}"#;
    let listing = parse_secret_listing(output).required()?;
    assert!(listing.services.is_empty());
    Ok(())
}

#[test]
fn the_global_scope_is_recognised_in_either_spelling() {
    assert!(is_global_scope("(global)"));
    assert!(is_global_scope("global"));
    assert!(!is_global_scope("sbxm-example"));
}

#[test]
fn an_answer_with_nothing_in_it_is_not_read_as_an_absence_of_secrets() -> Checked {
    // 何も書かれていない出力は観測ではない。1件もないことは空の一覧で示される。
    let error = parse_secret_listing("   \n").refused_because("nothing was said at all")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, "the output is empty");
    Ok(())
}

#[test]
fn a_listing_that_cannot_be_read_names_what_is_wrong() -> Checked {
    // 欠けた値ごとに読めなくなるものが違う。scopeがなければ消してよい登録を選べず、
    // placeholderがなければcustom secretを指せない。どれかを示す。
    for (output, cause) in [
        (
            "not json",
            "the output is not JSON: expected ident at line 1 column 2",
        ),
        ("[]", "the output is not an object"),
        (r#"{"secrets":{}}"#, "secrets is not a list"),
        (r#"{"secrets":[1]}"#, "a secret is not an object"),
        (
            r#"{"secrets":[{"type":"service","name":"github"}]}"#,
            "a service secret has no scope",
        ),
        (
            r#"{"secrets":[{"type":"service","scope":"s"}]}"#,
            "a service secret has no name",
        ),
        (r#"{"custom_secrets":"x"}"#, "custom_secrets is not a list"),
        (
            r#"{"custom_secrets":[1]}"#,
            "a custom secret is not an object",
        ),
        (
            r#"{"custom_secrets":[{"scope":"s","targets":"github.com","env":"E","placeholder":"p"}]}"#,
            "a custom secret's targets is not a list",
        ),
        (
            r#"{"custom_secrets":[{"scope":"s","targets":[1],"env":"E","placeholder":"p"}]}"#,
            "a custom secret target is not a string",
        ),
        (
            r#"{"custom_secrets":[{"targets":[],"env":"E","placeholder":"p"}]}"#,
            "a custom secret has no scope",
        ),
        (
            r#"{"custom_secrets":[{"scope":"s","targets":[],"placeholder":"p"}]}"#,
            "a custom secret has no env",
        ),
        (
            r#"{"custom_secrets":[{"scope":"s","targets":[],"env":"E"}]}"#,
            "a custom secret has no placeholder",
        ),
    ] {
        let error = parse_secret_listing(output).refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "{output}");
    }
    // targetsが無いcustom secretは、対象hostを持たない登録として読む。
    let listing =
        parse_secret_listing(r#"{"custom_secrets":[{"scope":"s","env":"E","placeholder":"p"}]}"#)
            .required()?;
    assert!(listing.customs[0].targets.is_empty());
    Ok(())
}
