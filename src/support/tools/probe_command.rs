use super::probe;

/// `Installed::observe`が読むcommand。まとめた確認に含める。
pub fn probe_command() -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), probe()]
}
