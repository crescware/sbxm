/// origin回収可能性の判定対象。ref名とcommitを分離した入力。
///
/// fieldは非公開とし、read-only accessorだけを公開する。`commit`は完全なobject ID、
/// `reference`と`upstream`は完全なref名を保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitCandidate {
    reference: String,
    commit: String,
    upstream: Option<String>,
}

impl CommitCandidate {
    pub fn new(reference: String, commit: String, upstream: Option<String>) -> CommitCandidate {
        CommitCandidate {
            reference,
            commit,
            upstream,
        }
    }

    pub fn reference(&self) -> &str {
        &self.reference
    }

    pub fn commit(&self) -> &str {
        &self.commit
    }

    pub fn upstream(&self) -> Option<&str> {
        self.upstream.as_deref()
    }
}
