use super::*;

impl SignalGuard {
    /// 既に割り込まれた状態のguard。
    pub(crate) fn interrupted_for_test() -> std::io::Result<SignalGuard> {
        let guard = SignalGuard::new()?;
        guard.interrupted.store(true, Ordering::SeqCst);
        Ok(guard)
    }
}
