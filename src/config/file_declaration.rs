use super::{HostFileSource, SandboxHomeRelativePath};

/// host上の通常fileをSandbox内へ配置する宣言。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDeclaration {
    pub source: HostFileSource,
    pub destination: SandboxHomeRelativePath,
}
