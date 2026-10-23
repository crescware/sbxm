use super::{TOKEN_ENV_FILE, TOKEN_ENV_READ, credential_key};

/// `observe_git_credential`と`observe_token_env`が読む2つのcommand。まとめた確認に含める。
pub fn credential_commands() -> Vec<Vec<String>> {
    vec![
        ["git", "config", "--global", "--get", &credential_key()]
            .iter()
            .map(|arg| (*arg).to_string())
            .collect(),
        ["sh", "-c", TOKEN_ENV_READ, "sh", TOKEN_ENV_FILE]
            .iter()
            .map(|arg| (*arg).to_string())
            .collect(),
    ]
}
