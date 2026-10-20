use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn only_the_complete_active_builtin_balanced_preset_is_supported() -> Checked {
    assert_eq!(
        parse_network_policy(&crate::testing::policy::balanced()).required()?,
        EXPECTED_NETWORK_POLICY
    );
    assert_eq!(
        parse_network_policy(&crate::testing::policy::allow_all()).required()?,
        "allow-all"
    );
    assert_eq!(
        parse_network_policy(&crate::testing::policy::deny_all()).required()?,
        "deny-all"
    );
    Ok(())
}

#[test]
fn incomplete_or_unavailable_policy_is_never_ready() -> Checked {
    for field in ["governance_blocked", "organization_unavailable"] {
        let mut document: serde_json::Value =
            serde_json::from_str(&crate::testing::policy::balanced()).required()?;
        document[field] = serde_json::json!(true);
        assert!(parse_network_policy(&document.to_string()).is_err());
    }
    for output in [
        r#"{"rules":[]}"#,
        r#"{"rules":[{"id":"default-deny-all","resource_type":"network","decision":"deny","resources":["*"]}]}"#,
    ] {
        assert!(parse_network_policy(output).is_err());
    }
    for (key, value) in [
        ("origin", serde_json::json!("org")),
        ("status", serde_json::json!("inactive")),
        ("sandbox_id", serde_json::json!("another")),
        ("decision", serde_json::json!("deny")),
        ("resources", serde_json::json!([])),
        ("provenance", serde_json::json!({"created_via":"added"})),
    ] {
        let mut document: serde_json::Value =
            serde_json::from_str(&crate::testing::policy::balanced()).required()?;
        document["rules"][0][key] = value;
        assert!(
            parse_network_policy(&document.to_string()).is_err(),
            "{key}"
        );
    }
    let mut document: serde_json::Value =
        serde_json::from_str(&crate::testing::policy::balanced()).required()?;
    document["rules"].as_array_mut().required()?.pop();
    assert!(parse_network_policy(&document.to_string()).is_err());
    Ok(())
}

#[test]
fn unknown_policy_shapes_are_refused() -> Checked {
    for output in [
        "",
        "true",
        "{}",
        r#"[{"name":"Balanced","active":true}]"#,
        r#"{"rules":null}"#,
        r#"{"rules":[1]}"#,
        r#"{"rules":[],"governance_blocked":"false"}"#,
        r#"{"rules":[],"organization_unavailable":null}"#,
    ] {
        let error = parse_network_policy(output).refused()?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}
