use super::*;

impl RenderingPolicy {
    /// ANSIを一切出さない条件。
    pub fn plain() -> RenderingPolicy {
        RenderingPolicy {
            stdout: StreamPolicy::plain(),
            stderr: StreamPolicy::plain(),
        }
    }
}
