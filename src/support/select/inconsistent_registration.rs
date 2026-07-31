use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ProjectPaths};
use crate::repository::RepositoryIdentity;

/// registry entryとproject metadataが別の案件を指している。
pub fn inconsistent_registration(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    expected: &RepositoryIdentity,
) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectInconsistent,
            msg!(
                "error-project-inconsistent",
                path = paths::display(&paths.metadata_file()),
                observed = metadata.repository.clone_url(),
                expected = expected.clone_url()
            ),
        )
        .remediation(msg!("remediation-project-inconsistent")),
    )
}
