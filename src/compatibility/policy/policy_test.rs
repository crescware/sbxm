use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn the_network_policy_parser_reads_the_active_entry_only() -> Checked {
    let balanced = r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#;
    assert_eq!(parse_network_policy(balanced).required()?, "Balanced");

    let other = r#"[{"name":"Balanced","active":false},{"name":"Open","active":true}]"#;
    assert_ne!(
        parse_network_policy(other).required()?,
        EXPECTED_NETWORK_POLICY
    );

    for output in [
        "{}",
        r#"[{"name":"Balanced","active":false}]"#,
        r#"[{"name":"Balanced","active":true},{"name":"Open","active":true}]"#,
    ] {
        let error =
            parse_network_policy(output).refused_because("an ambiguous policy is not guessed")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}
