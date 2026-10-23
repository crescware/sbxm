use super::GITHUB_HOST;

/// `observe_git_protocol`が読むcommand。まとめた確認に含める。
pub fn git_protocol_command() -> Vec<String> {
    ["gh", "config", "get", "git_protocol", "--host", GITHUB_HOST]
        .iter()
        .map(|arg| (*arg).to_string())
        .collect()
}
