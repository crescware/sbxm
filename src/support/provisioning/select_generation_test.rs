use crate::diagnostics::ErrorId;
use crate::support::provisioning::{Observation, ProvisioningState};
use crate::testing::metadata::attached;
use crate::testing::outcome::{Checked, Refused, Required};

use super::select_generation;

fn observation(current: &str, stored: &str) -> Observation {
    Observation::new(
        ProvisioningState::Incomplete,
        current.to_string(),
        stored.to_string(),
        "target".to_string(),
    )
}

#[test]
fn an_active_intent_without_artifacts_adopts_the_current_generation() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.target_generation = "target".to_string();
    let selected = select_generation(&observed, &metadata, true)
        .required_because("no target artifact means the current generation is adopted")?;
    assert_eq!(selected, "current");
    Ok(())
}

#[test]
fn an_active_intent_with_an_artifact_keeps_its_recorded_generation() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.target_generation = "target".to_string();
    observed.stored_image_present = true;
    let selected = select_generation(&observed, &metadata, true)
        .required_because("an existing target artifact keeps the recorded generation")?;
    assert_eq!(selected, "target");
    Ok(())
}

#[test]
fn an_unchanged_dockerfile_targets_the_stored_generation() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let observed = observation("same", "same");
    let selected = select_generation(&observed, &metadata, false)
        .required_because("the current and stored generation already agree")?;
    assert_eq!(selected, "same");
    Ok(())
}

#[test]
fn only_the_stored_generations_image_matching_keeps_the_stored_target() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = true;
    observed.current_image_matches = false;
    let selected = select_generation(&observed, &metadata, false)
        .required_because("only the stored generation has a verified image")?;
    assert_eq!(selected, "stored");
    Ok(())
}

#[test]
fn only_the_current_generations_image_matching_adopts_the_current_target() -> Checked {
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = false;
    observed.current_image_matches = true;
    let selected = select_generation(&observed, &metadata, false)
        .required_because("only the current generation has a verified image")?;
    assert_eq!(selected, "current");
    Ok(())
}

#[test]
fn neither_generations_image_matching_is_refused_as_ambiguous() -> Checked {
    // どちらのimageも一致しない場合、片方を推測で選ばない。`open`と`repair`は同じ
    // 拒否を受け取り、実行しないままdisplayとexecuteの照合が食い違うことも防ぐ。
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = false;
    observed.current_image_matches = false;
    observed.target_generation = "target".to_string();
    let error = select_generation(&observed, &metadata, false)
        .refused_because("neither generation's image can be verified")?;
    assert!(error.contains_id(ErrorId::InitialProvisioningGenerationMissing));
    Ok(())
}

#[test]
fn both_generations_image_matching_is_refused_as_ambiguous() -> Checked {
    // 両方一致する場合も一意に決められない事実は同じであり、片方を黙って選ばない。
    let metadata = attached("example-org", "example-repo")?;
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = true;
    observed.current_image_matches = true;
    let error = select_generation(&observed, &metadata, false)
        .refused_because("both generations' images can be verified")?;
    assert!(error.contains_id(ErrorId::InitialProvisioningGenerationMissing));
    Ok(())
}
