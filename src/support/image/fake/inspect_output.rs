pub fn inspect_output(labels: &[(&str, &str)]) -> String {
    let labels = labels
        .iter()
        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"[{{"Id":"sha256:image","Config":{{"Labels":{{{labels}}}}}}}]"#)
}
