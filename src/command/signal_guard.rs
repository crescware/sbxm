use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::consts::SIGINT;

/// Capture commandの実行中だけCtrl-Cを記録する。
///
/// Capture commandは専用のprocess groupに置くため、端末のforeground groupへ届いたSIGINTは
/// その子孫へ伝播しない。親側ではSIGINTをflagへ変換して実行loopへ渡し、直接の子を先に
/// 終わらせてから`Error::Canceled`を返す。
pub(super) struct SignalGuard {
    interrupted: Arc<AtomicBool>,
    registration: signal_hook::SigId,
}

impl SignalGuard {
    pub(super) fn new() -> std::io::Result<SignalGuard> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let registration = signal_hook::flag::register(SIGINT, Arc::clone(&interrupted))?;
        Ok(SignalGuard {
            interrupted,
            registration,
        })
    }

    pub(super) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        let _ = signal_hook::low_level::unregister(self.registration);
    }
}

#[cfg(test)]
#[path = "signal_guard_test.rs"]
mod signal_guard_test;
