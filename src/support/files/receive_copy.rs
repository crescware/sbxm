use std::fs;
use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::config::FileDeclaration;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_file_hex;
use crate::msg;
use crate::paths;
use crate::support::retrieve;

use super::{
    AGENT_HOME, MAX_SOURCE_BYTES, ReceivedCopy, destination_path, digest_in_sandbox,
    require_no_symlink_in_sandbox,
};

/// Sandboxが宣言fileの配置先に持つ内容を、hostの隔離領域`directory`へ受け取る。
///
/// Sandboxに無ければ`None`を返す。配置と同じく、配置先までの途中にsymbolic linkがあれば
/// 読まずに拒否する。受け取るのは宣言fileと同じ大きさの上限までとし、受け取った内容の
/// digestが受け取る直前に観測したdigestと違えば、読んでいる間に変わったものとして拒否する。
pub fn receive_copy(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declaration: &FileDeclaration,
    directory: &Path,
) -> Result<Option<ReceivedCopy>> {
    let source = declaration.source.as_path();
    let destination = destination_path(declaration.destination.as_path())?;
    let full = format!("{AGENT_HOME}/{destination}");
    require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;
    let Some(observed) = digest_in_sandbox(host, sandbox, &full)? else {
        return Ok(None);
    };

    // 同じ内容を以前に受け取った残りがあれば、それを置き換える。隔離領域はsbxmだけが書く。
    let name = format!("pull-{observed}");
    let _ = fs::remove_file(directory.join(&name));
    let path = retrieve::receive(
        host,
        sandbox,
        &["cat", "--", &full],
        directory,
        &name,
        MAX_SOURCE_BYTES,
        TimeoutClass::SandboxLifecycle,
    )?;
    let sha256 = match sha256_file_hex(&path) {
        Ok(sha256) => sha256,
        Err(error) => return Err(paths::atomic_write_failed(&path, &error.to_string())),
    };
    if sha256 != observed {
        let _ = fs::remove_file(&path);
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::destination(&full))
            .fact(Fact::reason(msg!(
                "cause-sandbox-copy-changed-while-reading"
            ))),
        ));
    }
    Ok(Some(ReceivedCopy { path, sha256 }))
}
