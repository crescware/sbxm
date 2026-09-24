use std::collections::BTreeMap;

use super::SentChange;

/// 送る前と後のref名から先端への対応を比べ、変わったrefだけをref名の順に並べる。
pub(super) fn sent_changes(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<SentChange> {
    let mut changes = Vec::new();
    for (reference, tip) in after {
        match before.get(reference) {
            None => changes.push(SentChange::Created {
                reference: reference.clone(),
            }),
            Some(previous) if previous != tip => changes.push(SentChange::Updated {
                reference: reference.clone(),
            }),
            Some(_) => {}
        }
    }
    for reference in before.keys() {
        if !after.contains_key(reference) {
            changes.push(SentChange::Removed {
                reference: reference.clone(),
            });
        }
    }
    changes.sort_by(|left, right| left.reference().cmp(right.reference()));
    changes
}

#[cfg(test)]
#[path = "sent_changes_test.rs"]
mod sent_changes_test;
