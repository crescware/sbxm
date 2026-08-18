use super::*;

use crate::design::Fact;
use crate::testing::outcome::{Checked, Required};

#[test]
fn an_incomplete_error_lists_the_observed_artifacts() -> Checked {
    let metadata = crate::testing::metadata::attached("Example-Org", "Example-Repo")?;
    let error = incomplete(
        &metadata,
        &[
            Artifact::Sandbox,
            Artifact::Workspace,
            Artifact::Archive("/tmp/example.tar".into()),
        ],
    );

    assert_eq!(
        error.first_id(),
        Some(crate::diagnostics::ErrorId::InitialProvisioningIncomplete)
    );
    let fact = error.diagnostics()[0]
        .facts
        .first()
        .required_because("the incomplete artifacts are named")?;
    let rendered = match fact {
        Fact::ManyLines { lines, .. } => lines.join("\n"),
        Fact::OneLine { value, .. } => value.as_str().to_string(),
        Fact::Translated { .. } => String::new(),
    };
    assert!(rendered.contains("Sandbox"), "{rendered}");
    assert!(rendered.contains("workspace"), "{rendered}");
    assert!(rendered.contains("archive /tmp/example.tar"), "{rendered}");
    Ok(())
}
