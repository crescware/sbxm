use std::time::Instant;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use crate::support::daemon;

use super::{Poll, single, still_present};

/// commandの戻り値だけを不在の根拠にしない。一覧から消えるまで待つ。
pub(super) fn wait_until_absent(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    poll: Poll,
) -> Result<()> {
    let deadline = Instant::now() + poll.limit;
    loop {
        if single(&daemon::list(host)?, name.as_str())?.is_none() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(still_present(name));
        }
        std::thread::sleep(poll.interval);
    }
}
