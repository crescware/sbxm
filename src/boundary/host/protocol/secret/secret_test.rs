use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn custom_secrets_are_read_from_v048_json_without_secret_values() -> Checked {
    let output = r#"{
        "secrets":[{"scope":"(global)","service":"github","secret":"unread-service-value"}],
        "custom_secrets":[
            {"scope":"sbxm-example","targets":["github.com","**.github.com","**.githubusercontent.com","ghcr.io"],"env":"GH_TOKEN","placeholder":"sbx-cs-example","secret":"unread-custom-value"},
            {"scope":"(global)","placeholder":"sbx-cs-global","kind":"command","source":"unread-source"}
        ],
        "shadowed_services":[],"env_only_count":0
    }"#;
    assert_eq!(
        parse_custom_secrets(output).required()?,
        vec![
            CustomSecret {
                scope: "sbxm-example".to_string(),
                targets: vec![
                    "github.com".to_string(),
                    "**.github.com".to_string(),
                    "**.githubusercontent.com".to_string(),
                    "ghcr.io".to_string()
                ],
                env: "GH_TOKEN".to_string(),
                placeholder: "sbx-cs-example".to_string(),
            },
            CustomSecret {
                scope: "(global)".to_string(),
                targets: Vec::new(),
                env: String::new(),
                placeholder: "sbx-cs-global".to_string(),
            },
        ]
    );
    assert!(
        parse_custom_secrets(
            r#"{"secrets":[],"custom_secrets":[],"shadowed_services":[],"env_only_count":8}"#
        )
        .required()?
        .is_empty()
    );
    Ok(())
}

#[test]
fn malformed_secrets_are_refused_without_exposing_values() -> Checked {
    for output in [
        "",
        "No secrets found",
        "{}",
        "[]",
        r#"{"custom_secrets":null}"#,
        r#"{"custom_secrets":["unread-custom-value"]}"#,
        r#"{"custom_secrets":[{"placeholder":"p","secret":"unread-custom-value"}]}"#,
        r#"{"custom_secrets":[{"scope":"","placeholder":"p","secret":"unread-custom-value"}]}"#,
        r#"{"custom_secrets":[{"scope":"s","secret":"unread-custom-value"}]}"#,
        r#"{"custom_secrets":[{"scope":"s","placeholder":"","secret":"unread-custom-value"}]}"#,
        r#"{"custom_secrets":[{"scope":"s","placeholder":"p","targets":"unread-custom-value"}]}"#,
        r#"{"custom_secrets":[{"scope":"s","placeholder":"p","targets":[1]}]}"#,
        r#"{"custom_secrets":[{"scope":"s","placeholder":"p","env":1}]}"#,
    ] {
        let error = parse_custom_secrets(output).refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert!(!format!("{error:?}").contains("unread-custom-value"));
    }
    Ok(())
}
