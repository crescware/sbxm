use crate::testing::outcome::{Checked, Required};

use std::path::Path;

use crate::config::{FileDeclaration, HostFileSource, SandboxHomeRelativePath};

pub fn declaration(source: &Path) -> Checked<FileDeclaration> {
    Ok(FileDeclaration {
        source: HostFileSource::new(&crate::paths::display(source)).required()?,
        destination: SandboxHomeRelativePath::new(".config/example/settings.yaml").required()?,
    })
}
