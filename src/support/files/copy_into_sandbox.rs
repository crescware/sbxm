use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use crate::support::sandbox;

use super::{copy_steps, read_source_bytes};

/// 扱いを決めたときと同じbyte列を送って配置する。失敗したら書きかけの`pending`を消す。
///
/// 扱いはsourceのdigestで決めた。送る直前に読み直したbyte列がそのdigestと違えば、
/// 決めた扱いの前提が崩れているため送らない。
pub(super) fn copy_into_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    source: &Path,
    digest: &str,
    destination: &str,
) -> Result<()> {
    let (bytes, read) = read_source_bytes(source)?;
    if read != digest {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileUnusable,
                msg!("error-declared-file-unusable"),
            )
            .fact(Fact::source(&paths::display(source)))
            .fact(Fact::reason(msg!(
                "cause-declared-file-changed-while-placing"
            ))),
        ));
    }
    let pending = format!("{destination}.sbxm-new");
    let result = copy_steps(host, sandbox, bytes, digest, destination, &pending);
    if result.is_err() {
        let _ = sandbox::exec_as_root(host, sandbox, &["rm", "-f", &pending]);
    }
    result
}
