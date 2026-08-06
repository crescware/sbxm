use std::collections::{BTreeMap, BTreeSet};

use super::UnobservableReason;

/// originの権威ある観測結果。
///
/// `tips`はoriginの完全なref名からtip commit IDへの対応、`reachable_from`は候補commit
/// IDから、そのcommitへ到達できるorigin ref名集合への対応である。remote URL、
/// credential、stderr、翻訳済みmessageは保持しない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginObservation {
    Observed {
        tips: BTreeMap<String, String>,
        reachable_from: BTreeMap<String, BTreeSet<String>>,
    },
    Unobservable {
        reason: UnobservableReason,
    },
}
