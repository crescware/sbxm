use std::collections::BTreeMap;

use super::super::SentChange;
use super::sent_changes;

fn refs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(reference, tip)| ((*reference).to_string(), (*tip).to_string()))
        .collect()
}

#[test]
fn only_the_refs_that_changed_are_listed_in_name_order() {
    let before = refs(&[
        ("refs/remotes/origin/main", "1"),
        ("refs/remotes/origin/old", "2"),
        ("refs/tags/v1", "3"),
    ]);
    let after = refs(&[
        ("refs/remotes/origin/feature", "4"),
        ("refs/remotes/origin/main", "5"),
        ("refs/tags/v1", "3"),
    ]);

    assert_eq!(
        sent_changes(&before, &after),
        [
            SentChange::Created {
                reference: "refs/remotes/origin/feature".to_string()
            },
            SentChange::Updated {
                reference: "refs/remotes/origin/main".to_string()
            },
            SentChange::Removed {
                reference: "refs/remotes/origin/old".to_string()
            },
        ]
    );
    assert!(sent_changes(&after, &after).is_empty());
}
