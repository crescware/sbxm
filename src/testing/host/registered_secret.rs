use super::{FakeSbx, service_secret_listing};

/// `github` serviceのtokenが登録済みで、Sandboxが`GH_TOKEN`のsentinelを持つhost。
pub fn registered_secret(host: FakeSbx, sandbox: &str) -> FakeSbx {
    host.answering("secret ls --json", 0, &service_secret_listing(sandbox))
        .answering(
            &format!(
                "exec {sandbox} -- sh -c {}",
                crate::support::secret::placeholder_probe()
            ),
            0,
            "gho_sbxproxymanaged000000000000000000000",
        )
}
