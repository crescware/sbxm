use super::{GITHUB_TOKEN_ENV, GITHUB_TOKEN_FALLBACK_ENV, TOKEN_ENV_MARKER};

/// token環境変数fileの3行。書き込みはこの3行を渡し、照合は改行で結んだものと比べる。
pub(super) fn token_env_lines(placeholder: &str) -> [String; 3] {
    [
        format!("{TOKEN_ENV_MARKER} The placeholder is substituted by the Docker Sandboxes proxy."),
        format!("export {GITHUB_TOKEN_ENV}={placeholder}"),
        format!("export {GITHUB_TOKEN_FALLBACK_ENV}={placeholder}"),
    ]
}
