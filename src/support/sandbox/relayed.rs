use crate::command::{EnvPolicy, TerminalCommand};

/// `sbx`が出す接続案内。
///
/// `sbx`はsandboxを作った直後に`sbx run`での入り方を案内する。sbxmは同じ実行の中で
/// `sbxm prepare`や`sbxm open`を次の一手として案内しており、2つの案内が並ぶと、
/// 利用者はまだ工程が続いているのに終わったものと読んでしまう。
const CONNECTION_HINT: [&str; 2] = ["To connect to this sandbox", "sbx run"];

/// 進捗を見せる`sbx`の起動。
///
/// `sbx`を端末へ出す起動はここだけが作る。SSH Agentを渡さないことと、`sbx`自身の接続案内
/// を見せないことは、どのsubcommandでも変わらないためである。
pub fn relayed(args: &[&str]) -> TerminalCommand {
    TerminalCommand::relayed("sbx", args)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .hiding(&CONNECTION_HINT)
}
