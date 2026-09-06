use std::fs;

use crate::config::{FileDeclaration, GlobalConfig, HostFileSource, SandboxHomeRelativePath};
use crate::diagnostics::ErrorId;
use crate::paths;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::repository::project_paths;

use super::*;

fn declared(source: &std::path::Path, destination: &str) -> Checked<FileDeclaration> {
    Ok(FileDeclaration {
        source: HostFileSource::new(&paths::display(source)).required_because("source")?,
        destination: SandboxHomeRelativePath::new(destination).required_because("destination")?,
    })
}

#[test]
fn a_snapshot_captured_after_the_source_changes_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    fs::write(paths.dockerfile(), b"FROM example\n").required_because("write the Dockerfile")?;
    let source = dir.path().join("a.yaml");
    fs::write(&source, b"original\n").required_because("write the original source")?;
    let config = GlobalConfig {
        language: None,
        git_identity: None,
        files: vec![declared(&source, "a.yaml")?],
    };
    let original = ProvisioningInputs::capture(&paths, &config, None)
        .required_because("capture the original input")?;
    let intent = initial_intent(&original);

    fs::write(&source, b"changed before repair snapshot\n")
        .required_because("change the live source")?;
    let raced =
        ProvisioningInputs::capture(&paths, &config, Some(&intent.target_dockerfile_sha256))
            .required_because("capture the input repair would use")?;

    let error = validate_captured_intent(&intent, &raced, "example-org/example-repo")
        .refused_because("repair may not place bytes outside the fixed intent")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningInputChanged)
    );
    Ok(())
}
