use crate::project::SandboxLayout;

/// `open`の観測がmanaged worktreeごとに読むcommand。まとめた確認に含める。
///
/// worktreeごとに、存在、所属するrepository（`Inspection::common_dir`）、
/// HEAD（`Inspection::head_outcome`）の順である。
pub fn inspection_commands(layout: &SandboxLayout, names: &[String]) -> Vec<Vec<String>> {
    let mut commands = Vec::with_capacity(names.len() * 3);
    for name in names {
        let path = format!("{}/{name}", layout.bare_root());
        commands.push(vec!["test".to_string(), "-e".to_string(), path.clone()]);
        for args in [
            ["rev-parse", "--path-format=absolute", "--git-common-dir"].as_slice(),
            ["rev-parse", "HEAD"].as_slice(),
        ] {
            let mut command = vec!["git".to_string(), "-C".to_string(), path.clone()];
            command.extend(args.iter().map(|arg| (*arg).to_string()));
            commands.push(command);
        }
    }
    commands
}
