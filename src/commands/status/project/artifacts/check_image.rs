use crate::command::HostEnvironment;
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::project::SandboxName;

use crate::support::image::{self, LABEL_CANONICAL_ID, LABEL_DOCKERFILE_SHA256};

use crate::commands::status::project::{ProjectStatus, Value};

/// 適用済み世代のimageが、この案件のものとして存在するか。
pub fn check_image(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    status: &mut ProjectStatus,
) {
    let generation = &metadata.provisioning.dockerfile_sha256;
    let image = image::image_name(name, generation);

    let value = match image::inspect(host, &image) {
        Ok(Some(identity)) => {
            let declares_project = identity.labels.get(LABEL_CANONICAL_ID)
                == Some(&metadata.canonical_id().to_string());
            let declares_generation =
                identity.labels.get(LABEL_DOCKERFILE_SHA256) == Some(generation);
            if declares_project && declares_generation {
                Value::Ready
            } else {
                status.diagnostics.push(
                    Diagnostic::new(ErrorId::ImageUnusable, msg!("error-image-unusable"))
                        .fact(Fact::image(&image))
                        .fact(Fact::reason(msg!("cause-labels-declare-something-else"))),
                );
                Value::Mismatch
            }
        }
        Ok(None) => Value::Missing,
        Err(error) => {
            status.global_scope_failure(&error);
            Value::NotObserved
        }
    };
    status.push("status-item-image", value);
}
