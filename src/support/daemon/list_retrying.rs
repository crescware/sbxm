use std::time::Duration;

use crate::boundary::host::protocol::{SandboxEntry, is_listing_cut_short, parse_sandbox_list};
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::{ErrorId, Result};
use crate::time::Clock;

/// daemonが暖まりきる前の`sbx ls`は、出力を出し切る前に終わることがある。
/// 途中で切れた`JSON`はparse errorとして観測されるが、commandを実行し直せば
/// 大抵直るため、この場合に限り内部で数回振り直す。JSONとして読み終えた出力の
/// 形の違いは、振り直さずにそのまま報告する。
const WARMUP_RETRY_LIMIT: u32 = 2;
const WARMUP_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 指定したtimeoutで現在のSandbox一覧を読む。daemon起動直後の不完全な出力は、`clock`で
/// 待ってから読み直す。
pub(super) fn list_retrying(
    host: &dyn HostEnvironment,
    timeout: TimeoutClass,
    clock: &dyn Clock,
) -> Result<Vec<SandboxEntry>> {
    let spec = CommandSpec::capture("sbx", &["ls", "--json"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(timeout);

    let mut remaining_retries = WARMUP_RETRY_LIMIT;
    loop {
        let output = host.run(&spec)?.require_success()?.stdout_text();
        match parse_sandbox_list(&output) {
            Ok(entries) => return Ok(entries),
            Err(error)
                if remaining_retries > 0
                    && error.contains_id(ErrorId::ExternalOutputUnparseable)
                    && is_listing_cut_short(&output) =>
            {
                remaining_retries -= 1;
                clock.sleep(WARMUP_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}
