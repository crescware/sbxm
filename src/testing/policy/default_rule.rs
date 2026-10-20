pub fn default_rule(
    id: &str,
    resource_type: &str,
    decision: &str,
    resource: &str,
) -> serde_json::Value {
    serde_json::json!({
        "id":id, "resource_type":resource_type, "decision":decision,
        "resources":[resource], "origin":"local", "status":"active",
        "provenance":{"created_via":"default"}, "editable":true
    })
}
