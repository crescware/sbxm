use super::selected_target;
use crate::support::provisioning::{Observation, ProvisioningState};

fn observation(current: &str, stored: &str) -> Observation {
    Observation::new(
        ProvisioningState::Incomplete,
        current.to_string(),
        stored.to_string(),
        "target".to_string(),
    )
}

#[test]
fn an_active_intent_always_targets_its_own_recorded_generation() {
    let mut observed = observation("current", "stored");
    observed.target_generation = "target".to_string();
    assert_eq!(selected_target(&observed, true), "target");
}

#[test]
fn an_unchanged_dockerfile_targets_the_stored_generation() {
    let observed = observation("same", "same");
    assert_eq!(selected_target(&observed, false), "same");
}

#[test]
fn only_the_stored_generations_image_matching_keeps_the_stored_target() {
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = true;
    observed.current_image_matches = false;
    assert_eq!(selected_target(&observed, false), "stored");
}

#[test]
fn only_the_current_generations_image_matching_adopts_the_current_target() {
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = false;
    observed.current_image_matches = true;
    assert_eq!(selected_target(&observed, false), "current");
}

#[test]
fn neither_generations_image_matching_falls_back_to_the_recorded_target() {
    let mut observed = observation("current", "stored");
    observed.stored_image_matches = false;
    observed.current_image_matches = false;
    observed.target_generation = "target".to_string();
    assert_eq!(selected_target(&observed, false), "target");
}
