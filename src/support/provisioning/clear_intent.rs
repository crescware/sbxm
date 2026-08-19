use crate::diagnostics::Result;
use crate::metadata;
use crate::support::select::Locked;

/// 構築完了をatomicにcommitし、intentをmetadataから消す。
pub(crate) fn clear_intent(locked: &mut Locked) -> Result<()> {
    if locked.metadata.initial_provisioning.is_none() {
        return Ok(());
    }
    let mut metadata = locked.metadata.clone();
    metadata.initial_provisioning = None;
    metadata::update(&locked.paths, &metadata)?;
    locked.metadata = metadata;
    Ok(())
}
