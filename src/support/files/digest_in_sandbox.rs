use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Result, unparseable};

use crate::support::sandbox;

/// Sandbox内のdestinationのdigest。存在しない場合は`None`。
pub(super) fn digest_in_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    destination: &str,
) -> Result<Option<String>> {
    let exists = sandbox::exec(host, sandbox, &["test", "-e", destination])?;
    match sandbox::inner_exit_code(&exists) {
        Some(0) => {}
        Some(1) => return Ok(None),
        _ => return Err(sandbox::unobservable(&exists, destination)),
    }

    let outcome = sandbox::exec(host, sandbox, &["sha256sum", destination])?.require_success()?;
    let text = outcome.stdout_text();
    let digest = text.split_whitespace().next().unwrap_or_default();
    // Sandboxが答えた値はhostのfile名にも使う。小文字16進の64桁以外は信用しない。
    let hex = digest
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if digest.len() != 64 || !hex {
        return Err(unparseable(
            "sha256sum",
            &format!("no digest was reported for {destination}"),
        ));
    }
    Ok(Some(digest.to_string()))
}
