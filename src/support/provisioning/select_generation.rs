use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::short_hex;
use crate::metadata::ProjectMetadata;
use crate::msg;

use super::Observation;

/// 観測結果から、次に完成させるべきgenerationを1つだけ選ぶ唯一の規則。
///
/// `open`（intentなしのIncomplete、Sandbox不在の経路）と`repair`は、同じ観測へ別の
/// 規則を当てない。intentがある場合は固定済みのtarget（成果物が一つも無ければ現在の
/// Dockerfile）、intentが無い場合は現在と保存済みのDockerfileのうちimageが一致する方を
/// 選ぶ。どちらのimageも一致しない、またはどちらも一致する場合は一意に決められない。
pub(crate) fn select_generation(
    observation: &Observation,
    metadata: &ProjectMetadata,
    has_intent: bool,
) -> Result<String> {
    if has_intent {
        let no_target_artifact = !observation.stored_image_present
            && !observation.stored_template_present
            && observation.sandbox.is_missing();
        if no_target_artifact {
            return Ok(observation.current_generation.clone());
        }
        return Ok(observation.target_generation.clone());
    }
    if observation.current_generation == observation.stored_generation {
        return Ok(observation.stored_generation.clone());
    }
    match (
        observation.stored_image_matches,
        observation.current_image_matches,
    ) {
        (true, false) => Ok(observation.stored_generation.clone()),
        (false, true) => Ok(observation.current_generation.clone()),
        _ => Err(generation_missing(observation, metadata)),
    }
}

#[cfg(test)]
#[path = "select_generation_test.rs"]
mod select_generation_test;

fn generation_missing(observation: &Observation, metadata: &ProjectMetadata) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningGenerationMissing,
            msg!(
                "error-initial-provisioning-generation-missing",
                project = metadata.display_id()
            ),
        )
        .fact(Fact::value(short_hex(&observation.target_generation)))
        .fact(Fact::reason(msg!(
            "cause-initial-provisioning-generation-ambiguous"
        )))
        .remediation(Remediation::text(msg!(
            "remediation-initial-provisioning-generation-missing"
        ))),
    )
}
