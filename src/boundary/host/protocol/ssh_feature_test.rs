use crate::testing::outcome::{Checked, Required};

use super::SshFeature;

#[test]
fn the_setting_record_captured_from_sbx_0_48_is_read_without_losing_restart_or_source() -> Checked {
    let setting = SshFeature::parse(include_str!(
        "../../../../tests/fixtures/sbx-ssh-feature.json"
    ))
    .required()?;
    assert!(!setting.enabled);
    assert!(setting.requires_restart);
    assert!(setting.can_enable());
    assert_eq!(setting.source, "default");
    Ok(())
}

#[test]
fn malformed_or_incomplete_settings_are_not_guessed() -> Checked {
    let captured: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/sbx-ssh-feature.json"
    ))
    .required()?;
    assert!(SshFeature::parse("not JSON").is_err());
    for (key, value) in [
        ("key", serde_json::json!("other")),
        ("value", serde_json::json!(false)),
        ("value", serde_json::json!({"enabled":"false"})),
        ("source", serde_json::json!("")),
        ("source", serde_json::json!(null)),
        ("requires_restart", serde_json::json!(null)),
        ("locked", serde_json::json!("true")),
    ] {
        let mut record = captured.clone();
        record[key] = value;
        assert!(SshFeature::parse(&record.to_string()).is_err(), "{record}");
    }
    let mut record = captured;
    record["locked"] = serde_json::json!(true);
    assert!(
        !SshFeature::parse(&record.to_string())
            .required()?
            .can_enable()
    );
    Ok(())
}

#[test]
fn the_experimental_features_gate_requires_an_explicit_boolean_for_the_right_key() -> Checked {
    for allowed in [true, false] {
        assert_eq!(
            SshFeature::parse_experimental(
                &serde_json::json!({"key":"platform.allowExperimentalFeatures","value":allowed})
                    .to_string()
            )
            .required()?,
            allowed
        );
    }
    for bad in [
        "not JSON",
        "{}",
        r#"{"key":"other","value":true}"#,
        r#"{"key":"platform.allowExperimentalFeatures","value":"true"}"#,
    ] {
        assert!(SshFeature::parse_experimental(bad).is_err());
    }
    Ok(())
}
