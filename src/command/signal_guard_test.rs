use super::*;

impl SignalGuard {
    /// 既に割り込まれた状態のguard。
    pub(crate) fn interrupted_for_test() -> std::io::Result<SignalGuard> {
        let guard = SignalGuard::new()?;
        guard.interrupted.store(true, Ordering::SeqCst);
        Ok(guard)
    }

    /// Ctrl-Cが届く時期をtestが決めるための握り。
    ///
    /// 実行のどこで割り込まれるかは、本来はsignalの到達時期が決める。同じflagを持てば、
    /// その時期をtestが選べる。
    pub(crate) fn interrupt_switch_for_test(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupted)
    }
}
