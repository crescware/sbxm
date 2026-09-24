use std::process::Child;

use crate::diagnostics::Error;

use super::{CommandSpec, spawn_failure, terminate_child};

/// 待てなくなった子processを終わらせ、待てなかったことを報告する。
///
/// 終わりを確かめられない相手をそのままにすると、出力を読むthreadはEOFに達しない。
/// 報告より先に、こちらから終わらせる。原因はOSが書いた原文である。
pub(super) fn unwaitable(child: &mut Child, spec: &CommandSpec, error: &std::io::Error) -> Error {
    terminate_child(child);
    spawn_failure(spec, error)
}
