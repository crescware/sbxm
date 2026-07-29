use super::*;
use crate::error::ErrorId;

#[test]
fn the_network_policy_parser_reads_the_active_entry_only() {
    let balanced = r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#;
    assert_eq!(parse_network_policy(balanced).unwrap(), "Balanced");

    let other = r#"[{"name":"Balanced","active":false},{"name":"Open","active":true}]"#;
    assert_ne!(
        parse_network_policy(other).unwrap(),
        EXPECTED_NETWORK_POLICY
    );

    for output in [
        "{}",
        r#"[{"name":"Balanced","active":false}]"#,
        r#"[{"name":"Balanced","active":true},{"name":"Open","active":true}]"#,
    ] {
        let error = parse_network_policy(output).expect_err("an ambiguous policy is not guessed");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}
