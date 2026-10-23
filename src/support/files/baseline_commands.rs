use std::path::Path;

use crate::metadata::InitialProvisioningFile;

use super::{AGENT_HOME, destination_path};

/// `observe_against_baseline`が読むcommand。まとめた確認に含める。
///
/// 宣言fileごとに、homeからの各階層のsymlink確認、配置先の存在確認、digestの順である。
/// 相対pathとして読めない行き先は、観測が同じ理由で拒否するため、ここでは何も並べない。
pub fn baseline_commands(baseline: &[InitialProvisioningFile]) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    for entry in baseline {
        let Ok(destination) = destination_path(Path::new(&entry.destination)) else {
            continue;
        };
        let mut current = AGENT_HOME.to_string();
        for part in destination.split('/') {
            current.push('/');
            current.push_str(part);
            commands.push(vec!["test".to_string(), "-h".to_string(), current.clone()]);
        }
        let full = format!("{AGENT_HOME}/{destination}");
        commands.push(vec!["test".to_string(), "-e".to_string(), full.clone()]);
        commands.push(vec!["sha256sum".to_string(), full]);
    }
    commands
}
