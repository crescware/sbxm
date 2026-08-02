use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::paths::{self, ProjectPaths};
use crate::testing::repository::project_paths;

/// `FROM scratch\n`のsha256。世代の綴りそのものが工程の合流点であるため、値で固定する。
const SCRATCH_DIGEST: &str = "bb57c7da220a8753d7bdabac0d3afdb6efa742e4c736c5bc93ab40dfd5e23b9b";

fn dockerfile_holding(paths: &ProjectPaths, contents: &str) -> Checked {
    fs::write(paths.dockerfile(), contents).required_because("write the Dockerfile")?;
    Ok(())
}

fn path_fact(diagnostic: &Diagnostic) -> Checked<String> {
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-path-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the diagnostic names the path it read")
}

#[test]
fn the_generation_of_a_dockerfile_is_the_sha256_of_the_bytes_on_disk() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    dockerfile_holding(&paths, "FROM scratch\n")?;

    assert_eq!(
        current_dockerfile_hash(&paths).required_because("the Dockerfile is readable")?,
        SCRATCH_DIGEST
    );

    // 内容が1文字でも変われば世代も変わる。
    dockerfile_holding(&paths, "FROM scratch\n\n")?;
    assert_ne!(
        current_dockerfile_hash(&paths).required_because("the Dockerfile is readable")?,
        SCRATCH_DIGEST
    );
    Ok(())
}

#[test]
fn a_project_without_a_dockerfile_is_told_which_file_is_absent() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;

    let error = current_dockerfile_hash(&paths)
        .refused_because("a generation is never derived from an absent Dockerfile")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::ProjectPathUnreadable);
    assert_eq!(path_fact(diagnostic)?, paths::display(&paths.dockerfile()));
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::Translated { label, value }
                if label.id == "diagnostic-cause-label" && value.id == "cause-dockerfile-absent"
        )),
        "the absence itself is the stated cause: {:?}",
        diagnostic.facts
    );
    Ok(())
}

#[test]
fn a_dockerfile_that_cannot_be_read_is_refused_rather_than_hashed_as_empty() -> Checked {
    if rustix::process::geteuid().is_root() {
        // rootはmodeに関わらず読めるため、この状態を作れない。
        return Ok(());
    }
    let dir = tempfile::tempdir().required()?;
    let paths = project_paths(dir.path())?;
    dockerfile_holding(&paths, "FROM scratch\n")?;
    let dockerfile = paths.dockerfile();
    fs::set_permissions(&dockerfile, fs::Permissions::from_mode(0o000)).required()?;

    let outcome = current_dockerfile_hash(&paths);
    fs::set_permissions(&dockerfile, fs::Permissions::from_mode(0o600)).required()?;

    let error = outcome.refused_because("a Dockerfile that cannot be read has no generation")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::ProjectPathUnreadable);
    assert_eq!(path_fact(diagnostic)?, paths::display(&dockerfile));
    assert!(
        diagnostic.facts.iter().any(|fact| matches!(
            fact,
            Fact::OneLine { label, .. } if label.id == "diagnostic-cause-label"
        )),
        "the operating system's own reason is passed on: {:?}",
        diagnostic.facts
    );
    Ok(())
}
