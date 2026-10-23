/// `observe`が読む2つのcommand。まとめた確認に含める。
pub fn observe_commands() -> Vec<Vec<String>> {
    ["user.name", "user.email"]
        .iter()
        .map(|key| {
            ["git", "config", "--global", "--get", key]
                .iter()
                .map(|arg| (*arg).to_string())
                .collect()
        })
        .collect()
}
