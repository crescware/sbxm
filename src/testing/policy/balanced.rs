use super::default_rule;

pub fn balanced() -> String {
    let rules: Vec<_> = [
        ("default-ai-services", "api.anthropic.com:443"),
        ("default-package-managers", "pypi.org:443"),
        ("default-code-and-containers", "github.com:443"),
        ("default-cloud-infrastructure", "aws.amazon.com:443"),
        ("default-os-packages", "archive.ubuntu.com:443"),
        ("default-cert-validation", "ocsp.digicert.com:80"),
    ]
    .iter()
    .map(|(id, resource)| default_rule(id, "network", "allow", resource))
    .collect();
    serde_json::json!({"rules":rules}).to_string()
}
