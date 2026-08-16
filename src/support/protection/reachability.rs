use super::{CommitCandidate, OriginObservation, UnobservableReason};

/// commitがoriginから回収できるという根拠。翻訳しない安定したenum。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reachability {
    /// candidateのupstreamがcandidate commitへ到達できる。
    Pushed { upstream: String },
    /// upstream以外の一つ以上のorigin refがcandidate commitへ到達できる。
    /// `origins`はref名昇順。
    Reachable { origins: Vec<String> },
    /// refresh済みoriginのどのrefからもcandidate commitへ到達できない。
    Unreachable,
    /// 権威ある観測または分類に必要なobjectがない。
    Unobservable { reason: UnobservableReason },
}

impl Reachability {
    /// `candidate`と`observation`だけから分類する。`observation`以外のGit状態を読まず、
    /// fetchやGit commandを実行しない。同じ入力には同じ結果を返す。
    ///
    /// `candidate`のcommitが`observation`の`reachable_from`に無い場合は、観測されて
    /// いないcommitとして`Unobservable`を返す。`observe_for_mutation`は渡された全
    /// candidateを必ず記録するため通常は起こらないが、candidateを網羅しない観測
    /// (`observe_read_only`等)がこの関数へそのまま流れても、していない観測を
    /// `Unreachable`と断定しない。
    pub fn classify(candidate: &CommitCandidate, observation: &OriginObservation) -> Reachability {
        let reachable_from = match observation {
            OriginObservation::Unobservable { reason } => {
                return Reachability::Unobservable { reason: *reason };
            }
            OriginObservation::Observed { reachable_from, .. } => reachable_from,
        };

        let Some(reaching) = reachable_from.get(candidate.commit()) else {
            return Reachability::Unobservable {
                reason: UnobservableReason::ObjectMissing,
            };
        };

        if let Some(upstream) = candidate.upstream()
            && reaching.contains(upstream)
        {
            return Reachability::Pushed {
                upstream: upstream.to_string(),
            };
        }
        if reaching.is_empty() {
            Reachability::Unreachable
        } else {
            Reachability::Reachable {
                origins: reaching.iter().cloned().collect(),
            }
        }
    }

    /// 翻訳しない安定した表記。fingerprintと表示に使う。
    pub fn as_str(&self) -> &'static str {
        match self {
            Reachability::Pushed { .. } => "pushed",
            Reachability::Reachable { .. } => "reachable",
            Reachability::Unreachable => "unreachable",
            Reachability::Unobservable { .. } => "unobservable",
        }
    }

    /// 表示用の安定した値。観測不能な場合は、断定できない理由も値へ残す。
    pub fn display(&self) -> String {
        match self {
            Reachability::Unobservable { reason } => {
                format!("unobservable({})", reason.as_str())
            }
            _ => self.as_str().to_string(),
        }
    }

    pub fn legend_id(&self) -> &'static str {
        match self {
            Reachability::Pushed { .. } => "legend-pushed",
            Reachability::Reachable { .. } => "legend-reachable",
            Reachability::Unreachable => "legend-unreachable",
            Reachability::Unobservable { .. } => "legend-unobservable",
        }
    }

    /// fingerprintの入力に使う、翻訳しない安定表記。
    ///
    /// 表記だけでなく根拠のref名まで写す。同じ`reachable`でも、到達できるorigin refが
    /// 入れ替われば別の状態である。
    pub(super) fn fingerprint_key(&self) -> String {
        match self {
            Reachability::Pushed { upstream } => format!("pushed\u{1f}{upstream}"),
            Reachability::Reachable { origins } => {
                format!("reachable\u{1f}{}", origins.join("\u{1e}"))
            }
            Reachability::Unreachable => "unreachable".to_string(),
            Reachability::Unobservable { reason } => {
                format!("unobservable\u{1f}{}", reason.fingerprint_key())
            }
        }
    }
}

#[cfg(test)]
#[path = "reachability_test.rs"]
mod reachability_test;
