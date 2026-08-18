use super::Phase;

/// repairの出力に必要な、診断結果のsnapshot。
#[derive(Debug, Clone)]
pub struct View {
    pub project: String,
    pub sandbox: Option<String>,
    pub target_generation: Option<String>,
    pub artifacts: Vec<String>,
    pub phase: Phase,
}
