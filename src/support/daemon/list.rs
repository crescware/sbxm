use std::thread;
use std::time::Duration;

use crate::boundary::host::protocol::{SandboxEntry, parse_sandbox_list};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::{ErrorId, Result};

/// daemonが暖まりきる前の`sbx ls`は、出力を出し切る前に終わることがある。
/// 途中で切れた`JSON`はparse errorとして観測されるが、commandを実行し直せば
/// 大抵直るため、この場合に限り内部で数回振り直す。
const WARMUP_RETRY_LIMIT: u32 = 2;
const WARMUP_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 現在のSandbox一覧。
pub fn list(host: &dyn HostEnvironment) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);

    let mut remaining_retries = WARMUP_RETRY_LIMIT;
    loop {
        let outcome = host.run(&spec)?.require_success()?;
        match parse_sandbox_list(&outcome.stdout_text()) {
            Ok(entries) => return Ok(entries),
            Err(error)
                if remaining_retries > 0
                    && error.contains_id(ErrorId::ExternalOutputUnparseable) =>
            {
                remaining_retries -= 1;
                thread::sleep(WARMUP_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}
