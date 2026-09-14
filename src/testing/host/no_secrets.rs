use super::{FakeSbx, no_secrets_listing};

/// tokenの登録がないhost。
pub fn no_secrets(host: FakeSbx, _sandbox: &str) -> FakeSbx {
    host.answering("secret ls --json", 0, &no_secrets_listing())
}
