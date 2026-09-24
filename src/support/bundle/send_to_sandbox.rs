use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PathScope};
use crate::support::files::TRANSFER_INCOMPLETE;
use crate::support::repository::host_git;
use crate::support::sandbox;

use super::PLACE_BUNDLE;

/// hostの`repository`の`revisions`を1つのbundleにして、Sandboxの`destination`へ置く。
///
/// `revisions`は`git bundle create`へ渡す範囲の指定である。bundleは案件の`directory`へ
/// 一時fileとして作り、Sandboxへ置けたかどうかにかかわらず消す。branchもtagも無い
/// repositoryは送らない。
pub fn send_to_sandbox(
    host: &dyn HostEnvironment,
    repository: &Path,
    revisions: &[&str],
    directory: &Path,
    sandbox_name: &str,
    destination: &str,
) -> Result<()> {
    require_something_to_send(host, repository)?;
    paths::ensure_private_dir(directory, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    // 一時fileはdropで消える。gitはこのpathへ書き直す。
    let temporary = match tempfile::Builder::new()
        .prefix(".sending-")
        .tempfile_in(directory)
    {
        Ok(temporary) => temporary,
        Err(error) => return Err(paths::atomic_write_failed(directory, &error.to_string())),
    };
    let bundle = temporary.path();
    let bundle_text = paths::display(bundle);
    let mut args = vec!["bundle", "create", "--quiet", bundle_text.as_str()];
    args.extend_from_slice(revisions);
    host_git(
        host,
        repository,
        &args,
        None,
        TimeoutClass::RepositoryTransfer,
    )?
    .require_success()?;
    let digest = match hash::sha256_file_hex(bundle) {
        Ok(digest) => digest,
        Err(error) => return Err(paths::atomic_write_failed(bundle, &error.to_string())),
    };

    let outcome = sandbox::exec_with_input_file(
        host,
        sandbox_name,
        &["sh", "-c", PLACE_BUNDLE, "sh", destination, &digest],
        bundle,
    )?;
    if sandbox::inner_exit_code(&outcome) == Some(TRANSFER_INCOMPLETE) {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::BundleTransferIncomplete,
                msg!("error-bundle-transfer-incomplete", sandbox = sandbox_name),
            )
            .remediation(msg!("remediation-bundle-transfer-incomplete")),
        ));
    }
    outcome.require_success()?;
    Ok(())
}

/// branchもtagも無いrepositoryからは、gitがbundleを作らない。理由を名指しして断る。
fn require_something_to_send(host: &dyn HostEnvironment, repository: &Path) -> Result<()> {
    let listed = host_git(
        host,
        repository,
        &[
            "for-each-ref",
            "--count=1",
            "--format=%(refname)",
            "refs/heads/",
            "refs/tags/",
        ],
        None,
        TimeoutClass::LocalFilesystem,
    )?
    .require_success()?;
    if listed.stdout_text().trim().is_empty() {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::HostRepositoryEmpty,
                msg!(
                    "error-host-repository-empty",
                    repository = paths::display(repository)
                ),
            )
            .remediation(msg!("remediation-host-repository-empty")),
        ));
    }
    Ok(())
}
