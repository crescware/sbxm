use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use signal_hook::SigId;
use signal_hook::consts::SIGINT;

use crate::boundary::host::Signals;

use super::RealHost;

impl Signals for RealHost {
    type Registration = SigId;

    fn watch_interrupts(&self, arrived: &Arc<AtomicBool>) -> io::Result<SigId> {
        signal_hook::flag::register(SIGINT, Arc::clone(arrived))
    }

    /// 最後の見張りを外しても、SIGINTはOSの既定の動作へ戻らない。signal-hookのhandlerが
    /// 残り、何もせずに戻る（C20、`signal-hook-registry` 1.4.8の`unregister`の注意書き）。
    fn stop_watching(&self, registration: SigId) -> bool {
        signal_hook::low_level::unregister(registration)
    }
}

#[cfg(test)]
#[path = "system_signal_test.rs"]
mod system_signal_test;
