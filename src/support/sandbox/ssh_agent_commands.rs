/// `ssh_agent_is_exposed`が読む2つのcommand。まとめた確認に含める。
pub fn ssh_agent_commands() -> Vec<Vec<String>> {
    vec![
        vec!["printenv".to_string(), "SSH_AUTH_SOCK".to_string()],
        vec!["ssh-add".to_string(), "-L".to_string()],
    ]
}
