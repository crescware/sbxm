/// command helpのlayout。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandLayout {
    Leaf,
    Positional,
}
