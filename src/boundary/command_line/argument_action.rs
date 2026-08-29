/// optionの値の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentAction {
    Value,
    Flag,
    Append,
}
