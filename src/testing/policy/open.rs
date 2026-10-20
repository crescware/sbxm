use super::default_rule;

pub fn open() -> String {
    serde_json::json!({"rules":[default_rule("default-allow-all", "network", "allow", "**")]})
        .to_string()
}
