use super::{FakeSbx, no_custom_secrets};

/// tokenの登録がないSandbox scope。
pub fn no_secrets(host: FakeSbx, sandbox: &str) -> FakeSbx {
    host.answering(
        &format!("secret ls {sandbox}"),
        0,
        &no_custom_secrets(sandbox),
    )
}
