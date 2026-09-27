use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Ctrl-C（SIGINT）を見張る基本操作。
pub(crate) trait Signals {
    /// 置いた見張り。外すときに渡す。
    type Registration: Copy;

    /// Ctrl-Cが届いたら`arrived`を立てる見張りを置く。
    fn watch_interrupts(&self, arrived: &Arc<AtomicBool>) -> io::Result<Self::Registration>;
    /// 見張りを外す。外せたかを返す。外してもOSの既定の動作へは戻らない（C20）。
    fn stop_watching(&self, registration: Self::Registration) -> bool;
}
