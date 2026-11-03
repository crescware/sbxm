/// 有効なSSH featureの設定record。判定に使わないfieldも実CLIと同じ形で残す。
pub const SSH_FEATURE_ENABLED: &str = r#"{"key":"feature.ssh","value":{"enabled":true},"source":"default","type":"json","requires_restart":true}"#;
