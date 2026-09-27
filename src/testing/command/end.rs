/// 子とつながるpipeの端。`ScriptedOs`が見張る端でもある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Stdin,
    Stdout,
    Stderr,
}
