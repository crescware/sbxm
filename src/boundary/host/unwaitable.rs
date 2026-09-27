use crate::diagnostics::Error;

use super::{CommandSpec, Processes, spawn_failure, terminate_child};

/// 待てなくなった子processを終わらせ、待てなかったことを報告する。
///
/// 終わりを確かめられない相手をそのままにすると、sbxmが終わったあとも動き続け、
/// 端末や出力のpipeを持ち続けうる。報告より先に、こちらから終わらせる。原因はOSが
/// 書いた原文である（契約test C11）。
pub(super) fn unwaitable<O: Processes>(
    os: &O,
    child: &mut O::Child,
    spec: &CommandSpec,
    error: &std::io::Error,
) -> Error {
    terminate_child(os, child);
    spawn_failure(spec, error)
}
