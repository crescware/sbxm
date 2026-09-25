use super::REF_KINDS;

/// Sandboxのrefのうち、bundleがhostへ運ぶものか。
///
/// branchとtagに加えて、各worktreeのHEADを運ぶ。worktreeのHEADは`HEAD`と表す。stashや
/// notesのようなrepository単位のrefは運ばない。
pub fn carries(reference: &str) -> bool {
    reference == "HEAD"
        || REF_KINDS
            .iter()
            .any(|(source, _)| reference.starts_with(source))
}
