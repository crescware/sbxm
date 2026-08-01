use super::*;

impl OutputPolicy {
    /// ANSIを一切出さない条件。
    pub fn plain() -> OutputPolicy {
        OutputPolicy {
            stdout: StreamPolicy::plain(),
            stderr: StreamPolicy::plain(),
        }
    }
}
