use super::default_rule;

pub fn locked_down() -> String {
    serde_json::json!({"rules":[
        default_rule("default-fs-read-allow-all", "filesystem", "allow", "**"),
        default_rule("default-fs-write-allow-all", "filesystem", "allow", "**")
    ]})
    .to_string()
}
