use crate::metadata::ProjectMetadata;
use crate::project::SandboxLayout;

use crate::support::{files, identity, repository, sandbox, secret, tools, worktree};

/// `observe`がSandboxの中について読むcommandの一覧。1回の`sbx exec`で走らせる。
///
/// 並べるのは、1件ずつ起動していたときに読んでいたものと同じcommandである。読んだ結果
/// をどう判定するかは各観測が今までどおり決め、ここは何を読むかだけを決める。
/// Sandboxの状態で要らなくなるもの（`gh`が無ければ読まないprotocol、無いworktreeの
/// HEADなど）も並べる。読まなかったことにする判定は各観測が持ち、余分に読んだ結果は
/// 使われないだけである。案件の記録で決まるもの（hostから送るrepositoryにtokenは無い）
/// は並べない。
///
/// 接続直前の確認（SSH agentの隔離、worktreeの一覧）も同じ一覧に含める。`open`は、
/// 観測の結果をそのまま接続直前の確認として使う。
pub(crate) fn inside_plan(
    metadata: &ProjectMetadata,
    origin: &repository::SandboxOrigin,
    layout: &SandboxLayout,
) -> Vec<Vec<String>> {
    let mut commands = sandbox::ssh_agent_commands();
    if matches!(origin, repository::SandboxOrigin::Github(_)) {
        commands.extend(secret::credential_commands());
    }
    let baseline = metadata.initial_provisioning.as_ref().map_or_else(
        || metadata.declared_files.as_deref().unwrap_or_default(),
        |intent| intent.files.as_slice(),
    );
    commands.extend(files::baseline_commands(baseline));
    commands.extend(identity::observe_commands());
    commands.push(tools::probe_command());
    commands.push(identity::git_protocol_command());
    commands.extend(repository::inspection_commands(&layout.bare_git_dir()));
    let names = layout.worktree_names(metadata.provisioning.requested_worktrees);
    commands.extend(worktree::inspection_commands(layout, &names));
    commands.push(worktree::list_command(layout));
    commands
}
