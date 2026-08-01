use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use signal_hook::consts::SIGINT;

/// Capture commandの実行中だけCtrl-Cを記録する。
///
/// Capture commandは専用のprocess groupへ置くため、親processへ届いたSIGINTをOSの既定動作
/// へ任せると、子processだけが残る。flagへ変換して実行loopへ渡し、子groupを先に終わらせて
///から`Error::Canceled`を返す。
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

    #[cfg(test)]
    pub(super) fn interrupted_for_test() -> std::io::Result<SignalGuard> {
        let guard = SignalGuard::new()?;
        guard.interrupted.store(true, Ordering::SeqCst);
        Ok(guard)
    }
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        let _ = signal_hook::low_level::unregister(self.registration);
    }
}
