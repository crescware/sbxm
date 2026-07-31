use super::Candidate;

/// `表示にはGitHub上の表記を使う`。
pub(super) fn labels(candidates: &[Candidate]) -> Vec<String> {
    candidates.iter().map(Candidate::display_id).collect()
}
