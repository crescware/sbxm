use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn v048_rules_are_observed_without_inventing_a_preset_name() -> Checked {
    for output in [
        r#"{"rules":[]}"#,
        r#"{"rules":[{"id":"rule-1","policy_name":"default-deny-all","resource_type":"network","decision":"deny","resources":["*"],"editable":true}]}"#,
        r#"{"rules":[],"organization":"example","governance_blocked":false,"organization_unavailable":false}"#,
    ] {
        assert_eq!(
            parse_network_policy(output).required()?,
            NetworkPolicyState::Available
        );
    }
    Ok(())
}

#[test]
fn blocked_governance_is_observed_separately_from_malformed_output() -> Checked {
    for output in [
        r#"{"rules":[],"governance_blocked":true}"#,
        r#"{"rules":[],"organization_unavailable":true}"#,
    ] {
        assert_eq!(
            parse_network_policy(output).required()?,
            NetworkPolicyState::Blocked
        );
    }
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
