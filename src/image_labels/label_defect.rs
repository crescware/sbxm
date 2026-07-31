/// labelとして読めなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelDefect {
    /// object でも`null`でもない値が置かれていた。
    NotAnObject,
    /// このkeyの値がstringではなかった。
    ValueNotAString(String),
}
