/// 1案件について組み立てた案内。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideOutput {
    pub project: String,
    pub sandbox: String,
    pub register_command: String,
}
