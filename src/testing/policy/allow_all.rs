use super::default_rule;

pub fn allow_all() -> String {
    serde_json::json!({"rules":[default_rule("default-allow-all", "network", "allow", "**")]})
        .to_string()
}
