use super::{FakeSbx, custom_secret_listing};

/// custom secretがこのSandboxのscopeへ登録されているhost。
pub fn registered_secret(host: FakeSbx, sandbox: &str) -> FakeSbx {
    host.answering(
        "secret ls",
        0,
        &custom_secret_listing(sandbox, "sbx-cs-example"),
    )
}
