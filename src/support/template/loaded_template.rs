/// 使用できる状態のTemplate。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTemplate {
    pub name: String,
    /// この実行でloadしたか。
    pub loaded: bool,
}
