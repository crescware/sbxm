use super::{FakeSbx, custom_secret_listing};

/// custom secretが登録済みで、placeholderも解決できるSandbox。
pub fn registered_secret(host: FakeSbx, sandbox: &str) -> FakeSbx {
    host.answering(
        &format!("secret ls {sandbox}"),
        0,
        &custom_secret_listing(sandbox, "sbx-cs-example"),
    )
    .answering(
        &format!(
            "exec {sandbox} -- sh -c {}",
            crate::support::secret::placeholder_probe()
        ),
        0,
        "sbx-cs-example",
    )
}
