use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::{ProjectId, SandboxLayout, SandboxName};
use crate::support::select::ProjectPrompt;
use crate::support::{generation, image, provisioning, secret};

use super::PrepareOutput;

/// 対象を引数またはpromptで解決し、登録済み案件のSandboxを構築する。
pub fn run(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
    progress: &mut dyn ProgressSink,
) -> Result<PrepareOutput> {
    let mut locked =
        crate::support::select::one(location, requested, &msg!("select-prepare-heading"), prompt)?
            .lock()?;
    let mut warnings = image::cleanup_stale_archives(&locked.paths)?;
    generation::require_no_rebuild(&locked.metadata)?;

    let canonical = locked.metadata.canonical_id().clone();
    let name = SandboxName::derive(&canonical);
    let layout = SandboxLayout::new(&canonical);

    if let Some(mut output) = provisioning::already_built(
        host,
        &locked.paths,
        &name,
        &locked.metadata,
        &layout,
        workspace_root,
    )? {
        output.warnings = warnings;
        return Ok(output);
    }

    // custom secretはSandboxの作成時に結び付く。あとから登録しても既存のSandboxには
    // 届かないため、作成より前に、そしてimageを組む前に確認する。
    secret::require_github(host, name.as_str())?;
    crate::support::docker::require_reachable(host)?;

    // 中断後もbaseと同じgeneration選択規則で続行する。intentがある場合も、既にimageが
    // 完成していればDockerfile変更のwarningを従来どおり出す。
    let (target, target_warnings) =
        provisioning::fresh_target(host, &locked.paths, &name, &locked.metadata)?;
    warnings.extend(target_warnings);

    provisioning::provision(
        &mut locked,
        config,
        &target,
        host,
        workspace_root,
        progress,
        warnings,
    )
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "resume_test.rs"]
mod resume_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
