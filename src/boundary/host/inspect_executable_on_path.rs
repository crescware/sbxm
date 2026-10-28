use super::{Executable, inspect_executable_at, locate_on_path};

/// PATH上で`program`として起動されるfileの実体を、実行せずに観測する。
pub fn inspect_executable_on_path(program: &str) -> Option<Executable> {
    inspect_executable_at(&locate_on_path(program)?)
}
