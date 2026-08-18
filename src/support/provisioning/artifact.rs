/// 初回構築の途中に残り得る成果物。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Artifact {
    Sandbox,
    Workspace,
    Image(String),
    Template(String),
    Archive(String),
}

impl Artifact {
    pub fn as_str(&self) -> String {
        match self {
            Self::Sandbox => "Sandbox".to_string(),
            Self::Workspace => "workspace directory".to_string(),
            Self::Image(name) => format!("image {name}"),
            Self::Template(name) => format!("Template {name}"),
            Self::Archive(path) => format!("archive {path}"),
        }
    }
}
