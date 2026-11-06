use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Signals;

/// 外部commandの実行中と、再試行を待つあいだのCtrl-Cを記録する。
///
/// Capture commandは専用のprocess groupに置くため、端末のforeground groupへ届いたSIGINTは
/// その子孫へ伝播しない。親側ではSIGINTをflagへ変換して実行loopへ渡し、直接の子を先に
/// 終わらせてから`Error::Canceled`を返す。見張りを置くのと外すのは`Signals`が行い、落とすと
/// 外す。
pub(crate) struct SignalGuard<'s, S: Signals> {
    signals: &'s S,
    interrupted: Arc<AtomicBool>,
    registration: S::Registration,
}

impl<'s, S: Signals> SignalGuard<'s, S> {
    /// 外部commandの間の待機中も、Ctrl-Cを取りこぼさずに見張る。
    pub(crate) fn watching<T>(
        signals: &'s S,
        spec: &super::CommandSpec,
        run: impl FnOnce(&dyn Fn() -> bool) -> crate::diagnostics::Result<T>,
    ) -> crate::diagnostics::Result<T> {
        let guard = Self::new(signals).map_err(|error| super::spawn_failure(spec, &error))?;
        run(&|| guard.interrupted())
    }

    pub(super) fn new(signals: &'s S) -> std::io::Result<SignalGuard<'s, S>> {
        let interrupted = Arc::new(AtomicBool::new(false));
        let registration = signals.watch_interrupts(&interrupted)?;
        Ok(SignalGuard {
            signals,
            interrupted,
            registration,
        })
    }

    pub(super) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }
}

impl<S: Signals> Drop for SignalGuard<'_, S> {
    fn drop(&mut self) {
        let _ = self.signals.stop_watching(self.registration);
    }
}

#[cfg(test)]
#[path = "signal_guard_test.rs"]
mod signal_guard_test;
