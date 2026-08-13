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

impl OriginObservation {
    /// fingerprintの入力に使う、翻訳しない安定表記。
    ///
    /// 観測したorigin refのtipまで写す。確認から削除までのあいだにoriginが進んだ場合、
    /// worktreeの側が1つも変わらなくても、古いpermitでは越えられないようにする。
    pub(super) fn fingerprint_key(&self) -> String {
        match self {
            OriginObservation::Observed {
                tips,
                reachable_from,
            } => {
                let tips: Vec<String> = tips
                    .iter()
                    .map(|(reference, commit)| format!("{reference}\u{1d}{commit}"))
                    .collect();
                let reachable_from: Vec<String> = reachable_from
                    .iter()
                    .map(|(commit, origins)| {
                        let origins: Vec<&str> =
                            origins.iter().map(std::string::String::as_str).collect();
                        format!("{commit}\u{1d}{}", origins.join("\u{1c}"))
                    })
                    .collect();
                format!(
                    "observed\u{1f}{}\u{1f}{}",
                    tips.join("\u{1e}"),
                    reachable_from.join("\u{1e}")
                )
            }
            OriginObservation::Unobservable { reason } => {
                format!("unobservable\u{1f}{}", reason.fingerprint_key())
            }
        }
    }
}
