use super::ReflectResult;

/// hostのbranchやtag1つへの反映。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reflected {
    /// hostのref。`refs/heads/<branch>`か`refs/tags/<tag>`。
    pub reference: String,
    pub result: ReflectResult,
}
