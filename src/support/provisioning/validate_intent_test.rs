use std::fs;

use crate::config::{FileDeclaration, HostFileSource, SandboxHomeRelativePath};
use crate::diagnostics::ErrorId;
use crate::metadata::InitialProvisioningFile;
use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

fn declared(source: &std::path::Path, destination: &str) -> Checked<FileDeclaration> {
    Ok(FileDeclaration {
        source: HostFileSource::new(&paths::display(source)).required_because("source")?,
        destination: SandboxHomeRelativePath::new(destination).required_because("destination")?,
    })
}

#[test]
fn a_different_number_of_declared_files_is_refused_as_changed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("a.yaml");
    fs::write(&source, b"a\n").required_because("write the source")?;
    let config = GlobalConfig {
        language: None,
        git_identity: None,
        files: vec![declared(&source, "a.yaml")?],
    };
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "target".to_string(),
        files: Vec::new(),
    };
    let error = validate_intent(&intent, &config, "example-org/example-repo")
        .refused_because("the file count no longer matches the recorded intent")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningInputChanged)
    );
    Ok(())
}

#[test]
fn a_declaration_moved_to_a_different_destination_is_refused_as_changed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = dir.path().join("a.yaml");
    fs::write(&source, b"a\n").required_because("write the source")?;
    let config = GlobalConfig {
        language: None,
        git_identity: None,
        files: vec![declared(&source, "moved.yaml")?],
    };
    let intent = InitialProvisioningIntent {
        target_dockerfile_sha256: "target".to_string(),
        files: vec![InitialProvisioningFile {
            source: paths::display(&source),
            destination: "original.yaml".to_string(),
            sha256: crate::hash::sha256_hex(b"a\n"),
        }],
    };
    let error = validate_intent(&intent, &config, "example-org/example-repo")
        .refused_because("a declaration whose destination moved is not the recorded input")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningInputChanged)
    );
    Ok(())
}
